//! Exact approved connector effects. Identity, payload, host and grant are pinned
//! before a network mutation; missing acknowledgements are never retried.
use super::*;
use serde_json::{Value, json};

pub struct ApprovedAppAction {
    decision_id: Uuid,
    run_id: Uuid,
    work_id: Uuid,
    attempt_id: Uuid,
    signature: String,
}
pub enum AppActionStart {
    Execute(ApprovedAppAction),
    Receipt(String),
}
pub(super) fn validate_action(work: &TeamAssignment, action: &TeamAppAction) -> Result<()> {
    ensure!(
        work.access.app_actions.contains(&action.grant),
        "App action is outside the assignment grant"
    );
    waku_credentials::composio::validate_action_identity(
        &action.grant.account_id,
        &action.grant.action,
        &action.grant.version,
    )?;
    ensure!(
        action.arguments_json.len() <= 4096,
        "App arguments must fit in 4 KiB for review"
    );
    let args: Value = serde_json::from_str(&action.arguments_json)?;
    ensure!(
        args.is_object() && serde_json::to_string(&args)? == action.arguments_json,
        "App arguments must be a canonical JSON object"
    );
    Ok(())
}
fn checked_action(
    db: &Connection,
    lease: &AssignmentLease,
    id: Uuid,
    now: u64,
) -> Result<(TeamDecision, TeamAppAction)> {
    let run = load(db, lease.run_id)?;
    active(&run, now)?;
    let work = checked_lease(&run, lease, now, false)?;
    let decision = super::decisions::get(db, id)?;
    ensure!(
        decision.status == TeamDecisionStatus::Approved
            && decision.run_id == run.id
            && decision.assignment_id == work.id
            && decision.attempt_id == lease.attempt_id
            && decision.host == work.attempts.last().context("Attempt missing")?.host
            && decision.definition_revision == run.definition_revision,
        "App approval is missing or belongs to another assignment/host"
    );
    let action = decision
        .app_action
        .clone()
        .context("This decision does not approve an app action")?;
    validate_action(work, &action)?;
    Ok((decision, action))
}
impl TeamCoordinator {
    pub fn validate_app_action(
        &self,
        lease: &AssignmentLease,
        action: &TeamAppAction,
        now: u64,
    ) -> Result<()> {
        let db = self.connect()?;
        let run = load(&db, lease.run_id)?;
        active(&run, now)?;
        validate_action(checked_lease(&run, lease, now, false)?, action)
    }

