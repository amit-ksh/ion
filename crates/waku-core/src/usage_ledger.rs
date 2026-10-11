//! Durable global metadata copied at owner reservation/settlement boundaries.
//! No client publication command. Reads and source triggers run only on workers.
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, params, types::Value};
use uuid::Uuid;
use waku_protocol::usage_ledger::*;
pub(crate) fn schema(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS ion_usage_ledger (
 seq INTEGER PRIMARY KEY AUTOINCREMENT,kind TEXT NOT NULL,source_id TEXT NOT NULL,
 project_id TEXT NOT NULL,task_id TEXT NOT NULL,run_id TEXT,work_id TEXT,attempt_id TEXT,
 agent_id TEXT,agent_name TEXT,provider TEXT,model TEXT,environment TEXT,host TEXT NOT NULL,
 observed_at INTEGER NOT NULL DEFAULT (CAST(strftime('%s','now') AS INTEGER)),started_at INTEGER,updated_at INTEGER NOT NULL,status TEXT NOT NULL,
 input_tokens INTEGER,output_tokens INTEGER,reserved_tokens INTEGER NOT NULL DEFAULT 0,
 elapsed_ms INTEGER,cost TEXT,allowance TEXT,UNIQUE(kind,source_id));
 CREATE INDEX IF NOT EXISTS ion_usage_project ON ion_usage_ledger(project_id,seq DESC);
 CREATE INDEX IF NOT EXISTS ion_usage_task ON ion_usage_ledger(task_id,seq DESC);
 CREATE INDEX IF NOT EXISTS ion_usage_agent ON ion_usage_ledger(agent_id,seq DESC);
 CREATE INDEX IF NOT EXISTS ion_usage_provider ON ion_usage_ledger(provider,seq DESC);
 CREATE INDEX IF NOT EXISTS ion_usage_model ON ion_usage_ledger(model,seq DESC);
 CREATE INDEX IF NOT EXISTS ion_usage_environment ON ion_usage_ledger(environment,seq DESC);
 CREATE INDEX IF NOT EXISTS ion_usage_kind ON ion_usage_ledger(kind,seq DESC);
 CREATE INDEX IF NOT EXISTS ion_usage_time ON ion_usage_ledger(COALESCE(started_at,observed_at),seq DESC);
 CREATE TABLE IF NOT EXISTS ion_usage_meta(id INTEGER PRIMARY KEY CHECK(id=1),revision INTEGER NOT NULL);
 INSERT OR IGNORE INTO ion_usage_meta VALUES(1,0);
 CREATE TABLE IF NOT EXISTS ion_usage_install(name TEXT PRIMARY KEY);
 CREATE TRIGGER IF NOT EXISTS ion_usage_added AFTER INSERT ON ion_usage_ledger BEGIN UPDATE ion_usage_meta SET revision=revision+1 WHERE id=1; END;
 CREATE TRIGGER IF NOT EXISTS ion_usage_changed AFTER UPDATE ON ion_usage_ledger BEGIN UPDATE ion_usage_meta SET revision=revision+1 WHERE id=1; END;")?;
    Ok(())
}
const COLUMNS: &str = "kind,source_id,project_id,task_id,run_id,work_id,attempt_id,agent_id,agent_name,provider,model,environment,host,started_at,updated_at,status,input_tokens,output_tokens,reserved_tokens,elapsed_ms,cost,allowance";
// All identifiers/expressions here are fixed trusted source, never a model/client value.
const JOINS: &str = " JOIN ion_team_work w ON w.id=c.work_id JOIN ion_team_runs r ON r.id=c.run_id LEFT JOIN json_each(r.body,'$.profiles') p ON json_extract(p.value,'$.id')=w.agent_id LEFT JOIN json_each(w.body,'$.attempts') a ON json_extract(a.value,'$.id')=c.attempt_id";
const IDENTITY: &str = "r.project_id,w.task_id,r.id,w.id,c.attempt_id,w.agent_id,json_extract(p.value,'$.name'),json_extract(p.value,'$.provider')";
const ACTUAL_HOST: &str =
    "json_extract(a.value,'$.environment'),COALESCE(json_extract(a.value,'$.host'),'Unknown')";
