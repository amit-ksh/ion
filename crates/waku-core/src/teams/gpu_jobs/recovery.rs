//! Private original-operation recovery. Public projections contain no inputs or credentials.
use super::*;
use waku_protocol::teams::{TeamGpuOperationPurpose, TeamGpuRecovery};

pub(in crate::teams) fn projection(
    db: &Connection,
    work: &TeamAssignment,
    attempt: Uuid,
) -> Result<Vec<TeamGpuRecovery>> {
    let rows:Vec<(String,String,Option<String>,Option<String>,Option<String>,bool,String,Option<String>)> = db.prepare(
        "SELECT j.request_id,j.project,j.job_id,j.creation_operation,j.cancellation_operation,j.cancel_requested,j.state,c.body FROM ion_team_gpu_jobs j LEFT JOIN ion_team_gpu_cleanup c ON c.request_id=j.request_id WHERE j.work_id=? AND j.attempt_id=? ORDER BY j.rowid LIMIT 129")?
        .query_map(params![work.id.to_string(),attempt.to_string()], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?)))?.collect::<rusqlite::Result<_>>()?;
    ensure!(rows.len() <= 128, "GPU recovery history exceeded its bound");
    rows.into_iter()
        .map(
            |(request, project, job, creation, cancellation, cancel_requested, state, cleanup)| {
                let cleanup: Option<GpuCleanupRecord> =
                    cleanup.map(|v| serde_json::from_str(&v)).transpose()?;
                Ok(TeamGpuRecovery {
                    request_id: Uuid::parse_str(&request)?,
                    project,
                    can_bind_creation: work.can_reconcile
                        && state == "submitting"
                        && job.is_none()
                        && creation.is_none(),
                    can_bind_cancellation: work.can_reconcile
                        && !matches!(state.as_str(), "settled" | "cleaned")
                        && job.is_some()
                        && creation.is_some()
                        && cancel_requested
                        && cancellation.is_none(),
                    can_bind_deletion: work.can_reconcile
                        && !matches!(state.as_str(), "settled" | "cleaned")
                        && cleanup
                            .as_ref()
                            .is_some_and(|c| c.state == "claimed" && c.operation.is_none()),
                    job_id: job,
                    creation_operation: creation,
                    cancellation_operation: cancellation,
                    deletion_operation: cleanup.as_ref().and_then(|c| c.operation.clone()),
                    cleanup_state: cleanup.map(|c| c.state),
                    cancel_requested,
                    state,
                })
            },
        )
        .collect()
}

