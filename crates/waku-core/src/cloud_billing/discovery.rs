//! Bounded GET-only discovery. Opaque continuations remain private to the owner store.
use super::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const TTL: u64 = 15 * 60;
const CURSORS: u64 = 512;
const PAGES: usize = 64;
const PAGE_LIMIT: u16 = 32;
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    scope_hash: String,
    revision: u64,
    limit: u16,
    created_at: u64,
    token: String,
    token_hashes: Vec<String>,
    export_ids: Vec<String>,
}
pub(super) fn schema(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS ion_cloud_billing_discovery_cursors(id TEXT PRIMARY KEY,created_at INTEGER NOT NULL,body TEXT NOT NULL); CREATE INDEX IF NOT EXISTS ion_cloud_billing_discovery_expiry ON ion_cloud_billing_discovery_cursors(created_at);")?;
    Ok(())
}
fn scope_hash(scope: &CloudBillingScope) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(scope)?)))
}
fn validate_query(query: &CloudBillingExportQuery) -> Result<()> {
    ensure!(
        !query.scope_id.is_nil()
            && query.expected_revision > 0
            && !query.after.is_some_and(|id| id.is_nil()),
        "Select a saved billing connection before discovery"
    );
    ensure!(
        (1..=PAGE_LIMIT).contains(&query.limit),
        "Billing discovery page limit must be 1–32"
    );
    Ok(())
}
fn cursor(
    db: &Connection,
    scope: &CloudBillingScope,
    query: &CloudBillingExportQuery,
    now: u64,
) -> Result<Cursor> {
    if let Some(id) = query.after {
        let body: String = db
            .query_row(
                "SELECT body FROM ion_cloud_billing_discovery_cursors WHERE id=?",
                [id.to_string()],
                |r| r.get(0),
            )
            .context("Billing discovery expired; discover exports again")?;
        let cursor: Cursor =
            serde_json::from_str(&body).context("Billing discovery continuation is unavailable")?;
        ensure!(
            cursor.scope_hash == scope_hash(scope)?
                && cursor.revision == query.expected_revision
                && cursor.limit == query.limit,
            "Billing discovery scope changed; discover exports again"
        );
        ensure!(
            now >= cursor.created_at && now - cursor.created_at < TTL,
            "Billing discovery expired; discover exports again"
        );
        ensure!(
            !cursor.token.is_empty()
                && cursor.token_hashes.len() <= PAGES
                && cursor.export_ids.len() <= PAGES * PAGE_LIMIT as usize,
            "Billing discovery continuation exceeds its bounds"
        );
        transport::page_token(&cursor.token)?;
        Ok(cursor)
    } else {
        Ok(Cursor {
            scope_hash: scope_hash(scope)?,
            revision: query.expected_revision,
            limit: query.limit,
            created_at: now,
            token: String::new(),
            token_hashes: vec![],
            export_ids: vec![],
        })
    }
}
fn prepare(
    db: &Connection,
    query: &CloudBillingExportQuery,
    now: u64,
) -> Result<(CloudBillingScope, Cursor)> {
    validate_query(query)?;
    let (scope, revision) = scope(db, query.scope_id)?;
    ensure!(
        revision == query.expected_revision,
        "Billing connection changed; refresh before discovery"
    );
    let cursor = cursor(db, &scope, query, now)?;
    ensure!(
        cursor.token_hashes.len() < PAGES,
        "Billing discovery reached 64 pages; discover exports again"
    );
    Ok((scope, cursor))
}
impl ControlStore {
    pub fn discover_cloud_billing_exports(
        &self,
        query: CloudBillingExportQuery,
        now: u64,
    ) -> Result<CloudBillingExportPage> {
        // Scope/cursor validation precedes credential access and every provider request.
        let (selected, cursor) = prepare(&self.connect()?, &query, now)?;
        let page = transport::discover(&selected, &cursor.token, query.limit, now)?;
        let received = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs()
            .max(now);
        self.commit_discovery(&selected, &query, cursor, page, received)
    }
    fn commit_discovery(
        &self,
        selected: &CloudBillingScope,
        query: &CloudBillingExportQuery,
        mut prior: Cursor,
        page: transport::ExportDiscovery,
        now: u64,
    ) -> Result<CloudBillingExportPage> {
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (current, mut fresh) = prepare(&tx, query, now)?;
        if query.after.is_none() {
            ensure!(
                now >= prior.created_at && now - prior.created_at < TTL,
                "Billing discovery expired during the provider request; discover exports again"
            );
            fresh.created_at = prior.created_at;
        }
        ensure!(
            &current == selected && serde_json::to_vec(&fresh)? == serde_json::to_vec(&prior)?,
            "Billing connection or discovery changed; no discovery page was accepted"
        );
        let mut seen: HashSet<&str> = prior.export_ids.iter().map(String::as_str).collect();
        for id in &page.export_ids {
            ensure!(
                seen.insert(id),
                "Billing export pagination repeated an identity; discover exports again"
            );
        }
        let next_cursor = if page.next_token.is_empty() {
            None
        } else {
            let token_hash = format!("{:x}", Sha256::digest(page.next_token.as_bytes()));
            ensure!(
                !prior.token_hashes.contains(&token_hash),
                "Billing export pagination did not advance; discover exports again"
            );
            ensure!(
                prior.token_hashes.len() + 1 < PAGES,
                "Billing discovery reached 64 pages; discover exports again"
            );
            prior.token_hashes.push(token_hash);
            prior.export_ids.extend(page.export_ids);
            prior.token = page.next_token;
            let cutoff = now.saturating_sub(TTL);
            tx.execute(
                "DELETE FROM ion_cloud_billing_discovery_cursors WHERE created_at<=?",
                [cutoff],
            )?;
            let count: u64 = tx.query_row(
                "SELECT count(*) FROM ion_cloud_billing_discovery_cursors",
                [],
                |r| r.get(0),
            )?;
            ensure!(
                count < CURSORS,
                "Billing discovery cache is full; retry after 15 minutes"
            );
            let id = Uuid::new_v4();
            tx.execute(
                "INSERT INTO ion_cloud_billing_discovery_cursors(id,created_at,body) VALUES(?,?,?)",
                params![
                    id.to_string(),
                    prior.created_at,
                    serde_json::to_string(&prior)?
                ],
            )?;
            Some(id)
        };
        // Expiration is checked again at receipt time; old ready rows do not survive a slow request.
        let items = page
            .items
            .into_iter()
            .filter(|row| {
                chrono::DateTime::parse_from_rfc3339(&row.expires_at)
                    .is_ok_and(|at| at.timestamp() > i64::try_from(now).unwrap_or(i64::MAX))
            })
            .collect();
        tx.commit()?;
        Ok(CloudBillingExportPage {
            scope_id: query.scope_id,
            revision: query.expected_revision,
            items,
            examined: page.examined,
            next_cursor,
            served_at: now,
        })
    }
}
#[cfg(test)]
mod tests;
