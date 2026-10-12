//! Durable command jobs are separate from model-call budgets and receipts.
pub(crate) mod admission;
use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_snapshot: Option<super::project::ProjectSnapshot>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_sha256: Option<String>,
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
    pub(crate) fn validate_against(&self, request: &JobRequest) -> Result<()> {
        ensure!(
            self.version == 1
                && self.project_sha256.as_deref()
                    == request.project_snapshot.as_ref().map(|p| p.sha256.as_str())
                && self.request_id == request.request_id
                && self.elapsed_milliseconds <= (u64::from(request.timeout_seconds) + 10) * 1000
                && self.outputs.len() + self.output_errors.len() == request.output_paths.len(),
            "Remote command receipt identity or output contract changed"
        );
        for stream in [&self.stdout, &self.stderr] {
            ensure!(
                stream.content_base64.len() <= 12 * 1024,
                "Job stream exceeds its bound"
            );
            let bytes = STANDARD.decode(&stream.content_base64)?;
            ensure!(
                bytes.len() <= 8192
                    && stream.bytes >= bytes.len() as u64
                    && stream.truncated == (stream.bytes > 8192)
                    && (stream.truncated || stream.bytes == bytes.len() as u64),
                "Job stream receipt is inconsistent"
            );
        }
        let mut seen = HashSet::new();
        let mut total = 0;
        for output in &self.outputs {
            path(&output.path)?;
            ensure!(
                request.output_paths.contains(&output.path)
                    && seen.insert(&output.path)
                    && output.content_base64.len() <= 88 * 1024
                    && output.sha256.len() == 64
                    && output.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
                "Unexpected job output"
            );
            let bytes = STANDARD.decode(&output.content_base64)?;
            ensure!(
                bytes.len() <= 64 * 1024
                    && bytes.len() as u64 == output.bytes
                    && format!("{:x}", Sha256::digest(&bytes)) == output.sha256,
                "Job file receipt is inconsistent"
            );
            total += bytes.len();
        }
        ensure!(total <= 256 * 1024, "Job output transfer limit exceeded");
        for error in &self.output_errors {
            ensure!(
                request.output_paths.contains(&error.path)
                    && seen.insert(&error.path)
                    && error.reason.len() <= 256,
                "Unexpected output failure"
            );
        }
        Ok(())
    }
    fn preview(stream: &JobStream) -> Result<serde_json::Value> {
        use base64::{Engine, engine::general_purpose::STANDARD};
        let bytes = STANDARD.decode(&stream.content_base64)?;
        Ok(
            serde_json::json!({"preview":String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]),
            "bytes":stream.bytes,"truncated":stream.truncated || bytes.len()>1024}),
        )
    }
    pub(crate) fn summary(&self) -> Result<serde_json::Value> {
        let mut value = serde_json::json!({"request_id":self.request_id,"exit_code":self.exit_code,
            "timed_out":self.timed_out,"canceled":self.canceled,"elapsed_milliseconds":self.elapsed_milliseconds,
            "stdout":Self::preview(&self.stdout)?,"stderr":Self::preview(&self.stderr)?,
            "outputs":self.outputs.iter().map(|o|serde_json::json!({"path":o.path,"bytes":o.bytes,"sha256":o.sha256})).collect::<Vec<_>>(),
            "output_errors":self.output_errors});
        if let Some(hash) = &self.project_sha256 {
            value["project_sha256"] = serde_json::json!(hash);
        }
        Ok(value)
    }
    pub(crate) fn tool_reply(&self, artifact: Uuid) -> Result<String> {
        let mut value = serde_json::json!({"artifact_id":artifact,"request_id":self.request_id,
            "exit_code":self.exit_code,"timed_out":self.timed_out,"canceled":self.canceled,
            "output_count":self.outputs.len(),"output_error_count":self.output_errors.len(),
            "stdout":Self::preview(&self.stdout)?,"stderr":Self::preview(&self.stderr)?});
        if let Some(hash) = &self.project_sha256 {
            value["project_sha256"] = serde_json::json!(hash);
        }
        Ok(value.to_string())
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
    #[cfg(test)]
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
        self.reserve_remote_job_with_context(
            lease, request, image, project, policy, maximum, now, None,
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn reserve_remote_job_with_context(
        &self,
        lease: &AssignmentLease,
        request: &JobRequest,
        image: Uuid,
        project: &str,
        policy: &str,
        maximum: u16,
        now: u64,
        check_context: Option<&super::checks::CommandCheckContext>,
    ) -> Result<Option<JobResponse>> {
        ensure!(
            request.project_snapshot.is_none(),
            "Use the project reservation path"
        );
        self.reserve_job_inner(
            lease,
            request,
            image,
            project,
            policy,
            maximum,
            now,
            check_context,
            None,
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn reserve_remote_project(
        &self,
        lease: &AssignmentLease,
        request: &JobRequest,
        image: Uuid,
        project: &str,
        policy: &str,
        maximum: u16,
        now: u64,
        check_context: &super::checks::CommandCheckContext,
        archive: &[u8],
    ) -> Result<Option<JobResponse>> {
        super::project::matches(
            archive,
            request
                .project_snapshot
                .as_ref()
                .context("Project snapshot missing")?,
        )?;
        self.reserve_job_inner(
            lease,
            request,
            image,
            project,
            policy,
            maximum,
            now,
            Some(check_context),
            Some(archive),
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn reserve_job_inner(
        &self,
        lease: &AssignmentLease,
        request: &JobRequest,
        image: Uuid,
        project: &str,
        policy: &str,
        maximum: u16,
        now: u64,
        check_context: Option<&super::checks::CommandCheckContext>,
        archive: Option<&[u8]>,
    ) -> Result<Option<JobResponse>> {
        self.authorize_remote_job(lease, now)?;
        let mut signature =
            serde_json::json!({"request":request,"image":image,"project":project,"policy":policy});
        if let Some(context) = check_context {
            signature["check_context"] = serde_json::to_value(context)?;
        }
        let signature = signature.to_string();
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
        if request.project_snapshot.is_some() {
            ensure!(
                archive.is_some()
                    && check_context == Some(&super::project::verify_source(&tx, &run, work)?),
                "Project source changed before reservation"
            );
        } else {
            super::checks::verify_context(&tx, &run, work, check_context)?;
        }
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
        let mut project_max = current["project_snapshot"]["max_run_bytes"]
            .as_u64()
            .unwrap_or(u64::MAX);
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
            if let Some(limit) = old_policy["project_snapshot"]["max_run_bytes"].as_u64() {
                project_max = project_max.min(limit);
            }
            if old_policy["target"] == current["target"] {
                ensure!(
                    old_policy == current
                        && old["image"] == serde_json::json!(image)
                        && old["project"] == project,
                    "Remote command policy changed during this run; use the original registered policy"
                );
            }
        }
        if let Some(archive) = archive {
            let approved: super::project::ProjectPolicy =
                serde_json::from_value(current["project_snapshot"].clone())?;
            approved.validate()?;
            let facts = request
                .project_snapshot
                .as_ref()
                .context("Project snapshot missing")?;
            ensure!(
                facts.files <= approved.max_files
                    && facts.source_bytes <= approved.max_bytes
                    && request.inputs.is_empty(),
                "Project snapshot exceeds approval or mixes file modes"
            );
            let used:u64=tx.query_row("SELECT coalesce(sum(length(p.archive)),0) FROM ion_team_job_projects p JOIN ion_team_jobs j ON j.request_id=p.request_id WHERE j.run_id=?",[run.id.to_string()],|r|r.get(0))?;
            ensure!(
                used + archive.len() as u64 <= project_max,
                "Project run transfer limit reached"
            );
        }
        super::workspace::require_settled_effects(&tx, run.id)?;
        let count: u16 = tx.query_row(
            "SELECT count(*) FROM ion_team_jobs WHERE run_id=?",
            [run.id.to_string()],
            |r| r.get(0),
        )?;
        ensure!(count < maximum, "Remote command count limit reached");
        tx.execute("INSERT INTO ion_team_jobs(request_id,run_id,work_id,attempt_id,image,project,signature,request,state) VALUES(?,?,?,?,?,?,?,?,?)",
            params![request.request_id.to_string(),run.id.to_string(),work.id.to_string(),lease.attempt_id.to_string(),image.to_string(),project,signature,serde_json::to_string(request)?,if archive.is_some(){"staging"}else{"submitting"}])?;
        admission::reserve(
            &tx,
            request.request_id,
            run.project_id,
            &current,
            archive.map_or(0, |a| a.len() as u64),
        )?;
        if let Some(archive) = archive {
            tx.execute(
                "INSERT INTO ion_team_job_projects(request_id,archive) VALUES(?,?)",
                params![request.request_id.to_string(), archive],
            )?;
        }
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
    pub(crate) fn confirm_project_upload(
        &self,
        lease: &AssignmentLease,
        request: Uuid,
        file: Uuid,
        now: u64,
    ) -> Result<()> {
        ensure!(!file.is_nil(), "Project file identity is empty");
        self.authorize_remote_job(lease, now)?;
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = load(&tx, lease.run_id)?;
        active(&run, now)?;
        let work = checked_lease(&run, lease, now, false)?;
        let signature:String=tx.query_row("SELECT signature FROM ion_team_jobs WHERE request_id=? AND work_id=? AND attempt_id=? AND state='staging' AND operation IS NULL",
            params![request.to_string(),work.id.to_string(),lease.attempt_id.to_string()],|r|r.get(0))?;
        let saved: serde_json::Value = serde_json::from_str(&signature)?;
        let context: super::checks::CommandCheckContext =
            serde_json::from_value(saved["check_context"].clone())?;
        ensure!(
            context == super::project::verify_source(&tx, &run, work)?,
            "Project source changed before submission"
        );
        ensure!(tx.execute("UPDATE ion_team_job_projects SET file_uuid=? WHERE request_id=? AND file_uuid IS NULL",params![file.to_string(),request.to_string()])?==1,"Project upload already confirmed");
        ensure!(tx.execute("UPDATE ion_team_jobs SET state='submitting' WHERE request_id=? AND state='staging'",[request.to_string()])?==1,"Project state changed");
        tx.commit()?;
        Ok(())
    }
    pub(crate) fn project_file(&self, request: Uuid) -> Result<Uuid> {
        let file: String = self.connect()?.query_row(
            "SELECT file_uuid FROM ion_team_job_projects WHERE request_id=?",
            [request.to_string()],
            |r| r.get(0),
        )?;
        let id = Uuid::parse_str(&file)?;
        ensure!(!id.is_nil(), "Project file identity missing");
        Ok(id)
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
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure!(tx.execute("UPDATE ion_team_jobs SET state='rejected' WHERE request_id=? AND state IN ('staging','submitting') AND operation IS NULL",[request.to_string()])? == 1,"Job submission outcome already changed");
        admission::reject(&tx, request)?;
        tx.commit()?;
        Ok(())
    }
    /// Only the trusted transport calls this after verifying a terminal operation.
    /// No current lease is required: Stop/restart must still preserve a final receipt.
    pub(crate) fn finish_job(&self, request: Uuid, receipt: Option<&JobResponse>) -> Result<()> {
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (run, work, state, saved_request): (String, String, String, String) = tx.query_row(
            "SELECT run_id,work_id,state,request FROM ion_team_jobs WHERE request_id=?",
            [request.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )?;
        ensure!(
            matches!(state.as_str(), "running" | "canceling"),
            "Remote job receipt already finalized or lacks an operation"
        );
        if let Some(receipt) = receipt {
            receipt.validate_against(&serde_json::from_str(&saved_request)?)?;
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
        if receipt.is_some() {
            admission::release_compute(&tx, request)?;
        }
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
        let rows = db.prepare("SELECT request_id,image,project,operation,state,request,json_extract(signature,'$.policy') FROM ion_team_jobs WHERE attempt_id=? AND state IN ('staging','submitting','running','canceling') LIMIT 129")?
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
        let expected = if saved.project_snapshot.is_some() {
            json!({"argv":saved.argv,"output_paths":saved.output_paths,"timeout_seconds":saved.timeout_seconds})
        } else {
            json!({"argv":saved.argv,"input_paths":saved.inputs.iter().map(|f|&f.path).collect::<Vec<_>>(),"output_paths":saved.output_paths,"timeout_seconds":saved.timeout_seconds})
        };
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

pub(crate) fn path(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty() && value.len() <= 512,
        "Job path is invalid"
    );
    for p in value.split('/') {
        let lower = p.to_ascii_lowercase();
        ensure!(
            !p.is_empty()
                && !matches!(p, "." | "..")
                && !p.contains(['\\', ':'])
                && !p.ends_with(['.', ' '])
                && !p.chars().any(char::is_control)
                && !matches!(
                    lower.as_str(),
                    ".git"
                        | ".ssh"
                        | ".aws"
                        | ".codex"
                        | ".agents"
                        | ".env"
                        | ".npmrc"
                        | ".pypirc"
                        | "credentials.json"
                        | "secrets.json"
                        | "node_modules"
                        | "target"
                        | ".waku-cache"
                )
                && !lower.starts_with(".env.")
                && !lower.starts_with(".ion-edit-")
                && !lower.ends_with(".pem")
                && !lower.ends_with(".p12")
                && !lower.ends_with(".pfx"),
            "Job path is private or non-relative"
        );
    }
    Ok(())
}
