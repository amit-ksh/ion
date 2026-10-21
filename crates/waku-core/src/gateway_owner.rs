//! Outbound workflow publication owned by the daemon, including headless owners.
//! Relay claims are durable before effects. Unknown claims are never replayed.
use crate::{
    credentials::{
        CredentialService, GatewayApi, GatewayAutomationAction, GatewayAutomationCommand,
        GatewayPublisherLease as PublisherLease, GatewaySyncCadence as Cadence,
    },
    persistence::{PersistedState, StateStore},
    teams::TeamCoordinator,
};
use anyhow::{Context as _, Result};
use crossbeam_channel::{Receiver, Sender, bounded};
use parking_lot::Mutex;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use uuid::Uuid;
use waku_protocol::control::{AutomationRun, AutomationStatus, ControlRecord};

const POLL_INTERVAL: Duration = Duration::from_secs(5);
const PASS_BUDGET: Duration = Duration::from_secs(20);
// There is exactly one owner worker per daemon process. The normal IPC server
// selects a daemon; this also fences accidentally constructed duplicate owners.
static OWNER_RUNNING: AtomicBool = AtomicBool::new(false);
struct OwnerLease;
impl Drop for OwnerLease {
    fn drop(&mut self) {
        OWNER_RUNNING.store(false, Ordering::Release);
    }
}

struct StopFence {
    stopping: AtomicBool,
    effects: Mutex<()>,
}
impl Default for StopFence {
    fn default() -> Self {
        Self {
            stopping: AtomicBool::new(false),
            effects: Mutex::new(()),
        }
    }
}
impl StopFence {
    fn stopped(&self) -> bool {
        self.stopping.load(Ordering::Acquire)
    }
}