fn model_select() -> String {
    format!(
        "SELECT 'modelCall',c.id,{IDENTITY},c.model,{ACTUAL_HOST},c.started_at,CAST(strftime('%s','now') AS INTEGER),CASE WHEN b.input_tokens IS NOT NULL AND b.output_tokens IS NOT NULL THEN 'reported' WHEN b.call_id IS NOT NULL THEN 'reserved' ELSE 'unknown' END,b.input_tokens,b.output_tokens,CASE WHEN b.input_tokens IS NULL THEN COALESCE(b.input_cap+b.output_cap,0) ELSE 0 END,NULL,NULL,NULL FROM ion_team_model_calls c{JOINS} LEFT JOIN ion_team_call_budgets b ON b.call_id=c.id WHERE 1"
    )
}
fn command_select() -> String {
    format!(
        "SELECT 'remoteCommand',c.request_id,{IDENTITY},NULL,'nebiusSandbox',c.project,NULL,CAST(strftime('%s','now') AS INTEGER),c.state,NULL,NULL,0,json_extract(c.receipt,'$.elapsed_milliseconds'),NULL,NULL FROM ion_team_jobs c{JOINS} WHERE 1"
    )
}
fn gpu_select() -> String {
    format!(
        "SELECT 'gpuJob',c.request_id,{IDENTITY},NULL,'nebiusSandbox',c.project,NULL,CAST(strftime('%s','now') AS INTEGER),c.state,NULL,NULL,0,(SELECT json_extract(receipt,'$.elapsed_milliseconds') FROM ion_team_gpu_results WHERE request_id=c.request_id),NULL,json_object('currency',c.currency,'micros',c.reserved_micros) FROM ion_team_gpu_jobs c{JOINS} WHERE 1"
    )
}
fn upsert(select: &str) -> String {
    format!(
        "INSERT INTO ion_usage_ledger({COLUMNS}) {select} ON CONFLICT(kind,source_id) DO UPDATE SET updated_at=excluded.updated_at,status=excluded.status,input_tokens=excluded.input_tokens,output_tokens=excluded.output_tokens,reserved_tokens=excluded.reserved_tokens,elapsed_ms=excluded.elapsed_ms WHERE ion_usage_ledger.status IS NOT excluded.status OR ion_usage_ledger.input_tokens IS NOT excluded.input_tokens OR ion_usage_ledger.output_tokens IS NOT excluded.output_tokens OR ion_usage_ledger.reserved_tokens IS NOT excluded.reserved_tokens OR ion_usage_ledger.elapsed_ms IS NOT excluded.elapsed_ms;"
    )
}
pub(crate) fn install_teams(db: &Connection) -> Result<()> {
    schema(db)?;
    for (name, table, key, select) in [
        ("models", "ion_team_model_calls", "id", model_select()),
        ("commands", "ion_team_jobs", "request_id", command_select()),
        ("gpu", "ion_team_gpu_jobs", "request_id", gpu_select()),
    ] {
        let event = upsert(&format!("{select} AND c.{key}=NEW.{key}"));
        db.execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS ion_usage_{name}_insert AFTER INSERT ON {table} BEGIN {event} END; CREATE TRIGGER IF NOT EXISTS ion_usage_{name}_update AFTER UPDATE ON {table} BEGIN {event} END;"))?;
        db.execute_batch("SAVEPOINT usage_install")?;
        if db.execute("INSERT OR IGNORE INTO ion_usage_install VALUES(?)", [name])? == 1 {
            db.execute_batch(&upsert(&select))?;
        }
        db.execute_batch("RELEASE usage_install")?;
    }
    let event = upsert(&format!("{} AND c.id=NEW.call_id", model_select()));
    db.execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS ion_usage_budget_insert AFTER INSERT ON ion_team_call_budgets BEGIN {event} END; CREATE TRIGGER IF NOT EXISTS ion_usage_budget_update AFTER UPDATE ON ion_team_call_budgets BEGIN {event} END;"))?;
    let event = upsert(&format!("{} AND c.request_id=NEW.request_id", gpu_select()));
    db.execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS ion_usage_gpu_result AFTER INSERT ON ion_team_gpu_results BEGIN {event} END;"))?;
    Ok(())
}
fn tag(v: impl serde::Serialize) -> Result<String> {
    Ok(serde_json::to_value(v)?
        .as_str()
        .context("Invalid usage tag")?
        .to_owned())
}
impl crate::control::ControlStore {
    pub fn usage_ledger(&self, query: UsageLedgerQuery) -> Result<UsageLedgerPage> {
        ensure!(
            (1..=64).contains(&query.limit),
            "Usage page size must be 1–64"
        );
        ensure!(
            query.after.is_none_or(|v| v > 0 && v <= i64::MAX as u64)
                && query.since.is_none_or(|v| v <= i64::MAX as u64),
            "Invalid usage cursor/time"
        );
        if let Some(model) = &query.model {
            ensure!(
                !model.trim().is_empty() && model.len() <= 512,
                "Invalid usage model filter"
            );
        }
        let mut db = self.connect()?;
        let tx = db.transaction()?;
        let mut predicates = vec!["1"];
        let mut values = Vec::<Value>::new();
        for (predicate, value) in [
            ("project_id=?", query.project_id.map(|i| i.to_string())),
            ("task_id=?", query.task_id.map(|i| i.to_string())),
            ("agent_id=?", query.agent_id.map(|i| i.to_string())),
            ("provider=?", query.provider.map(tag).transpose()?),
            ("environment=?", query.environment.map(tag).transpose()?),
            ("kind=?", query.kind.map(tag).transpose()?),
            ("model=?", query.model),
        ] {
            if let Some(value) = value {
                predicates.push(predicate);
                values.push(value.into());
            }
        }
        if let Some(since) = query.since {
            predicates.push("COALESCE(started_at,observed_at)>=?");
            values.push((since as i64).into());
        }
        let filter = predicates.join(" AND ");
        let mut summary=tx.query_row(&format!("SELECT count(*),COALESCE(sum(kind IN ('modelCall','soloModelCall')),0),COALESCE(sum(input_tokens),0),COALESCE(sum(output_tokens),0),COALESCE(sum(reserved_tokens),0),COALESCE(sum(kind IN ('modelCall','soloModelCall') AND (input_tokens IS NULL OR output_tokens IS NULL)),0),COALESCE(sum(cost IS NULL),0),COALESCE(sum(elapsed_ms),0) FROM ion_usage_ledger WHERE {filter}"),rusqlite::params_from_iter(values.iter()),|r|Ok(UsageLedgerSummary {records:r.get(0)?,model_calls:r.get(1)?,input_tokens:r.get(2)?,output_tokens:r.get(3)?,reserved_tokens:r.get(4)?,unknown_calls:r.get(5)?,unknown_cost_records:r.get(6)?,elapsed_milliseconds:r.get(7)?,costs:vec![]}))?;
        let mut costs=tx.prepare(&format!("SELECT json_extract(cost,'$.amount.currency'),sum(json_extract(cost,'$.amount.micros')),json_extract(cost,'$.quality'),json_extract(cost,'$.finality') FROM ion_usage_ledger WHERE {filter} AND cost IS NOT NULL GROUP BY json_extract(cost,'$.amount.currency'),json_extract(cost,'$.quality'),json_extract(cost,'$.finality') ORDER BY 1,3,4 LIMIT 257"))?;
        let amounts = costs
            .query_map(rusqlite::params_from_iter(values.iter()), |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, u64>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        ensure!(
            amounts.len() <= 256,
            "Usage currency groups exceed their bound"
        );
        for (currency, micros, quality, finality) in amounts {
            summary.costs.push(UsageCostTotal {
                amount: UsageMoney { currency, micros },
                quality: serde_json::from_value(quality.into())?,
                finality: serde_json::from_value(finality.into())?,
            });
        }
        drop(costs);
        if let Some(after) = query.after {
            predicates.push("seq<?");
            values.push((after as i64).into());
        }
        values.push((i64::from(query.limit) + 1).into());
        let mut rows=tx.prepare(&format!("SELECT json_object('sequence',seq,'kind',kind,'sourceId',source_id,'projectId',project_id,'taskId',task_id,'runId',run_id,'assignmentId',work_id,'attemptId',attempt_id,'agentId',agent_id,'agentName',agent_name,'provider',provider,'model',model,'environment',environment,'host',host,'observedAt',observed_at,'startedAt',started_at,'updatedAt',updated_at,'status',status,'inputTokens',input_tokens,'outputTokens',output_tokens,'reservedTokens',reserved_tokens,'elapsedMilliseconds',elapsed_ms,'cost',json(cost),'spendingAllowance',json(allowance)) FROM ion_usage_ledger WHERE {} ORDER BY seq DESC LIMIT ?",predicates.join(" AND ")))?;
        let mut entries = rows
            .query_map(rusqlite::params_from_iter(values.iter()), |r| {
                r.get::<_, String>(0)
            })?
            .map(|r| Ok(serde_json::from_str::<UsageLedgerEntry>(&r?)?))
            .collect::<Result<Vec<_>>>()?;
        let more = entries.len() > usize::from(query.limit);
        entries.truncate(usize::from(query.limit));
        let next_cursor = if more {
            entries.last().map(|e| e.sequence)
        } else {
            None
        };
        drop(rows);
        let revision = tx.query_row("SELECT revision FROM ion_usage_meta WHERE id=1", [], |r| {
            r.get(0)
        })?;
        tx.commit()?;
        Ok(UsageLedgerPage {
            revision,
            entries,
            next_cursor,
            summary,
        })
    }
}
/// Trusted adapter only: exact replay is harmless; a changed cost cannot rewrite history.
pub(crate) fn settle_cost(
    db: &Connection,
    kind: UsageLedgerKind,
    id: Uuid,
    cost: &UsageCost,
    now: u64,
) -> Result<()> {
    ensure!(
        cost.amount.currency.len() == 3
            && cost.amount.currency.bytes().all(|b| b.is_ascii_uppercase())
            && cost.amount.micros <= 1_000_000_000_000
            && !cost.origin.is_empty()
            && cost.origin.len() <= 160,
        "Invalid usage cost receipt"
    );
    let previous: Option<String> = db.query_row(
        "SELECT cost FROM ion_usage_ledger WHERE kind=? AND source_id=?",
        params![tag(kind)?, id.to_string()],
        |r| r.get(0),
    )?;
    if let Some(previous) = previous {
        ensure!(
            serde_json::from_str::<UsageCost>(&previous)? == *cost,
            "Usage cost receipt changed"
        );
        return Ok(());
    }
    db.execute(
        "UPDATE ion_usage_ledger SET cost=?,updated_at=? WHERE kind=? AND source_id=?",
        params![
            serde_json::to_string(cost)?,
            now,
            tag(kind)?,
            id.to_string()
        ],
    )?;
    Ok(())
}

impl crate::control::ControlStore {
    pub(crate) fn begin_solo_usage(
        &self,
        project: Uuid,
        task: Uuid,
        provider: waku_protocol::model::ProviderKind,
        model: &str,
        now: u64,
    ) -> Result<Uuid> {
        ensure!(
            !model.trim().is_empty() && model.len() <= 512,
            "Invalid usage model"
        );
        let db = self.connect()?;
        let id = Uuid::new_v4();
        db.execute("INSERT INTO ion_usage_ledger(kind,source_id,project_id,task_id,provider,model,environment,host,started_at,updated_at,status) VALUES('soloModelCall',?,?,?, ?,?,'local','local',?,?,'unknown')",params![id.to_string(),project.to_string(),task.to_string(),tag(provider)?,model,now,now])?;
        Ok(id)
    }
    pub(crate) fn settle_solo_usage(
        &self,
        id: Uuid,
        usage: crate::teams::ModelTokenUsage,
        now: u64,
    ) -> Result<()> {
        ensure!(
            usage.input_tokens <= 1_000_000_000_000 && usage.output_tokens <= 1_000_000_000_000,
            "Invalid provider usage"
        );
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let (input,output):(Option<u64>,Option<u64>)=tx.query_row("SELECT input_tokens,output_tokens FROM ion_usage_ledger WHERE kind='soloModelCall' AND source_id=?",[id.to_string()],|r|Ok((r.get(0)?,r.get(1)?)))?;
        if let (Some(input), Some(output)) = (input, output) {
            ensure!(
                input == usage.input_tokens && output == usage.output_tokens,
                "Provider usage changed"
            );
            return Ok(());
        }
        tx.execute("UPDATE ion_usage_ledger SET input_tokens=?,output_tokens=?,status='reported',updated_at=? WHERE kind='soloModelCall' AND source_id=?",params![usage.input_tokens,usage.output_tokens,now,id.to_string()])?;
        tx.commit()?;
        Ok(())
    }
}

/// Native CLI receipts have stable, hashed identities. No session/transcript bytes
/// enter the ledger, and an observed receipt cannot be attributed to a fork twice.
pub(crate) struct NativeSoloReceipt {
    pub source_id: Uuid,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub provider: waku_protocol::model::ProviderKind,
    pub model: Option<String>,
    pub usage: crate::teams::ModelTokenUsage,
}
impl crate::control::ControlStore {
    pub(crate) fn record_native_solo_usage(
        &self,
        receipt: &NativeSoloReceipt,
        now: u64,
    ) -> Result<()> {
        use rusqlite::OptionalExtension;
        use waku_protocol::model::ProviderKind;
        ensure!(
            matches!(
                receipt.provider,
                ProviderKind::Claude | ProviderKind::Codex | ProviderKind::OpenCode
            ),
            "Unsupported native usage source"
        );
        ensure!(
            receipt
                .model
                .as_ref()
                .is_none_or(|model| !model.trim().is_empty()
                    && model.len() <= 512
                    && !model.chars().any(char::is_control)),
            "Invalid usage model"
        );
        ensure!(
            receipt.usage.input_tokens <= 1_000_000_000_000
                && receipt.usage.output_tokens <= 1_000_000_000_000,
            "Invalid native usage receipt"
        );
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let existing: Option<(String, Option<String>, u64, u64)> = tx.query_row(
            "SELECT provider,model,input_tokens,output_tokens FROM ion_usage_ledger WHERE kind='soloModelCall' AND source_id=?",
            [receipt.source_id.to_string()], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        ).optional()?;
        let provider = tag(receipt.provider)?;
        if let Some((old_provider, model, input, output)) = existing {
            ensure!(
                old_provider == provider
                    && model == receipt.model
                    && input == receipt.usage.input_tokens
                    && output == receipt.usage.output_tokens,
                "Native usage receipt changed"
            );
            // A resumed/copied native message retains its first owner task and time.
            return Ok(());
        }
        tx.execute("INSERT INTO ion_usage_ledger(kind,source_id,project_id,task_id,provider,model,environment,host,observed_at,updated_at,status,input_tokens,output_tokens) VALUES('soloModelCall',?,?,?,?,?,'local','local',?,?,'reported',?,?)", params![receipt.source_id.to_string(), receipt.project_id.to_string(), receipt.task_id.to_string(), provider, receipt.model, now, now, receipt.usage.input_tokens, receipt.usage.output_tokens])?;
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
pub(crate) fn all_query() -> UsageLedgerQuery {
    UsageLedgerQuery {
        project_id: None,
        task_id: None,
        agent_id: None,
        provider: None,
        environment: None,
        kind: None,
        model: None,
        since: None,
        after: None,
        limit: 32,
    }
}

pub(crate) fn claude_sdk_estimate(micros: u64) -> UsageCost {
    UsageCost {
        amount: UsageMoney {
            currency: "USD".into(),
            micros,
        },
        quality: UsageCostQuality::Estimated,
        finality: UsageCostFinality::Provisional,
        origin: "Claude SDK price table".into(),
    }
}
