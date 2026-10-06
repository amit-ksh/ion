//! Durable remote operation identities. Dispatch never equates transport exit
//! with remote stop, and a lost spawn acknowledgement is never re-submitted.
use super::*;
#[derive(Clone, Debug)]
pub(crate) struct RemoteCall {
    pub id: Uuid,
    pub image: Uuid,
    pub project: String,
    pub operation: Option<Uuid>,
    pub state: String,
}
impl TeamCoordinator {
    pub(crate) fn begin_remote_call(
        &self,
        lease: &AssignmentLease,
        image: Uuid,
        project: &str,
        now: u64,
    ) -> Result<Uuid> {
        text(project, 160, "Sandbox project")?;
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = load(&tx, lease.run_id)?;
        active(&run, now)?;
        let work = checked_lease(&run, lease, now, false)?;
        ensure!(
            work.attempts
                .last()
                .is_some_and(|a| a.environment == ExecutionEnvironment::NebiusSandbox),
            "Remote transport requires a sandbox lease"
        );
        let target = run
            .profiles
            .iter()
            .find(|p| p.id == work.agent_id)
            .and_then(|p| p.execution.sandbox_target.as_deref())
            .context("Pinned remote image missing")?;
        ensure!(
            Uuid::parse_str(target.rsplit('@').next().unwrap_or(target))? == image,
            "Remote image differs from the pinned target"
        );
        let call:String=tx.query_row("SELECT c.id FROM ion_team_model_calls c JOIN ion_team_call_budgets b ON b.call_id=c.id WHERE c.attempt_id=? AND c.generation=? AND b.input_tokens IS NULL AND b.output_tokens IS NULL ORDER BY c.step DESC LIMIT 1",params![lease.attempt_id.to_string(),lease.generation],|r|r.get(0)).context("Reserve a model call before remote launch")?;
        tx.execute("INSERT INTO ion_team_remote_calls(call_id,work_id,attempt_id,image,project,state,started_at) VALUES(?,?,?,?,?,'submitting',?)",params![call,work.id.to_string(),lease.attempt_id.to_string(),image.to_string(),project,now]).context("Remote request already started; never repeat a submission")?;
        audit(
            &tx,
            run.id,
            Some(work.id),
            "remote_launch",
            &format!(
                "Reserved sandbox request {call} before submission; remote hostname ion-{call}"
            ),
            now,
        )?;
        tx.commit()?;
        Ok(Uuid::parse_str(&call)?)
    }
    pub(crate) fn pin_remote_input(&self, call: Uuid, input: &str) -> Result<()> {
        ensure!(
            !input.is_empty() && input.len() <= 128 * 1024,
            "Remote input exceeds its bound"
        );
        serde_json::from_str::<serde_json::Value>(input)?;
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let state: (String, Option<String>) = tx.query_row(
            "SELECT state,operation FROM ion_team_remote_calls WHERE call_id=?",
            [call.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        ensure!(
            state.0 == "submitting" && state.1.is_none(),
            "Remote submission already has an outcome"
        );
        tx.execute(
            "INSERT INTO ion_team_remote_inputs VALUES(?,?)",
            params![call.to_string(), input],
        )
        .context("Remote submission already prepared; never repeat its POST")?;
        tx.commit()?;
        Ok(())
    }
    /// A network-verified witness binds a user-supplied operation only after
    /// the trusted adapter checks the entire original worker input and host.
    pub(crate) fn bind_recovered_remote_operation(
        &self,
        run_id: Uuid,
        work_id: Uuid,
        attempt_id: Uuid,
        revision: u64,
        call: Uuid,
        operation: Uuid,
        now: u64,
    ) -> Result<()> {
        ensure!(!operation.is_nil(), "Remote operation identity required");
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = load(&tx, run_id)?;
        ensure!(
            run.revision == revision,
            "Run changed; refresh before binding an operation"
        );
        let work = run
            .work
            .iter()
            .find(|w| w.id == work_id)
            .context("Assignment missing")?;
        ensure!(
            work.can_reconcile
                && work
                    .attempts
                    .last()
                    .is_some_and(|a| a.id == attempt_id
                        && a.environment == ExecutionEnvironment::NebiusSandbox),
            "Remote recovery attempt or state changed"
        );
        ensure!(tx.execute("UPDATE ion_team_remote_calls SET operation=?,state='running' WHERE call_id=? AND work_id=? AND attempt_id=? AND state='submitting' AND operation IS NULL AND EXISTS(SELECT 1 FROM ion_team_remote_inputs WHERE call_id=?)",
            params![operation.to_string(),call.to_string(),work_id.to_string(),attempt_id.to_string(),call.to_string()])?==1,
            "Remote request changed or lacks an original input witness");
        tx.execute(
            "INSERT INTO ion_team_recovery_confirmations VALUES(?,?,?,?,?)",
            params![
                run_id.to_string(),
                work_id.to_string(),
                attempt_id.to_string(),
                revision,
                serde_json::to_string(&TeamRecoveryConfirmation::BindRemoteOperation {
                    request_id: call,
                    operation_id: operation
                })?
            ],
        )?;
        audit(
            &tx,
            run_id,
            Some(work_id),
            "remote_operation_recovered",
            &format!(
                "Verified operation {operation} against original request {call}; no work submitted"
            ),
            now,
        )?;
        bump(&tx, run_id)?;
        tx.commit()?;
        Ok(())
    }
    pub(crate) fn bind_remote_operation(&self, call: Uuid, operation: Uuid) -> Result<()> {
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure!(tx.execute("UPDATE ion_team_remote_calls SET operation=?,state='running' WHERE call_id=? AND state='submitting' AND operation IS NULL",params![operation.to_string(),call.to_string()])?==1,"Remote operation identity changed");
        let (run, work): (String, String) = tx.query_row(
            "SELECT c.run_id,c.work_id FROM ion_team_model_calls c WHERE c.id=?",
            [call.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let run = Uuid::parse_str(&run)?;
        audit(
            &tx,
            run,
            Some(Uuid::parse_str(&work)?),
            "remote_operation",
            &format!("Sandbox operation {operation} acknowledged for request {call}"),
            crate::model::unix_time(),
        )?;
        bump(&tx, run)?;
        tx.commit()?;
        Ok(())
    }
    pub(crate) fn finish_remote_call(
        &self,
        call: Uuid,
        operation: Option<Uuid>,
        state: &str,
        receipt: Option<&str>,
    ) -> Result<()> {
        ensure!(
            matches!(state, "SUCCESS" | "FAILED" | "CANCELLED" | "rejected"),
            "Remote stop requires a terminal receipt"
        );
        ensure!(
            receipt.is_none_or(|r| r.len() <= 64 * 1024),
            "Remote receipt exceeded its limit"
        );
        let db = self.connect()?;
        ensure!(db.execute("UPDATE ion_team_remote_calls SET state=?,receipt=? WHERE call_id=? AND (operation=? OR (operation IS NULL AND ? IS NULL)) AND state IN ('submitting','running')",params![state,receipt,call.to_string(),operation.map(|id|id.to_string()),operation.map(|id|id.to_string())])?==1,"Remote receipt identity changed or already settled");
        Ok(())
    }
    pub(crate) fn remote_calls(&self, attempt: Uuid) -> Result<Vec<RemoteCall>> {
        let db = self.connect()?;
        let rows:Vec<(String,String,String,Option<String>,String)>=db.prepare("SELECT call_id,image,project,operation,state FROM ion_team_remote_calls WHERE attempt_id=? ORDER BY started_at,call_id LIMIT 33")?.query_map([attempt.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))?.collect::<rusqlite::Result<_>>()?;
        ensure!(rows.len() <= 32, "Too many remote calls for one attempt");
        rows.into_iter()
            .map(|(id, image, project, operation, state)| {
                Ok(RemoteCall {
                    id: Uuid::parse_str(&id)?,
                    image: Uuid::parse_str(&image)?,
                    project,
                    operation: operation.map(|id| Uuid::parse_str(&id)).transpose()?,
                    state,
                })
            })
            .collect()
    }
    pub(crate) fn remote_input(&self, call: Uuid) -> Result<Option<String>> {
        Ok(self
            .connect()?
            .query_row(
                "SELECT body FROM ion_team_remote_inputs WHERE call_id=?",
                [call.to_string()],
                |r| r.get(0),
            )
            .optional()?)
    }
    pub(crate) fn remote_attempt_stopped(&self, attempt: Uuid) -> Result<bool> {
        attempt_stopped(&self.connect()?, attempt)
    }
}

pub(super) fn attempt_stopped(db: &Connection, attempt: Uuid) -> Result<bool> {
    Ok(!db.query_row("SELECT EXISTS(SELECT 1 FROM ion_team_remote_calls WHERE attempt_id=? AND state NOT IN ('SUCCESS','FAILED','CANCELLED','rejected'))",[attempt.to_string()],|r|r.get::<_,bool>(0))?)
}
