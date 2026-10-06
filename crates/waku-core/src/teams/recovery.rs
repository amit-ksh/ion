//! Explicit continuation of a confirmed stopped local assignment. The adapter
//! marks safe checkpoints; unknown workers and unfinished effects cannot resume.
use super::*;
use serde_json::{Value, json};
mod receipts;

pub(super) fn project(db: &Connection, run: &mut TeamRunSnapshot) -> Result<()> {
    let pending: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM ion_team_workspace_edits WHERE run_id=?1 AND state='pending') OR EXISTS(SELECT 1 FROM ion_team_app_effects WHERE run_id=?1 AND state='pending')",
        [run.id.to_string()], |r| r.get(0),
    )?;
    let points: Vec<(String,String,u32)> = db.prepare(
        "SELECT p.work_id,p.attempt_id,p.generation FROM ion_team_recovery_points p JOIN ion_team_work w ON w.id=p.work_id JOIN ion_team_continuations c ON c.work_id=p.work_id AND c.attempt_id=p.attempt_id AND c.generation=p.generation WHERE w.run_id=? AND w.status='blocked' AND c.dependencies='[]' LIMIT 257",
    )?.query_map([run.id.to_string()], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?.collect::<rusqlite::Result<_>>()?;
    let points: std::collections::HashMap<_, _> = points
        .into_iter()
        .map(|(work, attempt, generation)| (work, (attempt, generation)))
        .collect();
    let unfinished: HashSet<String> = db.prepare(
        "SELECT work_id FROM ion_team_workspace_edits WHERE run_id=?1 AND state='pending' UNION SELECT work_id FROM ion_team_app_effects WHERE run_id=?1 AND state='pending'",
    )?.query_map([run.id.to_string()], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
    let checkpoints: std::collections::HashMap<String,(String,u32)> = db.prepare(
        "SELECT c.work_id,c.attempt_id,c.generation FROM ion_team_continuations c JOIN ion_team_work w ON w.id=c.work_id WHERE w.run_id=? AND w.status='blocked' AND c.dependencies='[]' AND json_extract(c.checkpoint,'$.version')=1 AND json_extract(c.checkpoint,'$.steps')<16 LIMIT 257",
    )?.query_map([run.id.to_string()], |r| Ok((r.get(0)?,(r.get(1)?,r.get(2)?))))?.collect::<rusqlite::Result<_>>()?;
    let native: HashSet<Uuid> = run
        .profiles
        .iter()
        .filter(|p| {
            matches!(
                p.provider,
                waku_protocol::model::ProviderKind::Pi
                    | waku_protocol::model::ProviderKind::OhMyPi
                    | waku_protocol::model::ProviderKind::Codex
                    | waku_protocol::model::ProviderKind::OpenCode
                    | waku_protocol::model::ProviderKind::Nebius
                    | waku_protocol::model::ProviderKind::Claude
            )
        })
        .map(|p| p.id)
        .collect();
    for work in &mut run.work {
        let local = work
            .attempts
            .last()
            .filter(|a| a.environment == ExecutionEnvironment::Local);
        work.can_resume = !pending
            && run.status == TeamRunStatus::Active
            && work.status == AssignmentStatus::Blocked
            && local.is_some_and(|a| {
                a.status == AssignmentStatus::Blocked
                    && points
                        .get(&work.id.to_string())
                        .is_some_and(|(id, g)| id == &a.id.to_string() && *g == a.generation)
            });
        work.can_reconcile = local.is_some()
            && (work.status == AssignmentStatus::Uncertain
                || (matches!(
                    work.status,
                    AssignmentStatus::Blocked
                        | AssignmentStatus::Canceled
                        | AssignmentStatus::Failed
                ) && (unfinished.contains(&work.id.to_string())
                    || (!work.can_resume
                        && run.status == TeamRunStatus::Active
                        && work.status == AssignmentStatus::Blocked
                        && native.contains(&work.agent_id)
                        && local.is_some_and(|a| {
                            checkpoints
                                .get(&work.id.to_string())
                                .is_some_and(|(id, g)| {
                                    id == &a.id.to_string() && *g == a.generation
                                })
                        })))));
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

fn assignment(run: &TeamRunSnapshot, work: Uuid) -> Result<&TeamAssignment> {
    run.work
        .iter()
        .find(|w| w.id == work)
        .context("Assignment missing")
}
fn stopped(work: &TeamAssignment) -> bool {
    matches!(
        work.status,
        AssignmentStatus::Blocked | AssignmentStatus::Canceled | AssignmentStatus::Failed
    ) && work
        .attempts
        .last()
        .is_some_and(|a| a.status == work.status)
}
fn repairable(db: &Connection, run: &TeamRunSnapshot, work: &TeamAssignment) -> Result<bool> {
    let Some(attempt) = work.attempts.last() else {
        return Ok(false);
    };
    if run.status != TeamRunStatus::Active
        || work.status != AssignmentStatus::Blocked
        || !stopped(work)
        || attempt.environment != ExecutionEnvironment::Local
        || !run.profiles.iter().any(|p| {
            p.id == work.agent_id
                && matches!(
                    p.provider,
                    waku_protocol::model::ProviderKind::Pi
                        | waku_protocol::model::ProviderKind::OhMyPi
                        | waku_protocol::model::ProviderKind::Codex
                        | waku_protocol::model::ProviderKind::OpenCode
                        | waku_protocol::model::ProviderKind::Nebius
                        | waku_protocol::model::ProviderKind::Claude
                )
        })
    {
        return Ok(false);
    }
    let Some((id, generation, dependencies, checkpoint)) =
        continuation::checkpoint_row(db, work.id)?
    else {
        return Ok(false);
    };
    let value: Value = serde_json::from_str(&checkpoint)?;
    Ok(id==attempt.id.to_string() && generation==attempt.generation && dependencies=="[]"
        && value["version"]==1 && value["steps"].as_u64().is_some_and(|n|n<16)
        && !db.query_row("SELECT EXISTS(SELECT 1 FROM ion_team_workspace_edits WHERE run_id=?1 AND state='pending') OR EXISTS(SELECT 1 FROM ion_team_app_effects WHERE run_id=?1 AND state='pending')",[run.id.to_string()],|r|r.get::<_,bool>(0))?)
}
fn review(db: &Connection, run: &TeamRunSnapshot, work: Uuid) -> Result<TeamRecoveryReview> {
    let work = assignment(run, work)?;
    let attempt = work.attempts.last().context("Attempt missing")?;
    ensure!(
        attempt.environment == ExecutionEnvironment::Local,
        "Local recovery cannot confirm a sandbox host"
    );
    let pending:HashSet<String>=db.prepare("SELECT decision_id FROM ion_team_app_effects WHERE run_id=? AND work_id=? AND attempt_id=? AND state='pending'")?
        .query_map(params![run.id.to_string(),work.id.to_string(),attempt.id.to_string()],|r|r.get(0))?.collect::<rusqlite::Result<_>>()?;
    let apps = run
        .decisions
        .iter()
        .filter(|d| pending.contains(&d.id.to_string()))
        .cloned()
        .collect();
    Ok(TeamRecoveryReview {
        run_id: run.id,
        assignment_id: work.id,
        attempt_id: attempt.id,
        revision: run.revision,
        host: attempt.host.clone(),
        host_stopped: stopped(work),
        can_prepare_continuation: repairable(db, run, work)? && !work.can_resume,
        workspace_path: run.workspace.as_ref().map(|w| w.path.clone()),
        files: workspace::effect_recovery::files(db, run.id, work.id, attempt.id)?,
        apps,
    })
}
impl TeamCoordinator {
    pub fn recovery_review(
        &self,
        run: Uuid,
        work: Uuid,
    ) -> Result<(TeamRecoveryReview, TeamRunSnapshot)> {
        let mut db = self.connect()?;
        let tx = db.transaction()?;
        let snapshot = load(&tx, run)?;
        let review = review(&tx, &snapshot, work)?;
        tx.commit()?;
        Ok((review, snapshot))
    }
    /// The owner runtime must first establish that its current worker has joined.
    /// An unknown previous host additionally requires the user's explicit assertion.
    pub(crate) fn confirm_recovery(
        &self,
        run_id: Uuid,
        work_id: Uuid,
        attempt_id: Uuid,
        expected_revision: u64,
        confirmation: TeamRecoveryConfirmation,
        now: u64,
    ) -> Result<(TeamRecoveryReview, TeamRunSnapshot)> {
        let body = serde_json::to_string(&confirmation)?;
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = load(&tx, run_id)?;
        let old:Option<(String,String,String)>=tx.query_row("SELECT run_id,attempt_id,body FROM ion_team_recovery_confirmations WHERE work_id=? AND requested_revision=?",
            params![work_id.to_string(),expected_revision],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
        if let Some((r, a, b)) = old {
            ensure!(
                r == run_id.to_string() && a == attempt_id.to_string() && b == body,
                "Recovery confirmation was reused with changed evidence or identity"
            );
            return Ok((review(&tx, &run, work_id)?, run));
        }
        ensure!(
            run.revision == expected_revision,
            "Run changed; refresh before confirming recovery"
        );
        let mut work = assignment(&run, work_id)?.clone();
        let attempt = work.attempts.last().context("Attempt missing")?;
        ensure!(
            attempt.id == attempt_id && attempt.environment == ExecutionEnvironment::Local,
            "Recovery attempt or Local host changed"
        );
        ensure!(
            work.can_reconcile
                || (stopped(&work)
                    && matches!(confirmation, TeamRecoveryConfirmation::PrepareContinuation)),
            "Assignment has no recoverable stopped Local action"
        );
        match &confirmation {
            TeamRecoveryConfirmation::HostStopped => {
                ensure!(
                    work.status == AssignmentStatus::Uncertain,
                    "Host already has a known outcome"
                );
                let status = if run.status == TeamRunStatus::Active {
                    AssignmentStatus::Blocked
                } else {
                    AssignmentStatus::Canceled
                };
                work.status = status;
                work.attempts.last_mut().unwrap().status = status;
                work.reason=Some("User confirmed the previous Local host has stopped; review its receipts before continuing".into());
                tx.execute(
                    "UPDATE ion_team_attempts SET live=0 WHERE id=?",
                    [attempt_id.to_string()],
                )?;
                save_work(&tx, &work, false)?;
                settle_run(&tx, run_id)?;
                audit(
                    &tx,
                    run_id,
                    Some(work_id),
                    "host_stop_confirmed",
                    "User confirmed previous Local worker stopped",
                    now,
                )?;
            }
            TeamRecoveryConfirmation::FileApplied { id }
            | TeamRecoveryConfirmation::FileRestored { id } => {
                ensure!(
                    stopped(&work),
                    "Confirm the previous host stopped before reconciling files"
                );
                workspace::effect_recovery::confirm(
                    &tx,
                    run_id,
                    work_id,
                    attempt_id,
                    *id,
                    matches!(confirmation, TeamRecoveryConfirmation::FileApplied { .. }),
                    now,
                )?;
            }
            TeamRecoveryConfirmation::AppSucceeded {
                decision_id,
                evidence,
            }
            | TeamRecoveryConfirmation::AppFailed {
                decision_id,
                evidence,
            } => {
                ensure!(
                    stopped(&work),
                    "Confirm the previous host stopped before reconciling apps"
                );
                apps::confirm_effect(
                    &tx,
                    &run,
                    &work,
                    attempt_id,
                    *decision_id,
                    matches!(confirmation, TeamRecoveryConfirmation::AppSucceeded { .. }),
                    evidence,
                    now,
                )?;
            }
            TeamRecoveryConfirmation::PrepareContinuation => {
                active(&run, now)?;
                ensure!(
                    !work.can_resume && repairable(&tx, &run, &work)?,
                    "Settle all effects and retain a matching native checkpoint before preparing continuation"
                );
                let (_, generation, _, checkpoint) =
                    continuation::checkpoint_row(&tx, work_id)?.context("Checkpoint missing")?;
                let pending_wait:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM ion_team_decision_waits WHERE work_id=?1) OR EXISTS(SELECT 1 FROM ion_team_recovery_waits WHERE work_id=?1)",[work_id.to_string()],|r|r.get(0))?;
                ensure!(!pending_wait, "Recovery cannot replace an existing wait");
                let app_receipts = receipts::apps(&tx, &run, &work, attempt_id, now)?;
                let repaired = crate::driver::repair_native_team_checkpoint(
                    &checkpoint,
                    |request, name, args| {
                        receipts::tool(&tx, &run, &work, attempt_id, request, name, args)
                    },
                    app_receipts,
                )?;
                continuation::validate_checkpoint(&repaired)?;
                tx.execute("UPDATE ion_team_continuations SET checkpoint=? WHERE work_id=? AND attempt_id=? AND generation=?",params![repaired,work_id.to_string(),attempt_id.to_string(),generation])?;
                tx.execute("INSERT INTO ion_team_recovery_points VALUES(?,?,?) ON CONFLICT(work_id) DO UPDATE SET attempt_id=excluded.attempt_id,generation=excluded.generation",params![work_id.to_string(),attempt_id.to_string(),generation])?;
                audit(
                    &tx,
                    run_id,
                    Some(work_id),
                    "checkpoint_repaired",
                    "Prepared native continuation from durable receipts; no tools were executed",
                    now,
                )?;
            }
        }
        tx.execute(
            "INSERT INTO ion_team_recovery_confirmations VALUES(?,?,?,?,?)",
            params![
                run_id.to_string(),
                work_id.to_string(),
                attempt_id.to_string(),
                expected_revision,
                body
            ],
        )?;
        bump(&tx, run_id)?;
        let snapshot = load(&tx, run_id)?;
        let review = review(&tx, &snapshot, work_id)?;
        tx.commit()?;
        Ok((review, snapshot))
    }
}
