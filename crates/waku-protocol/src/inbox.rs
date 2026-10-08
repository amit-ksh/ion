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