/// Kept private between the trusted network check and its atomic database commit.
pub(crate) struct GpuRecoveryWitness {
    pub record: GpuJobRecord,
    pub key: Uuid,
    pub signature: String,
}
impl TeamCoordinator {
    pub(crate) fn gpu_recovery_witness(
        &self,
        run: Uuid,
        work: Uuid,
        attempt: Uuid,
        revision: u64,
        request: Uuid,
        purpose: TeamGpuOperationPurpose,
    ) -> Result<GpuRecoveryWitness> {
        let db = self.connect()?;
        let snapshot = load(&db, run)?;
        ensure!(
            snapshot.revision == revision,
            "Run changed; refresh before GPU recovery"
        );
        let assignment = snapshot
            .work
            .iter()
            .find(|w| w.id == work)
            .context("GPU recovery assignment missing")?;
        ensure!(
            assignment.can_reconcile
                && assignment.attempts.last().is_some_and(
                    |a| a.id == attempt && a.environment == ExecutionEnvironment::NebiusSandbox
                ),
            "GPU recovery attempt or state changed"
        );
        let (signature,key):(String,Option<String>)=db.query_row("SELECT signature,cancellation_key FROM ion_team_gpu_jobs WHERE request_id=? AND run_id=? AND work_id=? AND attempt_id=?",
            params![request.to_string(),run.to_string(),work.to_string(),attempt.to_string()],|r|Ok((r.get(0)?,r.get(1)?))).context("GPU request belongs to another assignment")?;
        let record = self
            .gpu_jobs(attempt)?
            .into_iter()
            .find(|r| r.request_id == request)
            .context("Original GPU record missing")?;
        let key = match purpose {
            TeamGpuOperationPurpose::Creation => {
                ensure!(
                    record.state == "submitting"
                        && record.creation_operation.is_none()
                        && record.job_id.is_none(),
                    "GPU creation already bound"
                );
                request
            }
            TeamGpuOperationPurpose::Cancellation => {
                ensure!(
                    !matches!(record.state.as_str(), "settled" | "cleaned")
                        && record.cancel_requested
                        && record.job_id.is_some()
                        && record.creation_operation.is_some()
                        && record.cancellation_operation.is_none(),
                    "GPU cancellation has no unbound original claim"
                );
                Uuid::parse_str(&key.context("GPU cancellation key missing")?)?
            }
            TeamGpuOperationPurpose::Deletion => {
                let cleanup = record
                    .cleanup
                    .as_ref()
                    .context("GPU deletion claim missing")?;
                ensure!(
                    !matches!(record.state.as_str(), "settled" | "cleaned")
                        && cleanup.state == "claimed"
                        && cleanup.operation.is_none(),
                    "GPU deletion already bound"
                );
                cleanup.key
            }
        };
        ensure!(!key.is_nil(), "GPU recovery key missing");
        Ok(GpuRecoveryWitness {
            record,
            key,
            signature,
        })
    }
    /// The trusted adapter has verified the typed request and exact idempotency key.
    /// The revision/signature/claim are rechecked here after network I/O.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn bind_recovered_gpu_operation(
        &self,
        run: Uuid,
        work: Uuid,
        attempt: Uuid,
        revision: u64,
        request: Uuid,
        purpose: TeamGpuOperationPurpose,
        operation: &str,
        job: &str,
        compute: &[String],
        witness: &GpuRecoveryWitness,
        now: u64,
    ) -> Result<()> {
        resource_id(operation)?;
        resource_id(job)?;
        ensure!(
            compute.len() <= 16
                && (purpose == TeamGpuOperationPurpose::Creation || compute.is_empty()),
            "GPU recovery compute witness invalid"
        );
        for id in compute {
            resource_id(id)?;
        }
        ensure!(operation != job, "GPU operation is not its resource");
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let snapshot = load(&tx, run)?;
        ensure!(
            snapshot.revision == revision,
            "Run changed during GPU verification; refresh"
        );
        let assignment = snapshot
            .work
            .iter()
            .find(|w| w.id == work)
            .context("GPU assignment missing")?;
        ensure!(
            assignment.can_reconcile
                && assignment.attempts.last().is_some_and(
                    |a| a.id == attempt && a.environment == ExecutionEnvironment::NebiusSandbox
                ),
            "GPU recovery host/attempt changed"
        );
        let (signature,create,cancel,saved_job,cancel_key,cancel_requested,state):(String,Option<String>,Option<String>,Option<String>,Option<String>,bool,String)=tx.query_row(
            "SELECT signature,creation_operation,cancellation_operation,job_id,cancellation_key,cancel_requested,state FROM ion_team_gpu_jobs WHERE request_id=? AND run_id=? AND work_id=? AND attempt_id=?",
            params![request.to_string(),run.to_string(),work.to_string(),attempt.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?)))?;
        ensure!(
            signature == witness.signature && request == witness.record.request_id,
            "GPU original request/policy changed"
        );
        match purpose {
            TeamGpuOperationPurpose::Creation => {
                ensure!(
                    state == "submitting"
                        && create.is_none()
                        && saved_job.is_none()
                        && witness.key == request,
                    "GPU create claim changed"
                );
                tx.execute("UPDATE ion_team_gpu_jobs SET creation_operation=?,job_id=?,state='running',compute_ids=? WHERE request_id=?",params![operation,job,serde_json::to_string(compute)?,request.to_string()])?;
            }
            TeamGpuOperationPurpose::Cancellation => {
                ensure!(
                    !matches!(state.as_str(), "settled" | "cleaned")
                        && saved_job.as_deref() == Some(job)
                        && create.as_deref() != Some(operation)
                        && cancel.is_none()
                        && cancel_requested
                        && cancel_key.as_deref() == Some(witness.key.to_string().as_str()),
                    "GPU cancellation claim changed"
                );
                tx.execute(
                    "UPDATE ion_team_gpu_jobs SET cancellation_operation=? WHERE request_id=?",
                    params![operation, request.to_string()],
                )?;
            }
            TeamGpuOperationPurpose::Deletion => {
                ensure!(
                    !matches!(state.as_str(), "settled" | "cleaned")
                        && saved_job.as_deref() == Some(job)
                        && create.as_deref() != Some(operation)
                        && cancel.as_deref() != Some(operation),
                    "GPU deletion reused a different resource/operation"
                );
                let body: String = tx.query_row(
                    "SELECT body FROM ion_team_gpu_cleanup WHERE request_id=? AND state='claimed'",
                    [request.to_string()],
                    |r| r.get(0),
                )?;
                let mut claim: GpuCleanupRecord = serde_json::from_str(&body)?;
                let saved = witness
                    .record
                    .cleanup
                    .as_ref()
                    .context("Original deletion claim missing")?;
                ensure!(
                    claim.operation.is_none()
                        && claim.key == witness.key
                        && claim.inventory == saved.inventory,
                    "GPU deletion claim/inventory changed"
                );
                claim.operation = Some(operation.into());
                claim.state = "acknowledged".into();
                tx.execute("UPDATE ion_team_gpu_cleanup SET body=?,state='acknowledged' WHERE request_id=?",params![serde_json::to_string(&claim)?,request.to_string()])?;
            }
        }
        tx.execute(
            "INSERT INTO ion_team_recovery_confirmations VALUES(?,?,?,?,?)",
            params![
                run.to_string(),
                work.to_string(),
                attempt.to_string(),
                revision,
                serde_json::to_string(&TeamRecoveryConfirmation::BindGpuOperation {
                    request_id: request,
                    purpose,
                    operation_id: operation.into()
                })?
            ],
        )?;
        audit(
            &tx,
            run,
            Some(work),
            "gpu_operation_recovered",
            &format!(
                "Verified original GPU {purpose:?} operation {operation} for {request}; no work submitted"
            ),
            now,
        )?;
        bump(&tx, run)?;
        tx.commit()?;
        Ok(())
    }
}
