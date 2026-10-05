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
    pub id: Uuid,
    pub root_task_id: Uuid,
    pub project_id: Uuid,
    pub definition_revision: u64,
    pub revision: u64,
    pub status: TeamRunStatus,
    pub team: Team,
    pub profiles: Vec<AgentProfile>,
    pub limits: TeamRunLimits,
    pub access: AgentAccess,
    pub created_at: u64,
    pub deadline: u64,
    pub work: Vec<TeamAssignment>,
    /// Original user text for a queued root, retained separately from its objective.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<TeamTaskOrigin>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TeamAssignment {
    pub id: Uuid,
    pub task_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub agent_id: Uuid,
    pub objective: String,
    pub expected_output: String,
    pub dependencies: Vec<Uuid>,
    pub access: AgentAccess,
    pub status: AssignmentStatus,
    pub reason: Option<String>,
    pub attempts: Vec<AssignmentAttempt>,
    pub result: Option<AssignmentResult>,
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
