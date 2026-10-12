//! Durable team coordination. All methods run on daemon workers, never in render.
//!
//! Definitions are immutable within a run. Task IDs are allocated before dispatch;
//! an outbox bridges the control/task databases without pretending they share a
//! transaction. A lease is an internal capability minted by the coordinator, not
//! an agent ID accepted from a model. Harness adapters must enforce access before
//! using this interface. This module does not launch providers or select hosts.
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde::Serialize;
use std::{collections::HashSet, path::PathBuf};
use uuid::Uuid;
use waku_protocol::{control::*, teams::*};

mod apps;
mod automation;
mod budget;
mod checks;
mod continuation;
mod decisions;
mod dispatch;
mod documents;
mod followups;
pub(crate) mod gpu_jobs;
mod inputs;
pub(crate) mod jobs;
mod materialize;
pub(crate) mod project;
mod projections;
mod recovery;
pub(crate) mod remote;
mod remote_control;
mod reviews;
mod storage;
#[cfg(test)]
mod tests;
mod workspace;
pub use apps::{AppActionStart, ApprovedAppAction};
pub(crate) use automation::{
    remove_definition as remove_automation_definition,
    sync_definition as sync_automation_definition,
    validate_definition as validate_automation_definition,
};
pub use budget::{ModelCallAllowance, ModelCallReservation, ModelTokenUsage};
pub use continuation::{AssignmentContinuation, AssignmentHandoff};
pub(crate) use inputs::PreparedInput;
pub use reviews::ReviewRequest;
use storage::*;

pub struct TeamCoordinator {
    control: crate::control::ControlStore,
}

#[derive(Clone, Serialize)]
pub struct PrepareRun {
    pub id: Uuid,
    pub root_task_id: Uuid,
    pub project_id: Uuid,
    pub team_id: Uuid,
    pub expected_definition_revision: u64,
    pub objective: String,
    pub limits: TeamRunLimits,
    /// Already intersected with project/task authorization by the trusted caller.
    pub access: AgentAccess,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_prompt: Option<String>,
    pub(crate) input_data: Vec<PreparedInput>,
    pub(crate) source_attachments: Vec<waku_protocol::model::MessageAttachment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<TeamTaskOrigin>,
}

pub struct Delegation {
    /// Stable tool invocation ID. Repeated IDs must carry identical arguments.
    pub request_id: Uuid,
    pub agent_id: Uuid,
    pub objective: String,
    pub expected_output: String,
    pub dependencies: Vec<Uuid>,
    pub access: AgentAccess,
}

/// Opaque and process-local; bridges bind it to their authenticated driver session.
#[derive(Clone, Debug)]
pub struct AssignmentLease {
    run_id: Uuid,
    work_id: Uuid,
    attempt_id: Uuid,
    generation: u32,
}
impl AssignmentLease {
    pub fn run_id(&self) -> Uuid {
        self.run_id
    }
    pub fn work_id(&self) -> Uuid {
        self.work_id
    }
    pub fn attempt_id(&self) -> Uuid {
        self.attempt_id
    }
}
pub struct ClaimedAssignment {
    pub lease: AssignmentLease,
    pub assignment: TeamAssignment,
    pub profile: AgentProfile,
}
pub struct TaskMaterialization {
    pub run_id: Uuid,
    pub project_id: Uuid,
    pub assignment: TeamAssignment,
    pub profile: AgentProfile,
    pub binding: TeamTaskBinding,
    pub source_prompt: Option<String>,
    pub source_attachments: Vec<waku_protocol::model::MessageAttachment>,
}

/// Explicit host supplied after adapter readiness checks, never workload routing.
pub enum DispatchHost {
    Local,
    NebiusSandbox {
        target: String,
    },
    /// Only a confirmed failure before remote execution is eligible here.
    LocalFallback {
        failure: SandboxStartFailure,
    },
}
pub enum SandboxStartFailure {
    Unavailable,
    HarnessUnavailable,
    AuthenticationFailed,
    StartupFailed,
}
impl SandboxStartFailure {
    fn reason(&self) -> &'static str {
        match self {
            Self::Unavailable => "Sandbox unavailable before execution",
            Self::HarnessUnavailable => "Selected harness unavailable in sandbox",
            Self::AuthenticationFailed => "Sandbox authentication failed before execution",
            Self::StartupFailed => "Sandbox failed to start the assignment",
        }
    }
}
#[derive(Serialize)]
pub enum AssignmentOutcome {
    Completed(AssignmentResult),
    Blocked(String),
    /// The adapter confirms no effects occurred; this is the only retryable failure.
    FailedBeforeEffects(String),
    Uncertain(String),
    /// The host confirms its process stopped. A cancel request is insufficient.
    Canceled,
}

pub fn intersect_access(a: &AgentAccess, b: &AgentAccess) -> AgentAccess {
    AgentAccess {
        workspace_read: a.workspace_read && b.workspace_read,
        workspace_write: a.workspace_write && b.workspace_write,
        shell: a.shell && b.shell,
        network: a.network && b.network,
        app_actions: a
            .app_actions
            .iter()
            .filter(|grant| b.app_actions.contains(grant))
            .cloned()
            .collect(),
    }
}
pub fn validate_limits(limits: &TeamRunLimits) -> Result<()> {
    ensure!(
        (1..=256).contains(&limits.max_work_items)
            && (1..=16).contains(&limits.max_parallel_tasks)
            && (1..=16).contains(&limits.max_depth)
            && (1..=8).contains(&limits.max_attempts)
            && limits.max_rework_rounds <= 8
            && (60..=604800).contains(&limits.timeout_seconds),
        "Invalid team execution limits"
    );
    ensure!(
        (1..=4096).contains(&limits.max_model_calls),
        "Team model-call limit must be 1-4096"
    );
    ensure!(
        limits
            .max_tokens
            .is_none_or(|tokens| (1..=1_000_000_000).contains(&tokens)),
        "Team token budget must be 1-1000000000"
    );
    Ok(())
}
pub fn validate_profile(profile: &AgentProfile) -> Result<()> {
    ensure!(
        (1..=16).contains(&profile.max_parallel_tasks),
        "Agent concurrency must be 1–16"
    );
    ensure!(
        profile.expected_output.len() <= 16_000,
        "Expected output is too long"
    );
    if let Some(model) = &profile.model {
        text(model, 512, "Model")?;
    }
    if let Some(target) = &profile.execution.sandbox_target {
        text(target, 160, "Sandbox target")?;
        ensure!(
            !target.contains(['\n', '\r', '\0']),
            "Sandbox target must be one line"
        );
    }
    validate_access(&profile.access)
}
fn validate_access(access: &AgentAccess) -> Result<()> {
    ensure!(
        !access.workspace_write || access.workspace_read,
        "Workspace write requires read access"
    );
    ensure!(
        access.app_actions.len() <= 64,
        "At most 64 app action grants are allowed"
    );
    let mut grants = HashSet::new();
    for grant in &access.app_actions {
        text(&grant.account_id, 256, "App account")?;
        text(&grant.action, 256, "App action")?;
        ensure!(
            grant.version.len() <= 128 && grant.version != "latest" && grant.version != "*",
            "App action must pin an exact version"
        );
        ensure!(
            grant.account_id != "*" && grant.action != "*",
            "App grants must name an exact account and action"
        );
        ensure!(
            grants.insert((&grant.account_id, &grant.action)),
            "Duplicate app action grant"
        );
    }
    Ok(())
}
fn text(value: &str, maximum: usize, label: &str) -> Result<()> {
    ensure!(
        !value.trim().is_empty() && value.len() <= maximum && !value.contains('\0'),
        "{label} is empty or too long"
    );
    Ok(())
}