pub(crate) struct GatewayOwner {
    fence: Arc<StopFence>,
    wake: Sender<()>,
    receiver: Mutex<Option<Receiver<()>>>,
    engine: Arc<OwnerEngine>,
    file_managed: bool,
}
impl GatewayOwner {
    pub(crate) fn new(
        control: Arc<crate::control::ControlStore>,
        teams: Arc<TeamCoordinator>,
        tasks: Arc<Mutex<PersistedState>>,
        task_store: Arc<StateStore>,
        runtime: &Arc<crate::team_runtime::TeamRuntime>,
        file_managed: bool,
    ) -> Self {
        let (wake, receiver) = bounded(1);
        Self {
            fence: Arc::new(StopFence::default()),
            wake,
            receiver: Mutex::new(Some(receiver)),
            engine: Arc::new(OwnerEngine {
                control,
                teams,
                tasks,
                task_store,
                runtime: Arc::downgrade(runtime),
            }),
            file_managed,
        }
    }
    pub(crate) fn start(&self) -> Result<()> {
        anyhow::ensure!(
            !OWNER_RUNNING.swap(true, Ordering::AcqRel),
            "Gateway owner already running in this process"
        );
        let lease = OwnerLease;
        let receiver = self
            .receiver
            .lock()
            .take()
            .context("Gateway owner already started")?;
        let fence = self.fence.clone();
        let engine = self.engine.clone();
        let file_managed = self.file_managed;
        std::thread::Builder::new().name("ion-gateway-automations".into()).spawn(move || {
            let mut warned = false; let mut connected = false;
            let _lease = lease;
            let mut cadence = Cadence::default();
            let mut last_regular = Instant::now() - POLL_INTERVAL;
            let mut next_regular = POLL_INTERVAL;
            let mut previous: Option<GatewayApi> = None;
            let mut published_projection = None;
            let mut publisher_lease: Option<PublisherLease> = None;
            while !fence.stopped() {
                let result = match CredentialService::global().gateway_api() {
                    Ok(api) => {
                        connected = true;
                        api.with_timeout(Duration::from_secs(5)).and_then(|api| {
                            let lock_key = api.owner_lock_key();
                            if publisher_lease.as_ref().is_none_or(|lease| !lease.matches(&lock_key)) {
                                publisher_lease = None;
                                publisher_lease = Some(PublisherLease::acquire(lock_key)?);
                            }
                            let rotated = previous.as_ref().is_none_or(|old| !old.same_pairing(&api));
                            if rotated { cadence.reset(); published_projection = None; }
                            let projection = engine.projection()?;
                            let changed = published_projection.as_ref() != Some(&projection);
                            if !cadence.should_poll(last_regular.elapsed(), next_regular, rotated, changed) {
                                return if engine.teams.pdf_transfer_active() && cadence.can_wake() {
                                    let pdf = api.with_owner_sync().and_then(|api| portfolio::sync_pdf(&api, &engine, &fence));
                                    if pdf.is_err() {
                                        next_regular = cadence.observe(&pdf.as_ref().map(|_| true));
                                        last_regular = Instant::now();
                                    }
                                    pdf
                                } else { Ok(()) };
                            }
                            last_regular = Instant::now();
                            // Remember the attempted pairing even if the request fails;
                            // otherwise every failure would look like a new pairing and
                            // reset connectivity backoff on the next local tick.
                            previous = Some(api.pairing_snapshot());
                            // A failed batch stops this pass; do not fan an auth or
                            // connectivity failure out into four more requests.
                            let result = api.with_owner_sync().and_then(|api| {
                                let active = api.owner_cycle_active() || changed;
                                let controls = engine.sync(&api, &fence);
                                let reads = portfolio::sync(&api, &engine, &fence);
                                let inbox = inbox::sync(&api, &engine, &fence);
                                let alerts = notifications::sync(&api, &engine, &fence);
                                let result = controls.and(reads).and(inbox).and(alerts);
                                // Include publication activity observed during the pass.
                                let active = active || api.owner_cycle_active();
                                previous = Some(api);
                                result.map(|_| active)
                            });
                            next_regular = cadence.observe(&result);
                            if result.is_ok() { published_projection = Some(projection); }
                            result.map(|_| ())
                        })
                    }
                    Err(_) if connected || file_managed => {
                        publisher_lease = None; previous = None; published_projection = None; cadence.reset();
                        Err(anyhow::anyhow!("Gateway owner pairing unavailable"))
                    },
                    Err(_) => {
                        previous = None; published_projection = None; cadence.reset();
                        publisher_lease = None;
                        Ok(()) // An unpaired desktop requires no gateway network work.
                    },
                };
                if result.is_err() && !warned {
                    // No credentials, definition bodies, paths or server payloads in logs.
                    eprintln!("Gateway workflow publisher unavailable; check owner pairing, publish grant and connectivity");
                    warned = true;
                } else if result.is_ok() { warned = false; }
                if fence.stopped() { break; }
                let interval = if engine.teams.pdf_transfer_active() && cadence.can_wake() { Duration::from_millis(500) } else { POLL_INTERVAL };
                match receiver.recv_timeout(interval) {
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
                    _ => {}
                }
            }
        }).context("Could not start gateway workflow publisher")?;
        Ok(())
    }
    pub(crate) fn shutdown(&self) {
        self.fence.stopping.store(true, Ordering::Release);
        // Only local DB mutation owns this gate, never network or credential I/O.
        // On return, no claimed command can start another local mutation.
        let _effects = self.fence.effects.lock();
        let _ = self.wake.try_send(());
    }
}
impl Drop for GatewayOwner {
    fn drop(&mut self) {
        self.shutdown();
    }
}

