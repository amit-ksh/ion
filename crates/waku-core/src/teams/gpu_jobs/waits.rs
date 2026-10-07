//! Durable GPU waits free the agent slot, retaining the exact request, host and money.
use super::super::continuation::{checkpoint_row, validate_checkpoint, write_checkpoint};
use super::*;
pub(in crate::teams) fn waiting(db: &Connection, work: Uuid) -> Result<Option<Uuid>> {
    let id: Option<String> = db
        .query_row(
            "SELECT request_id FROM ion_team_gpu_waits WHERE work_id=?",
            [work.to_string()],
            |r| r.get(0),
        )
        .optional()?;
    id.map(|s| Ok(Uuid::parse_str(&s)?)).transpose()
}
pub(in crate::teams) fn handoff(db: &Connection, work: &TeamAssignment) -> Result<Option<String>> {
    let Some(request) = waiting(db, work.id)? else {
        return Ok(None);
    };
    let attempt = work.attempts.last().context("GPU wait attempt missing")?;
    let (state, receipt, artifact): (String,Option<String>,Option<String>) = db.query_row(
        "SELECT j.state,g.receipt,g.artifact_id FROM ion_team_gpu_jobs j LEFT JOIN ion_team_gpu_results g ON g.request_id=j.request_id WHERE j.request_id=? AND j.work_id=? AND j.attempt_id=?",
        params![request.to_string(),work.id.to_string(),attempt.id.to_string()],|r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
    ensure!(
        matches!(state.as_str(), "settled" | "cleaned"),
        "GPU command result/cleanup is not settled"
    );
    let result: jobs::JobResponse =
        serde_json::from_str(&receipt.context("GPU wait result missing")?)?;
    let artifact = Uuid::parse_str(&artifact.context("GPU wait artifact missing")?)?;
    let body = result.tool_reply(artifact)?;
    ensure!(body.len() <= 16 * 1024, "GPU handoff exceeds its bound");
    Ok(Some(body))
}
impl TeamCoordinator {
    pub(crate) fn authorize_gpu_wait(
        &self,
        lease: &AssignmentLease,
        request: Uuid,
        now: u64,
    ) -> Result<()> {
        let db = self.connect()?;
        let run = load(&db, lease.run_id)?;
        active(&run, now)?;
        let work = checked_lease(&run, lease, now, false)?;
        ensure!(
            work.access.shell
                && work.access.workspace_read
                && work.access.network
                && work
                    .attempts
                    .last()
                    .is_some_and(|a| a.environment == ExecutionEnvironment::NebiusSandbox),
            "GPU wait requires its original Nebius command/read/network grants"
        );
        let (state,job,operation,remote,policy):(String,Option<String>,Option<String>,Option<String>,String)=db.query_row(
            "SELECT state,job_id,creation_operation,remote_state,policy FROM ion_team_gpu_jobs WHERE request_id=? AND run_id=? AND work_id=? AND attempt_id=?",
            params![request.to_string(),run.id.to_string(),work.id.to_string(),lease.attempt_id.to_string()],|r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))
            .context("GPU request does not belong to this assignment attempt")?;
        ensure!(
            matches!(state.as_str(), "running" | "terminal" | "settled")
                && job.is_some()
                && operation.is_some()
                && remote.is_some(),
            "Observe the original bound GPU job before waiting; unknown acknowledgements cannot be parked"
        );
        ensure!(
            serde_json::from_str::<GpuBudgetPolicy>(&policy)?
                .result_volume
                .is_some(),
            "GPU wait needs its originally approved result volume"
        );
        workspace::require_settled_effects_except_gpu(&db, run.id, Some(request))
    }
    pub(crate) fn suspend_for_gpu(
        &self,
        lease: &AssignmentLease,
        request: Uuid,
        checkpoint: &str,
        now: u64,
    ) -> Result<TeamRunSnapshot> {
        validate_checkpoint(checkpoint)?;
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = load(&tx, lease.run_id)?;
        active(&run, now)?;
        let mut work = run
            .work
            .iter()
            .find(|w| w.id == lease.work_id)
            .context("GPU wait assignment missing")?
            .clone();
        if work.status == AssignmentStatus::Waiting {
            let saved = checkpoint_row(&tx, work.id)?.context("GPU checkpoint missing")?;
            ensure!(
                waiting(&tx, work.id)? == Some(request)
                    && saved.0 == lease.attempt_id.to_string()
                    && saved.1 == lease.generation
                    && saved.2 == "[]"
                    && saved.3 == checkpoint
                    && work
                        .attempts
                        .last()
                        .is_some_and(|a| a.id == lease.attempt_id
                            && lease.generation.checked_add(1) == Some(a.generation)),
                "GPU suspension replay or generation changed"
            );
            return Ok(run);
        }
        checked_lease(&run, lease, now, false)?;
        // Authorize using this transaction as well; no lease can be revoked between checks.
        ensure!(
            work.access.shell
                && work.access.workspace_read
                && work.access.network
                && work
                    .attempts
                    .last()
                    .is_some_and(|a| a.environment == ExecutionEnvironment::NebiusSandbox),
            "GPU wait grants/host changed"
        );
        let (state,job,operation,remote,policy):(String,Option<String>,Option<String>,Option<String>,String)=tx.query_row(
            "SELECT state,job_id,creation_operation,remote_state,policy FROM ion_team_gpu_jobs WHERE request_id=? AND run_id=? AND work_id=? AND attempt_id=?",
            params![request.to_string(),run.id.to_string(),work.id.to_string(),lease.attempt_id.to_string()],|r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))?;
        ensure!(
            matches!(state.as_str(), "running" | "terminal" | "settled")
                && job.is_some()
                && operation.is_some()
                && remote.is_some()
                && serde_json::from_str::<GpuBudgetPolicy>(&policy)?
                    .result_volume
                    .is_some(),
            "GPU original observation/result policy missing"
        );
        workspace::require_settled_effects_except_gpu(&tx, run.id, Some(request))?;
        ensure!(
            super::super::decisions::waiting_decision(&tx, work.id)?.is_none(),
            "GPU wait cannot replace a decision"
        );
        let attempt = work.attempts.last_mut().unwrap();
        attempt.generation = attempt
            .generation
            .checked_add(1)
            .context("GPU generation exhausted")?;
        attempt.status = AssignmentStatus::Waiting;
        work.status = AssignmentStatus::Waiting;
        work.reason = Some("Waiting for GPU command result and resource cleanup".into());
        write_checkpoint(&tx, lease, "[]", checkpoint)?;
        tx.execute(
            "INSERT INTO ion_team_gpu_waits(work_id,request_id) VALUES(?,?)",
            params![work.id.to_string(), request.to_string()],
        )?;
        tx.execute(
            "UPDATE ion_team_attempts SET live=0 WHERE id=?",
            [lease.attempt_id.to_string()],
        )?;
        save_work(&tx, &work, false)?;
        bump(&tx, run.id)?;
        audit(
            &tx,
            run.id,
            Some(work.id),
            "gpu_waiting",
            &format!(
                "GPU request {request} parked with its original attempt; agent slot released and spending retained"
            ),
            now,
        )?;
        tx.commit()?;
        self.snapshot(run.id)
    }
    pub(crate) fn gpu_wait_request(&self, work: Uuid) -> Result<Option<(Uuid, Uuid, Uuid)>> {
        let db = self.connect()?;
        let row: Option<(String,String,String)> = db.query_row(
            "SELECT j.run_id,j.attempt_id,j.request_id FROM ion_team_gpu_waits w JOIN ion_team_gpu_jobs j ON j.request_id=w.request_id WHERE w.work_id=?",
            [work.to_string()],|r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
        row.map(|(r, a, j)| {
            Ok((
                Uuid::parse_str(&r)?,
                Uuid::parse_str(&a)?,
                Uuid::parse_str(&j)?,
            ))
        })
        .transpose()
    }
    /// Runtime settlement is not invoice settlement. Full spending reservations remain.
    pub(crate) fn settle_gpu_result(&self, request: Uuid, now: u64) -> Result<bool> {
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (run, work, state): (String, String, String) = tx.query_row(
            "SELECT run_id,work_id,state FROM ion_team_gpu_jobs WHERE request_id=?",
            [request.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        if matches!(state.as_str(), "settled" | "cleaned") {
            return Ok(true);
        };
        let proven:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM ion_team_gpu_jobs j JOIN ion_team_gpu_cleanup c ON c.request_id=j.request_id JOIN ion_team_gpu_results g ON g.request_id=j.request_id WHERE j.request_id=? AND j.state='terminal' AND c.state='verified' AND j.job_id IS NOT NULL AND j.creation_operation IS NOT NULL)",
            [request.to_string()],|r|r.get(0))?;
        if !proven {
            return Ok(false);
        };
        tx.execute(
            "UPDATE ion_team_gpu_jobs SET state='settled' WHERE request_id=? AND state='terminal'",
            [request.to_string()],
        )?;
        audit(
            &tx,
            Uuid::parse_str(&run)?,
            Some(Uuid::parse_str(&work)?),
            "gpu_runtime_settled",
            &format!(
                "GPU request {request}: actual receipt and verified cleanup available; monetary reservation unchanged"
            ),
            now,
        )?;
        bump(&tx, Uuid::parse_str(&run)?)?;
        tx.commit()?;
        Ok(true)
    }
}
