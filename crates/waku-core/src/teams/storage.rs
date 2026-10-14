//! SQL and serialization are private to the coordinator.
use super::*;

pub(super) fn schema(db: &Connection) -> Result<()> {
    crate::cloud_spending::schema(db)?;
    db.execute_batch("PRAGMA foreign_keys=ON;
        CREATE TABLE IF NOT EXISTS ion_team_runs (
            id TEXT PRIMARY KEY, root_task_id TEXT NOT NULL UNIQUE, project_id TEXT NOT NULL,
            status TEXT NOT NULL, revision INTEGER NOT NULL, request TEXT NOT NULL, body TEXT NOT NULL);
        CREATE INDEX IF NOT EXISTS ion_team_project ON ion_team_runs(project_id,status);
        CREATE TABLE IF NOT EXISTS ion_team_work (
            id TEXT PRIMARY KEY, run_id TEXT NOT NULL REFERENCES ion_team_runs(id),
            task_id TEXT NOT NULL UNIQUE, agent_id TEXT NOT NULL, parent_id TEXT,
            request_id TEXT, request TEXT, status TEXT NOT NULL, retry_safe INTEGER NOT NULL DEFAULT 0,
            body TEXT NOT NULL, UNIQUE(run_id,parent_id,request_id));
        CREATE INDEX IF NOT EXISTS ion_team_queue ON ion_team_work(run_id,status);
        CREATE TABLE IF NOT EXISTS ion_team_dependencies (
            work_id TEXT NOT NULL REFERENCES ion_team_work(id), depends_on TEXT NOT NULL REFERENCES ion_team_work(id),
            PRIMARY KEY(work_id,depends_on));
        CREATE TABLE IF NOT EXISTS ion_team_attempts (
            id TEXT PRIMARY KEY, run_id TEXT NOT NULL REFERENCES ion_team_runs(id),
            work_id TEXT NOT NULL REFERENCES ion_team_work(id), agent_id TEXT NOT NULL,
            live INTEGER NOT NULL, capacity INTEGER NOT NULL, report TEXT);
        CREATE INDEX IF NOT EXISTS ion_team_agent_capacity ON ion_team_attempts(agent_id,live);
        CREATE TABLE IF NOT EXISTS ion_team_outbox (
            task_id TEXT PRIMARY KEY, run_id TEXT NOT NULL REFERENCES ion_team_runs(id),
            work_id TEXT NOT NULL UNIQUE REFERENCES ion_team_work(id), materialized INTEGER NOT NULL);
        CREATE INDEX IF NOT EXISTS ion_team_pending_tasks ON ion_team_outbox(materialized,run_id);
        CREATE TABLE IF NOT EXISTS ion_team_continuations (
            work_id TEXT PRIMARY KEY REFERENCES ion_team_work(id), attempt_id TEXT NOT NULL,
            generation INTEGER NOT NULL, dependencies TEXT NOT NULL, checkpoint TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS ion_team_recovery_points (
            work_id TEXT PRIMARY KEY REFERENCES ion_team_work(id),attempt_id TEXT NOT NULL,
            generation INTEGER NOT NULL);
        CREATE TABLE IF NOT EXISTS ion_team_recovery_waits (
            work_id TEXT PRIMARY KEY REFERENCES ion_team_work(id),attempt_id TEXT NOT NULL,
            requested_revision INTEGER NOT NULL,reason TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS ion_team_document_receipts (
            work_id TEXT NOT NULL REFERENCES ion_team_work(id), request_id TEXT NOT NULL,
            artifact_id TEXT NOT NULL UNIQUE, name TEXT NOT NULL, format TEXT NOT NULL,
            content BLOB NOT NULL, PRIMARY KEY(work_id,request_id));
        CREATE TABLE IF NOT EXISTS ion_team_file_documents (
            artifact_id TEXT PRIMARY KEY REFERENCES ion_team_document_receipts(artifact_id),
            path TEXT NOT NULL,summary TEXT NOT NULL,signature TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS ion_team_pdf_summaries (
            artifact_id TEXT PRIMARY KEY REFERENCES ion_team_document_receipts(artifact_id),
            summary TEXT NOT NULL, signature TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS ion_team_followups (
            run_id TEXT NOT NULL REFERENCES ion_team_runs(id),work_id TEXT NOT NULL REFERENCES ion_team_work(id),
            id TEXT NOT NULL,signature TEXT NOT NULL,status TEXT NOT NULL,body TEXT NOT NULL,
            materialized INTEGER NOT NULL DEFAULT 0,PRIMARY KEY(run_id,id));
        CREATE INDEX IF NOT EXISTS ion_team_followup_pending ON ion_team_followups(run_id,status);
        CREATE TABLE IF NOT EXISTS ion_team_input_files (
            run_id TEXT NOT NULL REFERENCES ion_team_runs(id),key TEXT NOT NULL,name TEXT NOT NULL,
            content TEXT NOT NULL,PRIMARY KEY(run_id,key));
        CREATE TABLE IF NOT EXISTS ion_team_input_presentations (
            run_id TEXT PRIMARY KEY REFERENCES ion_team_runs(id),attachments TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS ion_team_invocations (
            task_id TEXT NOT NULL, request_id TEXT NOT NULL, signature TEXT NOT NULL,
            run_id TEXT NOT NULL UNIQUE REFERENCES ion_team_runs(id), PRIMARY KEY(task_id,request_id));
        CREATE TABLE IF NOT EXISTS ion_team_reviews (
            child_id TEXT PRIMARY KEY REFERENCES ion_team_work(id),
            run_id TEXT NOT NULL REFERENCES ion_team_runs(id), parent_id TEXT NOT NULL,
            request_id TEXT NOT NULL, signature TEXT NOT NULL, body TEXT NOT NULL,
            UNIQUE(run_id,parent_id,request_id));
        CREATE TABLE IF NOT EXISTS ion_team_decisions (
            id TEXT PRIMARY KEY,run_id TEXT NOT NULL REFERENCES ion_team_runs(id),
            work_id TEXT NOT NULL REFERENCES ion_team_work(id),request_id TEXT NOT NULL,
            project_id TEXT NOT NULL,status TEXT NOT NULL,revision INTEGER NOT NULL,
            created_at INTEGER NOT NULL,signature TEXT NOT NULL,body TEXT NOT NULL,
            UNIQUE(work_id,request_id));
        CREATE INDEX IF NOT EXISTS ion_team_decision_run ON ion_team_decisions(run_id,work_id,status);
        CREATE INDEX IF NOT EXISTS ion_team_decision_pending ON ion_team_decisions(status,project_id,created_at DESC,id DESC);
        CREATE INDEX IF NOT EXISTS ion_team_decision_recent ON ion_team_decisions(created_at DESC,id DESC);
        CREATE INDEX IF NOT EXISTS ion_team_decision_project ON ion_team_decisions(project_id,created_at DESC,id DESC);
        CREATE TABLE IF NOT EXISTS ion_team_decision_waits (
            work_id TEXT PRIMARY KEY REFERENCES ion_team_work(id),
            decision_id TEXT NOT NULL UNIQUE REFERENCES ion_team_decisions(id));
        CREATE TABLE IF NOT EXISTS ion_team_app_effects (
            decision_id TEXT PRIMARY KEY REFERENCES ion_team_decisions(id),
            run_id TEXT NOT NULL REFERENCES ion_team_runs(id),
            work_id TEXT NOT NULL REFERENCES ion_team_work(id),attempt_id TEXT NOT NULL,
            signature TEXT NOT NULL,state TEXT NOT NULL,receipt TEXT);
        CREATE INDEX IF NOT EXISTS ion_team_app_pending ON ion_team_app_effects(run_id,state);
        CREATE TABLE IF NOT EXISTS ion_team_jobs (
            request_id TEXT PRIMARY KEY, run_id TEXT NOT NULL REFERENCES ion_team_runs(id),
            work_id TEXT NOT NULL REFERENCES ion_team_work(id), attempt_id TEXT NOT NULL,
            image TEXT NOT NULL, project TEXT NOT NULL, signature TEXT NOT NULL,
            request TEXT NOT NULL, state TEXT NOT NULL, operation TEXT, receipt TEXT,
            UNIQUE(work_id,request_id));
        CREATE INDEX IF NOT EXISTS ion_team_jobs_attempt ON ion_team_jobs(attempt_id,state);
        CREATE INDEX IF NOT EXISTS ion_team_jobs_run ON ion_team_jobs(run_id,state);
        CREATE TABLE IF NOT EXISTS ion_team_job_projects (
            request_id TEXT PRIMARY KEY REFERENCES ion_team_jobs(request_id),
            archive BLOB NOT NULL, file_uuid TEXT);
        CREATE TABLE IF NOT EXISTS ion_team_job_admission (
            request_id TEXT PRIMARY KEY REFERENCES ion_team_jobs(request_id),
            project_id TEXT NOT NULL, allocation TEXT NOT NULL, policy TEXT,
            compute_released INTEGER NOT NULL DEFAULT 0 CHECK(compute_released IN (0,1)),
            layer_released INTEGER NOT NULL DEFAULT 0 CHECK(layer_released IN (0,1)));
        CREATE INDEX IF NOT EXISTS ion_team_job_admission_project ON ion_team_job_admission(project_id);
        CREATE TABLE IF NOT EXISTS ion_team_gpu_jobs (
            request_id TEXT PRIMARY KEY,run_id TEXT NOT NULL REFERENCES ion_team_runs(id),
            work_id TEXT NOT NULL REFERENCES ion_team_work(id),attempt_id TEXT NOT NULL,
            project TEXT NOT NULL,signature TEXT NOT NULL,body TEXT NOT NULL,policy TEXT NOT NULL,
            reserved_micros INTEGER NOT NULL,currency TEXT NOT NULL,deadline_at INTEGER NOT NULL,
            state TEXT NOT NULL,creation_operation TEXT,job_id TEXT,cancellation_operation TEXT,
            cancel_requested INTEGER NOT NULL DEFAULT 0,cancellation_key TEXT,remote_state TEXT,
            compute_ids TEXT NOT NULL,UNIQUE(work_id,request_id));
        CREATE INDEX IF NOT EXISTS ion_team_gpu_attempt ON ion_team_gpu_jobs(attempt_id,state);
        CREATE INDEX IF NOT EXISTS ion_team_gpu_project ON ion_team_gpu_jobs(project,state);
        CREATE INDEX IF NOT EXISTS ion_team_gpu_run ON ion_team_gpu_jobs(run_id,request_id);
        CREATE UNIQUE INDEX IF NOT EXISTS ion_team_gpu_resource ON ion_team_gpu_jobs(project,job_id) WHERE job_id IS NOT NULL;
        CREATE UNIQUE INDEX IF NOT EXISTS ion_team_gpu_operation ON ion_team_gpu_jobs(project,creation_operation) WHERE creation_operation IS NOT NULL;
        CREATE TABLE IF NOT EXISTS ion_team_gpu_preflight (
            request_id TEXT PRIMARY KEY REFERENCES ion_team_gpu_jobs(request_id),body TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS ion_team_gpu_prices (
            request_id TEXT PRIMARY KEY REFERENCES ion_team_gpu_jobs(request_id),body TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS ion_team_gpu_inventory (
            request_id TEXT NOT NULL REFERENCES ion_team_gpu_jobs(request_id),compute_id TEXT NOT NULL,
            body TEXT NOT NULL,PRIMARY KEY(request_id,compute_id));
        CREATE TABLE IF NOT EXISTS ion_team_gpu_cleanup (
            request_id TEXT PRIMARY KEY REFERENCES ion_team_gpu_jobs(request_id),body TEXT NOT NULL,state TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS ion_team_gpu_results (
            request_id TEXT PRIMARY KEY REFERENCES ion_team_gpu_jobs(request_id),
            receipt TEXT NOT NULL,artifact_id TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS ion_team_gpu_maintenance (
            request_id TEXT PRIMARY KEY REFERENCES ion_team_gpu_jobs(request_id),
            next_poll_at INTEGER NOT NULL DEFAULT 0,busy_until INTEGER NOT NULL DEFAULT 0,
            token TEXT,failures INTEGER NOT NULL DEFAULT 0,done INTEGER NOT NULL DEFAULT 0);
        CREATE INDEX IF NOT EXISTS ion_team_gpu_maintenance_due ON ion_team_gpu_maintenance(done,next_poll_at,busy_until);
        CREATE INDEX IF NOT EXISTS ion_team_gpu_collection ON ion_team_gpu_jobs(state,work_id,request_id);
        CREATE TABLE IF NOT EXISTS ion_team_gpu_waits (
            work_id TEXT PRIMARY KEY REFERENCES ion_team_work(id),
            request_id TEXT NOT NULL UNIQUE REFERENCES ion_team_gpu_jobs(request_id));
        CREATE TABLE IF NOT EXISTS ion_team_workspaces (
            run_id TEXT PRIMARY KEY REFERENCES ion_team_runs(id), state TEXT NOT NULL, body TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS ion_team_workspace_handoffs (
            run_id TEXT PRIMARY KEY REFERENCES ion_team_runs(id),
            previous_run_id TEXT NOT NULL REFERENCES ion_team_runs(id),
            state TEXT NOT NULL,body TEXT);
        CREATE TABLE IF NOT EXISTS ion_team_workspace_edits (
            id TEXT PRIMARY KEY,run_id TEXT NOT NULL REFERENCES ion_team_runs(id),
            work_id TEXT NOT NULL REFERENCES ion_team_work(id),attempt_id TEXT NOT NULL,
            request_id TEXT NOT NULL,signature TEXT NOT NULL,state TEXT NOT NULL,receipt TEXT,
            UNIQUE(work_id,request_id));
        CREATE INDEX IF NOT EXISTS ion_team_edit_pending ON ion_team_workspace_edits(run_id,state);
        CREATE TABLE IF NOT EXISTS ion_team_integrations (
            id TEXT PRIMARY KEY,run_id TEXT NOT NULL UNIQUE REFERENCES ion_team_runs(id),
            repository TEXT NOT NULL,state TEXT NOT NULL,revision INTEGER NOT NULL,body TEXT NOT NULL);
        CREATE INDEX IF NOT EXISTS ion_team_integration_pending ON ion_team_integrations(repository,state);
        CREATE TABLE IF NOT EXISTS ion_team_integration_steps (
            integration_id TEXT NOT NULL REFERENCES ion_team_integrations(id),step INTEGER NOT NULL,
            state TEXT NOT NULL,receipt TEXT,PRIMARY KEY(integration_id,step));
        CREATE TABLE IF NOT EXISTS ion_team_recovery_confirmations (
            run_id TEXT NOT NULL REFERENCES ion_team_runs(id),work_id TEXT NOT NULL REFERENCES ion_team_work(id),
            attempt_id TEXT NOT NULL,requested_revision INTEGER NOT NULL,body TEXT NOT NULL,
            PRIMARY KEY(work_id,requested_revision));
        CREATE TABLE IF NOT EXISTS ion_team_model_calls (
            id TEXT PRIMARY KEY, run_id TEXT NOT NULL REFERENCES ion_team_runs(id),
            work_id TEXT NOT NULL REFERENCES ion_team_work(id), attempt_id TEXT NOT NULL,
            generation INTEGER NOT NULL, step INTEGER NOT NULL, model TEXT NOT NULL,
            started_at INTEGER NOT NULL, UNIQUE(attempt_id,step));
        CREATE INDEX IF NOT EXISTS ion_team_calls_run ON ion_team_model_calls(run_id);
        CREATE TABLE IF NOT EXISTS ion_team_remote_calls (
            call_id TEXT PRIMARY KEY REFERENCES ion_team_model_calls(id),work_id TEXT NOT NULL,
            attempt_id TEXT NOT NULL,image TEXT NOT NULL,project TEXT NOT NULL,
            operation TEXT,state TEXT NOT NULL,receipt TEXT,started_at INTEGER NOT NULL);
        CREATE INDEX IF NOT EXISTS ion_team_remote_attempt ON ion_team_remote_calls(attempt_id,state);
        CREATE UNIQUE INDEX IF NOT EXISTS ion_team_remote_operation ON ion_team_remote_calls(project,operation) WHERE operation IS NOT NULL;
        CREATE TABLE IF NOT EXISTS ion_team_remote_inputs (
            call_id TEXT PRIMARY KEY REFERENCES ion_team_remote_calls(call_id), body TEXT NOT NULL);

        CREATE TABLE IF NOT EXISTS ion_team_call_budgets (
            call_id TEXT PRIMARY KEY REFERENCES ion_team_model_calls(id),
            input_cap INTEGER NOT NULL, output_cap INTEGER NOT NULL,
            input_tokens INTEGER, output_tokens INTEGER);
        CREATE INDEX IF NOT EXISTS ion_team_active_runs ON ion_team_runs(status,id);
        CREATE INDEX IF NOT EXISTS ion_team_collection_recent ON ion_team_runs(json_extract(body,'$.createdAt') DESC,id DESC);
        CREATE INDEX IF NOT EXISTS ion_team_collection_project ON ion_team_runs(project_id,json_extract(body,'$.createdAt') DESC,id DESC);
        CREATE INDEX IF NOT EXISTS ion_team_collection_team ON ion_team_runs(json_extract(body,'$.team.id'),json_extract(body,'$.createdAt') DESC,id DESC);
        CREATE TABLE IF NOT EXISTS ion_team_audit (
            cursor INTEGER PRIMARY KEY AUTOINCREMENT, run_id TEXT NOT NULL REFERENCES ion_team_runs(id),
            work_id TEXT, event TEXT NOT NULL, detail TEXT NOT NULL, timestamp INTEGER NOT NULL);
        CREATE INDEX IF NOT EXISTS ion_team_audit_cursor ON ion_team_audit(run_id,cursor);")?;
    Ok(())
}
pub(super) fn load(db: &Connection, id: Uuid) -> Result<TeamRunSnapshot> {
    let (body, revision, status): (String, u64, String) = db
        .query_row(
            "SELECT body,revision,status FROM ion_team_runs WHERE id=?",
            [id.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .context("Team run not found")?;
    let mut run: TeamRunSnapshot = serde_json::from_str(&body)?;
    run.revision = revision;
    run.status = serde_json::from_value(serde_json::Value::String(status))?;
    run.usage = super::budget::usage(db, id)?;
    run.workspace = super::workspace::projection(db, id)?;
    run.decisions = super::decisions::for_run(db, id)?;
    run.follow_ups = super::followups::for_run(db, id)?;
    let mut query =
        db.prepare("SELECT body FROM ion_team_work WHERE run_id=? ORDER BY rowid LIMIT 257")?;
    run.work = query
        .query_map([id.to_string()], |r| r.get::<_, String>(0))?
        .map(|body| Ok(serde_json::from_str(&body?)?))
        .collect::<Result<_>>()?;
    ensure!(
        run.work.len() <= 256,
        "Team run exceeded its work item limit"
    );
    super::recovery::project(db, &mut run)?;
    Ok(run)
}
pub(super) fn insert_work(
    tx: &Transaction<'_>,
    run_id: Uuid,
    work: &TeamAssignment,
    request_id: Option<Uuid>,
    request: Option<&str>,
    materialized: bool,
) -> Result<()> {
    tx.execute("INSERT INTO ion_team_work(id,run_id,task_id,agent_id,parent_id,request_id,request,status,body) VALUES(?,?,?,?,?,?,?,?,?)",
        params![work.id.to_string(),run_id.to_string(),work.task_id.to_string(),work.agent_id.to_string(),work.parent_id.map(|id|id.to_string()),request_id.map(|id|id.to_string()),request,assignment_status(work.status),bounded_work_body(work)?])?;
    for dependency in &work.dependencies {
        tx.execute(
            "INSERT INTO ion_team_dependencies VALUES(?,?)",
            params![work.id.to_string(), dependency.to_string()],
        )?;
    }
    tx.execute(
        "INSERT INTO ion_team_outbox VALUES(?,?,?,?)",
        params![
            work.task_id.to_string(),
            run_id.to_string(),
            work.id.to_string(),
            materialized
        ],
    )?;
    Ok(())
}
pub(super) fn save_work(
    tx: &Transaction<'_>,
    work: &TeamAssignment,
    retry_safe: bool,
) -> Result<()> {
    tx.execute(
        "UPDATE ion_team_work SET status=?,retry_safe=?,body=? WHERE id=?",
        params![
            assignment_status(work.status),
            retry_safe,
            bounded_work_body(work)?,
            work.id.to_string()
        ],
    )?;
    Ok(())
}
pub(super) fn bump(tx: &Transaction<'_>, run_id: Uuid) -> Result<()> {
    tx.execute(
        "UPDATE ion_team_runs SET revision=revision+1 WHERE id=?",
        [run_id.to_string()],
    )?;
    Ok(())
}
pub(super) fn audit(
    tx: &Transaction<'_>,
    run_id: Uuid,
    work: Option<Uuid>,
    event: &str,
    detail: &str,
    now: u64,
) -> Result<()> {
    tx.execute(
        "INSERT INTO ion_team_audit(run_id,work_id,event,detail,timestamp) VALUES(?,?,?,?,?)",
        params![
            run_id.to_string(),
            work.map(|id| id.to_string()),
            event,
            detail,
            now
        ],
    )?;
    Ok(())
}
pub(super) fn set_run_status(
    tx: &Transaction<'_>,
    run_id: Uuid,
    status: TeamRunStatus,
) -> Result<()> {
    tx.execute(
        "UPDATE ion_team_runs SET status=? WHERE id=?",
        params![run_status(status), run_id.to_string()],
    )?;
    Ok(())
}
pub(super) fn settle_run(tx: &Transaction<'_>, run_id: Uuid) -> Result<()> {
    let run = load(tx, run_id)?;
    if run.status == TeamRunStatus::Stopping && !run.work.iter().any(|w| w.status.holds_capacity())
    {
        set_run_status(tx, run_id, TeamRunStatus::Canceled)?;
    } else if run.status == TeamRunStatus::Active
        && run.work.iter().all(|w| {
            matches!(
                w.status,
                AssignmentStatus::Completed | AssignmentStatus::Superseded
            )
        })
    {
        set_run_status(tx, run_id, TeamRunStatus::Completed)?;
    }
    Ok(())
}
pub(super) fn assignment_status(status: AssignmentStatus) -> &'static str {
    match status {
        AssignmentStatus::Queued => "queued",
        AssignmentStatus::Running => "running",
        AssignmentStatus::Waiting => "waiting",
        AssignmentStatus::Blocked => "blocked",
        AssignmentStatus::Completed => "completed",
        AssignmentStatus::InReview => "inReview",
        AssignmentStatus::Superseded => "superseded",
        AssignmentStatus::Failed => "failed",
        AssignmentStatus::Canceling => "canceling",
        AssignmentStatus::Canceled => "canceled",
        AssignmentStatus::Uncertain => "uncertain",
    }
}
pub(super) fn run_status(status: TeamRunStatus) -> &'static str {
    match status {
        TeamRunStatus::Active => "active",
        TeamRunStatus::Stopping => "stopping",
        TeamRunStatus::Canceled => "canceled",
        TeamRunStatus::Completed => "completed",
    }
}

fn bounded_work_body(work: &TeamAssignment) -> Result<String> {
    let body = serde_json::to_string(work)?;
    ensure!(
        body.len() <= 128 * 1024,
        "Assignment projection exceeds 128 KiB; shorten its instructions or evidence"
    );
    Ok(body)
}
