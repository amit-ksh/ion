//! Process-only pairing uses the owner's same durable ascending history feed.
use crate::credentials::{CredentialService, GatewayApi};
use anyhow::{Result, bail};
use uuid::Uuid;
use waku_client::{Command, DaemonClient, ResponsePayload};
fn authorized(api: &GatewayApi) -> Result<bool> {
    let current = CredentialService::global().gateway_api()?;
    Ok(api.same_pairing(&current) && api.has_publish_grant()?)
}
pub(super) fn sync(client: &DaemonClient, api: &GatewayApi) -> Result<()> {
    if !authorized(api)? || !api.has_inbox_notifications()? {
        return Ok(());
    }
    let after = api.inbox_publication_cursor()?;
    let ResponsePayload::InboxNotifications { batch } = client.request(
        Uuid::nil(),
        Uuid::nil(),
        Command::LoadInboxNotifications { after },
    )?
    else {
        bail!("Invalid notification feed");
    };
    let Some(batch) = batch else {
        return Ok(());
    };
    if !authorized(api)? {
        return Ok(());
    }
    api.publish_inbox_notifications(serde_json::to_value(batch)?)
}
