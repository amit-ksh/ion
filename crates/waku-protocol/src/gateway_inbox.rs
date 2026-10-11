//! Revision-bound notification mutations with exact owner receipt identity.
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InboxWriteRequest {
    pub device_id: Uuid,
    pub publisher_id: Uuid,
    pub idempotency_key: Uuid,
    pub sequence: u64,
    pub expected_revision: u64,
    pub action: crate::inbox::InboxAction,
    /// Pinned milliseconds since the Unix epoch. A replay cannot renew its deadline.
    pub expires_at: u64,
}
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct InboxWriteReceipt {
    pub applied: bool,
    pub revision: u64,
}
pub fn claimed_request(
    value: serde_json::Value,
    publisher: &str,
) -> anyhow::Result<InboxWriteRequest> {
    anyhow::ensure!(
        value["status"] == "uncertain" && value["publisherId"] == publisher,
        "Inbox claim belongs to another owner or is not authorized"
    );
    let mut request = value
        .as_object()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Invalid Inbox claim"))?;
    request.remove("id");
    request.remove("status");
    request.remove("createdAt");
    request.remove("updatedAt");
    Ok(serde_json::from_value(serde_json::Value::Object(request))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inbox_claim_requires_exact_notification_action_and_owner_identity() {
        let publisher = Uuid::new_v4().to_string();
        let value = serde_json::json!({"id":Uuid::new_v4(),"deviceId":Uuid::new_v4(),"publisherId":publisher,"idempotencyKey":Uuid::new_v4(),"sequence":1,"expectedRevision":0,"action":"archive","expiresAt":1000,"status":"uncertain","createdAt":1,"updatedAt":1});
        assert!(claimed_request(value.clone(), &publisher).is_ok());
        assert!(claimed_request(value.clone(), &Uuid::new_v4().to_string()).is_err());
        let mut queued = value.clone();
        queued["status"] = serde_json::json!("queued");
        assert!(claimed_request(queued, &publisher).is_err());
        for action in ["approve", "start", "stop", "deploy"] {
            let mut changed = value.clone();
            changed["action"] = serde_json::json!(action);
            assert!(claimed_request(changed, &publisher).is_err());
        }
        let mut changed = value;
        changed["approved"] = serde_json::json!(true);
        assert!(claimed_request(changed, &publisher).is_err());
    }
}
