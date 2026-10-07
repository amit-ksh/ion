//! Durable yields release capacity while child work runs. Runtime checkpoints
//! stay private to trusted adapters; projections contain only bounded handoffs.
use super::*;

pub struct AssignmentContinuation {
    pub claimed: ClaimedAssignment,
    pub checkpoint: String,
    pub children: Vec<AssignmentHandoff>,
    pub decision: Option<TeamDecision>,
    pub recovery_note: Option<String>,
    pub(crate) gpu_result: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct AssignmentHandoff {
    pub assignment_id: Uuid,
    pub task_id: Uuid,
    pub agent_id: Uuid,
    pub status: AssignmentStatus,
    pub expected_output: String,
    pub expected_output_truncated: bool,
    pub rework_of: Option<Uuid>,
    pub review_feedback: Option<String>,
    pub review_feedback_truncated: bool,
    pub summary: Option<String>,
    pub reason: Option<String>,
    pub artifact_ids: Vec<Uuid>,
    pub checks: Vec<String>,
    pub omitted_artifacts: usize,
    pub omitted_checks: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub executed_checks: Vec<ExecutedCommandCheck>,
    #[serde(skip_serializing_if = "is_zero")]
    pub omitted_executed_checks: usize,
}

pub(super) fn check_capacity(
    tx: &Transaction<'_>,
    run: &TeamRunSnapshot,
    profile: &AgentProfile,
    access: &AgentAccess,
) -> Result<()> {
    super::workspace::require_settled_effects(tx, run.id)?;
    super::workspace::check_writer_capacity(tx, run.id, access)?;
    let (shared, capacity): (u32, Option<u16>) = tx.query_row(
        "SELECT count(*),min(capacity) FROM ion_team_attempts WHERE agent_id=? AND live=1",
        [profile.id.to_string()],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let capacity = capacity.map_or(profile.max_parallel_tasks, |c| {
        c.min(profile.max_parallel_tasks)
    });
    ensure!(
        shared < u32::from(capacity)
            && run
                .work
                .iter()
                .filter(|w| w.status.holds_capacity())
                .count()
                < usize::from(run.limits.max_parallel_tasks),
        "Agent or team concurrency limit reached"
    );
    Ok(())
}
pub(super) fn validate_checkpoint(checkpoint: &str) -> Result<()> {
    ensure!(
        !checkpoint.is_empty() && checkpoint.len() <= 128 * 1024,
        "Runtime checkpoint must fit in 128 KiB"
    );
    serde_json::from_str::<serde_json::Value>(checkpoint)
        .context("Invalid runtime checkpoint JSON")?;
    Ok(())
}
pub(super) fn checkpoint_row(
    tx: &Connection,
    work: Uuid,
) -> Result<Option<(String, u32, String, String)>> {
    Ok(tx.query_row("SELECT attempt_id,generation,dependencies,checkpoint FROM ion_team_continuations WHERE work_id=?",
        [work.to_string()], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?)
}
pub(super) fn write_checkpoint(
    tx: &Transaction<'_>,
    lease: &AssignmentLease,
    children: &str,
    checkpoint: &str,
) -> Result<()> {
    tx.execute(
        "DELETE FROM ion_team_recovery_points WHERE work_id=?",
        [lease.work_id.to_string()],
    )?;
    tx.execute("INSERT INTO ion_team_continuations(work_id,attempt_id,generation,dependencies,checkpoint) VALUES(?,?,?,?,?) ON CONFLICT(work_id) DO UPDATE SET attempt_id=excluded.attempt_id,generation=excluded.generation,dependencies=excluded.dependencies,checkpoint=excluded.checkpoint",
        params![lease.work_id.to_string(),lease.attempt_id.to_string(),lease.generation,children,checkpoint])?;
    Ok(())
}
fn settled(status: AssignmentStatus) -> bool {
    matches!(
        status,
        AssignmentStatus::Completed
            | AssignmentStatus::InReview
            | AssignmentStatus::Superseded
            | AssignmentStatus::Blocked
            | AssignmentStatus::Failed
            | AssignmentStatus::Canceled
    )
}
pub(super) fn bounded_text(value: &str, maximum: usize) -> String {
    // Budget JSON escaped bytes, so control characters cannot inflate a handoff.
    let mut size = 0;
    value
        .chars()
        .take_while(|c| {
            size += match c {
                '"' | '\\' => 2,
                '\u{0}'..='\u{1f}' => 6,
                _ => c.len_utf8(),
            };
            size <= maximum
        })
        .collect()
}
fn is_zero(value: &usize) -> bool {
    *value == 0
}
fn handoff(work: &TeamAssignment) -> AssignmentHandoff {
    let result = work.result.as_ref();
    let mut handoff = AssignmentHandoff {
        assignment_id: work.id,
        task_id: work.task_id,
        agent_id: work.agent_id,
        status: work.status,
        expected_output: bounded_text(&work.expected_output, 128),
        expected_output_truncated: bounded_text(&work.expected_output, 128).len()
            != work.expected_output.len(),
        rework_of: work.rework_of,
        review_feedback: work.review.as_ref().map(|r| bounded_text(&r.feedback, 128)),
        review_feedback_truncated: work
            .review
            .as_ref()
            .is_some_and(|r| bounded_text(&r.feedback, 128).len() != r.feedback.len()),
        summary: result.map(|r| bounded_text(&r.summary, 512)),
        reason: work.reason.as_ref().map(|r| bounded_text(r, 128)),
        artifact_ids: result
            .map(|r| r.artifact_ids.iter().take(4).copied().collect())
            .unwrap_or_default(),
        checks: result
            .map(|r| {
                r.checks
                    .iter()
                    .take(2)
                    .map(|c| bounded_text(c, 128))
                    .collect()
            })
            .unwrap_or_default(),
        omitted_artifacts: result.map_or(0, |r| r.artifact_ids.len().saturating_sub(4)),
        omitted_checks: result.map_or(0, |r| r.checks.len().saturating_sub(2)),
        executed_checks: result
            .map(|r| {
                r.executed_checks
                    .iter()
                    .filter(|c| serde_json::to_vec(&c.name).is_ok_and(|v| v.len() <= 130))
                    .take(2)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default(),
        omitted_executed_checks: 0,
    };
    // Preserve complete receipt names. Oversized evidence is omitted with a count,
    // never truncated into a different name. Sixteen handoffs still fit 32 KiB.
    handoff.omitted_executed_checks = result.map_or(0, |r| {
        r.executed_checks
            .len()
            .saturating_sub(handoff.executed_checks.len())
    });
    while !handoff.executed_checks.is_empty() && serde_json::to_vec(&handoff).unwrap().len() > 2000
    {
        handoff.executed_checks.pop();
        handoff.omitted_executed_checks += 1;
    }
    handoff
}

impl TeamCoordinator {
    /// Checkpoint model messages/tool receipts before effects. Credentials and
    /// unrelated transcripts must never be passed here. No public wire exposes it.
    pub fn save_runtime_checkpoint(
        &self,
        lease: &AssignmentLease,
        checkpoint: &str,
        now: u64,
    ) -> Result<()> {
        self.save_recoverable_runtime_checkpoint(lease, checkpoint, false, now)
    }
    /// Only the trusted adapter marks a checkpoint safe, after all tool receipts.
    pub(crate) fn save_recoverable_runtime_checkpoint(
        &self,
        lease: &AssignmentLease,
        checkpoint: &str,
        recoverable: bool,
        now: u64,
    ) -> Result<()> {
        validate_checkpoint(checkpoint)?;
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = load(&tx, lease.run_id)?;
        active(&run, now)?;
        let work = checked_lease(&run, lease, now, false)?;
        write_checkpoint(&tx, lease, "[]", checkpoint)?;
        if recoverable
            && work
                .attempts
                .last()
                .is_some_and(|a| a.environment == ExecutionEnvironment::Local)
        {
            workspace::require_settled_effects(&tx, run.id)?;
            tx.execute(
                "INSERT INTO ion_team_recovery_points VALUES(?,?,?)",
                params![
                    lease.work_id.to_string(),
                    lease.attempt_id.to_string(),
                    lease.generation
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn runtime_checkpoint(&self, lease: &AssignmentLease, now: u64) -> Result<Option<String>> {
        let mut db = self.connect()?;
        let tx = db.transaction()?;
        let run = load(&tx, lease.run_id)?;
        checked_lease(&run, lease, now, false)?;
        let row = checkpoint_row(&tx, lease.work_id)?;
        let checkpoint = row
            .filter(|(id, _, _, _)| id == &lease.attempt_id.to_string())
            .map(|(_, _, _, body)| body);
        tx.commit()?;
        Ok(checkpoint)
    }
    /// The host yields only after all in-flight tools are finished. Persist the
    /// continuation and revoke its lease in one transaction, freeing capacity.
    pub fn suspend_for_children(
        &self,
        lease: &AssignmentLease,
        children: &[Uuid],
        checkpoint: &str,
        now: u64,
    ) -> Result<TeamRunSnapshot> {
        ensure!(
            !children.is_empty() && children.len() <= 16,
            "Await 1–16 direct children per yield"
        );
        validate_checkpoint(checkpoint)?;
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = load(&tx, lease.run_id)?;
        active(&run, now)?;
        super::workspace::require_settled_effects(&tx, run.id)?;
        let mut work = run
            .work
            .iter()
            .find(|w| w.id == lease.work_id)
            .context("Assignment missing")?
            .clone();
        let dependencies = serde_json::to_string(children)?;
        if work.status == AssignmentStatus::Waiting {
            let (attempt, generation, stored, body) =
                checkpoint_row(&tx, work.id)?.context("Waiting checkpoint missing")?;
            ensure!(
                attempt == lease.attempt_id.to_string()
                    && generation == lease.generation
                    && work
                        .attempts
                        .last()
                        .is_some_and(|a| a.id == lease.attempt_id
                            && lease.generation.checked_add(1) == Some(a.generation))
                    && stored == dependencies
                    && body == checkpoint,
                "Suspension replay differs or its lease is stale"
            );
            return Ok(run);
        }
        checked_lease(&run, lease, now, false)?;
        let mut seen = HashSet::new();
        for id in children {
            ensure!(seen.insert(*id), "Duplicate awaited child");
            ensure!(
                run.work
                    .iter()
                    .any(|child| child.id == *id && child.parent_id == Some(work.id)),
                "Only this assignment's direct children may be awaited"
            );
        }
        let attempt = work.attempts.last_mut().context("Attempt missing")?;
        attempt.generation = attempt
            .generation
            .checked_add(1)
            .context("Lease generation exhausted")?;
        attempt.status = AssignmentStatus::Waiting;
        work.status = AssignmentStatus::Waiting;
        work.reason = Some("Waiting for child results".into());
        write_checkpoint(&tx, lease, &dependencies, checkpoint)?;
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
            "waiting",
            "Worker yielded; checkpoint committed and execution slot released",
            now,
        )?;
        tx.commit()?;
        self.snapshot(run.id)
    }
    /// Resume exactly once after all watched children have settled. This retains
    /// the attempt and host, then mints a fresh generation; yielding is not a retry.
    pub fn resume_after_children(
        &self,
        run_id: Uuid,
        work_id: Uuid,
        now: u64,
        lease_seconds: u32,
    ) -> Result<AssignmentContinuation> {
        ensure!(
            (1..=300).contains(&lease_seconds),
            "Lease duration must be 1–300 seconds"
        );
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
        ensure!(
            work.status == AssignmentStatus::Waiting,
            "Assignment is not waiting"
        );
        let (attempt_id, generation, dependencies, checkpoint) =
            checkpoint_row(&tx, work.id)?.context("Waiting checkpoint missing")?;
        let children: Vec<Uuid> = serde_json::from_str(&dependencies)?;
        let decision = super::decisions::waiting_decision(&tx, work.id)?;
        let recovery_note = super::recovery::waiting_note(&tx, &work)?;
        let gpu_result = gpu_jobs::waits::handoff(&tx, &work)?;
        if gpu_result.is_some() {
            ensure!(
                children.is_empty() && decision.is_none() && recovery_note.is_none(),
                "GPU wait cannot contain another continuation kind"
            );
        }
        ensure!(
            children.len() <= 16
                && (!children.is_empty()
                    || decision.is_some()
                    || recovery_note.is_some()
                    || gpu_result.is_some()),
            "Invalid continuation dependency count"
        );
        if recovery_note.is_some() {
            ensure!(
                children.is_empty() && decision.is_none(),
                "Recovery cannot replay a decision or child wait"
            );
        }
        if let Some(decision) = &decision {
            ensure!(
                children.is_empty(),
                "Decision wait cannot contain child dependencies"
            );
            ensure!(
                matches!(
                    decision.status,
                    TeamDecisionStatus::Approved | TeamDecisionStatus::Declined
                ),
                "User decision is still pending"
            );
            ensure!(
                decision.run_id == run.id
                    && decision.assignment_id == work.id
                    && decision.definition_revision == run.definition_revision
                    && work
                        .attempts
                        .last()
                        .is_some_and(|a| a.id == decision.attempt_id && a.host == decision.host),
                "Decision assignment or host changed"
            );
        }
        let results: Vec<_> = children
            .iter()
            .map(|id| {
                let child = run
                    .work
                    .iter()
                    .find(|w| w.id == *id && w.parent_id == Some(work.id))
                    .context("Awaited child missing")?;
                ensure!(settled(child.status), "Awaited child has not settled");
                Ok(handoff(child))
            })
            .collect::<Result<_>>()?;
        ensure!(
            serde_json::to_vec(&results)?.len() <= 32 * 1024,
            "Handoff exceeded context limit"
        );
        let profile = run
            .profiles
            .iter()
            .find(|p| p.id == work.agent_id)
            .context("Pinned profile missing")?
            .clone();
        check_capacity(&tx, &run, &profile, &work.access)?;
        let attempt = work.attempts.last_mut().context("Attempt missing")?;
        ensure!(
            attempt.id.to_string() == attempt_id
                && generation.checked_add(1) == Some(attempt.generation)
                && attempt.status == AssignmentStatus::Waiting,
            "Continuation attempt is stale"
        );
        attempt.generation = attempt
            .generation
            .checked_add(1)
            .context("Lease generation exhausted")?;
        attempt.lease_expires_at = now
            .saturating_add(u64::from(lease_seconds))
            .min(run.deadline);
        attempt.status = AssignmentStatus::Running;
        let lease = AssignmentLease {
            run_id,
            work_id,
            attempt_id: attempt.id,
            generation: attempt.generation,
        };
        work.status = AssignmentStatus::Running;
        work.reason = None;
        tx.execute(
            "UPDATE ion_team_attempts SET live=1 WHERE id=?",
            [attempt_id],
        )?;
        save_work(&tx, &work, false)?;
        tx.execute(
            "DELETE FROM ion_team_decision_waits WHERE work_id=?",
            [work.id.to_string()],
        )?;
        tx.execute(
            "DELETE FROM ion_team_recovery_waits WHERE work_id=?",
            [work.id.to_string()],
        )?;
        tx.execute(
            "DELETE FROM ion_team_gpu_waits WHERE work_id=?",
            [work.id.to_string()],
        )?;
        bump(&tx, run.id)?;
        audit(
            &tx,
            run.id,
            Some(work.id),
            "resumed",
            if recovery_note.is_some() {
                "User requested continuation from a safe stopped checkpoint"
            } else if gpu_result.is_some() {
                "Actual GPU command receipt and verified cleanup delivered under a fresh lease generation; spending retained"
            } else if decision.is_some() {
                "User decision receipt delivered under a fresh lease generation"
            } else {
                "Settled child handoffs delivered under a fresh lease generation"
            },
            now,
        )?;
        tx.commit()?;
        Ok(AssignmentContinuation {
            claimed: ClaimedAssignment {
                lease,
                assignment: work,
                profile,
            },
            checkpoint,
            children: results,
            decision,
            recovery_note,
            gpu_result,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn command_check_receipt_handoffs_keep_identity_and_fit_sixteen_children() {
        let text = "\n\t\"Evidence 界\"".repeat(100);
        let names = vec![
            "A".repeat(128),
            "B".repeat(128),
            "\n".repeat(128),
            "C".repeat(256),
        ];
        let work = TeamAssignment {
            can_resume: false,
            can_reconcile: false,
            id: Uuid::new_v4(),
            task_id: Uuid::new_v4(),
            parent_id: Some(Uuid::new_v4()),
            agent_id: Uuid::new_v4(),
            objective: text.clone(),
            expected_output: text.clone(),
            output_contract: Default::default(),
            requires_review: true,
            rework_of: Some(Uuid::new_v4()),
            review: Some(AssignmentReview {
                id: Uuid::new_v4(),
                verdict: ReviewVerdict::Accepted,
                feedback: text.clone(),
                artifact_ids: vec![],
                checks: vec![],
                reviewer_task_id: Uuid::new_v4(),
                replacement_assignment_id: None,
                created_at: 100,
            }),
            dependencies: vec![],
            access: Default::default(),
            status: AssignmentStatus::Completed,
            reason: Some(text.clone()),
            attempts: vec![],
            result: Some(AssignmentResult {
                summary: text.clone(),
                artifact_ids: (0..64).map(|_| Uuid::new_v4()).collect(),
                checks: vec![text; 64],
                executed_checks: names
                    .into_iter()
                    .map(|name| ExecutedCommandCheck {
                        name,
                        request_id: Uuid::new_v4(),
                    })
                    .collect(),
            }),
        };
        let full = &work.result.as_ref().unwrap().executed_checks;
        let items: Vec<_> = (0..16).map(|_| handoff(&work)).collect();
        assert!(serde_json::to_vec(&items).unwrap().len() <= 32 * 1024);
        for item in items {
            assert_eq!(
                item.executed_checks.len() + item.omitted_executed_checks,
                full.len()
            );
            assert!(
                item.executed_checks
                    .iter()
                    .all(|reference| full.contains(reference))
            );
            assert!(
                item.executed_checks
                    .iter()
                    .all(|reference| reference.name.len() == 128 && !reference.name.contains('\n'))
            );
        }
    }
}
