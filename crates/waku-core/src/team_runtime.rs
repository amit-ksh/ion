//! Daemon-owned Teams dispatch, live event streams and durable transcripts.
//! Workers run outside the task-state lock; the coordinator alone mints leases.
use crate::{
    EventSink, RuntimePublisher,
    driver::{self, DriverEventSender, NativeTeamExit, NativeTeamStart},
    persistence::{PersistedState, StateStore},
    settings::DaemonSettingsStore,
    teams::{
        AssignmentLease, AssignmentOutcome, DispatchHost, SandboxStartFailure, TeamCoordinator,
    },
};
use anyhow::{Context, Result, ensure};
use crossbeam_channel::{Receiver, Sender, bounded};
use parking_lot::Mutex;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use uuid::Uuid;
use waku_protocol::{control::AgentProfile, model::*, teams::*};
mod projection;
#[cfg(test)]
mod tests;

// Actual worker adapters and scripted tests share the same trusted seam.
trait Executor: Send + Sync {
    fn ready(&self, profile: &AgentProfile) -> Result<DispatchHost>;
    fn remote_stopped(&self, _coordinator: &TeamCoordinator, _attempt: Uuid) -> Result<bool> {
        Ok(false)
    }
    fn run(
        &self,
        coordinator: &TeamCoordinator,
        start: NativeTeamStart,
        workspace: &Path,
        events: &DriverEventSender,
        cancelled: &AtomicBool,
    ) -> Result<NativeTeamExit>;
}
struct NativeExecutor {
    catalog: Mutex<Option<(Instant, Result<Vec<String>, String>)>>,
    claude: Mutex<HashMap<PathBuf, (Instant, Result<(), String>)>>,
    readiness: Mutex<HashMap<String, (Instant, Result<(), String>)>>,
    remote_checks: Mutex<HashMap<Uuid, Instant>>,
    settings: Arc<DaemonSettingsStore>,
}
impl Executor for NativeExecutor {
    fn ready(&self, profile: &AgentProfile) -> Result<DispatchHost> {
        if profile.execution.environment == ExecutionEnvironment::NebiusSandbox {
            let settings = self.settings.get();
            let key = format!(
                "remote:{}:{}:{}:{}",
                profile.provider.id(),
                profile.model.as_deref().unwrap_or_default(),
                profile
                    .execution
                    .sandbox_target
                    .as_deref()
                    .unwrap_or_default(),
                settings
                    .extra
                    .get("nebius_sandbox_enabled")
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
            );
            let remote = self.cached_ready(key, || driver::team_remote::ready(&settings, profile));
            if remote.is_ok() {
                return Ok(DispatchHost::NebiusSandbox {
                    target: profile
                        .execution
                        .sandbox_target
                        .clone()
                        .context("Sandbox target missing")?,
                });
            }
            if !profile.execution.local_fallback {
                remote?;
            }
            // No spawn/model request has been sent. Fallback verifies the
            // same selected local harness/model before an attempt is claimed.
        }
        if profile.provider == ProviderKind::Grok {
            let model = profile
                .model
                .as_deref()
                .context("Choose a pinned Grok model for this agent")?;
            let binary = self.binary(ProviderKind::Grok)?;
            self.cached_ready(format!("local:grok:{}:{model}", binary.display()), || {
                driver::team_grok::ready(&binary, model)
            })?;
            return local_host(profile);
        }
        if profile.provider == ProviderKind::Kimi {
            let model = profile
                .model
                .as_deref()
                .context("Choose a pinned Kimi model for this agent")?;
            let binary = self.binary(ProviderKind::Kimi)?;
            self.cached_ready(format!("local:kimi:{}:{model}", binary.display()), || {
                driver::team_kimi::ready(&binary, model)
            })?;
            return local_host(profile);
        }
        if profile.provider == ProviderKind::DeepSeek {
            let model = profile
                .model
                .as_deref()
                .context("Choose a pinned DeepSeek model for this agent")?;
            let binary = self.binary(ProviderKind::DeepSeek)?;
            self.cached_ready(
                format!("local:deepseek:{}:{model}", binary.display()),
                || driver::team_deepseek::ready(&binary, model),
            )?;
            return local_host(profile);
        }
        if profile.provider == ProviderKind::OhMyPi {
            let model = profile
                .model
                .as_deref()
                .context("Choose a pinned Oh My Pi model for this agent")?;
            let binary = self.binary(ProviderKind::OhMyPi)?;
            self.cached_ready(format!("local:omp:{}:{model}", binary.display()), || {
                driver::team_omp::ready(&binary, model)
            })?;
            return local_host(profile);
        }
        if profile.provider == ProviderKind::Pi {
            let model = profile
                .model
                .as_deref()
                .context("Choose a pinned Pi model for this agent")?;
            let binary = self.binary(ProviderKind::Pi)?;
            self.cached_ready(format!("local:pi:{}:{model}", binary.display()), || {
                driver::team_pi::ready(&binary, model)
            })?;
            return local_host(profile);
        }
        if profile.provider == ProviderKind::OpenCode {
            let model = profile
                .model
                .as_deref()
                .context("Choose a pinned OpenCode model for this agent")?;
            let binary = self.binary(ProviderKind::OpenCode)?;
            self.cached_ready(
                format!("local:opencode:{}:{model}", binary.display()),
                || driver::team_opencode::ready(&binary, model),
            )?;
            return local_host(profile);
        }
        if profile.provider == ProviderKind::Codex {
            let model = profile
                .model
                .as_deref()
                .context("Choose a pinned Codex model for this agent")?;
            let binary = self.binary(ProviderKind::Codex)?;
            self.cached_ready(format!("local:codex:{}:{model}", binary.display()), || {
                driver::team_codex::ready(&binary, model)
            })?;
            return local_host(profile);
        }
        if profile.provider == ProviderKind::Claude {
            let model = profile
                .model
                .as_deref()
                .context("Choose a pinned Claude model for this agent")?;
            driver::team_claude::allowance(model)?;
            let binary = self.claude_binary()?;
            let mut cache = self.claude.lock();
            if cache
                .get(&binary)
                .is_none_or(|(time, _)| time.elapsed() >= Duration::from_secs(30))
            {
                cache.insert(
                    binary.clone(),
                    (
                        Instant::now(),
                        driver::team_claude::ready(&binary).map_err(|e| e.to_string()),
                    ),
                );
            }
            cache
                .get(&binary)
                .unwrap()
                .1
                .as_ref()
                .map_err(|e| anyhow::anyhow!("Claude setup is required: {e}"))?;
            return local_host(profile);
        }
        ensure!(
            profile.provider == ProviderKind::Nebius,
            "{} does not yet expose a verified Teams completion contract. Supported Teams harnesses: Nebius, Claude, Codex, OpenCode, Pi, Oh My Pi, DeepSeek and Kimi. The selected harness is retained",
            profile.provider.display_name()
        );
        let model = profile
            .model
            .as_deref()
            .filter(|m| !m.trim().is_empty())
            .context("Select an explicit model for this agent")?;
        let mut cache = self.catalog.lock();
        if cache
            .as_ref()
            .is_none_or(|(time, _)| time.elapsed() >= Duration::from_secs(30))
        {
            *cache = Some((
                Instant::now(),
                crate::credentials::CredentialService::global()
                    .nebius_model_ids()
                    .map_err(|e| e.to_string()),
            ));
        }
        let models = cache
            .as_ref()
            .unwrap()
            .1
            .as_ref()
            .map_err(|e| anyhow::anyhow!("Nebius setup is required: {e}"))?;
        ensure!(
            models.iter().any(|m| m == model),
            "Pinned model is unavailable for this Nebius account; stop this run and start a new task after updating the agent model"
        );
        local_host(profile)
    }
    fn remote_stopped(&self, coordinator: &TeamCoordinator, attempt: Uuid) -> Result<bool> {
        if coordinator.remote_attempt_stopped(attempt)? {
            return Ok(true);
        }
        {
            let mut checks = self.remote_checks.lock();
            if checks
                .get(&attempt)
                .is_some_and(|t| t.elapsed() < Duration::from_secs(5))
            {
                return Ok(false);
            }
            if checks.len() >= 256 {
                checks.clear();
            }
            checks.insert(attempt, Instant::now());
        }
        driver::team_remote::reconcile(coordinator, attempt)
    }
    fn run(
        &self,
        coordinator: &TeamCoordinator,
        start: NativeTeamStart,
        workspace: &Path,
        events: &DriverEventSender,
        cancelled: &AtomicBool,
    ) -> Result<NativeTeamExit> {
        let assignment = match &start {
            NativeTeamStart::Fresh(c) => &c.assignment,
            NativeTeamStart::Resumed(c) => &c.claimed.assignment,
        };
        if assignment
            .attempts
            .last()
            .is_some_and(|a| a.environment == ExecutionEnvironment::NebiusSandbox)
        {
            return driver::team_remote::run(coordinator, start, workspace, events, cancelled);
        }
        let provider = match &start {
            NativeTeamStart::Fresh(c) => c.profile.provider,
            NativeTeamStart::Resumed(c) => c.claimed.profile.provider,
        };
        if provider == ProviderKind::Grok {
            driver::team_grok::run(
                coordinator,
                start,
                workspace,
                events,
                cancelled,
                &self.binary(ProviderKind::Grok)?,
            )
        } else if provider == ProviderKind::Kimi {
            driver::team_kimi::run(
                coordinator,
                start,
                workspace,
                events,
                cancelled,
                &self.binary(ProviderKind::Kimi)?,
            )
        } else if provider == ProviderKind::DeepSeek {
            driver::team_deepseek::run(
                coordinator,
                start,
                workspace,
                events,
                cancelled,
                &self.binary(ProviderKind::DeepSeek)?,
            )
        } else if provider == ProviderKind::OhMyPi {
            driver::team_omp::run(
                coordinator,
                start,
                workspace,
                events,
                cancelled,
                &self.binary(ProviderKind::OhMyPi)?,
            )
        } else if provider == ProviderKind::Pi {
            driver::team_pi::run(
                coordinator,
                start,
                workspace,
                events,
                cancelled,
                &self.binary(ProviderKind::Pi)?,
            )
        } else if provider == ProviderKind::OpenCode {
            driver::team_opencode::run(
                coordinator,
                start,
                workspace,
                events,
                cancelled,
                &self.binary(ProviderKind::OpenCode)?,
            )
        } else if provider == ProviderKind::Codex {
            driver::team_codex::run(
                coordinator,
                start,
                workspace,
                events,
                cancelled,
                &self.binary(ProviderKind::Codex)?,
            )
        } else if provider == ProviderKind::Claude {
            driver::team_claude::run(
                coordinator,
                start,
                workspace,
                events,
                cancelled,
                &self.claude_binary()?,
            )
        } else {
            driver::run_native_team_assignment(coordinator, start, workspace, events, cancelled)
        }
    }
}
impl NativeExecutor {
    fn cached_ready(&self, key: String, check: impl FnOnce() -> Result<()>) -> Result<()> {
        if let Some((time, result)) = self.readiness.lock().get(&key) {
            if time.elapsed() < Duration::from_secs(30) {
                return result.clone().map_err(anyhow::Error::msg);
            }
        }
        let result = check().map_err(|e| e.to_string());
        let mut cache = self.readiness.lock();
        if cache.len() >= 256 {
            cache.retain(|_, (time, _)| time.elapsed() < Duration::from_secs(30));
            if cache.len() >= 256 {
                cache.clear();
            }
        }
        cache.insert(key, (Instant::now(), result.clone()));
        result.map_err(anyhow::Error::msg)
    }
    fn claude_binary(&self) -> Result<PathBuf> {
        self.binary(ProviderKind::Claude)
    }
    fn binary(&self, provider: ProviderKind) -> Result<PathBuf> {
        crate::model::provider_probe(
            provider,
            self.settings
                .get()
                .provider_binary_overrides
                .get(&provider)
                .map(String::as_str),
        )
        .path
        .with_context(|| {
            format!(
                "Install {} or set its executable in Settings",
                provider.display_name()
            )
        })
    }
}
fn local_host(profile: &AgentProfile) -> Result<DispatchHost> {
    match profile.execution.environment {
        ExecutionEnvironment::Local => Ok(DispatchHost::Local),
        ExecutionEnvironment::NebiusSandbox if profile.execution.local_fallback => {
            Ok(DispatchHost::LocalFallback {
                failure: SandboxStartFailure::Unavailable,
            })
        }
        ExecutionEnvironment::NebiusSandbox => anyhow::bail!(
            "Register a Nebius Teams execution host or enable Local fallback for this agent"
        ),
    }
}
struct Job {
    run: Uuid,
    cancelled: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}
