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
    Inbox { query: InboxQuery },
    Usage { query: UsageLedgerQuery },
    Projects { query: PortfolioProjectsQuery },
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
            Self::Projects { query } => query.limit,
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
            .and_then(|row| row.get(if rows == "projects" { "id" } else { "sequence" }))
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
#[cfg(test)]
mod tests;
