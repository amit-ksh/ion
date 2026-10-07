//! Durable command jobs are separate from model-call budgets and receipts.
use super::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct JobFile {
    pub path: String,
    pub content_base64: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct JobRequest {
    pub version: u16,
    pub request_id: Uuid,
    pub argv: Vec<String>,
    pub timeout_seconds: u32,
    pub inputs: Vec<JobFile>,
    pub output_paths: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct JobStream {
    pub content_base64: String,
    pub bytes: u64,
    pub truncated: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct JobOutput {
    pub path: String,
    pub content_base64: String,
    pub sha256: String,
    pub bytes: u64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct JobOutputError {
    pub path: String,
    pub reason: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct JobResponse {
    pub version: u16,
    pub request_id: Uuid,
    pub exit_code: i32,
    pub timed_out: bool,
    pub canceled: bool,
    pub elapsed_milliseconds: u64,
    pub stdout: JobStream,
    pub stderr: JobStream,
    pub outputs: Vec<JobOutput>,
    pub output_errors: Vec<JobOutputError>,
}
impl JobResponse {
    fn preview(stream: &JobStream) -> Result<serde_json::Value> {
        use base64::{Engine, engine::general_purpose::STANDARD};
        let bytes = STANDARD.decode(&stream.content_base64)?;
        Ok(
            serde_json::json!({"preview":String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]),
            "bytes":stream.bytes,"truncated":stream.truncated || bytes.len()>1024}),
        )
    }
    pub(crate) fn summary(&self) -> Result<serde_json::Value> {
        Ok(
            serde_json::json!({"request_id":self.request_id,"exit_code":self.exit_code,
            "timed_out":self.timed_out,"canceled":self.canceled,"elapsed_milliseconds":self.elapsed_milliseconds,
            "stdout":Self::preview(&self.stdout)?,"stderr":Self::preview(&self.stderr)?,
            "outputs":self.outputs.iter().map(|o|serde_json::json!({"path":o.path,"bytes":o.bytes,"sha256":o.sha256})).collect::<Vec<_>>(),
            "output_errors":self.output_errors}),
        )
    }
    pub(crate) fn tool_reply(&self, artifact: Uuid) -> Result<String> {
        Ok(
            serde_json::json!({"artifact_id":artifact,"request_id":self.request_id,
            "exit_code":self.exit_code,"timed_out":self.timed_out,"canceled":self.canceled,
            "output_count":self.outputs.len(),"output_error_count":self.output_errors.len(),
            "stdout":Self::preview(&self.stdout)?,"stderr":Self::preview(&self.stderr)?})
            .to_string(),
        )
    }
}
pub(crate) struct JobRecord {
    pub id: Uuid,
    pub image: Uuid,
    pub project: String,
    pub operation: Option<Uuid>,
    pub state: String,
    pub request: JobRequest,
    pub policy: String,
}
impl TeamCoordinator {
    pub(crate) fn authorize_remote_job(
        &self,
        lease: &AssignmentLease,
        now: u64,
    ) -> Result<AgentProfile> {
        let db = self.connect()?;
        let run = load(&db, lease.run_id)?;
        active(&run, now)?;
        let work = checked_lease(&run, lease, now, false)?;
        ensure!(
            work.access.shell && work.access.workspace_read,
            "Remote commands require command and workspace read grants"
        );
        ensure!(
            work.attempts
                .last()
                .is_some_and(|a| a.environment == ExecutionEnvironment::NebiusSandbox),
            "Cloud commands cannot run in a Local or fallback attempt"
        );
        let agent_id = work.agent_id;
        run.profiles
            .into_iter()
            .find(|p| p.id == agent_id)
            .context("Pinned job profile missing")
    }
    pub(crate) fn reserve_remote_job(
        &self,
        lease: &AssignmentLease,
        request: &JobRequest,
        image: Uuid,
        project: &str,
        policy: &str,
        maximum: u16,
        now: u64,
    ) -> Result<Option<JobResponse>> {
        self.authorize_remote_job(lease, now)?;
        let signature =
            serde_json::json!({"request":request,"image":image,"project":project,"policy":policy})
                .to_string();
        ensure!(
            signature.len() <= 768 * 1024
                && !request.request_id.is_nil()
                && (1..=128).contains(&maximum),
            "Invalid job reservation"
        );
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = load(&tx, lease.run_id)?;
        active(&run, now)?;
        let work = checked_lease(&run, lease, now, false)?;
        let previous: Option<(String,String,Option<String>)> = tx.query_row(
            "SELECT signature,state,receipt FROM ion_team_jobs WHERE work_id=? AND request_id=?",
            params![work.id.to_string(),request.request_id.to_string()], |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
        if let Some((old, state, receipt)) = previous {
            ensure!(
                old == signature,
                "Remote job replay arguments or policy changed"
            );
            ensure!(
                state == "settled",
                "Remote job already submitted or outcome uncertain; observe the original operation instead of resubmitting"
            );
            return Ok(Some(serde_json::from_str(
                &receipt.context("Job receipt missing")?,
            )?));
        }
        // Registry edits cannot enlarge or replace an already used target policy.
        // Across mixed targets, the strictest declared run count remains the cap.
        let current: serde_json::Value = serde_json::from_str(policy)?;
        ensure!(
            current["max_commands"].as_u64() == Some(u64::from(maximum)),
            "Command policy/count disagree"
        );
        let witnesses: Vec<String> = tx
            .prepare("SELECT signature FROM ion_team_jobs WHERE run_id=? LIMIT 129")?
            .query_map([run.id.to_string()], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        ensure!(witnesses.len() <= 128, "Remote command count exceeded");
        let mut maximum = maximum;
        for witness in witnesses {
            let old: serde_json::Value = serde_json::from_str(&witness)?;
            let old_policy: serde_json::Value =
                serde_json::from_str(old["policy"].as_str().context("Stored policy missing")?)?;
            maximum = maximum.min(
                old_policy["max_commands"]
                    .as_u64()
                    .and_then(|v| u16::try_from(v).ok())
                    .context("Stored count missing")?,
            );
            if old_policy["target"] == current["target"] {
                ensure!(
                    old_policy == current
                        && old["image"] == serde_json::json!(image)
                        && old["project"] == project,
                    "Remote command policy changed during this run; use the original registered policy"
                );
            }
        }
        super::workspace::require_settled_effects(&tx, run.id)?;
        let count: u16 = tx.query_row(
            "SELECT count(*) FROM ion_team_jobs WHERE run_id=?",
            [run.id.to_string()],
            |r| r.get(0),
        )?;
        ensure!(count < maximum, "Remote command count limit reached");
        tx.execute("INSERT INTO ion_team_jobs(request_id,run_id,work_id,attempt_id,image,project,signature,request,state) VALUES(?,?,?,?,?,?,?,?,'submitting')",
            params![request.request_id.to_string(),run.id.to_string(),work.id.to_string(),lease.attempt_id.to_string(),image.to_string(),project,signature,serde_json::to_string(request)?])?;
        audit(
            &tx,
            run.id,
            Some(work.id),
            "remote_command_reserved",
            &format!(
                "Command request {} persisted before submission",
                request.request_id
            ),
            now,
        )?;
        bump(&tx, run.id)?;
        tx.commit()?;
        Ok(None)
    }
    pub(crate) fn bind_job_operation(&self, request: Uuid, operation: Uuid) -> Result<()> {
        ensure!(!operation.is_nil(), "Job operation identity is empty");
        ensure!(self.connect()?.execute("UPDATE ion_team_jobs SET operation=?,state='running' WHERE request_id=? AND state='submitting' AND operation IS NULL",
            params![operation.to_string(),request.to_string()])? == 1,"Remote job acknowledgement changed");
        Ok(())
    }
    pub(crate) fn job_canceling(&self, request: Uuid) -> Result<()> {
        self.connect()?.execute(
            "UPDATE ion_team_jobs SET state='canceling' WHERE request_id=? AND state='running'",
            [request.to_string()],
        )?;
        Ok(())
    }
    pub(crate) fn reject_job(&self, request: Uuid) -> Result<()> {
        ensure!(self.connect()?.execute("UPDATE ion_team_jobs SET state='rejected' WHERE request_id=? AND state='submitting' AND operation IS NULL",[request.to_string()])? == 1,"Job submission outcome already changed");
        Ok(())
    }
    /// Only the trusted transport calls this after verifying a terminal operation.
    /// No current lease is required: Stop/restart must still preserve a final receipt.
    pub(crate) fn finish_job(&self, request: Uuid, receipt: Option<&JobResponse>) -> Result<()> {
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (run, work, state): (String, String, String) = tx.query_row(
            "SELECT run_id,work_id,state FROM ion_team_jobs WHERE request_id=?",
            [request.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        ensure!(
            matches!(state.as_str(), "running" | "canceling"),
            "Remote job receipt already finalized or lacks an operation"
        );
        if let Some(receipt) = receipt {
            ensure!(
                receipt.request_id == request,
                "Job receipt identity changed"
            );
        }
        let body = receipt.map(serde_json::to_string).transpose()?;
        ensure!(
            body.as_ref().is_none_or(|s| s.len() <= 512 * 1024),
            "Job receipt exceeds its bound"
        );
        tx.execute(
            "UPDATE ion_team_jobs SET state=?,receipt=? WHERE request_id=?",
            params![
                if receipt.is_some() {
                    "settled"
                } else {
                    "failed"
                },
                body,
                request.to_string()
            ],
        )?;
        if let Some(receipt) = receipt {
            let snapshot = load(&tx, Uuid::parse_str(&run)?)?;
            let assignment = snapshot
                .work
                .iter()
                .find(|w| w.id.to_string() == work)
                .context("Job assignment missing")?;
            super::documents::publish_in_transaction(
                &tx,
                &snapshot,
                assignment,
                request,
                &format!("command-{request}"),
                "json",
                &receipt.summary()?.to_string(),
                crate::model::unix_time(),
            )?;
        }
        audit(
            &tx,
            Uuid::parse_str(&run)?,
            Some(Uuid::parse_str(&work)?),
            "remote_command_stopped",
            &request.to_string(),
            crate::model::unix_time(),
        )?;
        bump(&tx, Uuid::parse_str(&run)?)?;
        tx.commit()?;
        Ok(())
    }
    pub(crate) fn unfinished_jobs(&self, attempt: Uuid) -> Result<Vec<JobRecord>> {
        let db = self.connect()?;
        let rows = db.prepare("SELECT request_id,image,project,operation,state,request,json_extract(signature,'$.policy') FROM ion_team_jobs WHERE attempt_id=? AND state IN ('submitting','running','canceling') LIMIT 129")?
            .query_map([attempt.to_string()],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,Option<String>>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?,r.get::<_,String>(6)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        ensure!(rows.len() <= 128, "Remote job limit exceeded");
        rows.into_iter()
            .map(|(id, image, project, operation, state, request, policy)| {
                Ok(JobRecord {
                    id: Uuid::parse_str(&id)?,
                    image: Uuid::parse_str(&image)?,
                    project,
                    operation: operation.map(|s| Uuid::parse_str(&s)).transpose()?,
                    state,
                    request: serde_json::from_str(&request)?,
                    policy,
                })
            })
            .collect()
    }
    pub(crate) fn bind_recovered_job_operation(
        &self,
        run_id: Uuid,
        work_id: Uuid,
        attempt_id: Uuid,
        revision: u64,
        request: Uuid,
        operation: Uuid,
        now: u64,
    ) -> Result<()> {
        ensure!(!operation.is_nil(), "Command operation identity is empty");
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = load(&tx, run_id)?;
        ensure!(
            run.revision == revision,
            "Run changed; refresh command recovery"
        );
        let work = run
            .work
            .iter()
            .find(|w| w.id == work_id)
            .context("Command assignment missing")?;
        ensure!(
            work.can_reconcile
                && work
                    .attempts
                    .last()
                    .is_some_and(|a| a.id == attempt_id
                        && a.environment == ExecutionEnvironment::NebiusSandbox),
            "Command recovery attempt/state changed"
        );
        ensure!(tx.execute("UPDATE ion_team_jobs SET operation=?,state='running' WHERE request_id=? AND run_id=? AND work_id=? AND attempt_id=? AND state='submitting' AND operation IS NULL",
            params![operation.to_string(),request.to_string(),run_id.to_string(),work_id.to_string(),attempt_id.to_string()])?==1,"Command request was changed or already bound");
        tx.execute(
            "INSERT INTO ion_team_recovery_confirmations VALUES(?,?,?,?,?)",
            params![
                run_id.to_string(),
                work_id.to_string(),
                attempt_id.to_string(),
                revision,
                serde_json::to_string(&TeamRecoveryConfirmation::BindRemoteOperation {
                    request_id: request,
                    operation_id: operation
                })?
            ],
        )?;
        audit(
            &tx,
            run_id,
            Some(work_id),
            "command_operation_recovered",
            &format!(
                "Verified original command {request} against operation {operation}; no submission"
            ),
            now,
        )?;
        bump(&tx, run_id)?;
        tx.commit()?;
        Ok(())
    }
    pub(crate) fn job_receipt(
        &self,
        lease: &AssignmentLease,
        request: Uuid,
        now: u64,
    ) -> Result<JobResponse> {
        let db = self.connect()?;
        let run = load(&db, lease.run_id)?;
        active(&run, now)?;
        checked_lease(&run, lease, now, false)?;
        let body:String=db.query_row("SELECT receipt FROM ion_team_jobs WHERE request_id=? AND work_id=? AND run_id=? AND state='settled'",
            params![request.to_string(),lease.work_id.to_string(),lease.run_id.to_string()],|r|r.get(0)).context("No completed command receipt belongs to this assignment")?;
        Ok(serde_json::from_str(&body)?)
    }
}

pub(crate) fn recover_receipt(
    db: &Connection,
    run: &TeamRunSnapshot,
    work: &TeamAssignment,
    attempt: Uuid,
    request: Uuid,
    args: &serde_json::Value,
) -> Result<Option<String>> {
    use serde_json::json;

    let row:Option<(String,String,Option<String>)>=db.query_row(
                "SELECT request,state,receipt FROM ion_team_jobs WHERE run_id=? AND work_id=? AND attempt_id=? AND request_id=?",
                params![run.id.to_string(),work.id.to_string(),attempt.to_string(),request.to_string()],
                |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
    if let Some((saved, state, receipt)) = row {
        let saved: JobRequest = serde_json::from_str(&saved)?;
        let expected = json!({"argv":saved.argv,"input_paths":saved.inputs.iter().map(|f|&f.path).collect::<Vec<_>>(),
                    "output_paths":saved.output_paths,"timeout_seconds":saved.timeout_seconds});
        ensure!(
            *args == expected,
            "Pending command arguments differ from its durable reservation"
        );
        ensure!(
            matches!(state.as_str(), "settled" | "failed" | "rejected"),
            "Command outcome remains uncertain"
        );
        if state == "settled" {
            let receipt: JobResponse =
                serde_json::from_str(&receipt.context("Command receipt missing")?)?;
            let id:String=db.query_row("SELECT artifact_id FROM ion_team_document_receipts WHERE work_id=? AND request_id=?",
                        params![work.id.to_string(),request.to_string()],|r|r.get(0))?;
            let body: String = db.query_row(
                "SELECT body FROM ion_control_records WHERE id=? AND kind='artifact'",
                [&id],
                |r| r.get(0),
            )?;
            let ControlRecord::Artifact(artifact) = serde_json::from_str(&body)? else {
                anyhow::bail!("Recovered command document changed")
            };
            ensure!(
                artifact.task_id == Some(work.task_id)
                    && artifact.project_id == Some(run.project_id),
                "Recovered command document provenance changed"
            );
            return Ok(Some(receipt.tool_reply(Uuid::parse_str(&id)?)?));
        }
        return Ok(Some(json!({"request_id":request,"outcome":state,"replayed":false,
                    "note":"The original worker stopped without a usable command receipt, or launch was rejected. Recovery did not repeat the command."}).to_string()));
    }
    Ok(None)
}
