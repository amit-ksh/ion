//! User notification preferences only; acknowledgement cannot authorize agent work.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum NotificationPolicy {
    #[default]
    All,
    IssuesOnly,
    Off,
}
impl NotificationPolicy {
    pub fn allows(self, needs_attention: bool) -> bool {
        match self {
            Self::All => true,
            Self::IssuesOnly => needs_attention,
            Self::Off => false,
        }
    }
}
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct InboxItemState {
    /// Content-version key; a new event does not inherit a previous event's archive.
    pub key: String,
    pub read: bool,
    pub acknowledged: bool,
    pub archived: bool,
}
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct InboxSnapshot {
    /// Independent of workspace definitions, run authorization and decision revisions.
    pub revision: u64,
    pub notification_policy: NotificationPolicy,
    pub items: Vec<InboxItemState>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum InboxAction {
    MarkRead,
    MarkUnread,
    Acknowledge,
    Archive,
    Restore,
}
impl InboxItemState {
    pub fn apply(&mut self, action: InboxAction) {
        match action {
            InboxAction::MarkRead => self.read = true,
            InboxAction::MarkUnread => {
                self.read = false;
                self.acknowledged = false;
            }
            InboxAction::Acknowledge => {
                self.read = true;
                self.acknowledged = true;
            }
            InboxAction::Archive => {
                self.read = true;
                self.archived = true;
            }
            InboxAction::Restore => self.archived = false,
        }
    }
}

/// Stable opaque identity for an observed version of a notification. The digest
/// protects persisted preferences from carrying prompt text or filesystem paths.
pub fn event_key(source: &str, id: uuid::Uuid, event: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    digest.update((source.len() as u64).to_be_bytes());
    digest.update(source.as_bytes());
    digest.update(id.as_bytes());
    digest.update((event.len() as u64).to_be_bytes());
    digest.update(event);
    format!("ion:v1{:x}", digest.finalize())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum InboxSource {
    Task,
    WorkspaceAction,
    Artifact,
    TeamAssignment,
    TeamDecision,
    Automation,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum InboxKind {
    Proposal,
    Decision,
    Approval,
    Question,
    Issue,
    Completed,
    Artifact,
    Update,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum InboxHistoryView {
    #[default]
    Inbox,
    Unread,
    Acknowledged,
    Archived,
    All,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum InboxHistoryStatus {
    #[default]
    All,
    NeedsAction,
    Resolved,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InboxQuery {
    pub project_id: Option<uuid::Uuid>,
    pub task_id: Option<uuid::Uuid>,
    pub kind: Option<InboxKind>,
    pub view: InboxHistoryView,
    pub status: InboxHistoryStatus,
    pub after: Option<u64>,
    pub limit: u16,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct InboxEntry {
    pub sequence: u64,
    pub source: InboxSource,
    pub source_id: uuid::Uuid,
    pub project_id: Option<uuid::Uuid>,
    pub task_id: Option<uuid::Uuid>,
    pub run_id: Option<uuid::Uuid>,
    pub kind: InboxKind,
    pub title: String,
    pub detail: String,
    pub occurred_at: u64,
    /// True only for a still-current notification; read/acknowledge never approves it.
    pub needs_action: bool,
    pub state: InboxItemState,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct InboxPage {
    pub revision: u64,
    pub history_revision: u64,
    pub notification_policy: NotificationPolicy,
    pub entries: Vec<InboxEntry>,
    pub next_cursor: Option<u64>,
    pub matching_count: u64,
    /// Across all projects, independent of this page and its filters.
    pub unread_count: u64,
    pub needs_action_count: u64,
    /// Task outbox import is bounded; counts may increase until caught up.
    pub task_history_syncing: bool,
}
