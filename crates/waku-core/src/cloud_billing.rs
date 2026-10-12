//! Owner-scoped exact provider report snapshots. No reserve release, invoice finality or provisioning.
use crate::control::ControlStore;
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use waku_protocol::cloud_billing::*;
mod decimal;
mod discovery;
mod focus;
#[cfg(test)]
mod tests;
mod transport;
pub(crate) fn schema(db: &Connection) -> Result<()> {
    discovery::schema(db)?;
    db.execute_batch("CREATE TABLE IF NOT EXISTS ion_cloud_billing_scopes(id TEXT PRIMARY KEY,account_id TEXT NOT NULL,contract_id TEXT NOT NULL,currency TEXT NOT NULL,revision INTEGER NOT NULL,body TEXT NOT NULL,UNIQUE(account_id,contract_id,currency));")?;
    let columns: Vec<String> = db
        .prepare("PRAGMA table_info(ion_cloud_billing_scopes)")?
        .query_map([], |r| r.get(1))?
        .collect::<rusqlite::Result<_>>()?;
    if !columns.iter().any(|c| c == "contract_id") {
        // Preserve every pre-release definition and revision atomically before changing uniqueness.
        db.execute_batch("BEGIN IMMEDIATE; CREATE TABLE ion_cloud_billing_scopes_v2(id TEXT PRIMARY KEY,account_id TEXT NOT NULL,contract_id TEXT NOT NULL,currency TEXT NOT NULL,revision INTEGER NOT NULL,body TEXT NOT NULL,UNIQUE(account_id,contract_id,currency)); INSERT INTO ion_cloud_billing_scopes_v2 SELECT id,account_id,json_extract(body,'$.contractId'),json_extract(body,'$.currency'),revision,body FROM ion_cloud_billing_scopes; DROP TABLE ion_cloud_billing_scopes; ALTER TABLE ion_cloud_billing_scopes_v2 RENAME TO ion_cloud_billing_scopes; COMMIT;").map_err(|e|{let _=db.execute_batch("ROLLBACK");e})?;
    }
    db.execute_batch("CREATE TABLE IF NOT EXISTS ion_cloud_billing_reports(export_id TEXT PRIMARY KEY,scope_id TEXT NOT NULL,period TEXT NOT NULL,body TEXT NOT NULL,current INTEGER NOT NULL); CREATE UNIQUE INDEX IF NOT EXISTS ion_cloud_billing_current ON ion_cloud_billing_reports(scope_id,period) WHERE current=1; CREATE INDEX IF NOT EXISTS ion_cloud_billing_scope_reports ON ion_cloud_billing_reports(scope_id,period); CREATE TABLE IF NOT EXISTS ion_cloud_billing_charges(export_id TEXT NOT NULL,ordinal INTEGER NOT NULL,body TEXT NOT NULL,PRIMARY KEY(export_id,ordinal));")?;
    Ok(())
}