trait Relay {
    fn publisher(&self) -> &str;
    fn authorized(&self) -> Result<bool>;
    fn preflight_authorized(&self) -> Result<bool> {
        self.authorized()
    }
    fn supported(&self) -> Result<bool>;
    fn publish(&self, projection: serde_json::Value) -> Result<()>;
    fn pending(&self) -> Result<Vec<GatewayAutomationCommand>>;
    fn claim(&self, id: &str) -> Result<GatewayAutomationCommand>;
    fn uncertain(&self) -> Result<Vec<GatewayAutomationCommand>>;
    fn settle(&self, id: &str, applied: bool) -> Result<()>;
}
impl Relay for GatewayApi {
    fn publisher(&self) -> &str {
        self.identity().1
    }
    fn authorized(&self) -> Result<bool> {
        let current = CredentialService::global().gateway_api()?;
        Ok(self.same_pairing(&current) && self.has_publish_grant()?)
    }
    fn preflight_authorized(&self) -> Result<bool> {
        let current = CredentialService::global().gateway_api()?;
        Ok(self.same_pairing(&current) && self.owner_preflight_grant()?)
    }
    fn supported(&self) -> Result<bool> {
        self.has_automation_relay()
    }
    fn publish(&self, p: serde_json::Value) -> Result<()> {
        self.publish_automations(p)
    }
    fn pending(&self) -> Result<Vec<GatewayAutomationCommand>> {
        self.pending_automations()
    }
    fn claim(&self, id: &str) -> Result<GatewayAutomationCommand> {
        self.claim_automation(id)
    }
    fn uncertain(&self) -> Result<Vec<GatewayAutomationCommand>> {
        self.uncertain_automations()
    }
    fn settle(&self, id: &str, applied: bool) -> Result<()> {
        self.settle_automation(id, applied).map(|_| ())
    }
}
struct OwnerEngine {
    control: Arc<crate::control::ControlStore>,
    teams: Arc<TeamCoordinator>,
    tasks: Arc<Mutex<PersistedState>>,
    task_store: Arc<StateStore>,
    runtime: std::sync::Weak<crate::team_runtime::TeamRuntime>,
}
#[derive(Debug, PartialEq)]
enum Effect {
    Applied,
    Rejected,
    Unknown,
}
impl OwnerEngine {
    fn projection(&self) -> Result<serde_json::Value> {
        let snapshot = self.control.load()?;
        let projects = self.tasks.lock().projects.clone();
        let mut runs = Vec::new();
        let mut more = false;
        for pending_only in [true, false] {
            let page = self
                .teams
                .automation_runs(pending_only, None, None, None, 32)?;
            more |= page.next_cursor.is_some();
            for run in page.runs {
                if !runs.iter().any(|known: &AutomationRun| known.id == run.id) {
                    runs.push(run);
                }
            }
        }
        Ok(waku_protocol::gateway::automation_projection(
            &snapshot, &projects, &runs, more,
        ))
    }
    fn sync(&self, relay: &dyn Relay, fence: &StopFence) -> Result<()> {
        let started = Instant::now();
        if fence.stopped() || !relay.preflight_authorized()? || !relay.supported()? {
            return Ok(());
        }
        let projection = self.projection()?;
        if fence.stopped() {
            return Ok(());
        }
        relay.publish(projection)?;
        // The durable relay orders stops first. Bound both batch count and time;
        // slow endpoints cannot monopolize subsequent publication passes.
        for pending in relay.pending()?.into_iter().take(32) {
            if fence.stopped() || started.elapsed() >= PASS_BUDGET {
                break;
            }
            if pending.status != "queued"
                || pending.publisher_id != relay.publisher()
                || Uuid::parse_str(&pending.id).is_err()
            {
                continue;
            }
            let claimed = relay.claim(&pending.id)?;
            if claimed.id != pending.id
                || claimed.action != pending.action
                || claimed.publisher_id != relay.publisher()
                || claimed.status != "uncertain"
            {
                continue;
            }
            // Re-read pairing (including rotations) and publisher revocation after
            // the claim. A failed check leaves an unknown receipt, never a replay.
            if fence.stopped() || !relay.authorized()? {
                return Ok(());
            }
            let effect = {
                let _effects = fence.effects.lock();
                if fence.stopped() {
                    return Ok(());
                }
                self.apply(&claimed.action)
            };
            match effect {
                Effect::Applied => {
                    relay.settle(&claimed.id, true)?;
                }
                Effect::Rejected => {
                    relay.settle(&claimed.id, false)?;
                }
                Effect::Unknown => {}
            }
        }
        if fence.stopped() || started.elapsed() >= PASS_BUDGET {
            return Ok(());
        }
        for uncertain in relay.uncertain()?.into_iter().take(32) {
            if fence.stopped() || started.elapsed() >= PASS_BUDGET {
                break;
            }
            if uncertain.status != "uncertain"
                || uncertain.publisher_id != relay.publisher()
                || Uuid::parse_str(&uncertain.id).is_err()
            {
                continue;
            }
            if let GatewayAutomationAction::Start {
                definition_id,
                run_id,
                expected_revision,
            } = &uncertain.action
                && let Ok(id) = Uuid::parse_str(run_id)
                && let Ok(run) = self.teams.automation_run(id)
                && run.id == id
                && run
                    .routine_id
                    .or(run.workflow_id)
                    .is_some_and(|d| d.to_string() == *definition_id)
                && run.definition_revision == *expected_revision
            {
                // Reading this exact durable receipt is the only recovery. An
                // absent run or a gate/stop/schedule claim proves no outcome.
                relay.settle(&uncertain.id, true)?;
            }
        }
        Ok(())
    }
    fn apply(&self, action: &GatewayAutomationAction) -> Effect {
        let result = self.apply_checked(action);
        match result {
            Ok(true) => {
                if let Some(runtime) = self.runtime.upgrade() {
                    runtime.wake();
                }
                Effect::Applied
            }
            Ok(false) => Effect::Unknown, // Unexpected acknowledgement after a possible commit.
            // A DB/commit failure can be ambiguous. Ordinary revision and schema
            // rejections occur before transaction commit and are safe to reject.
            Err(error) if error.chain().any(|cause| cause.is::<rusqlite::Error>()) => {
                Effect::Unknown
            }
            Err(_) => Effect::Rejected,
        }
    }
    fn apply_checked(&self, action: &GatewayAutomationAction) -> Result<bool> {
        let now = crate::model::unix_time();
        let applied = match action {
            GatewayAutomationAction::Start {
                definition_id,
                run_id,
                expected_revision,
            } => {
                let projects = self.tasks.lock().projects.iter().map(|p| p.id).collect();
                let run = self.teams.start_project_automation(
                    &projects,
                    Uuid::parse_str(run_id)?,
                    Uuid::parse_str(definition_id)?,
                    *expected_revision,
                    now,
                )?;
                run.id.to_string() == *run_id
                    && run
                        .routine_id
                        .or(run.workflow_id)
                        .is_some_and(|d| d.to_string() == *definition_id)
                    && run.definition_revision == *expected_revision
            }
            GatewayAutomationAction::Stop {
                run_id,
                expected_revision,
            } => {
                let run = self.teams.stop_automation(
                    Uuid::parse_str(run_id)?,
                    *expected_revision,
                    now,
                )?;
                run.status == AutomationStatus::Canceled
            }
            GatewayAutomationAction::Gate {
                run_id,
                expected_revision,
                stage,
                approved,
            } => {
                let run = self.teams.resolve_automation_gate(
                    Uuid::parse_str(run_id)?,
                    *expected_revision,
                    *stage,
                    *approved,
                    now,
                )?;
                run.stages.get(usize::from(*stage)).is_some_and(|s| {
                    if *approved {
                        s.approved
                    } else {
                        run.current_stage == *stage && run.status == AutomationStatus::Canceled
                    }
                })
            }
            GatewayAutomationAction::Routine {
                definition_id,
                expected_revision,
                paused,
            } => {
                let id = Uuid::parse_str(definition_id)?;
                let snapshot =
                    self.teams
                        .set_routine_paused(id, *expected_revision, *paused, now)?;
                snapshot.revision > *expected_revision && snapshot.records.iter().any(|record| matches!(record, ControlRecord::Routine(r) if r.id == id && r.paused == *paused))
            }
        };
        Ok(applied)
    }
}

#[cfg(test)]
mod tests;

mod inbox;
mod notifications;
mod portfolio;
