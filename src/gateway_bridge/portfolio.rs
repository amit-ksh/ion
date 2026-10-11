//! Temporary process-only pairing uses the desktop's existing daemon client.
use crate::{credentials::GatewayApi, model::Project};
use anyhow::{Result, bail};
use std::time::{Duration, Instant};
use uuid::Uuid;
use waku_client::{Command, DaemonClient, ResponsePayload};
use waku_protocol::gateway_portfolio::{self, PortfolioQuery};

pub(super) fn sync(client: &DaemonClient, api: &GatewayApi, projects: &[Project]) -> Result<()> {
    if !api.has_portfolio_relay()? {
        return Ok(());
    }
    let started = Instant::now();
    api.publish_portfolio_owner()?;
    for job in api.pending_portfolio()?.into_iter().take(16) {
        if started.elapsed() >= Duration::from_secs(20) {
            break;
        }
        if job.publisher_id != api.identity().1
            || job.status != "queued"
            || Uuid::parse_str(&job.id).is_err()
        {
            continue;
        }
        let result = read(client, projects, job.query).ok();
        api.complete_portfolio(&job.id, result)?;
    }
    Ok(())
}
fn read(
    client: &DaemonClient,
    projects: &[Project],
    value: serde_json::Value,
) -> Result<serde_json::Value> {
    let query: PortfolioQuery = serde_json::from_value(value)?;
    query.validate()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis()
        .try_into()?;
    match query {
        PortfolioQuery::Inbox { query } => {
            let ResponsePayload::InboxHistory { page } = client.request(
                Uuid::nil(),
                Uuid::nil(),
                Command::ListInboxHistory { query },
            )?
            else {
                bail!("Invalid Inbox response");
            };
            gateway_portfolio::inbox_result(page, now)
        }
        PortfolioQuery::Usage { query } => {
            let ResponsePayload::UsageLedger { page } =
                client.request(Uuid::nil(), Uuid::nil(), Command::LoadUsageLedger { query })?
            else {
                bail!("Invalid usage response");
            };
            gateway_portfolio::usage_result(page, now)
        }
        PortfolioQuery::Projects { query } => {
            gateway_portfolio::projects_result(projects, &query, now)
        }
    }
}