fn identifier(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= 160
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b)),
        "Billing identity is invalid"
    );
    Ok(())
}
fn validate(scope: &CloudBillingScope) -> Result<()> {
    ensure!(
        !scope.id.is_nil()
            && !scope.name.trim().is_empty()
            && scope.name.len() <= 160
            && !scope.name.chars().any(char::is_control),
        "Enter a billing connection name"
    );
    identifier(&scope.account_id)?;
    identifier(&scope.contract_id)?;
    ensure!(
        scope.currency.len() == 3 && scope.currency.bytes().all(|b| b.is_ascii_uppercase()),
        "Enter an uppercase billing currency"
    );
    transport::origin(&scope.archive_origin)?;
    Ok(())
}
fn scope(db: &Connection, id: Uuid) -> Result<(CloudBillingScope, u64)> {
    let (body, revision): (String, u64) = db
        .query_row(
            "SELECT body,revision FROM ion_cloud_billing_scopes WHERE id=?",
            [id.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .context("Billing connection no longer exists")?;
    Ok((serde_json::from_str(&body)?, revision))
}
fn status(db: &Connection, id: Uuid) -> Result<CloudBillingScopeStatus> {
    let (scope, revision) = scope(db, id)?;
    let reports:Vec<String>=db.prepare("SELECT body FROM ion_cloud_billing_reports WHERE scope_id=? AND current=1 ORDER BY period DESC LIMIT 12")?.query_map([id.to_string()],|r|r.get(0))?.collect::<rusqlite::Result<_>>()?;
    Ok(CloudBillingScopeStatus {
        scope,
        revision,
        reports: reports
            .into_iter()
            .map(|body| serde_json::from_str(&body))
            .collect::<std::result::Result<_, _>>()?,
    })
}
impl ControlStore {
    pub fn cloud_billing(&self, after: Option<Uuid>, limit: u16) -> Result<CloudBillingPage> {
        ensure!((1..=64).contains(&limit), "Billing page limit must be 1–64");
        let mut db = self.connect()?;
        let tx = db.transaction()?;
        let ids: Vec<String> = tx
            .prepare("SELECT id FROM ion_cloud_billing_scopes WHERE id>? ORDER BY id LIMIT ?")?
            .query_map(
                params![after.map(|a| a.to_string()).unwrap_or_default(), limit + 1],
                |r| r.get(0),
            )?
            .collect::<rusqlite::Result<_>>()?;
        let next_cursor = if ids.len() > limit as usize {
            Some(Uuid::parse_str(&ids[limit as usize - 1])?)
        } else {
            None
        };
        let total = tx.query_row("SELECT count(*) FROM ion_cloud_billing_scopes", [], |r| {
            r.get(0)
        })?;
        let scopes = ids
            .into_iter()
            .take(limit as usize)
            .map(|id| status(&tx, Uuid::parse_str(&id)?))
            .collect::<Result<_>>()?;
        tx.commit()?;
        Ok(CloudBillingPage {
            scopes,
            next_cursor,
            total,
        })
    }
    pub fn save_cloud_billing_scope(
        &self,
        mut input: CloudBillingScope,
        expected: Option<u64>,
    ) -> Result<CloudBillingScopeStatus> {
        validate(&input)?;
        input.archive_origin = transport::origin(&input.archive_origin)?
            .origin()
            .ascii_serialization();
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM ion_cloud_billing_scopes WHERE id=?)",
            [input.id.to_string()],
            |r| r.get(0),
        )?;
        let revision = if exists {
            let (old, revision) = scope(&tx, input.id)?;
            if old == input {
                let result = status(&tx, input.id)?;
                tx.commit()?;
                return Ok(result);
            }
            ensure!(
                expected == Some(revision),
                "Billing connection changed; refresh before saving"
            );
            let reports: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM ion_cloud_billing_reports WHERE scope_id=?)",
                [input.id.to_string()],
                |r| r.get(0),
            )?;
            ensure!(
                !reports
                    || (old.account_id == input.account_id
                        && old.contract_id == input.contract_id
                        && old.currency == input.currency
                        && old.archive_origin == input.archive_origin),
                "Imported billing identity and currency cannot be changed"
            );
            revision
                .checked_add(1)
                .context("Billing revision exceeds bound")?
        } else {
            ensure!(
                expected.is_none(),
                "Billing connection changed; refresh before saving"
            );
            let count: u64 =
                tx.query_row("SELECT count(*) FROM ion_cloud_billing_scopes", [], |r| {
                    r.get(0)
                })?;
            ensure!(count < 64, "Billing connection collection exceeds 64");
            1
        };
        let duplicate:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM ion_cloud_billing_scopes WHERE account_id=? AND contract_id=? AND currency=? AND id<>?)",params![input.account_id,input.contract_id,input.currency,input.id.to_string()],|r|r.get(0))?;
        ensure!(
            !duplicate,
            "This account, contract and currency already have a connection"
        );
        tx.execute("INSERT INTO ion_cloud_billing_scopes(id,account_id,contract_id,currency,revision,body) VALUES(?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET account_id=excluded.account_id,contract_id=excluded.contract_id,currency=excluded.currency,revision=excluded.revision,body=excluded.body",params![input.id.to_string(),input.account_id,input.contract_id,input.currency,revision,serde_json::to_string(&input)?])?;
        let result = status(&tx, input.id)?;
        tx.commit()?;
        Ok(result)
    }
    pub fn sync_cloud_billing(
        &self,
        id: Uuid,
        revision: u64,
        export_id: &str,
        now: u64,
    ) -> Result<CloudBillingReport> {
        identifier(export_id)?;
        let (scope, saved_revision) = scope(&self.connect()?, id)?;
        ensure!(
            revision == saved_revision,
            "Billing connection changed; refresh before sync"
        );
        let (export, archive) = transport::native(&scope, export_id, now)?;
        let received = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        self.commit_cloud_billing(&scope, revision, export, archive, received.max(now))
    }
    fn commit_cloud_billing(
        &self,
        selected: &CloudBillingScope,
        revision: u64,
        export: transport::Export,
        archive: Vec<u8>,
        now: u64,
    ) -> Result<CloudBillingReport> {
        let archive_sha256 = format!("{:x}", Sha256::digest(&archive));
        let parsed = focus::parse(selected, &export.period, &archive)?;
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (current, current_revision) = scope(&tx, selected.id)?;
        ensure!(
            &current == selected && current_revision == revision,
            "Billing connection changed during sync; no report was saved"
        );
        let old: Option<String> = tx
            .query_row(
                "SELECT body FROM ion_cloud_billing_reports WHERE export_id=?",
                [&export.id],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(body) = old {
            let report: CloudBillingReport = serde_json::from_str(&body)?;
            ensure!(
                report.scope_id == selected.id
                    && report.period == export.period
                    && report.created_at == export.created.to_rfc3339()
                    && report.archive_sha256 == archive_sha256
                    && report.csv_sha256 == parsed.csv_hash,
                "Previously imported billing export changed; use a new export"
            );
            tx.commit()?;
            return Ok(report);
        }
        let count: u64 =
            tx.query_row("SELECT count(*) FROM ion_cloud_billing_reports", [], |r| {
                r.get(0)
            })?;
        ensure!(count < 4096, "Billing report collection exceeds its limit");
        let previous:Option<String>=tx.query_row("SELECT body FROM ion_cloud_billing_reports WHERE scope_id=? AND period=? AND current=1",params![selected.id.to_string(),export.period],|r|r.get(0)).optional()?;
        let saved_rows: u64 =
            tx.query_row("SELECT count(*) FROM ion_cloud_billing_charges", [], |r| {
                r.get(0)
            })?;
        ensure!(
            saved_rows
                .checked_add(parsed.charges.len() as u64)
                .is_some_and(|n| n <= 1_000_000),
            "Billing history exceeds one million stored charge rows; no partial report was saved"
        );
        let mut latest = true;
        if let Some(body) = previous {
            let mut prior: CloudBillingReport = serde_json::from_str(&body)?;
            let at = chrono::DateTime::parse_from_rfc3339(&prior.created_at)?;
            ensure!(
                at != export.created || prior.csv_sha256 == parsed.csv_hash,
                "Billing exports share an ambiguous creation time; use a newer export"
            );
            latest = export.created > at;
            if latest {
                prior.current = false;
                tx.execute(
                    "UPDATE ion_cloud_billing_reports SET current=0,body=? WHERE export_id=?",
                    params![serde_json::to_string(&prior)?, prior.export_id],
                )?;
            }
        }
        let report = CloudBillingReport {
            scope_id: selected.id,
            export_id: export.id,
            period: export.period,
            currency: selected.currency.clone(),
            billed_cost: parsed.billed,
            effective_cost: parsed.effective,
            contracted_cost: parsed.contracted,
            rows: parsed.charges.len() as u64,
            unattributed_resource_rows: parsed.missing_resources,
            created_at: export.created.to_rfc3339(),
            fetched_at: now,
            archive_sha256,
            csv_sha256: parsed.csv_hash,
            current: latest,
        };
        tx.execute("INSERT INTO ion_cloud_billing_reports(export_id,scope_id,period,body,current) VALUES(?,?,?,?,?)",params![report.export_id,selected.id.to_string(),report.period,serde_json::to_string(&report)?,i64::from(latest)])?;
        for charge in parsed.charges {
            tx.execute(
                "INSERT INTO ion_cloud_billing_charges(export_id,ordinal,body) VALUES(?,?,?)",
                params![
                    report.export_id,
                    charge.ordinal,
                    serde_json::to_string(&charge)?
                ],
            )?;
        }
        tx.commit()?;
        Ok(report)
    }
    pub fn cloud_billing_reports(
        &self,
        id: Uuid,
        after: Option<&str>,
        limit: u16,
    ) -> Result<CloudBillingReportPage> {
        ensure!(
            (1..=32).contains(&limit),
            "Invalid billing export history page"
        );
        if let Some(after) = after {
            identifier(after)?;
        }
        let mut db = self.connect()?;
        let tx = db.transaction()?;
        scope(&tx, id)?;
        let rows:Vec<String>=tx.prepare("SELECT body FROM ion_cloud_billing_reports WHERE scope_id=? AND export_id>? ORDER BY export_id LIMIT ?")?.query_map(params![id.to_string(),after.unwrap_or(""),limit+1],|r|r.get(0))?.collect::<rusqlite::Result<_>>()?;
        let more = rows.len() > limit as usize;
        let reports: Vec<CloudBillingReport> = rows
            .into_iter()
            .take(limit as usize)
            .map(|r| serde_json::from_str(&r))
            .collect::<std::result::Result<_, _>>()?;
        let next_cursor = if more {
            reports.last().map(|r| r.export_id.clone())
        } else {
            None
        };
        let total = tx.query_row(
            "SELECT count(*) FROM ion_cloud_billing_reports WHERE scope_id=?",
            [id.to_string()],
            |r| r.get(0),
        )?;
        tx.commit()?;
        Ok(CloudBillingReportPage {
            scope_id: id,
            reports,
            next_cursor,
            total,
        })
    }
    pub fn cloud_billing_charges(
        &self,
        id: Uuid,
        export_id: &str,
        after: Option<u64>,
        limit: u16,
    ) -> Result<CloudBillingChargePage> {
        identifier(export_id)?;
        ensure!(
            (1..=64).contains(&limit) && after.unwrap_or(0) <= 100_000,
            "Invalid billing charge page"
        );
        let mut db = self.connect()?;
        let tx = db.transaction()?;
        let body: String = tx
            .query_row(
                "SELECT body FROM ion_cloud_billing_reports WHERE scope_id=? AND export_id=?",
                params![id.to_string(), export_id],
                |r| r.get(0),
            )
            .context("Billing report no longer exists")?;
        let report = serde_json::from_str(&body)?;
        let rows:Vec<String>=tx.prepare("SELECT body FROM ion_cloud_billing_charges WHERE export_id=? AND ordinal>? ORDER BY ordinal LIMIT ?")?.query_map(params![export_id,after.unwrap_or(0),limit+1],|r|r.get(0))?.collect::<rusqlite::Result<_>>()?;
        let more = rows.len() > limit as usize;
        let charges: Vec<CloudBillingCharge> = rows
            .into_iter()
            .take(limit as usize)
            .map(|r| serde_json::from_str(&r))
            .collect::<std::result::Result<_, _>>()?;
        let next_cursor = if more {
            charges.last().map(|c| c.ordinal)
        } else {
            None
        };
        tx.commit()?;
        Ok(CloudBillingChargePage {
            report,
            charges,
            next_cursor,
        })
    }
}
