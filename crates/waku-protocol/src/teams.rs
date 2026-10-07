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
    /// Immutable user input metadata, distinct from AI-generated artifacts.
    #[serde(default)]
    pub inputs: Vec<TeamInputFile>,
    #[serde(default)]
    pub follow_ups: Vec<TeamFollowUp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<TeamTaskOrigin>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum TeamFollowUpStatus {
    Queued,
    Delivered,
    Canceled,
}

/// Human follow-up for the lead of this existing run. It never resets limits.
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TeamFollowUp {
    pub run_id: Uuid,
    pub assignment_id: Uuid,
    pub message: crate::model::QueuedMessage,
    pub status: TeamFollowUpStatus,
    pub attempt_id: Option<Uuid>,
}

/// No host path or content is exposed in a run projection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TeamInputFile {
    pub key: String,
    pub name: String,
    pub bytes: u32,
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
    /// Host/receipt recovery is a separate explicit human action.
    #[serde(default)]
    pub can_reconcile: bool,
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

/// Document requirements, declared observations and pinned executed command checks.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct OutputContract {
    pub minimum_documents: u16,
    pub required_formats: Vec<String>,
    pub required_checks: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_commands: Vec<RequiredCommandCheck>,
}
/// Exact literal command and explicit source/output files, pinned before delegation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RequiredCommandCheck {
    pub name: String,
    pub argv: Vec<String>,
    pub input_paths: Vec<String>,
    pub output_paths: Vec<String>,
    pub timeout_seconds: u32,
}
/// References trusted owner receipts; names alone never establish successful execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ExecutedCommandCheck {
    pub name: String,
    pub request_id: Uuid,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub executed_checks: Vec<ExecutedCommandCheck>,
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

/// Host-generated immutable change set. Only explicit user approval can apply it.
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TeamIntegration {
    pub id: Uuid,
    pub run_id: Uuid,
    pub revision: u64,
    pub source_path: std::path::PathBuf,
    pub base_commit: String,
    pub branch: String,
    pub status: TeamIntegrationStatus,
    pub changes: Vec<TeamSourceChange>,
    pub conflicts: Vec<String>,
    pub applied_changes: u32,
    pub created_at: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum TeamIntegrationStatus {
    Ready,
    Applying,
    Uncertain,
    Applied,
    Reverted,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TeamSourceChange {
    pub path: String,
    pub directory: bool,
    pub before: Option<String>,
    pub after: Option<String>,
    pub edit_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TeamRecoveryReview {
    pub run_id: Uuid,
    pub assignment_id: Uuid,
    pub attempt_id: Uuid,
    pub revision: u64,
    pub host: String,
    pub host_stopped: bool,
    pub can_prepare_continuation: bool,
    pub workspace_path: Option<std::path::PathBuf>,
    pub files: Vec<TeamFileRecovery>,
    pub apps: Vec<TeamDecision>,
    pub environment: ExecutionEnvironment,
    pub remote_calls: Vec<TeamRemoteRecovery>,
    #[serde(default)]
    pub gpu_jobs: Vec<TeamGpuRecovery>,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum TeamRemotePurpose {
    #[default]
    ModelCompletion,
    Command,
}
/// Operation identities only. Private inputs, outputs and authentication are never projected.
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TeamRemoteRecovery {
    #[serde(default)]
    pub purpose: TeamRemotePurpose,
    pub request_id: Uuid,
    pub image_id: Uuid,
    pub project: String,
    pub operation_id: Option<Uuid>,
    pub state: String,
    pub can_bind: bool,
}
/// GPU operation IDs are provider resource strings, distinct from ConTree UUIDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum TeamGpuOperationPurpose {
    Creation,
    Cancellation,
    Deletion,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TeamGpuRecovery {
    pub request_id: Uuid,
    pub project: String,
    pub job_id: Option<String>,
    pub creation_operation: Option<String>,
    pub cancellation_operation: Option<String>,
    pub deletion_operation: Option<String>,
    pub state: String,
    pub cancel_requested: bool,
    pub cleanup_state: Option<String>,
    pub can_bind_creation: bool,
    pub can_bind_cancellation: bool,
    pub can_bind_deletion: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TeamFileRecovery {
    pub id: Uuid,
    pub path: String,
    pub operation: String,
    pub before: Option<String>,
    pub after: Option<String>,
    pub private_stages: Vec<String>,
}
/// Human assertions are labeled as such; they never become provider receipts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum TeamRecoveryConfirmation {
    HostStopped,
    BindRemoteOperation {
        request_id: Uuid,
        operation_id: Uuid,
    },
    BindGpuOperation {
        request_id: Uuid,
        purpose: TeamGpuOperationPurpose,
        operation_id: String,
    },
    VerifyRemoteStopped,
    FileApplied {
        id: Uuid,
    },
    FileRestored {
        id: Uuid,
    },
    AppSucceeded {
        decision_id: Uuid,
        evidence: String,
    },
    AppFailed {
        decision_id: Uuid,
        evidence: String,
    },
    PrepareContinuation,
}

#[cfg(test)]
mod recovery_protocol_tests {
    use super::*;
    #[test]
    fn gpu_recovery_confirmation_preserves_provider_strings_and_exact_purpose() {
        let value = serde_json::json!({"kind":"bindGpuOperation","requestId":Uuid::new_v4(),"purpose":"deletion","operationId":"aijoboperation-original"});
        let confirmation: TeamRecoveryConfirmation = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(confirmation).unwrap(), value);
        let mut changed = value.clone();
        changed["purpose"] = serde_json::json!("auto");
        assert!(serde_json::from_value::<TeamRecoveryConfirmation>(changed).is_err());
        let mut changed = value;
        changed["credentials"] = serde_json::json!("unapproved");
        assert!(serde_json::from_value::<TeamRecoveryConfirmation>(changed).is_err());
    }
}
