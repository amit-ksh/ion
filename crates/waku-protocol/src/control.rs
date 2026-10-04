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
}
impl ControlRecord {
    pub fn id(&self) -> Uuid {
        match self {
            Self::Agent(v) => v.id,
            Self::Team(v) => v.id,
            Self::Workflow(v) => v.id,
            Self::Routine(v) => v.id,
            Self::Artifact(v) => v.id,
        }
    }
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Agent(_) => "agent",
            Self::Team(_) => "team",
            Self::Workflow(_) => "workflow",
            Self::Routine(_) => "routine",
            Self::Artifact(_) => "artifact",
        }
    }
    pub fn name(&self) -> &str {
        match self {
            Self::Agent(v) => &v.name,
            Self::Team(v) => &v.name,
            Self::Workflow(v) => &v.name,
            Self::Routine(v) => &v.name,
            Self::Artifact(v) => &v.name,
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
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Team {
    pub id: Uuid,
    pub name: String,
    pub objective: String,
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
