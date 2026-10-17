//! Read-only owner queries for the outbound personal gateway. No effect command
//! or credential is representable. Bounded page transfers retain full counts.
use crate::{
    inbox::{InboxPage, InboxQuery},
    model::Project,
    usage_ledger::{UsageLedgerPage, UsageLedgerQuery},
};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum PortfolioQuery {
    Inbox {
        query: InboxQuery,
    },
    Usage {
        query: UsageLedgerQuery,
    },
    RemoteExecutions {
        query: crate::remote_control::RemoteExecutionQuery,
    },
    Projects {
        query: PortfolioProjectsQuery,
    },
    CloudBudgets {
        query: PortfolioProjectsQuery,
    },
    CloudBilling {
        query: PortfolioProjectsQuery,
    },
    CloudBillingReports {
        query: billing::BillingReportsQuery,
    },
    CloudBillingCharges {
        query: billing::BillingChargesQuery,
    },
    WorkflowHistory {
        query: history::WorkflowHistoryQuery,
    },
    WorkflowRun {
        query: history::WorkflowRunQuery,
    },
    WorkflowEvidence {
        query: history::WorkflowEvidenceQuery,
    },
    WorkflowHtmlManifest {
        query: crate::html_resources::WorkflowHtmlManifestQuery,
    },
    WorkflowHtmlSource {
        query: crate::html_resources::WorkflowHtmlSourceQuery,
    },
    WorkflowDocument {
        query: history::WorkflowDocumentQuery,
    },
    WorkflowPdfPage {
        query: pdf::WorkflowPdfQuery,
    },
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PortfolioProjectsQuery {
    pub after: Option<Uuid>,
    pub limit: u16,
}
impl PortfolioQuery {
    pub fn validate(&self) -> Result<()> {
        let limit = match self {
            Self::Inbox { query } => query.limit,
            Self::Usage { query } => query.limit,
            Self::RemoteExecutions { query } => {
                if let Some(cursor) = &query.after {
                    ensure!(
                        !cursor.request_id.is_nil(),
                        "Remote cursor must identify an original request"
                    );
                    ensure!(
                        query.kind.is_none_or(|kind| kind == cursor.kind),
                        "Remote cursor kind differs from filter"
                    );
                }
                query.limit
            }
            Self::Projects { query }
            | Self::CloudBudgets { query }
            | Self::CloudBilling { query } => query.limit,
            Self::CloudBillingReports { query } => {
                query.validate()?;
                query.limit
            }
            Self::CloudBillingCharges { query } => {
                query.validate()?;
                query.limit
            }
            Self::WorkflowHistory { query } => query.limit,
            Self::WorkflowEvidence { query } => query.limit,
            Self::WorkflowRun { .. } => 1,
            Self::WorkflowPdfPage { query } => {
                query.validate()?;
                return Ok(());
            }
            Self::WorkflowHtmlManifest { query } => return query.validate(),
            Self::WorkflowHtmlSource { query } => {
                query.source.validate()?;
                return crate::html_resources::WorkflowHtmlManifestQuery {
                    run_id: query.run_id,
                    artifact_id: query.artifact_id,
                    sha256: query.sha256.clone(),
                }
                .validate();
            }
            Self::WorkflowDocument { query } => {
                ensure!(
                    (1..=16_384).contains(&query.limit),
                    "Document chunk must contain 1–16384 bytes"
                );
                ensure!(
                    query.offset < 16 * 1024 * 1024,
                    "Document offset exceeds bound"
                );
                ensure!(
                    query.sha256.len() == 64
                        && query
                            .sha256
                            .bytes()
                            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
                    "Invalid document digest"
                );
                return Ok(());
            }
        };
        ensure!(
            (1..=32).contains(&limit),
            "Gateway read page must contain 1–32 rows"
        );
        Ok(())
    }
}
fn label(value: &str) -> String {
    value.chars().take(80).collect()
}
fn bounded(mut value: Value, rows: &str) -> Result<Value> {
    while value.to_string().len() > 60_000 {
        let entries = value["page"][rows]
            .as_array_mut()
            .ok_or_else(|| anyhow::anyhow!("Invalid gateway page"))?;
        ensure!(
            !entries.is_empty(),
            "Gateway summary exceeds transfer bound"
        );
        entries.pop();
        value["pageTruncated"] = true.into();
        let last = value["page"][rows]
            .as_array()
            .and_then(|rows| rows.last())
            .and_then(|row| row.get(if rows == "entries" { "sequence" } else { "id" }))
            .cloned();
        // An empty oversized page must fail, never suggest that history is done.
        ensure!(last.is_some(), "Gateway row exceeds transfer bound");
        value["page"]["nextCursor"] = last.unwrap();
    }
    Ok(value)
}
pub fn inbox_result(mut page: InboxPage, served_at: u64) -> Result<Value> {
    let complete: Vec<bool> = page
        .entries
        .iter()
        .map(|e| e.detail.chars().count() <= 500)
        .collect();
    for entry in &mut page.entries {
        entry.title = label(&entry.title);
        entry.detail = entry.detail.chars().take(500).collect();
    }
    let mut page = serde_json::to_value(page)?;
    for (row, complete) in page["entries"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .zip(complete)
    {
        row["detailComplete"] = complete.into();
    }
    bounded(
        json!({"kind":"inbox","page":page,"pageTruncated":false,"servedAt":served_at}),
        "entries",
    )
}
pub fn usage_result(page: UsageLedgerPage, served_at: u64) -> Result<Value> {
    let mut page = serde_json::to_value(page)?;
    for row in page["entries"].as_array_mut().unwrap() {
        row.as_object_mut().unwrap().remove("host");
        if let Some(cost) = row["cost"].as_object_mut() {
            cost.remove("origin");
        }
        if let Some(name) = row["agentName"].as_str() {
            row["agentName"] = label(name).into();
        }
        if let Some(status) = row["status"].as_str() {
            row["status"] = label(status).into();
        }
    }
    bounded(
        json!({"kind":"usage","page":page,"pageTruncated":false,"servedAt":served_at}),
        "entries",
    )
}
pub fn projects_result(
    projects: &[Project],
    query: &PortfolioProjectsQuery,
    served_at: u64,
) -> Result<Value> {
    ensure!(
        (1..=32).contains(&query.limit),
        "Gateway project page must contain 1–32 rows"
    );
    let mut ordered: Vec<_> = projects
        .iter()
        .filter(|p| query.after.is_none_or(|id| p.id > id))
        .collect();
    ordered.sort_by_key(|p| p.id);
    let more = ordered.len() > usize::from(query.limit);
    ordered.truncate(usize::from(query.limit));
    let next = if more {
        ordered.last().map(|p| p.id)
    } else {
        None
    };
    let rows: Vec<_> = ordered
        .into_iter()
        .map(|p| json!({"id":p.id,"name":label(&p.name)}))
        .collect();
    bounded(
        json!({"kind":"projects","page":{"projects":rows,"nextCursor":next,"total":projects.len()},"pageTruncated":false,"servedAt":served_at}),
        "projects",
    )
}
pub mod billing;
pub mod history;
pub mod pdf;

pub mod remote;

#[cfg(test)]
mod tests;

/// Phone cost readers receive totals, not target aliases or cloud resource identities.
pub fn cloud_budgets_result(
    page: crate::cloud_spending::CloudBudgetPage,
    served_at: u64,
) -> Result<Value> {
    let budgets:Vec<Value>=page.budgets.into_iter().map(|row|json!({"id":row.budget.id,"name":label(&row.budget.name),"currency":row.budget.currency,"limitMicros":row.budget.limit_micros,"paused":row.budget.paused,"heldMicros":row.held_micros,"availableMicros":row.available_micros,"requests":row.requests})).collect();
    bounded(
        json!({"kind":"cloudBudgets","page":{"budgets":budgets,"nextCursor":page.next_cursor,"total":page.total},"pageTruncated":false,"servedAt":served_at}),
        "budgets",
    )
}
