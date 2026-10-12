//! Durable content-free Inbox publication on the existing owner worker.
use super::*;
pub(super) fn sync(api: &GatewayApi, engine: &OwnerEngine, fence: &StopFence) -> Result<()> {
    if fence.stopped() || !api.authorized()? || !api.has_inbox_notifications()? {
        return Ok(());
    }
    if engine.control.sync_task_inbox(&engine.task_store)? {
        return Ok(());
    }
    let after = api.inbox_publication_cursor()?;
    let batch = engine
        .control
        .inbox_notification_batch(after, crate::model::unix_time())?;
    if fence.stopped() || !api.authorized()? {
        return Ok(());
    }
    api.publish_inbox_notifications(serde_json::to_value(batch)?)
}