impl TeamCoordinator {
    pub fn new(path: PathBuf) -> Self {
        Self {
            control: crate::control::ControlStore::new(path),
        }
    }
    fn connect(&self) -> Result<Connection> {
        let db = self.control.connect()?;
        schema(&db)?;
        automation::schema(&db)?;
        crate::control::inbox_history::install_teams(&db)?;
        crate::usage_ledger::install_teams(&db)?;
        Ok(db)
    }

    /// Prepare a durable queue only. Starting harness execution is a separate step.
    pub fn prepare(&self, request: PrepareRun, now: u64) -> Result<TeamRunSnapshot> {
        ensure!(
            request.source_prompt.is_none(),
            "Use the new-task outbox for a team invocation"
        );
        self.prepare_inner(request, now, true)
    }

    /// Persist the root in the same recoverable outbox as children. No harness launch.
    pub fn queue_task(&self, request: PrepareRun, now: u64) -> Result<TeamRunSnapshot> {
        ensure!(
            request.source_prompt.is_some(),
            "Explicit team invocation required"
        );
        self.prepare_inner(request, now, false)
    }

    /// User/agent invocation commands carry no run-limit choices. Their exact
    /// replay must retain saved limits when defaults change after an upgrade.
    /// Explicit PrepareRun callers still compare every requested limit exactly.
    pub(crate) fn queue_default_task(
        &self,
        mut request: PrepareRun,
        now: u64,
    ) -> Result<TeamRunSnapshot> {
        let mut db = self.connect()?;
        let tx = db.transaction()?;
        let existing: Option<String> = if let Some(origin) = &request.origin {
            tx.query_row(
                "SELECT run_id FROM ion_team_invocations WHERE task_id=? AND request_id=?",
                params![origin.task_id.to_string(), origin.request_id],
                |r| r.get(0),
            )
            .optional()?
        } else {
            tx.query_row(
                "SELECT id FROM ion_team_runs WHERE root_task_id=? OR id=?",
                params![request.root_task_id.to_string(), request.id.to_string()],
                |r| r.get(0),
            )
            .optional()?
        };
        let pinned_limits = existing
            .map(|id| load(&tx, Uuid::parse_str(&id)?).map(|run| run.limits))
            .transpose()?;
        tx.commit()?;
        // ControlStore initializes its own connection; never hold a read
        // transaction on the same file across that initialization.
        request.limits = if let Some(limits) = pinned_limits {
            limits
        } else {
            self.control
                .load()?
                .records
                .into_iter()
                .find_map(|record| match record {
                    ControlRecord::Team(team) if team.id == request.team_id => {
                        Some(team.limits.unwrap_or_default())
                    }
                    _ => None,
                })
                .context("Team no longer exists")?
        };
        self.queue_task(request, now)
    }

