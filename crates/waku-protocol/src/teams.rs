//! Team definitions and execution projections. No scheduler or host I/O lives here.
pub mod mentions;

use crate::control::{AgentProfile, Team};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ExecutionEnvironment {
    #[default]
    Local,
    NebiusSandbox,
}
impl ExecutionEnvironment {
    pub fn label(self) -> &'static str {
        match self {
            Self::Local => "Local",
            Self::NebiusSandbox => "Nebius sandbox",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
pub struct AgentExecution {
    pub environment: ExecutionEnvironment,
    pub sandbox_target: Option<String>,
    pub local_fallback: bool,
}
impl Default for AgentExecution {
    fn default() -> Self {
        Self {
            environment: ExecutionEnvironment::Local,
            sandbox_target: None,
            local_fallback: true,
        }
    }
}

/// An exact connector account/action pair, never a credential.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AppActionGrant {
    pub account_id: String,
    pub action: String,
    /// Exact tool revision. Legacy grants must be reselected before execution.
    #[serde(default)]
    pub version: String,
}

/// This is authorization data. Adapters must enforce it before becoming team-ready.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
pub struct AgentAccess {
    pub workspace_read: bool,
    pub workspace_write: bool,
    pub shell: bool,
    pub network: bool,
    pub app_actions: Vec<AppActionGrant>,
}
impl Default for AgentAccess {
    fn default() -> Self {
        Self {
            workspace_read: true,
            workspace_write: false,
            shell: false,
            network: false,
            app_actions: vec![],
        }
    }
}
pub fn default_agent_concurrency() -> u16 {
    1
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
pub struct TeamRunLimits {
    pub max_work_items: u16,
    pub max_parallel_tasks: u16,
    pub max_depth: u16,
    pub max_attempts: u16,
    pub max_model_calls: u16,
    /// Missing on old pinned runs means no token budget was configured.
    #[serde(default)]
    pub max_tokens: Option<u64>,
    /// Old pinned runs retain their original completion policy.
    #[serde(default)]
    pub require_child_review: bool,
    pub max_rework_rounds: u16,
    pub timeout_seconds: u32,
}
impl Default for TeamRunLimits {
    fn default() -> Self {
        Self {
            max_work_items: 64,
            max_parallel_tasks: 4,
            max_depth: 8,
            max_attempts: 2,
            max_model_calls: 64,
            max_tokens: Some(2_000_000),
            require_child_review: true,
            max_rework_rounds: 2,
            timeout_seconds: 3600,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum TeamRunStatus {
    Active,
    Stopping,
    Canceled,
    Completed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum AssignmentStatus {
    Queued,
    Running,
    Waiting,
    Blocked,
    Completed,
    InReview,
    Superseded,
    Failed,
    Canceling,
    Canceled,
    Uncertain,
}
impl AssignmentStatus {
    pub fn holds_capacity(self) -> bool {
        matches!(self, Self::Running | Self::Canceling | Self::Uncertain)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TeamRunSnapshot {
    /// A durable run workspace, distinct from the user's checkout.
    #[serde(default)]
    pub workspace: Option<TeamWorkspace>,
    pub id: Uuid,
    pub root_task_id: Uuid,
    pub project_id: Uuid,
    pub definition_revision: u64,
    pub revision: u64,
    pub status: TeamRunStatus,
    pub team: Team,
    pub profiles: Vec<AgentProfile>,
    pub limits: TeamRunLimits,
    #[serde(default)]
    pub usage: TeamTokenUsage,
    pub access: AgentAccess,
    pub created_at: u64,
    pub deadline: u64,
    pub work: Vec<TeamAssignment>,
    #[serde(default)]
    pub decisions: Vec<TeamDecision>,
    /// Original user text for a queued root, retained separately from its objective.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<TeamTaskOrigin>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TeamWorkspace {
    pub path: std::path::PathBuf,
    pub source_path: std::path::PathBuf,
    pub branch: String,
    pub base_commit: String,
}

/// Bounded collection projection. Full assignments are loaded only for an opened run.
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TeamRunSummary {
    pub id: Uuid,
    pub root_task_id: Uuid,
    pub project_id: Uuid,
    pub team_id: Uuid,
    pub team_name: String,
    pub objective: String,
    pub status: TeamRunStatus,
    pub revision: u64,
    pub created_at: u64,
    pub updated_at: u64,
    pub assignments: u32,
    pub running: u32,
    pub waiting: u32,
    pub completed: u32,
    pub needs_attention: u32,
    pub usage: TeamTokenUsage,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TeamRunPage {
    pub runs: Vec<TeamRunSummary>,
    pub next_cursor: Option<Uuid>,
}

/// A planning decision binds the exact proposal to a pinned assignment and host.
/// Approval does not enlarge tool, filesystem, connector or spending permissions.
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TeamDecision {
    pub id: Uuid,
    pub run_id: Uuid,
    pub project_id: Uuid,
    pub assignment_id: Uuid,
    pub task_id: Uuid,
    pub agent_id: Uuid,
    pub definition_revision: u64,
    pub run_revision: u64,
    pub attempt_id: Uuid,
    pub host: String,
    pub title: String,
    pub proposal: String,
    /// Host-generated exact app action; approval never grants other actions.
    #[serde(default)]
    pub app_action: Option<TeamAppAction>,
    pub status: TeamDecisionStatus,
    pub revision: u64,
    pub created_at: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TeamAppAction {
    pub grant: AppActionGrant,
    /// Canonical JSON object, shown in full before approval.
    pub arguments_json: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum TeamDecisionStatus {
    Pending,
    Approved,
    Declined,
    Canceled,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TeamDecisionPage {
    pub decisions: Vec<TeamDecision>,
    pub next_cursor: Option<Uuid>,
    pub pending_count: u32,
}

/// Completed provider usage and allowances retained for requests without a receipt.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TeamTokenUsage {
    pub model_calls: u32,
    pub consumed_tokens: u64,
    pub reserved_tokens: u64,
    pub unknown_calls: u32,
    /// Historical calls without an allowance cannot be treated as zero-cost evidence.
    pub unmetered_calls: u32,
    pub allowance_violations: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TeamAssignment {
    /// A stopped local worker has a safe private checkpoint available.
    #[serde(default)]
    pub can_resume: bool,
    pub id: Uuid,
    pub task_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub agent_id: Uuid,
    pub objective: String,
    pub expected_output: String,
    #[serde(default)]
    pub output_contract: OutputContract,
    #[serde(default)]
    pub requires_review: bool,
    #[serde(default)]
    pub rework_of: Option<Uuid>,
    #[serde(default)]
    pub review: Option<AssignmentReview>,
    pub dependencies: Vec<Uuid>,
    pub access: AgentAccess,
    pub status: AssignmentStatus,
    pub reason: Option<String>,
    pub attempts: Vec<AssignmentAttempt>,
    pub result: Option<AssignmentResult>,
}

/// Structural output requirements; declared checks are evidence, not proof of execution.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct OutputContract {
    pub minimum_documents: u16,
    pub required_formats: Vec<String>,
    pub required_checks: Vec<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ReviewVerdict {
    Accepted,
    Rework,
}
/// Review of one immutable submitted result. Rework retains a linked replacement task.
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AssignmentReview {
    pub id: Uuid,
    pub verdict: ReviewVerdict,
    pub feedback: String,
    pub artifact_ids: Vec<Uuid>,
    pub checks: Vec<String>,
    pub reviewer_task_id: Uuid,
    pub replacement_assignment_id: Option<Uuid>,
    pub created_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AssignmentAttempt {
    pub id: Uuid,
    pub environment: ExecutionEnvironment,
    pub host: String,
    pub fallback_reason: Option<String>,
    pub started_at: u64,
    pub lease_expires_at: u64,
    /// Incremented across suspension and resume; stale workers cannot reuse a lease.
    #[serde(default)]
    pub generation: u32,
    pub status: AssignmentStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AssignmentResult {
    pub summary: String,
    pub artifact_ids: Vec<Uuid>,
    pub checks: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TeamAuditEvent {
    pub cursor: u64,
    pub work_id: Option<Uuid>,
    pub event: String,
    pub detail: String,
    pub timestamp: u64,
}

/// Daemon-owned task identity. A team task cannot enter a solo provider runtime.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TeamTaskBinding {
    pub run_id: Uuid,
    pub assignment_id: Uuid,
    pub team_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TeamTaskOrigin {
    pub task_id: Uuid,
    pub request_id: String,
}
