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
        PortfolioQuery::WorkflowHistory { query } => {
            let ResponsePayload::AutomationRuns { page } = client.request(
                Uuid::nil(),
                Uuid::nil(),
                Command::ListAutomationRuns {
                    pending_only: query.pending_only,
                    project_id: query.project_id,
                    definition_id: query.definition_id,
                    after: query.after,
                    limit: query.limit,
                },
            )?
            else {
                bail!("Invalid historical run response");
            };
            gateway_portfolio::history::history_result(page, now)
        }
        PortfolioQuery::WorkflowRun { query } => {
            gateway_portfolio::history::run_result(&automation_run(client, query.run_id)?, now)
        }
        PortfolioQuery::WorkflowEvidence { query } => {
            let run = automation_run(client, query.run_id)?;
            let (teams, records) = evidence_sources(client, &run)?;
            gateway_portfolio::history::evidence_result(&run, &teams, &records, &query, now)
        }
        PortfolioQuery::WorkflowDocument { query } => {
            let run = automation_run(client, query.run_id)?;
            let (teams, records) = evidence_sources(client, &run)?;
            anyhow::ensure!(
                gateway_portfolio::history::accepted_artifacts(&run, &teams, &records)?
                    .iter()
                    .any(|row| row["id"] == query.artifact_id.to_string()),
                "Document is not accepted evidence"
            );
            let ResponsePayload::BlobData { bytes } = client.request(
                Uuid::nil(),
                Uuid::nil(),
                Command::ReadWorkspaceArtifact {
                    id: query.artifact_id,
                },
            )?
            else {
                bail!("Invalid document response");
            };
            gateway_portfolio::history::document_result(&run, &teams, &records, &query, &bytes, now)
        }
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
        PortfolioQuery::RemoteExecutions { query } => {
            let ResponsePayload::RemoteExecutions { page } = client.request(
                Uuid::nil(),
                Uuid::nil(),
                Command::ListRemoteExecutions { query },
            )?
            else {
                bail!("Invalid remote execution response");
            };
            gateway_portfolio::remote::result(page, now)
        }
        PortfolioQuery::CloudBudgets { query } => {
            let ResponsePayload::CloudBudgets { page } = client.request(
                Uuid::nil(),
                Uuid::nil(),
                Command::LoadCloudBudgets {
                    after: query.after,
                    limit: query.limit,
                },
            )?
            else {
                bail!("Invalid GPU budget response");
            };
            gateway_portfolio::cloud_budgets_result(page, now)
        }
        PortfolioQuery::CloudBilling { query } => {
            let ResponsePayload::CloudBilling { page } = client.request(
                Uuid::nil(),
                Uuid::nil(),
                Command::LoadCloudBilling {
                    after: query.after,
                    limit: query.limit,
                },
            )?
            else {
                bail!("Invalid billing account response");
            };
            gateway_portfolio::billing::accounts_result(page, now)
        }
        PortfolioQuery::CloudBillingReports { query } => {
            let scope_id = query.scope_id;
            let ResponsePayload::CloudBillingReports { page } = client.request(
                Uuid::nil(),
                Uuid::nil(),
                Command::LoadCloudBillingReports {
                    scope_id,
                    after: query.after,
                    limit: query.limit,
                },
            )?
            else {
                bail!("Invalid billing export response");
            };
            anyhow::ensure!(
                page.scope_id == scope_id,
                "Billing account identity changed"
            );
            gateway_portfolio::billing::reports_result(page, now)
        }
        PortfolioQuery::CloudBillingCharges { query } => {
            let scope_id = query.scope_id;
            let export_id = query.export_id;
            let ResponsePayload::CloudBillingCharges { page } = client.request(
                Uuid::nil(),
                Uuid::nil(),
                Command::LoadCloudBillingCharges {
                    scope_id,
                    export_id: export_id.clone(),
                    after: query.after,
                    limit: query.limit,
                },
            )?
            else {
                bail!("Invalid billing charge response");
            };
            anyhow::ensure!(
                page.report.scope_id == scope_id && page.report.export_id == export_id,
                "Billing export identity changed"
            );
            gateway_portfolio::billing::charges_result(page, now)
        }
        PortfolioQuery::Projects { query } => {
            gateway_portfolio::projects_result(projects, &query, now)
        }
    }
}

fn automation_run(
    client: &DaemonClient,
    id: Uuid,
) -> Result<waku_protocol::control::AutomationRun> {
    let ResponsePayload::AutomationRun { run } =
        client.request(Uuid::nil(), Uuid::nil(), Command::GetAutomationRun { id })?
    else {
        bail!("Historical run unavailable");
    };
    anyhow::ensure!(run.id == id, "Run identity changed");
    Ok(run)
}
fn evidence_sources(
    client: &DaemonClient,
    run: &waku_protocol::control::AutomationRun,
) -> Result<(
    Vec<waku_protocol::teams::TeamRunSnapshot>,
    waku_protocol::control::ControlSnapshot,
)> {
    let mut teams = Vec::new();
    for stage in run
        .stages
        .iter()
        .filter(|stage| stage.started && !stage.skipped)
    {
        let ResponsePayload::TeamRun { snapshot } = client.request(
            Uuid::nil(),
            Uuid::nil(),
            Command::GetTeamRun {
                run_id: stage.team_run_id,
            },
        )?
        else {
            bail!("Historical stage unavailable");
        };
        teams.push(snapshot);
    }
    let ResponsePayload::WorkspaceControl { snapshot } =
        client.request(Uuid::nil(), Uuid::nil(), Command::LoadWorkspaceControl)?
    else {
        bail!("Evidence catalogue unavailable");
    };
    Ok((teams, snapshot))
}
