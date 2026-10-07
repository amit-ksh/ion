//! Owner-approved result storage and atomic receipts; neither settles GPU costs.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GpuResultVolume {
    pub version: u16,
    pub bucket_id: String,
    pub bucket_name: String,
    pub region: String,
    pub prefix: String,
}
impl GpuResultVolume {
    pub(crate) fn validate(&self) -> Result<()> {
        resource_id(&self.bucket_id)?;
        ensure!(
            self.version == 1
                && (3..=63).contains(&self.bucket_name.len())
                && self
                    .bucket_name
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b".-".contains(&b))
                && self.bucket_name.as_bytes()[0].is_ascii_alphanumeric()
                && self
                    .bucket_name
                    .as_bytes()
                    .last()
                    .is_some_and(u8::is_ascii_alphanumeric)
                && !self.bucket_name.contains("..")
                && self.bucket_name.parse::<std::net::Ipv4Addr>().is_err(),
            "Invalid GPU result bucket name"
        );
        ensure!(
            !self.region.is_empty()
                && self.region.len() <= 40
                && self
                    .region
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'),
            "Invalid GPU result region"
        );
        jobs::path(&self.prefix)?;
        ensure!(
            self.prefix.len() <= 256
                && self
                    .prefix
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"/-_.".contains(&b)),
            "Invalid GPU result prefix"
        );
        Ok(())
    }
    pub(crate) fn host(&self) -> String {
        format!("storage.{}.nebius.cloud", self.region)
    }
    pub(crate) fn object_key(&self, request: Uuid) -> String {
        format!("{}/{request}/receipt.json", self.prefix)
    }
    pub(crate) fn mount(&self) -> Value {
        json!({"source":self.bucket_id,"sourcePath":self.prefix,"containerPath":"/ion/results","mode":"READ_WRITE"})
    }
}
impl TeamCoordinator {
    /// Called only by the trusted result transport. Late/Stop results still persist.
    /// An exact replay returns the original artifact without revision/audit churn.
    pub(crate) fn record_gpu_result(
        &self,
        request: Uuid,
        job: &str,
        receipt: &jobs::JobResponse,
        now: u64,
    ) -> Result<Uuid> {
        resource_id(job)?;
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (run_id, work_id, state, signature): (String,String,String,String) = tx.query_row(
            "SELECT run_id,work_id,state,signature FROM ion_team_gpu_jobs WHERE request_id=? AND job_id=?",
            params![request.to_string(),job], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
        ensure!(
            matches!(state.as_str(), "terminal" | "settled"),
            "GPU output needs the original terminal job observation"
        );
        let saved: Value = serde_json::from_str(&signature)?;
        let policy: GpuBudgetPolicy = serde_json::from_value(saved["policy"].clone())?;
        let volume = policy
            .result_volume
            .context("GPU result volume was not approved at launch")?;
        volume.validate()?;
        let original: jobs::JobRequest = serde_json::from_value(saved["request"].clone())?;
        ensure!(
            original.request_id == request,
            "GPU saved request identity changed"
        );
        receipt.validate_against(&original)?;
        let body = serde_json::to_string(receipt)?;
        ensure!(body.len() <= 512 * 1024, "GPU receipt exceeds its bound");
        let previous: Option<(String, String)> = tx
            .query_row(
                "SELECT receipt,artifact_id FROM ion_team_gpu_results WHERE request_id=?",
                [request.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((old, artifact)) = previous {
            ensure!(old == body, "GPU result replay changed");
            // Also validate the immutable document/provenance on replay below.
            let snapshot = load(&tx, Uuid::parse_str(&run_id)?)?;
            let work = snapshot
                .work
                .iter()
                .find(|w| w.id.to_string() == work_id)
                .context("GPU assignment missing")?;
            let document = documents::publish_in_transaction(
                &tx,
                &snapshot,
                work,
                request,
                &format!("gpu-command-{request}"),
                "json",
                &receipt.summary()?.to_string(),
                now,
            )?;
            ensure!(
                document.id.to_string() == artifact,
                "GPU result artifact changed"
            );
            tx.commit()?;
            return Ok(document.id);
        }
        ensure!(
            state == "terminal",
            "A settled GPU result cannot be recreated"
        );
        let run = load(&tx, Uuid::parse_str(&run_id)?)?;
        let work = run
            .work
            .iter()
            .find(|w| w.id.to_string() == work_id)
            .context("GPU assignment missing")?;
        let artifact = documents::publish_in_transaction(
            &tx,
            &run,
            work,
            request,
            &format!("gpu-command-{request}"),
            "json",
            &receipt.summary()?.to_string(),
            now,
        )?;
        tx.execute(
            "INSERT INTO ion_team_gpu_results(request_id,receipt,artifact_id) VALUES(?,?,?)",
            params![request.to_string(), body, artifact.id.to_string()],
        )?;
        audit(
            &tx,
            run.id,
            Some(work.id),
            "gpu_result_received",
            &format!(
                "GPU request {request}: execution receipt and artifact {} saved; spending remains reserved",
                artifact.id
            ),
            now,
        )?;
        tx.commit()?;
        Ok(artifact.id)
    }
    pub(crate) fn gpu_result(
        &self,
        lease: &AssignmentLease,
        request: Uuid,
        now: u64,
    ) -> Result<(jobs::JobResponse, Uuid)> {
        let db = self.connect()?;
        let run = load(&db, lease.run_id)?;
        active(&run, now)?;
        let work = checked_lease(&run, lease, now, false)?;
        ensure!(
            work.access.workspace_read,
            "GPU output needs workspace read access"
        );
        let (body,artifact): (String,String) = db.query_row(
            "SELECT g.receipt,g.artifact_id FROM ion_team_gpu_results g JOIN ion_team_gpu_jobs j ON j.request_id=g.request_id WHERE j.request_id=? AND j.run_id=? AND j.work_id=? AND j.attempt_id=?",
            params![request.to_string(),run.id.to_string(),work.id.to_string(),lease.attempt_id.to_string()], |r| Ok((r.get(0)?,r.get(1)?)))
            .context("GPU result is unavailable to this assignment attempt")?;
        Ok((serde_json::from_str(&body)?, Uuid::parse_str(&artifact)?))
    }
}
