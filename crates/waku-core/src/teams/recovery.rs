//! Explicit continuation of a confirmed stopped local assignment. The adapter
//! marks safe checkpoints; unknown workers and unfinished effects cannot resume.
use super::*;

pub(super) fn project(db: &Connection, run: &mut TeamRunSnapshot) -> Result<()> {
    let pending: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM ion_team_workspace_edits WHERE run_id=?1 AND state='pending') OR EXISTS(SELECT 1 FROM ion_team_app_effects WHERE run_id=?1 AND state='pending')",
        [run.id.to_string()], |r| r.get(0),
    )?;
    let points: Vec<(String,String,u32)> = db.prepare(
        "SELECT p.work_id,p.attempt_id,p.generation FROM ion_team_recovery_points p JOIN ion_team_work w ON w.id=p.work_id JOIN ion_team_continuations c ON c.work_id=p.work_id AND c.attempt_id=p.attempt_id AND c.generation=p.generation WHERE w.run_id=? AND c.dependencies='[]' LIMIT 257",
    )?.query_map([run.id.to_string()], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?.collect::<rusqlite::Result<_>>()?;
    let points: std::collections::HashMap<_, _> = points
        .into_iter()
        .map(|(work, attempt, generation)| (work, (attempt, generation)))
        .collect();
    for work in &mut run.work {
        work.can_resume = !pending
            && run.status == TeamRunStatus::Active
            && work.status == AssignmentStatus::Blocked
            && work.attempts.last().is_some_and(|a| {
                a.environment == ExecutionEnvironment::Local
                    && a.status == AssignmentStatus::Blocked
                    && points
                        .get(&work.id.to_string())
                        .is_some_and(|(id, g)| id == &a.id.to_string() && *g == a.generation)
            });
    }
    Ok(())
}

pub(super) fn waiting_note(db: &Connection, work: &TeamAssignment) -> Result<Option<String>> {
    let row: Option<(String, String)> = db
        .query_row(
            "SELECT attempt_id,reason FROM ion_team_recovery_waits WHERE work_id=?",
            [work.id.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    row.map(|(attempt, reason)| {
        ensure!(
            work.attempts.last().is_some_and(
                |a| a.id.to_string() == attempt && a.environment == ExecutionEnvironment::Local
            ),
            "Recovery attempt or host changed"
        );
        Ok(reason)
    })
    .transpose()
}

impl TeamCoordinator {
    /// Called by the owner runtime only after checking its local worker has joined.
    /// Queues a continuation, never a fresh attempt or an enlarged authorization.
    pub(crate) fn queue_checkpoint_resume(
        &self,
        run_id: Uuid,
        work_id: Uuid,
        expected_revision: u64,
        now: u64,
    ) -> Result<TeamRunSnapshot> {
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = load(&tx, run_id)?;
        active(&run, now)?;
        let mut work = run
            .work
            .iter()
            .find(|w| w.id == work_id)
            .context("Assignment missing")?
            .clone();
        if work.status == AssignmentStatus::Waiting {
            let revision: Option<u64> = tx
                .query_row(
                    "SELECT requested_revision FROM ion_team_recovery_waits WHERE work_id=?",
                    [work.id.to_string()],
                    |r| r.get(0),
                )
                .optional()?;
            if revision == Some(expected_revision) {
                return Ok(run);
            }
        }
        ensure!(
            run.revision == expected_revision,
            "Run changed; refresh before continuing"
        );
        ensure!(
            work.can_resume,
            "Only a stopped Local assignment with a safe checkpoint can continue; reconcile unknown hosts and effects first"
        );
        workspace::require_settled_effects(&tx, run.id)?;
        let count: u32 = tx.query_row(
            "SELECT count(*) FROM ion_team_audit WHERE run_id=? AND work_id=? AND event='checkpoint_resume_requested'",
            params![run_id.to_string(),work_id.to_string()], |r| r.get(0),
        )?;
        ensure!(
            count < 8,
            "Assignment continuation limit reached; review the blocker before starting another task"
        );
        let (id, generation, dependencies, checkpoint) =
            continuation::checkpoint_row(&tx, work_id)?.context("Safe checkpoint is missing")?;
        continuation::validate_checkpoint(&checkpoint)?;
        ensure!(
            dependencies == "[]",
            "Recovery cannot replace a pending child wait"
        );
        let reason = work
            .reason
            .clone()
            .unwrap_or_else(|| "Local worker stopped without a result".into());
        text(&reason, 4096, "Previous blocker")?;
        let attempt = work.attempts.last_mut().context("Attempt missing")?;
        ensure!(
            attempt.id.to_string() == id
                && attempt.generation == generation
                && attempt.environment == ExecutionEnvironment::Local,
            "Recovery checkpoint is stale"
        );
        attempt.generation = generation
            .checked_add(1)
            .context("Lease generation exhausted")?;
        attempt.status = AssignmentStatus::Waiting;
        work.status = AssignmentStatus::Waiting;
        work.reason = Some("Continuing from the saved Local checkpoint".into());
        work.can_resume = false;
        tx.execute(
            "INSERT INTO ion_team_recovery_waits VALUES(?,?,?,?)",
            params![work_id.to_string(), id, expected_revision, reason],
        )?;
        tx.execute(
            "DELETE FROM ion_team_recovery_points WHERE work_id=?",
            [work_id.to_string()],
        )?;
        // The old blocker is retained in the audited recovery wait. Its stale
        // lease cannot report again; the same attempt may now report a new result.
        tx.execute(
            "UPDATE ion_team_attempts SET live=0,report=NULL WHERE id=?",
            [work.attempts.last().unwrap().id.to_string()],
        )?;
        save_work(&tx, &work, false)?;
        bump(&tx, run_id)?;
        audit(
            &tx,
            run_id,
            Some(work_id),
            "checkpoint_resume_requested",
            &reason,
            now,
        )?;
        tx.commit()?;
        self.snapshot(run_id)
    }
}
