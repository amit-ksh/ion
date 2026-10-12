//! Content-free owner notification publication, separate from Inbox controls.
use crate::inbox::NotificationPolicy;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InboxNotificationEvent {
    pub sequence: u64,
    pub needs_attention: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InboxNotificationBatch {
    pub after: Option<u64>,
    pub next: u64,
    pub policy: NotificationPolicy,
    pub events: Vec<InboxNotificationEvent>,
}
