//! Persisted human planning decisions; no approval can expand execution grants.
use super::continuation::{validate_checkpoint, write_checkpoint};
use super::*;
fn decode(body: String, status: String, revision: u64) -> Result<TeamDecision> {
    let mut decision: TeamDecision = serde_json::from_str(&body)?;
    decision.status = serde_json::from_value(serde_json::Value::String(status))?;
    decision.revision = revision;
    Ok(decision)
}
fn get(db: &Connection, id: Uuid) -> Result<TeamDecision> {
    let (body, status, revision) = db
        .query_row(
            "SELECT body,status,revision FROM ion_team_decisions WHERE id=?",
            [id.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .context("Team decision not found")?;
    decode(body, status, revision)
}
pub(super) fn for_run(db: &Connection, run: Uuid) -> Result<Vec<TeamDecision>> {
    let mut q=db.prepare("SELECT body,status,revision FROM ion_team_decisions WHERE run_id=? ORDER BY created_at,id LIMIT 257")?;
    let records = q
        .query_map([run.to_string()], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .map(|row| {
            let (body, status, revision) = row?;
            decode(body, status, revision)
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(records.len() <= 256, "Team decision history limit exceeded");
    Ok(records)
}
pub(super) fn waiting_decision(db: &Connection, work: Uuid) -> Result<Option<TeamDecision>> {
    let row: Option<String> = db
        .query_row(
            "SELECT decision_id FROM ion_team_decision_waits WHERE work_id=?",
            [work.to_string()],
            |r| r.get(0),
        )
        .optional()?;
    row.map(|id| get(db, Uuid::parse_str(&id)?)).transpose()
}
pub(super) fn cancel_pending(tx: &Transaction<'_>, run: Uuid) -> Result<()> {
    tx.execute("UPDATE ion_team_decisions SET status='canceled',revision=revision+1 WHERE run_id=? AND status='pending'",[run.to_string()])?;
    Ok(())
}
impl TeamCoordinator {
    /// The adapter checkpoints only after all in-flight effects have settled.
    /// Request identity and the exact proposal are immutable across replay.
    pub fn suspend_for_decision(
        &self,
        lease: &AssignmentLease,
        request: Uuid,
        title: &str,
        proposal: &str,
        checkpoint: &str,
        now: u64,
    ) -> Result<TeamDecision> {
        ensure!(!request.is_nil(), "Decision request identity required");
        text(title, 160, "Decision title")?;
        text(proposal, 8192, "Decision proposal")?;
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
        let signature = serde_json::to_string(
            &serde_json::json!({"title":title,"proposal":proposal,"checkpoint":checkpoint,"attempt":lease.attempt_id,"generation":lease.generation}),
        )?;
        let existing: Option<(String, String)> = tx
            .query_row(
                "SELECT id,signature FROM ion_team_decisions WHERE work_id=? AND request_id=?",
                params![work.id.to_string(), request.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((id, old)) = existing {
            ensure!(
                old == signature
                    && work.status == AssignmentStatus::Waiting
                    && work
                        .attempts
                        .last()
                        .is_some_and(|a| a.id == lease.attempt_id
                            && lease.generation.checked_add(1) == Some(a.generation)),
                "Decision replay differs or its lease is stale"
            );
            return get(&tx, Uuid::parse_str(&id)?);
        }
        checked_lease(&run, lease, now, false)?;
        ensure!(
            run.decisions.len() < 256,
            "Team decision history limit reached"
        );
        let attempt = work.attempts.last_mut().context("Attempt missing")?;
        let decision = TeamDecision {
            id: Uuid::new_v4(),
            run_id: run.id,
            project_id: run.project_id,
            assignment_id: work.id,
            task_id: work.task_id,
            agent_id: work.agent_id,
            definition_revision: run.definition_revision,
            run_revision: run.revision,
            attempt_id: attempt.id,
            host: attempt.host.clone(),
            title: title.into(),
            proposal: proposal.into(),
            status: TeamDecisionStatus::Pending,
            revision: 1,
            created_at: now,
        };
        tx.execute(
            "INSERT INTO ion_team_decisions VALUES(?,?,?,?,?,'pending',1,?,?,?)",
            params![
                decision.id.to_string(),
                run.id.to_string(),
                work.id.to_string(),
                request.to_string(),
                run.project_id.to_string(),
                now,
                signature,
                serde_json::to_string(&decision)?
            ],
        )?;
        tx.execute(
            "INSERT INTO ion_team_decision_waits VALUES(?,?)",
            params![work.id.to_string(), decision.id.to_string()],
        )?;
        attempt.generation = attempt
            .generation
            .checked_add(1)
            .context("Lease generation exhausted")?;
        attempt.status = AssignmentStatus::Waiting;
        work.status = AssignmentStatus::Waiting;
        work.reason = Some("Waiting for your decision".into());
        write_checkpoint(&tx, lease, "[]", checkpoint)?;
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
            "decision_requested",
            &decision.id.to_string(),
            now,
        )?;
        tx.commit()?;
        Ok(decision)
    }
    /// Called only by the authenticated user control plane, never by model tools.
    pub fn resolve_decision(
        &self,
        id: Uuid,
        revision: u64,
        approve: bool,
        now: u64,
    ) -> Result<TeamDecision> {
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let decision = get(&tx, id)?;
        let run = load(&tx, decision.run_id)?;
        let verdict = if approve {
            TeamDecisionStatus::Approved
        } else {
            TeamDecisionStatus::Declined
        };
        if decision.status != TeamDecisionStatus::Pending {
            ensure!(
                decision.status == verdict && revision.checked_add(1) == Some(decision.revision),
                "Decision was already answered or canceled"
            );
            return Ok(decision);
        }
        active(&run, now)?;
        ensure!(
            decision.revision == revision,
            "Decision changed; refresh before answering"
        );
        let work = run
            .work
            .iter()
            .find(|w| w.id == decision.assignment_id)
            .context("Decision assignment missing")?;
        ensure!(
            work.status == AssignmentStatus::Waiting
                && waiting_decision(&tx, work.id)?.is_some_and(|d| d.id == id)
                && work
                    .attempts
                    .last()
                    .is_some_and(|a| a.id == decision.attempt_id && a.host == decision.host)
                && run.definition_revision == decision.definition_revision,
            "Decision scope or host changed"
        );
        tx.execute(
            "UPDATE ion_team_decisions SET status=?,revision=revision+1 WHERE id=?",
            params![
                if approve { "approved" } else { "declined" },
                id.to_string()
            ],
        )?;
        bump(&tx, run.id)?;
        audit(
            &tx,
            run.id,
            Some(work.id),
            "decision_answered",
            &format!("{id}: {}", if approve { "approved" } else { "declined" }),
            now,
        )?;
        let result = get(&tx, id)?;
        tx.commit()?;
        Ok(result)
    }
    pub fn list_decisions(
        &self,
        project: Option<Uuid>,
        pending_only: bool,
        after: Option<Uuid>,
        limit: usize,
    ) -> Result<TeamDecisionPage> {
        ensure!((1..=64).contains(&limit), "Decision page size must be 1–64");
        let mut db = self.connect()?;
        let tx = db.transaction()?;
        let cursor = after
            .map(|id| {
                tx.query_row(
                    "SELECT created_at FROM ion_team_decisions WHERE id=?",
                    [id.to_string()],
                    |r| r.get::<_, u64>(0),
                )
                .context("Decision page cursor no longer exists")
            })
            .transpose()?;
        let pending_count=tx.query_row("SELECT COUNT(*) FROM ion_team_decisions WHERE status='pending' AND (?1 IS NULL OR project_id=?1)",[project.map(|id|id.to_string())],|r|r.get(0))?;
        let mut q=tx.prepare("SELECT body,status,revision FROM ion_team_decisions WHERE (?1 IS NULL OR project_id=?1) AND (?2 IS NULL OR created_at<?2 OR (created_at=?2 AND id<?3)) AND (?4=0 OR status='pending') ORDER BY created_at DESC,id DESC LIMIT ?5")?;
        let mut decisions = q
            .query_map(
                params![
                    project.map(|id| id.to_string()),
                    cursor,
                    after.map(|id| id.to_string()),
                    pending_only,
                    limit + 1
                ],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?
            .map(|row| {
                let (body, status, revision) = row?;
                decode(body, status, revision)
            })
            .collect::<Result<Vec<_>>>()?;
        let next_cursor = if decisions.len() > limit {
            decisions.truncate(limit);
            decisions.last().map(|d| d.id)
        } else {
            None
        };
        drop(q);
        tx.commit()?;
        Ok(TeamDecisionPage {
            decisions,
            next_cursor,
            pending_count,
        })
    }
}
