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
        PortfolioQuery::WorkflowHistory { query } => gateway_portfolio::history::history_result(
            engine.teams.automation_runs(
                query.pending_only,
                query.project_id,
                query.definition_id,
                query.after,
                query.limit,
            )?,
            now,
        ),
        PortfolioQuery::WorkflowRun { query } => {
            gateway_portfolio::history::run_result(&engine.teams.automation_run(query.run_id)?, now)
        }
        PortfolioQuery::WorkflowEvidence { query } => {
            let run = engine.teams.automation_run(query.run_id)?;
            let teams = run
                .stages
                .iter()
                .filter(|stage| stage.started && !stage.skipped)
                .map(|stage| engine.teams.snapshot(stage.team_run_id))
                .collect::<Result<Vec<_>>>()?;
            gateway_portfolio::history::evidence_result(
                &run,
                &teams,
                &engine.control.load()?,
                &query,
                now,
            )
        }
        PortfolioQuery::WorkflowDocument { query } => {
            let run = engine.teams.automation_run(query.run_id)?;
            let teams = run
                .stages
                .iter()
                .filter(|stage| stage.started && !stage.skipped)
                .map(|stage| engine.teams.snapshot(stage.team_run_id))
                .collect::<Result<Vec<_>>>()?;
            let records = engine.control.load()?;
            anyhow::ensure!(
                gateway_portfolio::history::accepted_artifacts(&run, &teams, &records)?
                    .iter()
                    .any(|row| row["id"] == query.artifact_id.to_string()),
                "Document is not accepted evidence"
            );
            let bytes = engine.control.read_document(query.artifact_id)?;
            gateway_portfolio::history::document_result(&run, &teams, &records, &query, &bytes, now)
        }
        PortfolioQuery::Inbox { query } => {
            let syncing = engine.control.sync_task_inbox(&engine.task_store)?;
            let mut page = engine.control.inbox_history(query)?;
            page.task_history_syncing = syncing;
            gateway_portfolio::inbox_result(page, now)
        }
        PortfolioQuery::Usage { query } => {
            gateway_portfolio::usage_result(engine.control.usage_ledger(query)?, now)
        }
        PortfolioQuery::RemoteExecutions { query } => {
            gateway_portfolio::remote::result(engine.teams.list_remote_executions(query)?, now)
        }
        PortfolioQuery::CloudBudgets { query } => gateway_portfolio::cloud_budgets_result(
            engine.control.cloud_budgets(query.after, query.limit)?,
            now,
        ),
        PortfolioQuery::CloudBilling { query } => gateway_portfolio::billing::accounts_result(
            engine.control.cloud_billing(query.after, query.limit)?,
            now,
        ),
        PortfolioQuery::CloudBillingReports { query } => {
            gateway_portfolio::billing::reports_result(
                engine.control.cloud_billing_reports(
                    query.scope_id,
                    query.after.as_deref(),
                    query.limit,
                )?,
                now,
            )
        }
        PortfolioQuery::CloudBillingCharges { query } => {
            gateway_portfolio::billing::charges_result(
                engine.control.cloud_billing_charges(
                    query.scope_id,
                    &query.export_id,
                    query.after,
                    query.limit,
                )?,
                now,
            )
        }
        PortfolioQuery::Projects { query } => {
            let projects = engine.tasks.lock().projects.clone();
            gateway_portfolio::projects_result(&projects, &query, now)
        }
    }
}
