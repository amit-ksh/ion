use super::*;
use waku_protocol::gateway_portfolio::{self, PortfolioQuery};

pub(super) fn sync(api: &GatewayApi, engine: &OwnerEngine, fence: &StopFence) -> Result<()> {
    if fence.stopped() || !api.has_portfolio_relay()? || !api.authorized()? {
        return Ok(());
    }
    let started = Instant::now();
    api.publish_portfolio_owner()?;
    for job in api.pending_portfolio()?.into_iter().take(16) {
        if fence.stopped() || started.elapsed() >= PASS_BUDGET {
            break;
        }
        if job.publisher_id != api.identity().1
            || job.status != "queued"
            || Uuid::parse_str(&job.id).is_err()
        {
            continue;
        }
        let result = read(engine, job.query).ok();
        if fence.stopped() || !api.authorized()? {
            break;
        }
        // The HTTP route rechecks the requesting phone's read grant at completion.
        // Retrying this exact read after a lost response has no task/app effects.
        api.complete_portfolio(&job.id, result)?;
    }
    Ok(())
}
pub(super) fn read(engine: &OwnerEngine, value: serde_json::Value) -> Result<serde_json::Value> {
    let query: PortfolioQuery = serde_json::from_value(value)?;
    query.validate()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis()
        .try_into()?;
    match query {
        PortfolioQuery::Inbox { query } => {
            let syncing = engine.control.sync_task_inbox(&engine.task_store)?;
            let mut page = engine.control.inbox_history(query)?;
            page.task_history_syncing = syncing;
            gateway_portfolio::inbox_result(page, now)
        }
        PortfolioQuery::Usage { query } => {
            gateway_portfolio::usage_result(engine.control.usage_ledger(query)?, now)
        }
        PortfolioQuery::Projects { query } => {
            let projects = engine.tasks.lock().projects.clone();
            gateway_portfolio::projects_result(&projects, &query, now)
        }
    }
}