#[derive(Clone)]
struct Stream {
    run: Uuid,
    work: Uuid,
    runtime: Uuid,
    sink: EventSink,
}

pub(crate) struct TeamRuntime {
    coordinator: Arc<TeamCoordinator>,
    store: Arc<StateStore>,
    state: Arc<Mutex<PersistedState>>,
    settings: Arc<DaemonSettingsStore>,
    removed: Arc<Mutex<HashSet<Uuid>>>,
    executor: Arc<dyn Executor>,
    jobs: Mutex<HashMap<Uuid, Job>>,
    streams: Mutex<HashMap<Uuid, Stream>>,
    stopped: Mutex<HashMap<Uuid, (AssignmentLease, Option<String>)>>,
    publisher: Mutex<Option<RuntimePublisher>>,
    stopping: AtomicBool,
    wake: Sender<()>,
    wakes: Receiver<()>,
    scheduler: Mutex<Option<JoinHandle<()>>>,
    automation_tick: Mutex<Option<Instant>>,
}
impl TeamRuntime {
    pub(crate) fn new(
        coordinator: Arc<TeamCoordinator>,
        store: Arc<StateStore>,
        state: Arc<Mutex<PersistedState>>,
        settings: Arc<DaemonSettingsStore>,
        removed: Arc<Mutex<HashSet<Uuid>>>,
    ) -> Arc<Self> {
        Self::with_executor(
            coordinator,
            store,
            state,
            settings.clone(),
            removed,
            Arc::new(NativeExecutor {
                catalog: Mutex::new(None),
                claude: Mutex::new(HashMap::new()),
                readiness: Mutex::new(HashMap::new()),
                remote_checks: Mutex::new(HashMap::new()),
                settings,
            }),
        )
    }
    fn with_executor(
        coordinator: Arc<TeamCoordinator>,
        store: Arc<StateStore>,
        state: Arc<Mutex<PersistedState>>,
        settings: Arc<DaemonSettingsStore>,
        removed: Arc<Mutex<HashSet<Uuid>>>,
        executor: Arc<dyn Executor>,
    ) -> Arc<Self> {
        let (wake, wakes) = bounded(1);
        Arc::new(Self {
            coordinator,
            store,
            state,
            settings,
            removed,
            executor,
            jobs: Mutex::new(HashMap::new()),
            streams: Mutex::new(HashMap::new()),
            stopped: Mutex::new(HashMap::new()),
            publisher: Mutex::new(None),
            stopping: AtomicBool::new(false),
            wake,
            wakes,
            scheduler: Mutex::new(None),
            automation_tick: Mutex::new(None),
        })
    }
    pub(crate) fn start(self: &Arc<Self>, publisher: RuntimePublisher) -> Result<()> {
        let mut scheduler = self.scheduler.lock();
        ensure!(scheduler.is_none(), "Teams dispatcher already started");
        *self.publisher.lock() = Some(publisher);
        let weak = Arc::downgrade(self);
        let wakes = self.wakes.clone();
        *scheduler = Some(
            thread::Builder::new()
                .name("ion-team-dispatch".into())
                .spawn(move || Self::schedule(weak, wakes))?,
        );
        self.wake();
        Ok(())
    }
    pub(crate) fn wake(&self) {
        let _ = self.wake.try_send(());
    }
    pub(crate) fn runtime(&self, task: Uuid) -> Option<Uuid> {
        self.streams.lock().get(&task).map(|s| s.runtime)
    }
    fn stream_capacity_available(&self, task: Uuid) -> bool {
        let streams = self.streams.lock();
        streams.contains_key(&task) || streams.len() < 256
    }
    pub(crate) fn pinned(&self) -> HashSet<Uuid> {
        self.streams.lock().keys().copied().collect()
    }
    pub(crate) fn resume_checkpoint(
        &self,
        run: Uuid,
        work: Uuid,
        expected_revision: u64,
    ) -> Result<TeamRunSnapshot> {
        // The scheduler is the sole launcher. A blocked assignment cannot be
        // launched until this transaction queues it, so this absence check is stable.
        ensure!(
            !self.jobs.lock().contains_key(&work),
            "The assignment worker is still finishing; refresh and try again"
        );
        let snapshot =
            self.coordinator
                .queue_checkpoint_resume(run, work, expected_revision, unix_time())?;
        self.wake();
        Ok(snapshot)
    }
    pub(crate) fn confirm_recovery(
        &self,
        run: Uuid,
        work: Uuid,
        attempt: Uuid,
        revision: u64,
        confirmation: waku_protocol::teams::TeamRecoveryConfirmation,
    ) -> Result<(waku_protocol::teams::TeamRecoveryReview, TeamRunSnapshot)> {
        ensure!(
            !self.jobs.lock().contains_key(&work),
            "The assignment worker is still finishing; refresh and try again"
        );
        if self.coordinator.recovery_confirmation_replayed(
            run,
            work,
            attempt,
            revision,
            &confirmation,
        )? {
            return self.coordinator.recovery_review(run, work);
        }
        let mut verified_revision = None;
        match &confirmation {
            TeamRecoveryConfirmation::BindRemoteOperation {
                request_id,
                operation_id,
            } => {
                driver::team_remote::bind_recovered_operation(
                    &self.coordinator,
                    run,
                    work,
                    attempt,
                    revision,
                    *request_id,
                    *operation_id,
                )?;
                self.wake();
                return self.coordinator.recovery_review(run, work);
            }
            TeamRecoveryConfirmation::VerifyRemoteStopped => {
                let snapshot = self.coordinator.snapshot(run)?;
                ensure!(
                    snapshot.revision == revision
                        && snapshot.work.iter().any(|w| w.id == work
                            && w.attempts.last().is_some_and(|a| a.id == attempt
                                && a.environment == ExecutionEnvironment::NebiusSandbox)),
                    "Remote recovery attempt or revision changed"
                );
                ensure!(
                    self.executor.remote_stopped(&self.coordinator, attempt)?,
                    "Sandbox operation is still stopping. Refresh recovery and verify again; no new work was submitted"
                );
                verified_revision = Some(self.coordinator.snapshot(run)?.revision);
            }
            _ => {}
        }
        let result = if verified_revision.is_some() {
            self.coordinator.confirm_recovery_verified(
                run,
                work,
                attempt,
                revision,
                confirmation,
                verified_revision,
                unix_time(),
            )?
        } else {
            self.coordinator.confirm_recovery(
                run,
                work,
                attempt,
                revision,
                confirmation,
                unix_time(),
            )?
        };
        self.wake();
        Ok(result)
    }
    pub(crate) fn cancel_run(&self, run: Uuid) {
        self.cancel_jobs(run);
        let started = self.publisher.lock().is_some();
        if started {
            if let Ok(snapshot) = self.coordinator.snapshot(run) {
                for work in &snapshot.work {
                    if !self.jobs.lock().contains_key(&work.id) {
                        let _ = self.sync_inactive(&snapshot, work);
                    }
                }
            }
        }
        self.wake();
    }
    pub(crate) fn publish_followup(
        &self,
        task: Uuid,
        message: crate::model::QueuedMessage,
    ) -> Result<()> {
        if let Some(stream) = self.streams.lock().get(&task).cloned() {
            let mut state = self.state.lock();
            let session = state.session_mut(task).context("Team task missing")?;
            let event = DriverEvent::TeamFollowUpQueued { message };
            let mut projection = projection::Projection::existing(session);
            projection.apply(session, &event);
            session.runtime_event_cursor =
                Some(stream.sink.record(crate::daemon::event_to_wire(event)?)?);
            state.mark_session_dirty(task);
            self.store.save(&mut state)?;
        }
        if let Some(publisher) = self.publisher.lock().as_ref() {
            publisher.tasks_changed();
        }
        Ok(())
    }
    fn cancel_jobs(&self, run: Uuid) {
        for job in self.jobs.lock().values().filter(|j| j.run == run) {
            job.cancelled.store(true, Ordering::Release);
        }
    }
    pub(crate) fn shutdown(&self) {
        self.stopping.store(true, Ordering::Release);
        for job in self.jobs.lock().values() {
            job.cancelled.store(true, Ordering::Release);
        }
        self.wake();
    }
    fn publisher(&self) -> RuntimePublisher {
        self.publisher
            .lock()
            .as_ref()
            .expect("Started dispatcher")
            .clone()
    }
    fn schedule(weak: Weak<Self>, wakes: Receiver<()>) {
        let mut after = None;
        loop {
            let Some(runtime) = weak.upgrade() else { break };
            if runtime.stopping.load(Ordering::Acquire) {
                break;
            }
            if let Err(error) = runtime.tick(&mut after) {
                eprintln!("Teams dispatch: {error:#}");
            }
            drop(runtime);
            let _ = wakes.recv_timeout(Duration::from_millis(500));
        }
    }
    fn tick(self: &Arc<Self>, after: &mut Option<Uuid>) -> Result<()> {
        let finished = {
            let mut jobs = self.jobs.lock();
            let ids: Vec<_> = jobs
                .iter()
                .filter(|(_, j)| j.thread.is_finished())
                .map(|(id, _)| *id)
                .collect();
            ids.into_iter()
                .filter_map(|id| jobs.remove(&id))
                .collect::<Vec<_>>()
        };
        for job in finished {
            let _ = job.thread.join();
        }
        // A transient database failure after join must not discard the known
        // Local host outcome. Retry confirmation, never the provider effects.
        let stopped: Vec<_> = self.stopped.lock().values().cloned().collect();
        for (lease, reason) in stopped {
            if self.confirm_stopped(&lease, reason).is_ok() {
                self.stopped.lock().remove(&lease.work_id());
            }
        }
        // A stopped run can have left the dispatch index while its parked
        // stream still needs a terminal event. Capacity is released only by
        // actual worker completion; parked workers have already joined.
        let streams: Vec<_> = self
            .streams
            .lock()
            .iter()
            .map(|(task, s)| (*task, s.clone()))
            .collect();
        for (task, stream) in streams {
            if self.jobs.lock().contains_key(&stream.work) {
                continue;
            }
            let run = self
                .coordinator
                .reconcile_expired(stream.run, unix_time())?;
            if let Some(work) = run.work.iter().find(|w| w.id == stream.work) {
                self.sync_inactive(&run, work)?;
            }
            if self.removed.lock().contains(&task) {
                self.streams.lock().remove(&task);
                self.publisher().finish(task, stream.runtime);
            }
        }
        let automation_due = {
            let mut last = self.automation_tick.lock();
            if last.is_none_or(|at| at.elapsed() >= Duration::from_secs(1)) {
                *last = Some(Instant::now());
                true
            } else {
                false
            }
        };
        if automation_due {
            let projects = self.state.lock().projects.iter().map(|p| p.id).collect();
            if !self
                .coordinator
                .tick_automations(&projects, unix_time())?
                .is_empty()
            {
                self.publisher().tasks_changed();
            }
        }
        let runs = self.coordinator.dispatch_runs(*after, 32)?;
        if runs.is_empty() {
            *after = None;
            return Ok(());
        }
        *after = runs.last().map(|r| r.id);
        for run in runs {
            if let Err(error) = self.dispatch_run(run) {
                eprintln!("Teams run dispatch: {error:#}");
            }
        }
        Ok(())
    }
    fn dispatch_run(self: &Arc<Self>, snapshot: TeamRunSnapshot) -> Result<()> {
        let run = self
            .coordinator
            .reconcile_expired(snapshot.id, unix_time())?;
        if run.status == TeamRunStatus::Stopping {
            self.cancel_jobs(run.id);
        }
        self.sync_review_statuses(&run)?;
        if run.status == TeamRunStatus::Active {
            let count = {
                let mut state = self.state.lock();
                let tasks = self.coordinator.materialize_run(
                    run.id,
                    &self.store,
                    &mut state,
                    unix_time(),
                )?;
                tasks
                    + self
                        .coordinator
                        .materialize_followups(run.id, &self.store, &mut state)?
            };
            if count > 0 {
                self.publisher().tasks_changed();
            }
        }
        for work in &run.work {
            if let Some(job) = self.jobs.lock().get(&work.id) {
                if work.status == AssignmentStatus::Uncertain {
                    job.cancelled.store(true, Ordering::Release);
                }
                continue;
            }
            if work.status == AssignmentStatus::Waiting
                && self.stream_capacity_available(work.task_id)
            {
                if let Some(attempt) = work.attempts.last() {
                    self.stream(run.id, work.id, work.task_id, attempt.id);
                }
            }
            if work.status == AssignmentStatus::Uncertain {
                if let Some(attempt) = work
                    .attempts
                    .last()
                    .filter(|a| a.environment == ExecutionEnvironment::NebiusSandbox)
                {
                    if self
                        .executor
                        .remote_stopped(&self.coordinator, attempt.id)
                        .unwrap_or(false)
                    {
                        let outcome = if run.status == TeamRunStatus::Stopping {
                            AssignmentOutcome::Canceled
                        } else {
                            AssignmentOutcome::Blocked("Remote process confirmed stopped; no request was repeated. Review retained usage and checkpoint before starting further work.".into())
                        };
                        self.coordinator.resolve_uncertain(
                            run.id,
                            work.id,
                            attempt.id,
                            outcome,
                            unix_time(),
                        )?;
                    }
                }
            }
            if !matches!(
                work.status,
                AssignmentStatus::Queued | AssignmentStatus::Waiting
            ) {
                self.sync_inactive(&run, work)?;
                continue;
            }
            if run.status != TeamRunStatus::Active
                || self.jobs.lock().len() >= 16
                || !self.stream_capacity_available(work.task_id)
            {
                continue;
            }
            let profile = run
                .profiles
                .iter()
                .find(|p| p.id == work.agent_id)
                .context("Pinned profile missing")?;
            let ready = (|| {
                waku_protocol::distribution::ProviderPolicy::require_new_task(profile.provider)?;
                ensure!(
                    !self
                        .settings
                        .get()
                        .disabled_providers
                        .contains(&profile.provider),
                    "Selected harness is disabled in Settings"
                );
                ensure!(
                    !self.removed.lock().contains(&work.task_id),
                    "Team task was removed"
                );
                let workspace = {
                    let state = self.state.lock();
                    let task = state
                        .sessions
                        .iter()
                        .find(|s| s.id == work.task_id)
                        .context("Team task missing")?;
                    let project = state
                        .projects
                        .iter()
                        .find(|p| p.id == run.project_id)
                        .context("Team project missing")?;
                    task.workspace.path().unwrap_or(&project.path).to_owned()
                };
                ensure!(workspace.is_dir(), "Team project folder is unavailable");
                // Continuation readiness follows its actual pinned host. Fallback
                // remains a pre-execution choice and cannot move an existing attempt.
                let mut readiness_profile = profile.clone();
                if work.status == AssignmentStatus::Waiting {
                    readiness_profile.execution.environment = work
                        .attempts
                        .last()
                        .context("Continuation attempt missing")?
                        .environment;
                    readiness_profile.execution.local_fallback = false;
                }
                let host = self.executor.ready(&readiness_profile)?;
                let workspace = match &host {
                    DispatchHost::Local | DispatchHost::LocalFallback { .. } => self
                        .coordinator
                        .prepare_local_workspace(run.id, &workspace, unix_time())?,
                    DispatchHost::NebiusSandbox { .. } => self
                        .coordinator
                        .prepare_local_workspace(run.id, &workspace, unix_time())?,
                };
                Ok((workspace, host))
            })();
            let (workspace, host) = match ready {
                Ok(value) => value,
                Err(error) => {
                    let reason = bounded_reason(&error);
                    if work.status == AssignmentStatus::Queued
                        && work.reason.as_deref() != Some(&reason)
                    {
                        self.coordinator.defer_dispatch(
                            run.id,
                            work.id,
                            Some(reason.clone()),
                            unix_time(),
                        )?;
                        self.setup_message(work.task_id, AssignmentStatus::Queued, &reason)?;
                    }
                    continue;
                }
            };
            let start = if work.status == AssignmentStatus::Queued {
                match self
                    .coordinator
                    .claim(run.id, work.id, host, unix_time(), 180)
                {
                    Ok(claim) => NativeTeamStart::Fresh(claim),
                    Err(_) => continue,
                }
            } else {
                match self
                    .coordinator
                    .resume_after_children(run.id, work.id, unix_time(), 180)
                {
                    Ok(continuation) => NativeTeamStart::Resumed(continuation),
                    Err(_) => continue,
                }
            };
            self.launch(start, workspace)?;
        }
        Ok(())
    }
    // A parent can accept or supersede an already detached child. Persist
    // metadata in one background pass, including when the run has just ended
    // and therefore no longer appears in the active dispatch index.
    fn sync_review_statuses(&self, run: &TeamRunSnapshot) -> Result<()> {
        let mut state = self.state.lock();
        let mut changed = false;
        for work in run.work.iter().filter(|w| {
            matches!(
                w.status,
                AssignmentStatus::Completed
                    | AssignmentStatus::InReview
                    | AssignmentStatus::Superseded
            )
        }) {
            let needs_update = state
                .sessions
                .iter()
                .find(|s| s.id == work.task_id)
                .is_some_and(|s| !s.detail_loaded || s.team_assignment_status != Some(work.status));
            if needs_update {
                let task = state.session_mut(work.task_id).unwrap();
                self.store.hydrate(task)?;
                task.team_assignment_status = Some(work.status);
                changed = true;
            }
        }
        if changed {
            self.store.save(&mut state)?;
        }
        drop(state);
        if changed {
            self.publisher().tasks_changed();
        }
        Ok(())
    }
    fn setup_message(&self, task: Uuid, status: AssignmentStatus, reason: &str) -> Result<()> {
        let mut state = self.state.lock();
        let mut changed = false;
        let task_status = if status == AssignmentStatus::Canceled {
            SessionStatus::Idle
        } else {
            SessionStatus::Failed
        };
        if let Some(index) = state.sessions.iter().position(|s| s.id == task) {
            let session = &mut state.sessions[index];
            self.store.hydrate(session)?;
            if session.status != task_status
                || session.team_assignment_status != Some(status)
                || session
                    .messages
                    .last()
                    .is_none_or(|m| m.role != MessageRole::System || m.content != reason)
            {
                session.status = task_status;
                session.team_assignment_status = Some(status);
                session.push_message(MessageRole::System, reason);
                state.mark_session_dirty(task);
                self.store.save(&mut state)?;
                changed = true;
            }
        }
        drop(state);
        if changed {
            self.publisher().tasks_changed();
        }
        Ok(())
    }
    fn stream(&self, run: Uuid, work: Uuid, task: Uuid, runtime: Uuid) -> Stream {
        let mut streams = self.streams.lock();
        if let Some(stream) = streams.get(&task).filter(|s| s.runtime == runtime) {
            return stream.clone();
        }
        let stream = Stream {
            run,
            work,
            runtime,
            sink: self.publisher().begin(task, runtime),
        };
        streams.insert(task, stream.clone());
        stream
    }
    fn launch(self: &Arc<Self>, start: NativeTeamStart, workspace: PathBuf) -> Result<()> {
        let lease = match &start {
            NativeTeamStart::Fresh(c) => &c.lease,
            NativeTeamStart::Resumed(c) => &c.claimed.lease,
        }
        .clone();
        let task = match &start {
            NativeTeamStart::Fresh(c) => c.assignment.task_id,
            NativeTeamStart::Resumed(c) => c.claimed.assignment.task_id,
        };
        let stream = self.stream(lease.run_id(), lease.work_id(), task, lease.attempt_id());
        let pinned_workspace = self.coordinator.snapshot(lease.run_id())?.workspace;
        let projection = (|| -> Result<_> {
            let mut state = self.state.lock();
            let session = state
                .session_mut(task)
                .context("Team task removed before launch")?;
            self.store.hydrate(session)?;
            if session
                .runtime_event_cursor
                .is_some_and(|cursor| cursor.runtime_id != stream.runtime)
            {
                session.runtime_event_cursor = None;
            }
            if let Some(pinned) = pinned_workspace {
                session.workspace = SessionWorkspace::Worktree {
                    path: pinned.path,
                    branch: pinned.branch,
                };
            }
            let projection = projection::Projection::new(session);
            session.status = SessionStatus::Connecting;
            session.team_assignment_status = Some(AssignmentStatus::Running);
            self.store.save(&mut state)?;
            Ok(projection)
        })();
        let projection = match projection {
            Ok(projection) => projection,
            Err(error) => {
                self.confirm_stopped(&lease, Some(bounded_reason(&error)))?;
                return Err(error);
            }
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let cancelled = cancel.clone();
        let runtime = self.clone();
        let work = lease.work_id();
        let run = lease.run_id();
        let recovery = lease.clone();
        // Insert only after successful spawn; the scheduler is the sole launcher.
        let handle = match thread::Builder::new()
            .name(format!("ion-team-{work}"))
            .spawn(move || {
                runtime.collect(start, workspace, lease, task, stream, projection, cancelled)
            }) {
            Ok(handle) => handle,
            Err(error) => {
                self.confirm_stopped(&recovery, Some(error.to_string()))?;
                self.streams.lock().remove(&task);
                self.publisher().finish(task, recovery.attempt_id());
                return Err(error.into());
            }
        };
        self.jobs.lock().insert(
            work,
            Job {
                run,
                cancelled: cancel,
                thread: handle,
            },
        );
        if self.stopping.load(Ordering::Acquire)
            || self.coordinator.snapshot(run)?.status != TeamRunStatus::Active
        {
            self.cancel_run(run);
        }
        self.publisher().tasks_changed();
        Ok(())
    }
    fn collect(
        self: Arc<Self>,
        start: NativeTeamStart,
        workspace: PathBuf,
        lease: AssignmentLease,
        task: Uuid,
        stream: Stream,
        mut projection: projection::Projection,
        cancelled: Arc<AtomicBool>,
    ) {
        let (wake, _) = smol::channel::bounded(1);
        let (events, receiver) = driver::event_channel(wake);
        let executor = self.executor.clone();
        let coordinator = self.coordinator.clone();
        let cancel = cancelled.clone();
        let worker = thread::Builder::new()
            .name("ion-team-worker".into())
            .spawn(move || executor.run(&coordinator, start, &workspace, &events, &cancel));
        let mut persisted = Instant::now();
        let mut dirty = false;
        loop {
            match receiver.recv_timeout(Duration::from_millis(120)) {
                Ok(event) => {
                    let boundary = matches!(
                        event,
                        DriverEvent::TurnStarted
                            | DriverEvent::TurnParked
                            | DriverEvent::TurnFinished { .. }
                    );
                    let result = (|| -> Result<()> {
                        let mut state = self.state.lock();
                        let session = state.session_mut(task).context("Team task was removed")?;
                        let wire = crate::daemon::event_to_wire(event.clone())?;
                        projection.apply(session, &event);
                        session.runtime_event_cursor = Some(stream.sink.record(wire)?);
                        Ok(())
                    })();
                    if let Err(error) = result {
                        cancelled.store(true, Ordering::Release);
                        eprintln!("Teams projection: {error:#}");
                    } else {
                        dirty = true;
                    }
                    if boundary {
                        self.publisher().tasks_changed();
                    }
                }
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
            }
            if dirty && persisted.elapsed() >= Duration::from_millis(120) {
                if let Err(error) = self.store.save(&mut self.state.lock()) {
                    cancelled.store(true, Ordering::Release);
                    eprintln!("Teams transcript persistence: {error}");
                }
                persisted = Instant::now();
                dirty = false;
            }
        }
        let outcome = worker
            .map_err(anyhow::Error::from)
            .and_then(|handle| {
                handle
                    .join()
                    .map_err(|_| anyhow::anyhow!("Team worker panicked"))
            })
            .and_then(|result| result);
        let yielded = matches!(outcome, Ok(NativeTeamExit::Yielded));
        let reason = outcome.err().map(|error| bounded_reason(&error));
        let confirmed = match self.confirm_stopped(&lease, reason.clone()) {
            Ok(()) => true,
            Err(error) => {
                self.stopped
                    .lock()
                    .insert(lease.work_id(), (lease.clone(), reason));
                eprintln!("Teams stopped-host reconciliation: {error:#}");
                false
            }
        };
        if !yielded && confirmed {
            match self.emit_terminal(task, &stream, &mut projection) {
                Ok(()) => {
                    self.streams.lock().remove(&task);
                    self.publisher().finish(task, stream.runtime);
                }
                Err(error) => eprintln!("Teams terminal projection: {error:#}"),
            }
        }
        if let Err(error) = self.store.save(&mut self.state.lock()) {
            eprintln!("Teams final persistence: {error}");
        }
        if let Err(error) = self
            .coordinator
            .snapshot(lease.run_id())
            .and_then(|run| self.sync_review_statuses(&run))
        {
            eprintln!("Teams review status persistence: {error:#}");
        }
        self.publisher().tasks_changed();
        self.wake();
    }
    /// This method is reached only after the local worker has joined. Unknown
    /// remote outcomes are never resolved here, and no automatic retry is queued.
    fn confirm_stopped(&self, lease: &AssignmentLease, reason: Option<String>) -> Result<()> {
        let run = self
            .coordinator
            .reconcile_expired(lease.run_id(), unix_time())?;
        let work = run
            .work
            .iter()
            .find(|w| w.id == lease.work_id())
            .context("Assignment missing")?;
        if !matches!(
            work.status,
            AssignmentStatus::Running | AssignmentStatus::Canceling | AssignmentStatus::Uncertain
        ) {
            return Ok(());
        }
        let attempt = work
            .attempts
            .last()
            .filter(|a| a.id == lease.attempt_id())
            .context("Stopped worker attempt changed")?;
        if attempt.environment == ExecutionEnvironment::NebiusSandbox {
            ensure!(
                self.executor
                    .remote_stopped(&self.coordinator, attempt.id)?,
                "Remote process stop is unconfirmed; no fallback/retry is allowed"
            );
        }
        let outcome = if run.status == TeamRunStatus::Stopping
            || work.status == AssignmentStatus::Canceling
        {
            AssignmentOutcome::Canceled
        } else {
            AssignmentOutcome::Blocked(
                reason.unwrap_or_else(|| "Worker exited without a durable result".into()),
            )
        };
        if work.status == AssignmentStatus::Uncertain {
            self.coordinator.resolve_uncertain(
                run.id,
                work.id,
                lease.attempt_id(),
                outcome,
                unix_time(),
            )?;
        } else {
            self.coordinator.report(lease, outcome, unix_time())?;
        }
        Ok(())
    }
    fn emit_terminal(
        &self,
        task: Uuid,
        stream: &Stream,
        projection: &mut projection::Projection,
    ) -> Result<()> {
        let run = self.coordinator.snapshot(stream.run)?;
        let work = run
            .work
            .iter()
            .find(|w| w.id == stream.work)
            .context("Assignment missing")?;
        let mut state = self.state.lock();
        if let Some(session) = state.session_mut(task) {
            if session.active_turn_id().is_some() {
                if let Some(reason) = &work.reason {
                    let event = DriverEvent::Error(reason.clone());
                    projection.apply(session, &event);
                    session.runtime_event_cursor =
                        Some(stream.sink.record(crate::daemon::event_to_wire(event)?)?);
                }
                let event = DriverEvent::TurnFinished {
                    success: matches!(
                        work.status,
                        AssignmentStatus::Completed
                            | AssignmentStatus::InReview
                            | AssignmentStatus::Superseded
                    ),
                    summary: None,
                };
                projection.apply(session, &event);
                session.runtime_event_cursor =
                    Some(stream.sink.record(crate::daemon::event_to_wire(event)?)?);
                if work.status == AssignmentStatus::Canceled {
                    session.status = SessionStatus::Idle;
                    if let Some(turn) = session.turns.last_mut() {
                        turn.status = TurnStatus::Interrupted;
                    }
                }
            }
            session.team_assignment_status = Some(work.status);
            session.runtime_event_cursor = Some(
                stream
                    .sink
                    .record(crate::daemon::event_to_wire(DriverEvent::ProcessExited)?)?,
            );
        }
        self.store.save(&mut state)?;
        Ok(())
    }
    fn sync_inactive(&self, run: &TeamRunSnapshot, work: &TeamAssignment) -> Result<()> {
        let stream = self.streams.lock().get(&work.task_id).cloned();
        if let Some(stream) = stream {
            if stream.run == run.id
                && matches!(
                    work.status,
                    AssignmentStatus::Completed
                        | AssignmentStatus::InReview
                        | AssignmentStatus::Superseded
                        | AssignmentStatus::Blocked
                        | AssignmentStatus::Failed
                        | AssignmentStatus::Canceled
                )
            {
                let mut projection = {
                    let mut state = self.state.lock();
                    let Some(session) = state.session_mut(work.task_id) else {
                        return Ok(());
                    };
                    self.store.hydrate(session)?;
                    projection::Projection::existing(session)
                };
                self.emit_terminal(work.task_id, &stream, &mut projection)?;
                self.streams.lock().remove(&work.task_id);
                self.publisher().finish(work.task_id, stream.runtime);
                self.publisher().tasks_changed();
            }
        } else if matches!(
            work.status,
            AssignmentStatus::Uncertain
                | AssignmentStatus::Blocked
                | AssignmentStatus::Failed
                | AssignmentStatus::Canceled
        ) {
            if let Some(reason) = &work.reason {
                self.setup_message(work.task_id, work.status, reason)?;
            }
        }
        Ok(())
    }
}
fn bounded_reason(error: &anyhow::Error) -> String {
    format!("{error:#}").chars().take(1000).collect()
}