    #[cfg(test)]
    pub(crate) fn queue_from_agent(
        &self,
        task_id: Uuid,
        project_id: Uuid,
        request_id: String,
        team_id: Uuid,
        expected_definition_revision: u64,
        objective: String,
        now: u64,
    ) -> Result<TeamRunSnapshot> {
        self.queue_from_agent_with_access(
            task_id,
            project_id,
            request_id,
            team_id,
            expected_definition_revision,
            objective,
            AgentAccess::default(),
            now,
        )
    }
    /// Authorization is supplied by the trusted source session, never model arguments.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn queue_from_agent_with_access(
        &self,
        task_id: Uuid,
        project_id: Uuid,
        request_id: String,
        team_id: Uuid,
        expected_definition_revision: u64,
        objective: String,
        access: AgentAccess,
        now: u64,
    ) -> Result<TeamRunSnapshot> {
        let objective = objective.trim().to_owned();
        self.queue_default_task(
            PrepareRun {
                id: Uuid::new_v4(),
                root_task_id: Uuid::new_v4(),
                project_id,
                team_id,
                expected_definition_revision,
                input_data: vec![],
                source_attachments: vec![],
                source_prompt: Some(format!("@team:{team_id} {objective}")),
                objective,
                limits: Default::default(),
                access,
                origin: Some(TeamTaskOrigin {
                    task_id,
                    request_id,
                }),
            },
            now,
        )
    }
    pub(crate) fn runs_for_source(&self, task_id: Uuid) -> Result<Vec<TeamRunSnapshot>> {
        let mut db = self.connect()?;
        let tx = db.transaction()?;
        let ids: Vec<String> = tx
            .prepare(
                "SELECT run_id FROM ion_team_invocations WHERE task_id=? ORDER BY rowid LIMIT 4",
            )?
            .query_map([task_id.to_string()], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let runs = ids
            .into_iter()
            .map(|id| load(&tx, Uuid::parse_str(&id)?))
            .collect::<Result<_>>()?;
        tx.commit()?;
        Ok(runs)
    }

    fn prepare_inner(
        &self,
        request: PrepareRun,
        now: u64,
        root_materialized: bool,
    ) -> Result<TeamRunSnapshot> {
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = Self::prepare_in_transaction(
            &tx,
            request,
            now,
            root_materialized,
            None,
            OutputContract::default(),
        )?;
        tx.commit()?;
        Ok(run)
    }
    fn prepare_in_transaction(
        tx: &Transaction<'_>,
        request: PrepareRun,
        now: u64,
        root_materialized: bool,
        pinned: Option<ControlSnapshot>,
        root_contract: OutputContract,
    ) -> Result<TeamRunSnapshot> {
        reviews::validate_contract(&root_contract)?;
        ensure!(
            !request.id.is_nil() && !request.root_task_id.is_nil() && !request.project_id.is_nil(),
            "Run and task identities are required"
        );
        text(&request.objective, 32_000, "Task objective")?;
        validate_access(&request.access)?;
        validate_limits(&request.limits)?;
        if pinned.is_some() {
            ensure!(
                request.source_attachments.is_empty(),
                "Automation handoffs use document receipts, not upload presentations"
            );
            inputs::validate_files(&request.input_data)?;
        } else {
            inputs::validate(&request.input_data, &request.source_attachments)?;
        }
        let body = serde_json::to_string(&request)?;
        let origin_signature = if let Some(origin) = &request.origin {
            ensure!(!origin.task_id.is_nil(), "Source task identity required");
            text(&origin.request_id, 256, "Invocation ID")?;
            let signature = serde_json::to_string(
                &serde_json::json!({"project":request.project_id,"team":request.team_id,"objective":request.objective,"access":request.access,"limits":request.limits}),
            )?;
            if let Some((id,existing)) = tx.query_row("SELECT run_id,signature FROM ion_team_invocations WHERE task_id=? AND request_id=?",params![origin.task_id.to_string(),origin.request_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional()? {
                ensure!(same_request(&existing,&signature)?,"Invocation ID reused with changed arguments");
                let run = load(&tx,Uuid::parse_str(&id)?)?;
                ensure!(run.work.iter().find(|w| w.parent_id.is_none()).is_some_and(|w| w.output_contract == root_contract),
                    "This root task already has a different output contract");
                return Ok(run);
            }
            let count: u16 = tx.query_row(
                "SELECT count(*) FROM ion_team_invocations WHERE task_id=?",
                [origin.task_id.to_string()],
                |r| r.get(0),
            )?;
            ensure!(
                count < 4,
                "This source task reached its four team invocation limit"
            );
            Some(signature)
        } else {
            None
        };
        if let Some((id, original)) = tx
            .query_row(
                "SELECT id,request FROM ion_team_runs WHERE root_task_id=? OR id=?",
                params![request.root_task_id.to_string(), request.id.to_string()],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()?
        {
            ensure!(
                same_request(&original, &body)?,
                "This root task already has a different team run"
            );
            let run = load(&tx, Uuid::parse_str(&id)?)?;
            ensure!(
                run.work
                    .iter()
                    .find(|w| w.parent_id.is_none())
                    .is_some_and(|w| w.output_contract == root_contract),
                "This root task already has a different output contract"
            );
            return Ok(run);
        }
        let definitions = pinned.unwrap_or(crate::control::ControlStore::read(tx)?);
        ensure!(
            definitions.revision == request.expected_definition_revision,
            "Team definitions changed; refresh before starting"
        );
        crate::control::validate(&definitions)?;
        let team = definitions
            .records
            .iter()
            .find_map(|r| match r {
                ControlRecord::Team(t) if t.id == request.team_id => Some(t.clone()),
                _ => None,
            })
            .context("Team no longer exists")?;
        if let Some(prompt) = &request.source_prompt {
            text(prompt, 32_000, "Team invocation")?;
            let invocation = mentions::resolve(
                prompt,
                &definitions
                    .records
                    .iter()
                    .filter_map(|r| {
                        if let ControlRecord::Team(t) = r {
                            Some(t.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>(),
            )
            .map_err(anyhow::Error::msg)?
            .context("Explicit team invocation required")?;
            ensure!(
                invocation.team_id == request.team_id && invocation.objective == request.objective,
                "Team invocation does not match the requested work"
            );
        }
        let profiles: Vec<_> = team
            .members
            .iter()
            .map(|member| {
                definitions
                    .records
                    .iter()
                    .find_map(|r| match r {
                        ControlRecord::Agent(a) if a.id == member.agent_id => Some(a.clone()),
                        _ => None,
                    })
                    .context("Team agent no longer exists")
            })
            .collect::<Result<_>>()?;
        let lead_id = team
            .members
            .iter()
            .find(|m| m.parent_id.is_none())
            .context("Team needs a lead")?
            .agent_id;
        let lead = profiles
            .iter()
            .find(|a| a.id == lead_id)
            .context("Lead profile missing")?;
        let root = TeamAssignment {
            can_resume: false,
            can_reconcile: false,
            id: Uuid::new_v4(),
            task_id: request.root_task_id,
            parent_id: None,
            agent_id: lead_id,
            objective: request.objective.clone(),
            expected_output: lead.expected_output.clone(),
            output_contract: root_contract,
            requires_review: false,
            rework_of: None,
            review: None,
            dependencies: vec![],
            access: intersect_access(&request.access, &lead.access),
            status: AssignmentStatus::Queued,
            reason: None,
            attempts: vec![],
            result: None,
        };
        ensure!(
            request.input_data.is_empty() || root.access.workspace_read,
            "The team lead needs workspace read access to use attached inputs"
        );
        let deadline = now.saturating_add(u64::from(request.limits.timeout_seconds));
        let run = TeamRunSnapshot {
            id: request.id,
            root_task_id: request.root_task_id,
            project_id: request.project_id,
            definition_revision: definitions.revision,
            revision: 1,
            status: TeamRunStatus::Active,
            team,
            profiles,
            limits: request.limits,
            usage: TeamTokenUsage::default(),
            workspace: None,
            access: request.access,
            created_at: now,
            deadline,
            work: vec![],
            decisions: vec![],
            inputs: request
                .input_data
                .iter()
                .map(PreparedInput::metadata)
                .collect(),
            follow_ups: vec![],
            source_prompt: request.source_prompt,
            origin: request.origin.clone(),
        };
        tx.execute("INSERT INTO ion_team_runs(id,root_task_id,project_id,status,revision,request,body) VALUES(?,?,?,?,1,?,?)",
            params![run.id.to_string(),run.root_task_id.to_string(),run.project_id.to_string(),run_status(run.status),body,serde_json::to_string(&run)?])?;
        inputs::insert(
            &tx,
            run.id,
            &request.input_data,
            &request.source_attachments,
        )?;
        insert_work(&tx, run.id, &root, None, None, root_materialized)?;
        if let Some(origin) = &run.origin {
            tx.execute("INSERT INTO ion_team_invocations(task_id,request_id,signature,run_id) VALUES(?,?,?,?)",params![origin.task_id.to_string(),origin.request_id,origin_signature,run.id.to_string()])?;
        }
        audit(
            &tx,
            run.id,
            Some(root.id),
            "prepared",
            "Team/profile definitions pinned; waiting for dispatcher",
            now,
        )?;
        load(tx, run.id)
    }
    /// One connection and an indexed lookup per incoming projection; never scan transcripts.
    pub fn team_task_ids(&self, ids: impl IntoIterator<Item = Uuid>) -> Result<HashSet<Uuid>> {
        let db = self.connect()?;
        let mut query = db.prepare("SELECT 1 FROM ion_team_work WHERE task_id=?")?;
        let mut bound = HashSet::new();
        for id in ids {
            if query
                .query_row([id.to_string()], |r| r.get::<_, u8>(0))
                .optional()?
                .is_some()
            {
                bound.insert(id);
            }
        }
        Ok(bound)
    }

    /// Indexed membership lookup used to fence solo entry points, even before outbox replay.
    pub fn binding_for_task(&self, task_id: Uuid) -> Result<Option<TeamTaskBinding>> {
        let db = self.connect()?;
        let row: Option<(String, String, String)> = db.query_row("SELECT w.run_id,w.id,r.body FROM ion_team_work w JOIN ion_team_runs r ON r.id=w.run_id WHERE w.task_id=?", [task_id.to_string()], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?))).optional()?;
        row.map(|(run, id, body)| {
            let run_body: TeamRunSnapshot = serde_json::from_str(&body)?;
            Ok(TeamTaskBinding {
                run_id: Uuid::parse_str(&run)?,
                assignment_id: Uuid::parse_str(&id)?,
                team_name: run_body.team.name,
            })
        })
        .transpose()
    }

    /// Called once by the owner daemon after a restart. An old host may still
    /// be running, so revoke its lease and require reconciliation, preserving capacity.
    pub fn reconcile_owner_restart(&self, now: u64) -> Result<()> {
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        workspace::reconcile_integrations(&tx, now)?;
        let run_ids: Vec<String> = tx
            .prepare(
                "SELECT DISTINCT run_id FROM ion_team_work WHERE status IN ('running','canceling')",
            )?
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        for id in run_ids {
            let run = load(&tx, Uuid::parse_str(&id)?)?;
            for mut work in run.work.into_iter().filter(|w| {
                matches!(
                    w.status,
                    AssignmentStatus::Running | AssignmentStatus::Canceling
                )
            }) {
                work.status = AssignmentStatus::Uncertain;
                work.reason =
                    Some("Owner daemon restarted; confirm the previous host outcome".into());
                work.attempts.last_mut().context("Attempt missing")?.status =
                    AssignmentStatus::Uncertain;
                save_work(&tx, &work, false)?;
                audit(
                    &tx,
                    run.id,
                    Some(work.id),
                    "owner_restarted",
                    "Previous lease revoked; execution outcome is unknown",
                    now,
                )?;
            }
            bump(&tx, run.id)?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn snapshot(&self, run_id: Uuid) -> Result<TeamRunSnapshot> {
        let mut db = self.connect()?;
        let tx = db.transaction()?;
        let result = load(&tx, run_id)?;
        tx.commit()?;
        Ok(result)
    }
    pub fn audit_events(
        &self,
        run_id: Uuid,
        after: u64,
        limit: usize,
    ) -> Result<Vec<TeamAuditEvent>> {
        let db = self.connect()?;
        let mut query = db.prepare("SELECT cursor,work_id,event,detail,timestamp FROM ion_team_audit WHERE run_id=? AND cursor>? ORDER BY cursor LIMIT ?")?;
        let rows = query.query_map(
            params![run_id.to_string(), after, limit.clamp(1, 100)],
            |row| {
                Ok((
                    row.get::<_, u64>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, u64>(4)?,
                ))
            },
        )?;
        rows.map(|row| {
            let (cursor, id, event, detail, timestamp) = row?;
            Ok(TeamAuditEvent {
                cursor,
                work_id: id.map(|id| Uuid::parse_str(&id)).transpose()?,
                event,
                detail,
                timestamp,
            })
        })
        .collect()
    }
    /// The bridge passes its authenticated lease. Model arguments cannot impersonate a manager.
    pub fn delegate(
        &self,
        lease: &AssignmentLease,
        request: Delegation,
        now: u64,
    ) -> Result<TeamAssignment> {
        self.delegate_with_contract(lease, request, OutputContract::default(), now)
    }
    pub fn delegate_with_contract(
        &self,
        lease: &AssignmentLease,
        request: Delegation,
        contract: OutputContract,
        now: u64,
    ) -> Result<TeamAssignment> {
        reviews::validate_contract(&contract)?;
        text(&request.objective, 32_000, "Assignment objective")?;
        text(&request.expected_output, 16_000, "Expected output")?;
        validate_access(&request.access)?;
        ensure!(
            !request.request_id.is_nil(),
            "Delegation request ID is required"
        );
        let request_body = serde_json::to_string(
            &serde_json::json!({"agent":request.agent_id,"objective":request.objective,"output":request.expected_output,"dependencies":request.dependencies,"access":request.access,"contract":contract}),
        )?;
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = load(&tx, lease.run_id)?;
        active(&run, now)?;
        let parent = checked_lease(&run, lease, now, false)?;
        if let Some((original,body)) = tx.query_row("SELECT request,body FROM ion_team_work WHERE run_id=? AND parent_id=? AND request_id=?",
            params![run.id.to_string(),parent.id.to_string(),request.request_id.to_string()], |row| Ok((row.get::<_,String>(0)?, row.get::<_,String>(1)?))).optional()? {
            ensure!(reviews::same_delegation(&original, &request_body)?, "Delegation ID reused with changed arguments");
            return Ok(serde_json::from_str(&body)?);
        }
        ensure!(
            run.work.len() < usize::from(run.limits.max_work_items),
            "Team assignment limit reached"
        );
        ensure!(
            run.team
                .members
                .iter()
                .any(|m| m.agent_id == request.agent_id && m.parent_id == Some(parent.agent_id)),
            "Agents can delegate only to their direct reports"
        );
        let mut ancestors = HashSet::new();
        let mut id = Some(parent.id);
        while let Some(current) = id {
            ensure!(
                ancestors.insert(current),
                "Assignment ancestry contains a cycle"
            );
            id = run
                .work
                .iter()
                .find(|w| w.id == current)
                .context("Assignment parent missing")?
                .parent_id;
        }
        ensure!(
            ancestors.len() <= usize::from(run.limits.max_depth),
            "Delegation depth limit reached"
        );
        ensure!(
            request.dependencies.len() <= 64,
            "Too many assignment dependencies"
        );
        let mut dependencies = HashSet::new();
        for dependency in &request.dependencies {
            ensure!(dependencies.insert(*dependency), "Duplicate dependency");
            ensure!(
                !ancestors.contains(dependency),
                "A child cannot depend on its waiting ancestor"
            );
            ensure!(
                run.work.iter().any(|w| w.id == *dependency),
                "Dependency must already belong to this run"
            );
        }
        let profile = run
            .profiles
            .iter()
            .find(|p| p.id == request.agent_id)
            .context("Pinned profile missing")?;
        let permitted = intersect_access(&parent.access, &profile.access);
        ensure!(
            intersect_access(&request.access, &permitted) == request.access,
            "Delegation cannot expand parent/profile access"
        );
        let work = TeamAssignment {
            can_resume: false,
            can_reconcile: false,
            id: Uuid::new_v4(),
            task_id: Uuid::new_v4(),
            parent_id: Some(parent.id),
            agent_id: profile.id,
            objective: request.objective,
            expected_output: request.expected_output,
            output_contract: contract,
            requires_review: run.limits.require_child_review,
            rework_of: None,
            review: None,
            dependencies: request.dependencies,
            access: request.access,
            status: AssignmentStatus::Queued,
            reason: None,
            attempts: vec![],
            result: None,
        };
        insert_work(
            &tx,
            run.id,
            &work,
            Some(request.request_id),
            Some(&request_body),
            false,
        )?;
        bump(&tx, run.id)?;
        audit(
            &tx,
            run.id,
            Some(work.id),
            "delegated",
            "Child task UUID allocated in durable outbox",
            now,
        )?;
        tx.commit()?;
        Ok(work)
    }
    pub fn pending_materializations(&self, limit: usize) -> Result<Vec<TaskMaterialization>> {
        self.pending_materializations_for_run(None, limit)
    }
    pub(super) fn pending_materializations_for_run(
        &self,
        run: Option<Uuid>,
        limit: usize,
    ) -> Result<Vec<TaskMaterialization>> {
        let mut db = self.connect()?;
        let tx = db.transaction()?;
        let ids: Vec<(String,String)> = tx.prepare("SELECT o.run_id,o.work_id FROM ion_team_outbox o JOIN ion_team_work w ON w.id=o.work_id JOIN ion_team_runs r ON r.id=o.run_id WHERE o.materialized=0 AND w.status='queued' AND r.status='active' AND (? IS NULL OR o.run_id=?) ORDER BY o.rowid LIMIT ?")?
            .query_map(params![run.map(|r|r.to_string()),run.map(|r|r.to_string()),limit.clamp(1,128)], |row| Ok((row.get(0)?,row.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        let mut result = Vec::new();
        for (run_id, work_id) in ids {
            let run = load(&tx, Uuid::parse_str(&run_id)?)?;
            let assignment = run
                .work
                .into_iter()
                .find(|w| w.id.to_string() == work_id)
                .context("Outbox assignment missing")?;
            let profile = run
                .profiles
                .into_iter()
                .find(|p| p.id == assignment.agent_id)
                .context("Outbox profile missing")?;
            result.push(TaskMaterialization {
                run_id: run.id,
                project_id: run.project_id,
                binding: TeamTaskBinding {
                    run_id: run.id,
                    assignment_id: assignment.id,
                    team_name: run.team.name.clone(),
                },
                source_attachments: if assignment.parent_id.is_none() {
                    inputs::presentations(&tx, run.id)?
                } else {
                    vec![]
                },
                source_prompt: if assignment.parent_id.is_none() {
                    run.source_prompt
                } else {
                    None
                },
                assignment,
                profile,
            });
        }
        tx.commit()?;
        Ok(result)
    }
    /// A task DB consumer uses create-if-absent with this exact UUID before ack.
    pub fn acknowledge_materialized(
        &self,
        run_id: Uuid,
        work_id: Uuid,
        task_id: Uuid,
        now: u64,
    ) -> Result<()> {
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let materialized: bool = tx.query_row("SELECT materialized FROM ion_team_outbox WHERE run_id=? AND work_id=? AND task_id=?",
            params![run_id.to_string(),work_id.to_string(),task_id.to_string()], |row| row.get(0)).context("Task outbox identity mismatch")?;
        if !materialized {
            tx.execute(
                "UPDATE ion_team_outbox SET materialized=1 WHERE work_id=?",
                [work_id.to_string()],
            )?;
            bump(&tx, run_id)?;
            audit(
                &tx,
                run_id,
                Some(work_id),
                "materialized",
                "Child task persisted",
                now,
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    /// Transactional claim checks run, dependencies and shared profile capacity.
    /// Local fallback is possible only before remote execution, using the same profile.
    pub fn claim(
        &self,
        run_id: Uuid,
        work_id: Uuid,
        host: DispatchHost,
        now: u64,
        lease_seconds: u32,
    ) -> Result<ClaimedAssignment> {
        ensure!(
            (1..=300).contains(&lease_seconds),
            "Lease duration must be 1–300 seconds"
        );
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = load(&tx, run_id)?;
        active(&run, now)?;
        workspace::inheritance::require_ready(&tx, run_id)?;
        let mut work = run
            .work
            .iter()
            .find(|w| w.id == work_id)
            .context("Assignment missing")?
            .clone();
        ensure!(
            work.status == AssignmentStatus::Queued,
            "Assignment is not queued"
        );
        ensure!(
            work.attempts.len() < usize::from(run.limits.max_attempts),
            "Attempt limit reached"
        );
        for dependency in &work.dependencies {
            ensure!(
                run.work.iter().any(|w| w.id == *dependency
                    && (w.status == AssignmentStatus::Completed
                        || work.rework_of == Some(w.id)
                            && w.status == AssignmentStatus::Superseded)),
                "Assignment dependency has not completed"
            );
        }
        if let Some(parent_id) = work.parent_id {
            ensure!(
                run.work.iter().any(|w| w.id == parent_id
                    && matches!(
                        w.status,
                        AssignmentStatus::Running
                            | AssignmentStatus::Waiting
                            | AssignmentStatus::Blocked
                    )),
                "Parent is no longer accepting child work"
            );
        }
        ensure!(
            tx.query_row(
                "SELECT materialized FROM ion_team_outbox WHERE work_id=?",
                [work.id.to_string()],
                |r| r.get::<_, bool>(0)
            )?,
            "Child task has not been persisted"
        );
        let profile = run
            .profiles
            .iter()
            .find(|p| p.id == work.agent_id)
            .context("Pinned profile missing")?
            .clone();
        continuation::check_capacity(&tx, &run, &profile, &work.access)?;
        let (environment, host_name, fallback_reason) = match host {
            DispatchHost::Local => {
                ensure!(
                    profile.execution.environment == ExecutionEnvironment::Local,
                    "Nebius-selected agent requires explicit pre-execution fallback"
                );
                (ExecutionEnvironment::Local, "local".to_owned(), None)
            }
            DispatchHost::NebiusSandbox { target } => {
                ensure!(
                    profile.execution.environment == ExecutionEnvironment::NebiusSandbox
                        && profile.execution.sandbox_target.as_deref() == Some(&target),
                    "Sandbox target differs from pinned agent settings"
                );
                ensure!(
                    !work
                        .attempts
                        .iter()
                        .any(|a| a.environment == ExecutionEnvironment::Local),
                    "Local fallback cannot loop back to the sandbox"
                );
                (ExecutionEnvironment::NebiusSandbox, target, None)
            }
            DispatchHost::LocalFallback { failure } => {
                ensure!(
                    profile.execution.environment == ExecutionEnvironment::NebiusSandbox
                        && profile.execution.local_fallback,
                    "Local fallback is not authorized"
                );
                ensure!(
                    work.attempts
                        .iter()
                        .all(|a| a.status == AssignmentStatus::Failed),
                    "Existing remote attempts must be confirmed without effects before fallback"
                );
                (
                    ExecutionEnvironment::Local,
                    "local".to_owned(),
                    Some(failure.reason().to_owned()),
                )
            }
        };
        let attempt = AssignmentAttempt {
            id: Uuid::new_v4(),
            environment,
            host: host_name,
            fallback_reason,
            started_at: now,
            lease_expires_at: now
                .saturating_add(u64::from(lease_seconds))
                .min(run.deadline),
            generation: 0,
            status: AssignmentStatus::Running,
        };
        if let Some(reason) = &attempt.fallback_reason {
            audit(
                &tx,
                run_id,
                Some(work_id),
                "sandbox_start_failed",
                reason,
                now,
            )?;
        }
        let lease = AssignmentLease {
            run_id,
            work_id,
            attempt_id: attempt.id,
            generation: attempt.generation,
        };
        tx.execute("INSERT INTO ion_team_attempts(id,run_id,work_id,agent_id,live,capacity) VALUES(?,?,?,?,1,?)",params![attempt.id.to_string(),run_id.to_string(),work_id.to_string(),profile.id.to_string(),profile.max_parallel_tasks])?;
        work.attempts.push(attempt);
        work.status = AssignmentStatus::Running;
        work.reason = None;
        save_work(&tx, &work, false)?;
        bump(&tx, run_id)?;
        audit(
            &tx,
            run_id,
            Some(work_id),
            "claimed",
            "Assignment lease issued",
            now,
        )?;
        tx.commit()?;
        Ok(ClaimedAssignment {
            lease,
            assignment: work,
            profile,
        })
    }
    pub fn renew(&self, lease: &AssignmentLease, now: u64, lease_seconds: u32) -> Result<()> {
        ensure!(
            (1..=300).contains(&lease_seconds),
            "Lease duration must be 1–300 seconds"
        );
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = load(&tx, lease.run_id)?;
        active(&run, now)?;
        let mut work = checked_lease(&run, lease, now, false)?.clone();
        work.attempts
            .last_mut()
            .context("Attempt missing")?
            .lease_expires_at = now
            .saturating_add(u64::from(lease_seconds))
            .min(run.deadline);
        save_work(&tx, &work, false)?;
        bump(&tx, run.id)?;
        tx.commit()?;
        Ok(())
    }
    /// Expired workers stay uncertain and hold capacity until confirmed stopped.
    pub fn reconcile_expired(&self, run_id: Uuid, now: u64) -> Result<TeamRunSnapshot> {
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = load(&tx, run_id)?;
        let mut changed = false;
        for mut work in run
            .work
            .iter()
            .filter(|w| {
                (run.deadline <= now
                    && matches!(
                        w.status,
                        AssignmentStatus::Queued
                            | AssignmentStatus::Waiting
                            | AssignmentStatus::InReview
                    ))
                    || (matches!(
                        w.status,
                        AssignmentStatus::Running | AssignmentStatus::Canceling
                    ) && w.attempts.last().is_some_and(|a| a.lease_expires_at <= now))
            })
            .cloned()
        {
            let gpu_wait = gpu_jobs::waits::waiting(&tx, work.id)?.is_some();
            let inactive = !gpu_wait
                && matches!(
                    work.status,
                    AssignmentStatus::Queued
                        | AssignmentStatus::Waiting
                        | AssignmentStatus::InReview
                );
            work.status = if inactive {
                AssignmentStatus::Blocked
            } else {
                AssignmentStatus::Uncertain
            };
            work.reason = Some(
                if inactive {
                    "Team deadline reached before dispatch or resume"
                } else if gpu_wait {
                    "Team deadline reached while waiting for GPU work; confirm original resources stopped"
                } else {
                    "Worker lease expired; confirm host outcome before retry"
                }
                .into(),
            );
            if let Some(attempt) = work.attempts.last_mut() {
                attempt.status = work.status;
            }
            save_work(&tx, &work, false)?;
            audit(
                &tx,
                run_id,
                Some(work.id),
                if inactive { "deadline" } else { "uncertain" },
                if inactive {
                    "Unstarted work blocked after deadline"
                } else {
                    "Expired lease is not proof the process stopped"
                },
                now,
            )?;
            changed = true;
        }
        if run.deadline <= now {
            decisions::cancel_pending(&tx, run_id)?;
            changed |= followups::cancel_pending(&tx, run_id)?;
        }
        if changed {
            bump(&tx, run_id)?;
        }
        tx.commit()?;
        self.snapshot(run_id)
    }
    pub fn report(
        &self,
        lease: &AssignmentLease,
        outcome: AssignmentOutcome,
        now: u64,
    ) -> Result<TeamRunSnapshot> {
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = load(&tx, lease.run_id)?;
        let signature = serde_json::to_string(&outcome)?;
        ensure!(
            run.work
                .iter()
                .find(|w| w.id == lease.work_id)
                .and_then(|w| w.attempts.iter().find(|a| a.id == lease.attempt_id))
                .is_some_and(|a| a.generation == lease.generation),
            "Assignment lease generation is stale"
        );
        let previous: Option<String> = tx
            .query_row(
                "SELECT report FROM ion_team_attempts WHERE id=? AND run_id=? AND work_id=?",
                params![
                    lease.attempt_id.to_string(),
                    lease.run_id.to_string(),
                    lease.work_id.to_string()
                ],
                |row| row.get(0),
            )
            .context("Attempt identity mismatch")?;
        if let Some(previous) = previous {
            ensure!(
                previous == signature,
                "Attempt already reported a different outcome"
            );
            return Ok(run);
        }
        let mut work = checked_lease(&run, lease, now, true)?.clone();
        let retry_safe = matches!(&outcome, AssignmentOutcome::FailedBeforeEffects(_));
        let (status, reason, result) = match outcome {
            AssignmentOutcome::Completed(result) => {
                followups::require_delivered(&tx, run.id, work.id)?;
                workspace::require_settled_effects(&tx, run.id)?;
                validate_result(&tx, &run, &work, &result)?;
                reviews::validate_output(&tx, &run, &work, &result)?;
                ensure!(
                    run.work
                        .iter()
                        .filter(|w| w.parent_id == Some(work.id))
                        .all(|w| matches!(
                            w.status,
                            AssignmentStatus::Completed | AssignmentStatus::Superseded
                        )),
                    "Assignment cannot complete with unfinished child work"
                );
                (
                    if run.status == TeamRunStatus::Active {
                        reviews::submitted_status(&work)
                    } else {
                        AssignmentStatus::Completed
                    },
                    None,
                    Some(result),
                )
            }
            AssignmentOutcome::Blocked(reason) => {
                text(&reason, 4096, "Blocker")?;
                (AssignmentStatus::Blocked, Some(reason), None)
            }
            AssignmentOutcome::FailedBeforeEffects(reason) => {
                text(&reason, 4096, "Failure")?;
                (AssignmentStatus::Failed, Some(reason), None)
            }
            AssignmentOutcome::Uncertain(reason) => {
                text(&reason, 4096, "Unknown outcome")?;
                (AssignmentStatus::Uncertain, Some(reason), None)
            }
            AssignmentOutcome::Canceled => (
                AssignmentStatus::Canceled,
                Some("Host confirmed cancellation".into()),
                None,
            ),
        };
        work.status = status;
        work.reason = reason;
        work.result = result;
        work.attempts.last_mut().context("Attempt missing")?.status = status;
        tx.execute(
            "UPDATE ion_team_attempts SET live=? WHERE id=?",
            params![status.holds_capacity(), lease.attempt_id.to_string()],
        )?;
        save_work(&tx, &work, retry_safe)?;
        tx.execute(
            "UPDATE ion_team_attempts SET report=? WHERE id=?",
            params![signature, lease.attempt_id.to_string()],
        )?;
        bump(&tx, run.id)?;
        audit(
            &tx,
            run.id,
            Some(work.id),
            "reported",
            assignment_status(status),
            now,
        )?;
        settle_run(&tx, run.id)?;
        tx.commit()?;
        self.snapshot(run.id)
    }
    /// Reconciliation is a trusted host operation, distinct from a stale worker report.
    pub fn resolve_uncertain(
        &self,
        run_id: Uuid,
        work_id: Uuid,
        attempt_id: Uuid,
        outcome: AssignmentOutcome,
        now: u64,
    ) -> Result<TeamRunSnapshot> {
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = load(&tx, run_id)?;
        let mut work = run
            .work
            .iter()
            .find(|w| w.id == work_id)
            .context("Assignment missing")?
            .clone();
        ensure!(
            work.status == AssignmentStatus::Uncertain
                && work.attempts.last().is_some_and(|a| a.id == attempt_id),
            "Reconciliation attempt is stale"
        );
        let safe = matches!(&outcome, AssignmentOutcome::FailedBeforeEffects(_));
        let (status, reason, result) = match outcome {
            AssignmentOutcome::Canceled => (
                AssignmentStatus::Canceled,
                Some("Host confirmed cancellation".into()),
                None,
            ),
            AssignmentOutcome::Blocked(reason) => {
                text(&reason, 4096, "Confirmed host blocker")?;
                (AssignmentStatus::Blocked, Some(reason), None)
            }
            AssignmentOutcome::FailedBeforeEffects(reason) => {
                text(&reason, 4096, "Failure")?;
                (AssignmentStatus::Failed, Some(reason), None)
            }
            AssignmentOutcome::Completed(result) => {
                followups::require_delivered(&tx, run.id, work.id)?;
                workspace::require_settled_effects(&tx, run.id)?;
                validate_result(&tx, &run, &work, &result)?;
                reviews::validate_output(&tx, &run, &work, &result)?;
                ensure!(
                    run.work
                        .iter()
                        .filter(|w| w.parent_id == Some(work_id))
                        .all(|w| matches!(
                            w.status,
                            AssignmentStatus::Completed | AssignmentStatus::Superseded
                        )),
                    "Child work is unfinished"
                );
                (
                    if run.status == TeamRunStatus::Active {
                        reviews::submitted_status(&work)
                    } else {
                        AssignmentStatus::Completed
                    },
                    None,
                    Some(result),
                )
            }
            _ => anyhow::bail!("Reconciliation requires a confirmed terminal outcome"),
        };
        work.status = status;
        work.reason = reason;
        work.result = result;
        work.attempts.last_mut().context("Attempt missing")?.status = status;
        tx.execute(
            "UPDATE ion_team_attempts SET live=0 WHERE id=?",
            [attempt_id.to_string()],
        )?;
        save_work(&tx, &work, safe)?;
        bump(&tx, run_id)?;
        audit(
            &tx,
            run_id,
            Some(work_id),
            "reconciled",
            assignment_status(status),
            now,
        )?;
        settle_run(&tx, run_id)?;
        tx.commit()?;
        self.snapshot(run_id)
    }
    pub fn retry(
        &self,
        run_id: Uuid,
        work_id: Uuid,
        expected_revision: u64,
        now: u64,
    ) -> Result<TeamRunSnapshot> {
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = load(&tx, run_id)?;
        active(&run, now)?;
        ensure!(
            run.revision == expected_revision,
            "Run changed; refresh before retry"
        );
        retry_in_transaction(&tx, &run, work_id, now)?;
        tx.commit()?;
        self.snapshot(run_id)
    }
    pub fn stop(&self, run_id: Uuid, expected_revision: u64, now: u64) -> Result<TeamRunSnapshot> {
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = load(&tx, run_id)?;
        if matches!(
            run.status,
            TeamRunStatus::Stopping | TeamRunStatus::Canceled
        ) {
            return Ok(run);
        }
        ensure!(
            run.revision == expected_revision,
            "Run changed; refresh before stopping"
        );
        ensure!(run.status == TeamRunStatus::Active, "Run already completed");
        for mut work in run
            .work
            .iter()
            .filter(|w| {
                matches!(
                    w.status,
                    AssignmentStatus::Queued
                        | AssignmentStatus::Waiting
                        | AssignmentStatus::Running
                        | AssignmentStatus::Blocked
                        | AssignmentStatus::Failed
                        | AssignmentStatus::InReview
                )
            })
            .cloned()
        {
            work.status = if work.status == AssignmentStatus::Waiting
                && gpu_jobs::waits::waiting(&tx, work.id)?.is_some()
            {
                AssignmentStatus::Uncertain
            } else if work.status == AssignmentStatus::Running {
                AssignmentStatus::Canceling
            } else {
                AssignmentStatus::Canceled
            };
            work.reason = Some("User stopped the team".into());
            if let Some(attempt) = work.attempts.last_mut() {
                attempt.status = work.status;
            }
            save_work(&tx, &work, false)?;
        }
        decisions::cancel_pending(&tx, run_id)?;
        followups::cancel_pending(&tx, run_id)?;
        set_run_status(&tx, run_id, TeamRunStatus::Stopping)?;
        bump(&tx, run_id)?;
        audit(
            &tx,
            run_id,
            None,
            "stop_requested",
            "Queued work canceled; active hosts must confirm cancellation",
            now,
        )?;
        settle_run(&tx, run_id)?;
        tx.commit()?;
        self.snapshot(run_id)
    }
}
/// Shared queue mutation; callers retain the same run, assignment, limits and grants.
fn retry_in_transaction(
    tx: &Transaction<'_>,
    run: &TeamRunSnapshot,
    work_id: Uuid,
    now: u64,
) -> Result<()> {
    active(run, now)?;
    let mut work = run
        .work
        .iter()
        .find(|w| w.id == work_id)
        .context("Assignment missing")?
        .clone();
    let safe: bool = tx.query_row(
        "SELECT retry_safe FROM ion_team_work WHERE id=?",
        [work_id.to_string()],
        |r| r.get(0),
    )?;
    ensure!(
        work.status == AssignmentStatus::Failed
            && safe
            && work.attempts.len() < usize::from(run.limits.max_attempts),
        "Only confirmed failures before effects may be retried within the attempt limit"
    );
    work.status = AssignmentStatus::Queued;
    work.reason = None;
    save_work(tx, &work, false)?;
    bump(tx, run.id)?;
    audit(
        tx,
        run.id,
        Some(work_id),
        "retry_queued",
        "Confirmed safe retry",
        now,
    )?;
    Ok(())
}

// Adding a defaulted run limit must not invalidate an exact durable invocation
// saved by protocol 13. Normalize only this documented default; all identities,
// access and requested limits still compare exactly.
fn same_request(old: &str, new: &str) -> Result<bool> {
    let normalize = |body: &str| -> Result<serde_json::Value> {
        let mut value: serde_json::Value = serde_json::from_str(body)?;
        if let Some(object) = value.as_object_mut() {
            object.entry("input_data").or_insert(serde_json::json!([]));
            object
                .entry("source_attachments")
                .or_insert(serde_json::json!([]));
        }
        if let Some(limits) = value.get_mut("limits").and_then(|v| v.as_object_mut()) {
            limits
                .entry("maxModelCalls")
                .or_insert(serde_json::json!(TeamRunLimits::default().max_model_calls));
            limits
                .entry("requireChildReview")
                .or_insert(serde_json::json!(false));
            limits
                .entry("maxReworkRounds")
                .or_insert(serde_json::json!(2));
            limits.entry("maxTokens").or_insert(serde_json::Value::Null);
        }
        Ok(value)
    };
    Ok(normalize(old)? == normalize(new)?)
}

fn active(run: &TeamRunSnapshot, now: u64) -> Result<()> {
    ensure!(
        run.status == TeamRunStatus::Active,
        "Team run is not active"
    );
    ensure!(now < run.deadline, "Team run deadline reached");
    Ok(())
}
fn checked_lease<'a>(
    run: &'a TeamRunSnapshot,
    lease: &AssignmentLease,
    now: u64,
    allow_canceling: bool,
) -> Result<&'a TeamAssignment> {
    let work = run
        .work
        .iter()
        .find(|w| w.id == lease.work_id)
        .context("Assignment missing")?;
    let attempt = work.attempts.last().context("Assignment has no attempt")?;
    ensure!(
        attempt.id == lease.attempt_id
            && attempt.generation == lease.generation
            && attempt.lease_expires_at > now
            && (work.status == AssignmentStatus::Running
                || allow_canceling && work.status == AssignmentStatus::Canceling),
        "Assignment lease is expired, revoked or superseded"
    );
    Ok(work)
}
pub(crate) fn verify_binary_document(
    db: &Connection,
    artifact: &Artifact,
    bytes: &[u8],
) -> Result<()> {
    documents::verify_pdf(db, artifact, bytes).map(|_| ())
}

fn validate_result(
    db: &Connection,
    run: &TeamRunSnapshot,
    work: &TeamAssignment,
    result: &AssignmentResult,
) -> Result<()> {
    checks::validate_references(result)?;
    text(&result.summary, 32_000, "Result summary")?;
    ensure!(
        serde_json::to_vec(result)?.len() <= 32 * 1024,
        "Result evidence must fit in 32 KiB; use stored documents for larger output"
    );
    ensure!(
        result.checks.len() <= 64
            && result.artifact_ids.len() <= 64
            && (!result.checks.is_empty() || !result.artifact_ids.is_empty()),
        "A result needs bounded validation evidence or documents"
    );
    ensure!(
        result.artifact_ids.iter().collect::<HashSet<_>>().len() == result.artifact_ids.len(),
        "Duplicate result document"
    );
    for check in &result.checks {
        text(check, 4096, "Validation evidence")?;
    }
    if !result.artifact_ids.is_empty() {
        let definitions = crate::control::ControlStore::read(db)?;
        let mut tasks = HashSet::from([work.task_id]);
        let mut work_ids = HashSet::from([work.id]);
        work_ids.extend(work.dependencies.iter());
        loop {
            let size = work_ids.len();
            for child in &run.work {
                if child.parent_id.is_some_and(|id| work_ids.contains(&id)) {
                    work_ids.insert(child.id);
                }
            }
            if size == work_ids.len() {
                break;
            }
        }
        tasks.extend(
            run.work
                .iter()
                .filter(|w| work_ids.contains(&w.id))
                .map(|w| w.task_id),
        );
        for id in &result.artifact_ids {
            let artifact = definitions
                .records
                .iter()
                .find_map(|r| match r {
                    ControlRecord::Artifact(a)
                        if a.id == *id
                            && a.project_id == Some(run.project_id)
                            && a.task_id.is_some_and(|task| tasks.contains(&task)) =>
                    {
                        Some(a)
                    }
                    _ => None,
                })
                .context("Result document does not belong to this assignment or its handoffs")?;
            crate::control::verified_document(db, artifact)?;
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn test_pdf_bytes() -> Vec<u8> {
    tests::binary_documents::pdf_bytes()
}
