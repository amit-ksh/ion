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
    #[serde(default)]
    pub limits: Option<crate::teams::TeamRunLimits>,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowStage {
    pub title: String,
    pub instructions: String,
    pub requires_decision: bool,
    /// Explicit team per stage. Legacy drafts remain editable but cannot run until assigned.
    #[serde(default)]
    pub team_id: Option<Uuid>,
    /// One generated document of each selected format before this stage can complete.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_formats: Vec<String>,
}
pub const WORKFLOW_DOCUMENT_FORMATS: &[&str] = &["html", "md", "txt", "csv", "json"];
impl WorkflowStage {
    pub fn document_contract(&self) -> Result<crate::teams::OutputContract, &'static str> {
        if self.required_formats.len() > WORKFLOW_DOCUMENT_FORMATS.len()
            || self
                .required_formats
                .iter()
                .any(|f| !WORKFLOW_DOCUMENT_FORMATS.contains(&f.as_str()))
        {
            return Err(
                "Workflow document requirements support HTML, Markdown, text, CSV and JSON",
            );
        }
        if self
            .required_formats
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
            != self.required_formats.len()
        {
            return Err("Duplicate required workflow document format");
        }
        Ok(crate::teams::OutputContract {
            minimum_documents: self.required_formats.len() as u16,
            required_formats: self.required_formats.clone(),
            ..Default::default()
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CalendarSchedule {
    pub time_zone: String,
    pub hour: u8,
    pub minute: u8,
    pub recurrence: CalendarRecurrence,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", deny_unknown_fields, rename_all = "camelCase")]
pub enum CalendarRecurrence {
    Daily,
    /// Monday = 0 through Sunday = 6.
    Weekly {
        days: Vec<u8>,
    },
    /// Calendar days 1 through 31; unavailable dates are skipped.
    Monthly {
        days: Vec<u8>,
    },
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum MissedRunPolicy {
    #[default]
    RunLatest,
    Skip,
}
impl MissedRunPolicy {
    pub fn is_default(&self) -> bool {
        *self == Self::RunLatest
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum OverlapPolicy {
    #[default]
    Skip,
    Allow,
}
impl OverlapPolicy {
    pub fn is_default(&self) -> bool {
        *self == Self::Skip
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RoutineScheduleRequest {
    pub interval_minutes: u32,
    pub calendar: Option<CalendarSchedule>,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledFire {
    pub at: u64,
    pub display: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RoutineSchedulePreview {
    pub observed_at: u64,
    pub time_zone: String,
    pub fires: Vec<ScheduledFire>,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RoutineTrigger {
    pub interval_minutes: u32,
    pub calendar: Option<CalendarSchedule>,
    pub missed_run_policy: MissedRunPolicy,
    pub overlap_policy: OverlapPolicy,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Routine {
    pub id: Uuid,
    pub name: String,
    pub project_id: Uuid,
    pub instructions: String,
    pub interval_minutes: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub calendar: Option<CalendarSchedule>,
    #[serde(default, skip_serializing_if = "MissedRunPolicy::is_default")]
    pub missed_run_policy: MissedRunPolicy,
    #[serde(default, skip_serializing_if = "OverlapPolicy::is_default")]
    pub overlap_policy: OverlapPolicy,
    pub paused: bool,
    pub next_run: u64,
    pub provider: ProviderKind,
    #[serde(default)]
    pub team_id: Option<Uuid>,
    #[serde(default)]
    pub workflow_id: Option<Uuid>,
    #[serde(default)]
    pub last_error: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub evidence: Option<ArtifactEvidence>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ArtifactOrigin {
    #[default]
    AgentGenerated,
    AgentEdit,
    TeamAssignment,
}
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactProvenance {
    pub origin: ArtifactOrigin,
    pub turn_id: Option<Uuid>,
    pub activity_id: Option<String>,
    pub source_path: Option<String>,
    pub run_id: Option<Uuid>,
    pub assignment_id: Option<Uuid>,
    pub agent_id: Option<Uuid>,
    pub request_id: Option<Uuid>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactEvidence {
    pub version: u16,
    pub bytes: u64,
    pub sha256: String,
    pub producer: ArtifactProvenance,
}
impl ArtifactEvidence {
    pub fn from_bytes(bytes: &[u8], producer: ArtifactProvenance) -> Self {
        use sha2::{Digest, Sha256};
        Self {
            version: 1,
            bytes: bytes.len() as u64,
            sha256: format!("{:x}", Sha256::digest(bytes)),
            producer,
        }
    }
    pub fn matches(&self, bytes: &[u8]) -> bool {
        use sha2::{Digest, Sha256};
        self.version == 1
            && self.bytes > 0
            && self.bytes <= 16 * 1024 * 1024
            && self.bytes == bytes.len() as u64
            && format!("{:x}", Sha256::digest(bytes)) == self.sha256
    }
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

/// Metadata-only automation projection; pinned profile bodies remain private.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum AutomationStatus {
    Running,
    WaitingDecision,
    Blocked,
    Completed,
    Canceled,
    Skipped,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AutomationStage {
    pub title: String,
    pub instructions: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_formats: Vec<String>,
    pub team_id: Uuid,
    pub team_run_id: Uuid,
    pub task_id: Uuid,
    pub requires_decision: bool,
    pub approved: bool,
    pub started: bool,
    pub completed: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRun {
    pub id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub trigger: Option<RoutineTrigger>,
    pub name: String,
    pub project_id: Uuid,
    pub workflow_id: Option<Uuid>,
    pub routine_id: Option<Uuid>,
    pub definition_revision: u64,
    pub revision: u64,
    pub created_at: u64,
    pub scheduled_at: Option<u64>,
    pub deadline: u64,
    pub status: AutomationStatus,
    pub reason: Option<String>,
    pub current_stage: u16,
    pub stages: Vec<AutomationStage>,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AutomationPage {
    pub runs: Vec<AutomationRun>,
    pub next_cursor: Option<Uuid>,
    pub pending_count: u32,
}
