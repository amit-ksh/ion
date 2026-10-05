//! Durable workspace definitions shared by desktop, phone, and daemon.
use crate::model::ProviderKind;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Debug, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ControlSnapshot {
    pub revision: u64,
    pub records: Vec<ControlRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ControlRecord {
    Agent(AgentProfile),
    Team(Team),
    Workflow(Workflow),
    Routine(Routine),
    Artifact(Artifact),
    Action(WorkspaceAction),
}
impl ControlRecord {
    pub fn id(&self) -> Uuid {
        match self {
            Self::Agent(v) => v.id,
            Self::Team(v) => v.id,
            Self::Workflow(v) => v.id,
            Self::Routine(v) => v.id,
            Self::Artifact(v) => v.id,
            Self::Action(v) => v.id,
        }
    }
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Agent(_) => "agent",
            Self::Team(_) => "team",
            Self::Workflow(_) => "workflow",
            Self::Routine(_) => "routine",
            Self::Artifact(_) => "artifact",
            Self::Action(_) => "action",
        }
    }
    pub fn name(&self) -> &str {
        match self {
            Self::Agent(v) => &v.name,
            Self::Team(v) => &v.name,
            Self::Workflow(v) => &v.name,
            Self::Routine(v) => &v.name,
            Self::Artifact(v) => &v.name,
            Self::Action(v) => &v.title,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AgentProfile {
    pub id: Uuid,
    pub name: String,
    pub expertise: String,
    pub instructions: String,
    pub provider: ProviderKind,
    pub model: Option<String>,
    #[serde(default)]
    pub expected_output: String,
    #[serde(default)]
    pub execution: crate::teams::AgentExecution,
    #[serde(default)]
    pub access: crate::teams::AgentAccess,
    #[serde(default = "crate::teams::default_agent_concurrency")]
    pub max_parallel_tasks: u16,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Team {
    pub id: Uuid,
    pub name: String,
    pub objective: String,
    /// Defaults for new runs; an existing run retains its independently pinned limits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub limits: Option<crate::teams::TeamRunLimits>,
    pub members: Vec<TeamMember>,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TeamMember {
    pub agent_id: Uuid,
    pub parent_id: Option<Uuid>,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Workflow {
    pub id: Uuid,
    pub name: String,
    pub project_id: Uuid,
    pub prompt: String,
    pub stages: Vec<WorkflowStage>,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowStage {
    pub title: String,
    pub instructions: String,
    pub requires_decision: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Routine {
    pub id: Uuid,
    pub name: String,
    pub project_id: Uuid,
    pub instructions: String,
    pub interval_minutes: u32,
    pub paused: bool,
    pub next_run: u64,
    pub provider: ProviderKind,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    pub id: Uuid,
    pub name: String,
    pub project_id: Option<Uuid>,
    pub task_id: Option<Uuid>,
    pub reference: String,
    pub format: String,
    pub created_at: u64,
}

/// Agent proposals are durable. Accepting one never silently starts a task.
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceAction {
    pub id: Uuid,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub title: String,
    pub detail: String,
    pub action_kind: WorkspaceActionKind,
    pub status: WorkspaceActionStatus,
    pub created_at: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum WorkspaceActionKind {
    ProposedTask,
    Decision,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum WorkspaceActionStatus {
    Pending,
    Accepted,
    Declined,
}
