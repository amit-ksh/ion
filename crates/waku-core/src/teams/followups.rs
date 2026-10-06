//! Human messages reach the existing lead at a checkpoint boundary. Queueing
//! does not resume a blocked worker, approve a decision, or issue a new budget.
use super::*;
use waku_protocol::model::{Message, MessageRole, QueuedMessage};

pub(super) fn for_run(db: &Connection, run: Uuid) -> Result<Vec<TeamFollowUp>> {
    let bodies: Vec<String> = db
        .prepare("SELECT body FROM ion_team_followups WHERE run_id=? ORDER BY rowid LIMIT 9")?
        .query_map([run.to_string()], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    ensure!(bodies.len() <= 8, "Team follow-up count exceeds its bound");
    bodies
        .into_iter()
        .map(|b| serde_json::from_str(&b).map_err(Into::into))
        .collect()
}
pub(super) fn require_delivered(db: &Connection, run: Uuid, work: Uuid) -> Result<()> {
    let pending:bool = db.query_row("SELECT EXISTS(SELECT 1 FROM ion_team_followups WHERE run_id=? AND work_id=? AND status='queued')",params![run.to_string(),work.to_string()],|r|r.get(0))?;
    ensure!(
        !pending,
        "Read the queued user follow-up at the next model boundary before completing this assignment"
    );
    Ok(())
}
pub(super) fn cancel_pending(tx: &Transaction<'_>, run: Uuid) -> Result<bool> {
    let mut changed = false;
    for mut followup in for_run(tx, run)?
        .into_iter()
        .filter(|f| f.status == TeamFollowUpStatus::Queued)
    {
        changed = true;
        followup.status = TeamFollowUpStatus::Canceled;
        tx.execute(
            "UPDATE ion_team_followups SET status='canceled',body=? WHERE run_id=? AND id=?",
            params![
                serde_json::to_string(&followup)?,
                run.to_string(),
                followup.message.id.to_string()
            ],
        )?;
    }
    Ok(changed)
}
impl TeamCoordinator {
    pub(crate) fn queue_followup(
        &self,
        run_id: Uuid,
        root_task: Uuid,
        message: QueuedMessage,
        now: u64,
    ) -> Result<TeamRunSnapshot> {
        ensure!(!message.id.is_nil(), "Follow-up message identity required");
        text(&message.content, 4096, "Follow-up message")?;
        ensure!(
            serde_json::to_string(&message.content)?.len() <= 8192
                && message.attachments.is_empty()
                && message.display_content.is_none(),
            "Team follow-ups support bounded text; use New task for additional attachments"
        );
        let signature = serde_json::to_string(&message)?;
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = load(&tx, run_id)?;
        ensure!(
            run.root_task_id == root_task,
            "Send this follow-up from the team lead's task"
        );
        if let Some(previous) = tx
            .query_row(
                "SELECT signature FROM ion_team_followups WHERE run_id=? AND id=?",
                params![run_id.to_string(), message.id.to_string()],
                |r| r.get::<_, String>(0),
            )
            .optional()?
        {
            ensure!(
                previous == signature,
                "Follow-up ID reused with changed text or presentation"
            );
            return Ok(run);
        }
        active(&run, now)?;
        let root = run
            .work
            .iter()
            .find(|w| w.task_id == root_task && w.parent_id.is_none())
            .context("Team lead assignment missing")?;
        ensure!(
            matches!(
                root.status,
                AssignmentStatus::Queued | AssignmentStatus::Running | AssignmentStatus::Waiting
            ) || root.status == AssignmentStatus::Blocked && root.can_resume,
            "This team cannot receive follow-ups now; use New task after completion or reconcile the stopped host first"
        );
        ensure!(
            run.follow_ups.len() < 8,
            "This run reached its eight follow-up limit; use New task for more work"
        );
        let profile = run
            .profiles
            .iter()
            .find(|p| p.id == root.agent_id)
            .context("Pinned lead profile missing")?;
        ensure!(
            matches!(
                profile.provider,
                waku_protocol::model::ProviderKind::Nebius
                    | waku_protocol::model::ProviderKind::Claude
            ),
            "The selected lead harness needs a verified follow-up adapter"
        );
        ensure!(
            decisions::waiting_decision(&tx, root.id)?.is_none(),
            "Resolve the current decision before sending another follow-up"
        );
        workspace::require_settled_effects(&tx, run.id)?;
        let checkpoint = continuation::checkpoint_row(&tx, root.id)?;
        let mut queued = run
            .follow_ups
            .iter()
            .filter(|f| f.status == TeamFollowUpStatus::Queued)
            .cloned()
            .collect::<Vec<_>>();
        queued.push(TeamFollowUp {
            run_id,
            assignment_id: root.id,
            message: message.clone(),
            status: TeamFollowUpStatus::Queued,
            attempt_id: None,
        });
        crate::driver::validate_native_team_followups(
            checkpoint.as_ref().map(|row| row.3.as_str()),
            &root.objective,
            &queued,
        )?;
        ensure!(
            run.usage.model_calls < u32::from(run.limits.max_model_calls),
            "Team model-call budget is exhausted"
        );
        let mut message = message;
        message.created_at = now;
        let followup = TeamFollowUp {
            run_id,
            assignment_id: root.id,
            message,
            status: TeamFollowUpStatus::Queued,
            attempt_id: None,
        };
        tx.execute("INSERT INTO ion_team_followups(run_id,work_id,id,signature,status,body) VALUES(?,?,?,?, 'queued',?)",params![run_id.to_string(),root.id.to_string(),followup.message.id.to_string(),signature,serde_json::to_string(&followup)?])?;
        bump(&tx, run_id)?;
        audit(
            &tx,
            run_id,
            Some(root.id),
            "followup_queued",
            "User follow-up queued for the existing lead; authorization and budgets retained",
            now,
        )?;
        let result = load(&tx, run_id)?;
        tx.commit()?;
        Ok(result)
    }
    /// The trusted native journal builder validates pending calls and all context
    /// bounds. Delivery and the new private checkpoint commit in one transaction.
    pub(crate) fn deliver_followups(
        &self,
        lease: &AssignmentLease,
        checkpoint: &str,
        now: u64,
    ) -> Result<Option<(String, Vec<TeamFollowUp>)>> {
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let pending:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM ion_team_followups WHERE run_id=? AND work_id=? AND status='queued')",params![lease.run_id.to_string(),lease.work_id.to_string()],|r|r.get(0))?;
        if !pending {
            return Ok(None);
        }
        let run = load(&tx, lease.run_id)?;
        active(&run, now)?;
        let work = checked_lease(&run, lease, now, false)?.clone();
        ensure!(
            work.parent_id.is_none() && work.task_id == run.root_task_id,
            "Only the lead receives user follow-ups"
        );
        workspace::require_settled_effects(&tx, run.id)?;
        let (attempt, generation, dependencies, stored) =
            continuation::checkpoint_row(&tx, work.id)?.context("Follow-up checkpoint missing")?;
        ensure!(
            attempt == lease.attempt_id.to_string()
                && generation == lease.generation
                && dependencies == "[]"
                && stored == checkpoint,
            "Follow-up checkpoint or lease changed"
        );
        let mut messages = run
            .follow_ups
            .into_iter()
            .filter(|f| f.status == TeamFollowUpStatus::Queued && f.assignment_id == work.id)
            .collect::<Vec<_>>();
        let updated = crate::driver::append_native_team_followups(checkpoint, &messages)?;
        continuation::write_checkpoint(&tx, lease, "[]", &updated)?;
        if work
            .attempts
            .last()
            .is_some_and(|a| a.environment == ExecutionEnvironment::Local)
        {
            tx.execute(
                "INSERT INTO ion_team_recovery_points VALUES(?,?,?)",
                params![
                    work.id.to_string(),
                    lease.attempt_id.to_string(),
                    lease.generation
                ],
            )?;
        }
        for followup in &mut messages {
            followup.status = TeamFollowUpStatus::Delivered;
            followup.attempt_id = Some(lease.attempt_id);
            tx.execute("UPDATE ion_team_followups SET status='delivered',body=? WHERE run_id=? AND id=? AND status='queued'",params![serde_json::to_string(followup)?,run.id.to_string(),followup.message.id.to_string()])?;
        }
        bump(&tx, run.id)?;
        audit(
            &tx,
            run.id,
            Some(work.id),
            "followup_delivered",
            "User follow-ups and the native checkpoint committed together under the existing lease",
            now,
        )?;
        tx.commit()?;
        Ok(Some((updated, messages)))
    }
    /// Root transcript updates have the same recoverable outbox contract as child
    /// task materialization. A crash after task save reuses each exact message ID.
    pub(crate) fn materialize_followups(
        &self,
        run_id: Uuid,
        store: &crate::persistence::StateStore,
        state: &mut crate::persistence::PersistedState,
    ) -> Result<usize> {
        let db = self.connect()?;
        let pending: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM ion_team_followups WHERE run_id=? AND materialized=0)",
            [run_id.to_string()],
            |r| r.get(0),
        )?;
        if !pending {
            return Ok(0);
        }
        let run = self.snapshot(run_id)?;
        let session = state
            .session_mut(run.root_task_id)
            .context("Team lead task missing")?;
        store.hydrate(session)?;
        let mut added = 0;
        for f in &run.follow_ups {
            if let Some(existing) = session.messages.iter().find(|m| m.id == f.message.id) {
                ensure!(
                    existing.role == MessageRole::User && existing.content == f.message.content,
                    "Follow-up transcript identity changed"
                );
            } else {
                let mut message = Message::new(MessageRole::User, &f.message.content);
                message.id = f.message.id;
                message.created_at = f.message.created_at;
                message.turn_id = session.active_turn_id();
                session.messages.push(message);
                added += 1;
            }
        }
        state.mark_session_dirty(run.root_task_id);
        store.save(state)?;
        for f in &run.follow_ups {
            db.execute(
                "UPDATE ion_team_followups SET materialized=1 WHERE run_id=? AND id=?",
                params![run.id.to_string(), f.message.id.to_string()],
            )?;
        }
        Ok(added)
    }
}
