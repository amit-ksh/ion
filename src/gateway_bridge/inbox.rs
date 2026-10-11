use crate::credentials::{CredentialService, GatewayApi};
use anyhow::{Context as _, Result, bail};
use std::time::{Duration, Instant};
use uuid::Uuid;
use waku_client::{Command, DaemonClient, ResponsePayload};
pub(super) fn sync(client: &DaemonClient, api: &GatewayApi) -> Result<()> {
    if !authorized(api)? || !api.has_inbox_control()? {
        return Ok(());
    }
    let started = Instant::now();
    for command in api.pending_inbox()?.into_iter().take(16) {
        if started.elapsed() >= Duration::from_secs(20) || !authorized(api)? {
            break;
        }
        let id = command["id"].as_str().context("Missing Inbox command")?;
        Uuid::parse_str(id)?;
        if command["publisherId"] != api.identity().1 {
            continue;
        }
        let claimed = api.claim_inbox(id)?;
        if claimed["id"] != id || claimed["status"] != "uncertain" {
            continue;
        }
        if !authorized(api)? {
            break;
        }
        let request = waku_protocol::gateway_inbox::claimed_request(claimed, api.identity().1)?;
        let ResponsePayload::InboxWrite { receipt } = client.request(
            Uuid::nil(),
            Uuid::nil(),
            Command::ApplyGatewayInbox { request },
        )?
        else {
            bail!("Missing Inbox receipt");
        };
        if !authorized(api)? {
            break;
        }
        api.settle_inbox(id, receipt.applied)?;
    }
    Ok(())
}

fn authorized(api: &GatewayApi) -> Result<bool> {
    let current = CredentialService::global().gateway_api()?;
    Ok(api.same_pairing(&current) && api.has_publish_grant()?)
}
