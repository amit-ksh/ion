use super::*;
pub(super) fn sync(api: &GatewayApi, engine: &OwnerEngine, fence: &StopFence) -> Result<()> {
    if fence.stopped() || !api.authorized()? || !api.has_inbox_control()? {
        return Ok(());
    }
    let started = Instant::now();
    for command in api.pending_inbox()?.into_iter().take(16) {
        if fence.stopped() || started.elapsed() >= PASS_BUDGET || !api.authorized()? {
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
        let request = waku_protocol::gateway_inbox::claimed_request(claimed, api.identity().1)?;
        if fence.stopped() || !api.authorized()? {
            break;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis()
            .try_into()?;
        // Repeating this exact DB operation recovers its original receipt, even
        // if another client changed notification state after the first commit.
        let receipt = engine.control.apply_gateway_inbox(request, now)?;
        if fence.stopped() || !api.authorized()? {
            break;
        }
        api.settle_inbox(id, receipt.applied)?;
    }
    Ok(())
}
