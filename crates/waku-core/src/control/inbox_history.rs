//! Append-only owner facts with indexed state and bounded pages. No client can publish events.
use super::*;
use waku_protocol::inbox::*;
pub(crate) fn schema(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS ion_inbox_items (key TEXT PRIMARY KEY,body TEXT NOT NULL); CREATE TABLE IF NOT EXISTS ion_inbox_preferences (id INTEGER PRIMARY KEY CHECK(id=1),revision INTEGER NOT NULL,policy TEXT NOT NULL); INSERT OR IGNORE INTO ion_inbox_preferences VALUES(1,0,'all');
        CREATE TABLE IF NOT EXISTS ion_inbox_history (
          seq INTEGER PRIMARY KEY AUTOINCREMENT,source TEXT NOT NULL,source_id TEXT NOT NULL,fingerprint TEXT NOT NULL,
          project_id TEXT,task_id TEXT,run_id TEXT,kind TEXT NOT NULL,title TEXT NOT NULL,detail TEXT NOT NULL,
          occurred_at INTEGER NOT NULL,needs_action INTEGER NOT NULL,read INTEGER NOT NULL DEFAULT 0,
          acknowledged INTEGER NOT NULL DEFAULT 0,archived INTEGER NOT NULL DEFAULT 0,UNIQUE(source,source_id,fingerprint));
        CREATE INDEX IF NOT EXISTS ion_inbox_project_history ON ion_inbox_history(project_id,seq DESC);
        CREATE INDEX IF NOT EXISTS ion_inbox_task_history ON ion_inbox_history(task_id,seq DESC);
        CREATE INDEX IF NOT EXISTS ion_inbox_kind_history ON ion_inbox_history(kind,seq DESC);
        CREATE INDEX IF NOT EXISTS ion_inbox_unread ON ion_inbox_history(archived,read);
        CREATE INDEX IF NOT EXISTS ion_inbox_pending ON ion_inbox_history(needs_action);
        CREATE INDEX IF NOT EXISTS ion_inbox_source_history ON ion_inbox_history(source,source_id,seq DESC);
        CREATE TABLE IF NOT EXISTS ion_inbox_history_install (name TEXT PRIMARY KEY);
        CREATE TABLE IF NOT EXISTS ion_inbox_task_cursor (id INTEGER PRIMARY KEY CHECK(id=1),cursor INTEGER NOT NULL);
        INSERT OR IGNORE INTO ion_inbox_task_cursor VALUES(1,0);
        CREATE TRIGGER IF NOT EXISTS ion_inbox_supersede AFTER INSERT ON ion_inbox_history BEGIN
          UPDATE ion_inbox_history SET needs_action=0 WHERE source=NEW.source AND source_id=NEW.source_id AND seq<NEW.seq AND needs_action=1;
        END;")?;
    Ok(())
}
/// SQL expressions and names below are fixed in source, never supplied by a client/model.
#[allow(clippy::too_many_arguments)]
fn install(
    db: &Connection,
    name: &str,
    table: &str,
    source: &str,
    filter: &str,
    changed: &str,
    fingerprint: &str,
    project: &str,
    task: &str,
    run: &str,
    kind: &str,
    title: &str,
    detail: &str,
    time: &str,
    pending: &str,
) -> Result<()> {
    let select = format!(
        "SELECT '{source}',n.id,{fingerprint},{project},{task},{run},{kind},substr({title},1,512),substr({detail},1,8192),{time},{pending} FROM {table} n WHERE {filter}"
    );
    let insert = format!(
        "INSERT OR IGNORE INTO ion_inbox_history(source,source_id,fingerprint,project_id,task_id,run_id,kind,title,detail,occurred_at,needs_action) {select}"
    );
    let event_insert = insert.replace(
        &format!(",{time},{pending} FROM"),
        &format!(",CAST(strftime('%s','now') AS INTEGER),{pending} FROM"),
    );
    db.execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS ion_inbox_{name}_insert AFTER INSERT ON {table} BEGIN {event_insert} AND n.id=NEW.id; END;
        CREATE TRIGGER IF NOT EXISTS ion_inbox_{name}_update AFTER UPDATE ON {table} WHEN {changed} BEGIN UPDATE ion_inbox_history SET needs_action=0 WHERE source='{source}' AND source_id=NEW.id AND NOT EXISTS(SELECT 1 FROM {table} n WHERE n.id=NEW.id AND {filter}); {event_insert} AND n.id=NEW.id; END;
        CREATE TRIGGER IF NOT EXISTS ion_inbox_{name}_delete AFTER DELETE ON {table} BEGIN UPDATE ion_inbox_history SET needs_action=0 WHERE source='{source}' AND source_id=OLD.id; END;"))?;
    // Backfill once: durable existing facts are observed, not reconstructed transitions.
    db.execute_batch("SAVEPOINT inbox_install")?;
    if db.execute(
        "INSERT OR IGNORE INTO ion_inbox_history_install VALUES(?)",
        [name],
    )? == 1
    {
        db.execute_batch(&insert)?;
    }
    db.execute_batch("RELEASE inbox_install")?;
    Ok(())
}
pub(crate) fn install_control(db: &Connection) -> Result<()> {
    schema(db)?;
    install(
        db,
        "actions",
        "ion_control_records",
        "workspaceAction",
        "n.kind='action'",
        "OLD.body IS NOT NEW.body",
        "json_array(json_extract(n.body,'$.status'),json_extract(n.body,'$.title'),json_extract(n.body,'$.detail'))",
        "json_extract(n.body,'$.projectId')",
        "json_extract(n.body,'$.taskId')",
        "NULL",
        "CASE json_extract(n.body,'$.actionKind') WHEN 'proposedTask' THEN 'proposal' ELSE 'decision' END",
        "json_extract(n.body,'$.title')",
        "json_extract(n.body,'$.status')||' · '||json_extract(n.body,'$.detail')",
        "COALESCE(json_extract(n.body,'$.createdAt'),0)",
        "json_extract(n.body,'$.status')='pending'",
    )?;
    install(
        db,
        "artifacts",
        "ion_control_records",
        "artifact",
        "n.kind='artifact'",
        "OLD.body IS NOT NEW.body",
        "n.id",
        "json_extract(n.body,'$.projectId')",
        "json_extract(n.body,'$.taskId')",
        "json_extract(n.body,'$.evidence.producer.runId')",
        "'artifact'",
        "json_extract(n.body,'$.name')",
        "'Generated '||json_extract(n.body,'$.format')||' document'",
        "COALESCE(json_extract(n.body,'$.createdAt'),0)",
        "0",
    )?;
    Ok(())
}
pub(crate) fn install_teams(db: &Connection) -> Result<()> {
    schema(db)?;
    install(
        db,
        "decisions",
        "ion_team_decisions",
        "teamDecision",
        "1",
        "OLD.status IS NOT NEW.status OR OLD.revision IS NOT NEW.revision",
        "json_array(n.status,n.revision)",
        "json_extract(n.body,'$.projectId')",
        "json_extract(n.body,'$.taskId')",
        "n.run_id",
        "'decision'",
        "json_extract(n.body,'$.title')",
        "n.status||' · '||json_extract(n.body,'$.proposal')",
        "COALESCE(json_extract(n.body,'$.createdAt'),0)",
        "n.status='pending'",
    )?;
    install(
        db,
        "assignments",
        "ion_team_work",
        "teamAssignment",
        "n.status IN ('waiting','inReview','blocked','failed','uncertain','completed','canceled')",
        "OLD.status IS NOT NEW.status",
        "json_array(n.status,(SELECT revision FROM ion_team_runs WHERE id=n.run_id),json_extract(n.body,'$.attempts[#-1].id'),json_extract(n.body,'$.attempts[#-1].generation'))",
        "(SELECT project_id FROM ion_team_runs WHERE id=n.run_id)",
        "n.task_id",
        "n.run_id",
        "CASE WHEN n.status='completed' THEN 'completed' WHEN n.status IN ('failed','blocked','uncertain') THEN 'issue' WHEN n.status IN ('waiting','inReview') THEN 'approval' ELSE 'update' END",
        "CASE WHEN n.status='completed' THEN 'Agent work completed' WHEN n.status='inReview' THEN 'Agent work ready for review' ELSE 'Agent work '||n.status END",
        "n.status||' · '||COALESCE(json_extract(n.body,'$.reason'),json_extract(n.body,'$.objective'),'')",
        "CAST(strftime('%s','now') AS INTEGER)",
        "n.status IN ('waiting','inReview','blocked','failed','uncertain')",
    )?;
    install(
        db,
        "automations",
        "ion_automation_runs",
        "automation",
        "1",
        "OLD.status IS NOT NEW.status OR json_extract(OLD.body,'$.currentStage') IS NOT json_extract(NEW.body,'$.currentStage') OR json_extract(OLD.body,'$.reason') IS NOT json_extract(NEW.body,'$.reason')",
        "json_array(n.status,n.revision,json_extract(n.body,'$.currentStage'),json_extract(n.body,'$.reason'))",
        "n.project_id",
        "json_extract(n.body,'$.stages['||json_extract(n.body,'$.currentStage')||'].taskId')",
        "n.id",
        "CASE json_extract(n.body,'$.status') WHEN 'waitingDecision' THEN 'decision' WHEN 'blocked' THEN 'issue' WHEN 'completed' THEN 'completed' ELSE 'update' END",
        "json_extract(n.body,'$.name')",
        "json_extract(n.body,'$.status')||' · '||COALESCE(json_extract(n.body,'$.reason'),'Stage '||(json_extract(n.body,'$.currentStage')+1))",
        "COALESCE(json_extract(n.body,'$.createdAt'),0)",
        "json_extract(n.body,'$.status') IN ('waitingDecision','blocked')",
    )?;
    Ok(())
}
pub(crate) fn history_key(seq: u64) -> String {
    format!("ion:v2{seq:016x}")
}
pub(crate) fn history_sequence(key: &str) -> Option<u64> {
    (key.len() == 22 && key.starts_with("ion:v2"))
        .then(|| u64::from_str_radix(&key[6..], 16).ok())
        .flatten()
        .filter(|s| *s > 0 && *s <= i64::MAX as u64)
}
impl ControlStore {
    pub fn inbox_history(&self, query: InboxQuery) -> Result<InboxPage> {
        ensure!(
            (1..=64).contains(&query.limit),
            "Inbox page size must be 1–64"
        );
        ensure!(
            query.after.is_none_or(|a| a > 0 && a <= i64::MAX as u64),
            "Invalid Inbox cursor"
        );
        let mut db = self.connect()?;
        let tx = db.transaction()?;
        // Only fixed predicates are composed; every client value is bound. This
        // lets project/task/type queries seek their indexes instead of OR scans.
        let mut clauses = vec!["1"];
        let mut values = Vec::<rusqlite::types::Value>::new();
        for (column, value) in [
            ("project_id=?", query.project_id.map(|i| i.to_string())),
            ("task_id=?", query.task_id.map(|i| i.to_string())),
            (
                "kind=?",
                query.kind.map(|k| {
                    serde_json::to_value(k)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_owned()
                }),
            ),
        ] {
            if let Some(value) = value {
                clauses.push(column);
                values.push(value.into());
            }
        }
        match query.view {
            InboxHistoryView::Inbox => clauses.push("archived=0"),
            InboxHistoryView::Unread => clauses.push("archived=0 AND read=0"),
            InboxHistoryView::Acknowledged => clauses.push("archived=0 AND acknowledged=1"),
            InboxHistoryView::Archived => clauses.push("archived=1"),
            InboxHistoryView::All => {}
        }
        match query.status {
            InboxHistoryStatus::NeedsAction => clauses.push("needs_action=1"),
            InboxHistoryStatus::Resolved => clauses.push("needs_action=0"),
            InboxHistoryStatus::All => {}
        }
        let matching_count = tx.query_row(
            &format!(
                "SELECT count(*) FROM ion_inbox_history WHERE {}",
                clauses.join(" AND ")
            ),
            rusqlite::params_from_iter(values.iter()),
            |r| r.get(0),
        )?;
        if let Some(after) = query.after {
            clauses.push("seq<?");
            values.push((after as i64).into());
        }
        values.push((i64::from(query.limit) + 1).into());
        let mut q=tx.prepare(&format!("SELECT seq,source,source_id,project_id,task_id,run_id,kind,title,detail,occurred_at,needs_action,read,acknowledged,archived FROM ion_inbox_history WHERE {} ORDER BY seq DESC LIMIT ?",clauses.join(" AND ")))?;
        let rows = q
            .query_map(rusqlite::params_from_iter(values.iter()), |r| {
                Ok((
                    r.get::<_, u64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, String>(7)?,
                    r.get::<_, String>(8)?,
                    r.get::<_, u64>(9)?,
                    r.get::<_, bool>(10)?,
                    r.get::<_, bool>(11)?,
                    r.get::<_, bool>(12)?,
                    r.get::<_, bool>(13)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut entries = Vec::new();
        for (
            sequence,
            source,
            id,
            p,
            t,
            run,
            kind,
            title,
            detail,
            occurred_at,
            needs_action,
            read,
            acknowledged,
            archived,
        ) in rows
        {
            entries.push(InboxEntry {
                sequence,
                source: serde_json::from_value(source.into())?,
                source_id: Uuid::parse_str(&id)?,
                project_id: p.map(|i| Uuid::parse_str(&i)).transpose()?,
                task_id: t.map(|i| Uuid::parse_str(&i)).transpose()?,
                run_id: run.map(|i| Uuid::parse_str(&i)).transpose()?,
                kind: serde_json::from_value(kind.into())?,
                title,
                detail,
                occurred_at,
                needs_action,
                state: InboxItemState {
                    key: history_key(sequence),
                    read,
                    acknowledged,
                    archived,
                },
            });
        }
        let more = entries.len() > usize::from(query.limit);
        entries.truncate(usize::from(query.limit));
        let next_cursor = if more {
            entries.last().map(|e| e.sequence)
        } else {
            None
        };
        let (revision, policy): (u64, String) = tx.query_row(
            "SELECT revision,policy FROM ion_inbox_preferences WHERE id=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let (history_revision,unread_count,needs_action_count)=tx.query_row("SELECT (SELECT COALESCE(MAX(seq),0) FROM ion_inbox_history),(SELECT count(*) FROM ion_inbox_history WHERE archived=0 AND read=0),(SELECT count(*) FROM ion_inbox_history WHERE needs_action=1)",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
        drop(q);
        tx.commit()?;
        Ok(InboxPage {
            revision,
            history_revision,
            notification_policy: serde_json::from_value(policy.into())?,
            entries,
            next_cursor,
            matching_count,
            unread_count,
            needs_action_count,
            task_history_syncing: false,
        })
    }
    pub(crate) fn update_history_item(
        &self,
        expected: u64,
        sequence: u64,
        action: InboxAction,
    ) -> Result<InboxSnapshot> {
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let mut state = tx
            .query_row(
                "SELECT read,acknowledged,archived FROM ion_inbox_history WHERE seq=?",
                [sequence],
                |r| {
                    Ok(InboxItemState {
                        key: history_key(sequence),
                        read: r.get(0)?,
                        acknowledged: r.get(1)?,
                        archived: r.get(2)?,
                    })
                },
            )
            .context("Inbox notification no longer exists")?;
        let old = state.clone();
        state.apply(action);
        let revision: u64 = tx.query_row(
            "SELECT revision FROM ion_inbox_preferences WHERE id=1",
            [],
            |r| r.get(0),
        )?;
        if state != old {
            ensure!(
                revision == expected,
                "Inbox changed on another client. Refresh before updating."
            );
            tx.execute(
                "UPDATE ion_inbox_history SET read=?,acknowledged=?,archived=? WHERE seq=?",
                params![state.read, state.acknowledged, state.archived, sequence],
            )?;
            tx.execute(
                "UPDATE ion_inbox_preferences SET revision=revision+1 WHERE id=1",
                [],
            )?;
        }
        let snapshot = super::inbox::read(&tx)?;
        tx.commit()?;
        Ok(snapshot)
    }
}

/// Durable lightweight task outbox. Triggers run only on lifecycle/status changes,
/// never per text delta; payloads exclude transcript, permissions and credentials.
pub(crate) fn install_task_outbox(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS ion_task_inbox_outbox(seq INTEGER PRIMARY KEY AUTOINCREMENT,body TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS ion_task_inbox_install(id INTEGER PRIMARY KEY CHECK(id=1));
        CREATE TRIGGER IF NOT EXISTS ion_task_inbox_insert AFTER INSERT ON sessions WHEN NEW.status!='idle' OR NEW.last_reply_at>0 BEGIN
          INSERT INTO ion_task_inbox_outbox(body) VALUES(json_object('id',NEW.id,'projectId',NEW.project_id,'title',substr(COALESCE(NEW.auto_title,NEW.title),1,512),'status',NEW.status,'at',NEW.updated_at,'lastReplyAt',NEW.last_reply_at));
        END;
        CREATE TRIGGER IF NOT EXISTS ion_task_inbox_update AFTER UPDATE ON sessions WHEN OLD.status IS NOT NEW.status OR OLD.last_reply_at IS NOT NEW.last_reply_at BEGIN
          INSERT INTO ion_task_inbox_outbox(body) VALUES(json_object('id',NEW.id,'projectId',NEW.project_id,'title',substr(COALESCE(NEW.auto_title,NEW.title),1,512),'status',NEW.status,'at',NEW.updated_at,'lastReplyAt',NEW.last_reply_at));
        END;
        CREATE TRIGGER IF NOT EXISTS ion_task_inbox_delete AFTER DELETE ON sessions BEGIN
          INSERT INTO ion_task_inbox_outbox(body) VALUES(json_object('id',OLD.id,'projectId',OLD.project_id,'title',substr(COALESCE(OLD.auto_title,OLD.title),1,512),'status','removed','at',CAST(strftime('%s','now') AS INTEGER)));
        END;
        SAVEPOINT task_inbox_install;")?;
    if db.execute("INSERT OR IGNORE INTO ion_task_inbox_install VALUES(1)", [])? == 1 {
        db.execute_batch("INSERT INTO ion_task_inbox_outbox(body) SELECT json_object('id',id,'projectId',project_id,'title',substr(COALESCE(auto_title,title),1,512),'status',status,'at',updated_at,'lastReplyAt',last_reply_at) FROM sessions WHERE status!='idle' OR last_reply_at>0 ORDER BY updated_at,id;")?;
    }
    db.execute_batch("RELEASE task_inbox_install")?;
    Ok(())
}
#[derive(serde::Deserialize)]
struct TaskFact {
    id: Uuid,
    #[serde(rename = "projectId")]
    project_id: Uuid,
    title: String,
    status: String,
    at: u64,
    #[serde(default, rename = "lastReplyAt")]
    last_reply_at: Option<u64>,
}
impl ControlStore {
    pub(crate) fn sync_task_inbox(&self, tasks: &crate::persistence::StateStore) -> Result<bool> {
        let mut db = self.connect()?;
        let cursor: u64 = db.query_row(
            "SELECT cursor FROM ion_inbox_task_cursor WHERE id=1",
            [],
            |r| r.get(0),
        )?;
        let mut rows = tasks.task_inbox_rows(cursor)?;
        let syncing = rows.len() > 512;
        rows.truncate(512);
        if rows.is_empty() {
            return Ok(false);
        }
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let current: u64 = tx.query_row(
            "SELECT cursor FROM ion_inbox_task_cursor WHERE id=1",
            [],
            |r| r.get(0),
        )?;
        let mut next = current;
        for (sequence, body) in rows.into_iter().filter(|(s, _)| *s > current) {
            let fact: TaskFact = serde_json::from_str(&body)?;
            let (kind, pending) = match fact.status.as_str() {
                "waiting" => ("approval", true),
                "failed" => ("issue", true),
                "idle" if fact.last_reply_at.is_some_and(|t| t > 0) => ("completed", false),
                _ => ("update", false),
            };
            tx.execute("INSERT OR IGNORE INTO ion_inbox_history(source,source_id,fingerprint,project_id,task_id,run_id,kind,title,detail,occurred_at,needs_action) VALUES('task',?,?,?, ?,NULL,?,?,?,?,?)",params![fact.id.to_string(),format!("task:{sequence}"),fact.project_id.to_string(),fact.id.to_string(),kind,fact.title,format!("Task {}",fact.status),fact.at,pending])?;
            next = sequence;
        }
        tx.execute(
            "UPDATE ion_inbox_task_cursor SET cursor=? WHERE id=1",
            [next],
        )?;
        tx.commit()?;
        Ok(syncing)
    }
}

#[cfg(test)]
#[path = "inbox_history_tests.rs"]
mod tests;