    pub fn suspend_for_app_action(
        &self,
        lease: &AssignmentLease,
        request: Uuid,
        grant: AppActionGrant,
        arguments: Value,
        checkpoint: &str,
        now: u64,
    ) -> Result<TeamDecision> {
        let action = TeamAppAction {
            grant,
            arguments_json: serde_json::to_string(&arguments)?,
        };
        // The user reviews host-generated exact fields, never a model's summary of the effect.
        let title: String = format!("Run {}", action.grant.action)
            .chars()
            .take(160)
            .collect();
        let proposal = format!(
            "App action: {}\nConnected account: {}\nTool version: {}\nArguments:\n{}",
            action.grant.action,
            action.grant.account_id,
            action.grant.version,
            action.arguments_json
        );
        self.suspend_for_decision_action(
            lease,
            request,
            &title,
            &proposal,
            Some(action),
            checkpoint,
            now,
        )
    }
    pub fn approved_app_action(
        &self,
        lease: &AssignmentLease,
        decision: Uuid,
        now: u64,
    ) -> Result<TeamAppAction> {
        Ok(checked_action(&self.connect()?, lease, decision, now)?.1)
    }
    pub fn app_action_receipt(
        &self,
        lease: &AssignmentLease,
        decision: Uuid,
        now: u64,
    ) -> Result<Option<String>> {
        let db = self.connect()?;
        checked_action(&db, lease, decision, now)?;
        let prior: Option<(String, Option<String>)> = db
            .query_row(
                "SELECT state,receipt FROM ion_team_app_effects WHERE decision_id=?",
                [decision.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        prior
            .map(|(state, receipt)| {
                ensure!(
                    state == "settled",
                    "A started app action is uncertain; it must not be sent again"
                );
                receipt.context("App acknowledgment missing")
            })
            .transpose()
    }
    pub fn begin_app_action(
        &self,
        lease: &AssignmentLease,
        decision: Uuid,
        now: u64,
    ) -> Result<AppActionStart> {
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (decision, action) = checked_action(&tx, lease, decision, now)?;
        let signature = serde_json::to_string(
            &json!({"action":action,"host":decision.host,"attempt":lease.attempt_id,"definition":decision.definition_revision}),
        )?;
        let prior: Option<(String, String, Option<String>)> = tx
            .query_row(
                "SELECT signature,state,receipt FROM ion_team_app_effects WHERE decision_id=?",
                [decision.id.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        if let Some((old, state, receipt)) = prior {
            ensure!(old == signature, "Approved app action changed");
            ensure!(
                state == "settled",
                "App action outcome is uncertain; verify it at the provider before further work"
            );
            return Ok(AppActionStart::Receipt(
                receipt.context("App receipt missing")?,
            ));
        }
        super::workspace::require_settled_effects(&tx, lease.run_id)?;
        tx.execute(
            "INSERT INTO ion_team_app_effects VALUES(?,?,?,?,?,'pending',NULL)",
            params![
                decision.id.to_string(),
                lease.run_id.to_string(),
                lease.work_id.to_string(),
                lease.attempt_id.to_string(),
                signature
            ],
        )?;
        audit(
            &tx,
            lease.run_id,
            Some(lease.work_id),
            "app_action_started",
            &decision.id.to_string(),
            now,
        )?;
        bump(&tx, lease.run_id)?;
        tx.commit()?;
        Ok(AppActionStart::Execute(ApprovedAppAction {
            decision_id: decision.id,
            run_id: lease.run_id,
            work_id: lease.work_id,
            attempt_id: lease.attempt_id,
            signature,
        }))
    }
    /// A late acknowledgment after Stop records only the already-started effect.
    pub fn finish_app_action(
        &self,
        ticket: &ApprovedAppAction,
        receipt: &str,
        now: u64,
    ) -> Result<String> {
        ensure!(receipt.len() <= 48 * 1024, "App receipt exceeds 48 KiB");
        let value: Value = serde_json::from_str(receipt)?;
        ensure!(
            value.is_object() && value["successful"].is_boolean(),
            "App result must include an explicit outcome"
        );
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (signature, state, old): (String, String, Option<String>) = tx.query_row("SELECT signature,state,receipt FROM ion_team_app_effects WHERE decision_id=? AND run_id=? AND work_id=? AND attempt_id=?", params![ticket.decision_id.to_string(),ticket.run_id.to_string(),ticket.work_id.to_string(),ticket.attempt_id.to_string()], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
        ensure!(signature == ticket.signature, "App effect ticket differs");
        if state == "settled" {
            ensure!(
                old.as_deref() == Some(receipt),
                "App acknowledgment differs from its durable receipt"
            );
            return Ok(receipt.into());
        }
        ensure!(
            state == "pending",
            "App effect is not awaiting acknowledgement"
        );
        tx.execute(
            "UPDATE ion_team_app_effects SET state='settled',receipt=? WHERE decision_id=?",
            params![receipt, ticket.decision_id.to_string()],
        )?;
        audit(
            &tx,
            ticket.run_id,
            Some(ticket.work_id),
            "app_action_settled",
            &ticket.decision_id.to_string(),
            now,
        )?;
        bump(&tx, ticket.run_id)?;
        tx.commit()?;
        Ok(receipt.into())
    }
    pub fn has_pending_effect(&self, run: Uuid) -> Result<bool> {
        Ok(self.connect()?.query_row("SELECT EXISTS(SELECT 1 FROM ion_team_workspace_edits WHERE run_id=?1 AND state='pending') OR EXISTS(SELECT 1 FROM ion_team_app_effects WHERE run_id=?1 AND state='pending')", [run.to_string()], |r|r.get(0))?)
    }
}

/// Called only by stopped-host human recovery, without another provider request.
pub(super) fn confirm_effect(
    tx: &Transaction<'_>,
    run: &TeamRunSnapshot,
    work: &TeamAssignment,
    attempt: Uuid,
    decision_id: Uuid,
    successful: bool,
    evidence: &str,
    now: u64,
) -> Result<()> {
    text(evidence, 4096, "Provider confirmation evidence")?;
    let decision = super::decisions::get(tx, decision_id)?;
    ensure!(
        decision.run_id == run.id
            && decision.assignment_id == work.id
            && decision.attempt_id == attempt
            && decision.status == TeamDecisionStatus::Approved,
        "App decision belongs to another attempt or lacks approval"
    );
    let action = decision
        .app_action
        .as_ref()
        .context("Decision is not an app action")?;
    validate_action(work, action)?;
    let (signature,state):(String,String)=tx.query_row("SELECT signature,state FROM ion_team_app_effects WHERE decision_id=? AND run_id=? AND work_id=? AND attempt_id=?",params![decision_id.to_string(),run.id.to_string(),work.id.to_string(),attempt.to_string()],|r|Ok((r.get(0)?,r.get(1)?)))?;
    ensure!(
        state == "pending",
        "App effect is no longer awaiting recovery"
    );
    let signature: Value = serde_json::from_str(&signature)?;
    ensure!(
        signature["action"] == serde_json::to_value(action)?
            && signature["host"] == decision.host
            && signature["attempt"] == attempt.to_string()
            && signature["definition"] == run.definition_revision,
        "App effect no longer matches its pinned approval"
    );
    let receipt=json!({"successful":successful,"confirmed_by":"user","evidence":evidence,"decision_id":decision_id,"not_started":false}).to_string();
    ensure!(tx.execute("UPDATE ion_team_app_effects SET state='settled',receipt=? WHERE decision_id=? AND state='pending'",params![receipt,decision_id.to_string()])?==1,"App effect changed during recovery");
    super::documents::publish_in_transaction(
        tx,
        run,
        work,
        decision_id,
        &format!("App-action-{}.json", decision_id.simple()),
        "json",
        &receipt,
        now,
    )?;
    audit(
        tx,
        run.id,
        Some(work.id),
        "app_effect_reconciled",
        &format!(
            "{decision_id}: user confirmed {}",
            if successful { "success" } else { "failure" }
        ),
        now,
    )?;
    Ok(())
}
