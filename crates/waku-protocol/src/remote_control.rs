//! Persisted remote operations across runs. Reads never contact a provider.
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum RemoteExecutionKind {
    Model,
    Command,
    Gpu,
}
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoteExecutionCursor {
    pub kind: RemoteExecutionKind,
    pub request_id: Uuid,
}
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoteExecutionQuery {
    pub project_id: Option<Uuid>,
    pub kind: Option<RemoteExecutionKind>,
    pub unresolved_only: bool,
    pub after: Option<RemoteExecutionCursor>,
    pub limit: u16,
}
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RemoteExecution {
    pub kind: RemoteExecutionKind,
    pub request_id: Uuid,
    pub run_id: Uuid,
    pub root_task_id: Uuid,
    pub project_id: Uuid,
    pub assignment_id: Uuid,
    pub task_id: Uuid,
    pub attempt_id: Uuid,
    pub agent_id: Uuid,
    pub team_name: String,
    pub objective: String,
    /// Current local receipt state; absence of a handle never proves no resource exists.
    pub state: String,
    pub remote_project: String,
    pub operation_id: Option<String>,
    pub job_id: Option<String>,
    pub cancellation_operation_id: Option<String>,
    pub cleanup_state: Option<String>,
    pub stop_requested: bool,
    /// A persisted terminal/cleanup observation, not invoice finality or budget release.
    pub settled: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RemoteExecutionPage {
    pub entries: Vec<RemoteExecution>,
    pub next_cursor: Option<RemoteExecutionCursor>,
}
