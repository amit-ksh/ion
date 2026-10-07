//! Persistent GPU resource inventory and cleanup claims, independent of output/billing receipts.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GpuDiskInventory {
    pub id: String,
    pub managed: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GpuComputeInventory {
    pub compute_id: String,
    pub disks: Vec<GpuDiskInventory>,
    pub complete: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GpuCleanupRecord {
    pub key: Uuid,
    pub operation: Option<String>,
    pub state: String,
    pub inventory: Vec<GpuComputeInventory>,
}
impl TeamCoordinator {
    pub(crate) fn gpu_inventory(&self, request: Uuid) -> Result<Vec<GpuComputeInventory>> {
        let db = self.connect()?;
        let bodies: Vec<String>=db.prepare("SELECT body FROM ion_team_gpu_inventory WHERE request_id=? ORDER BY compute_id LIMIT 17")?
            .query_map([request.to_string()],|r|r.get(0))?.collect::<rusqlite::Result<_>>()?;
        ensure!(bodies.len() <= 16, "GPU inventory count exceeded");
        bodies
            .into_iter()
            .map(|body| Ok(serde_json::from_str(&body)?))
            .collect()
    }
    pub(crate) fn record_gpu_inventory(
        &self,
        request: Uuid,
        job: &str,
        inventory: &GpuComputeInventory,
        now: u64,
    ) -> Result<()> {
        resource_id(&inventory.compute_id)?;
        ensure!(
            inventory.disks.len() <= 17,
            "GPU disk inventory exceeded its bound"
        );
        let mut seen = HashSet::new();
        for disk in &inventory.disks {
            resource_id(&disk.id)?;
            ensure!(seen.insert(&disk.id), "Duplicate GPU disk identity");
        }
        let body = serde_json::to_string(inventory)?;
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (run,work,ids):(String,String,String)=tx.query_row("SELECT run_id,work_id,compute_ids FROM ion_team_gpu_jobs WHERE request_id=? AND job_id=?",
            params![request.to_string(),job],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
        ensure!(
            serde_json::from_str::<Vec<String>>(&ids)?.contains(&inventory.compute_id),
            "GPU inventory is not attached to this job"
        );
        let previous: Option<String> = tx
            .query_row(
                "SELECT body FROM ion_team_gpu_inventory WHERE request_id=? AND compute_id=?",
                params![request.to_string(), inventory.compute_id],
                |r| r.get(0),
            )
            .optional()?;
        if previous.as_deref() == Some(&body) {
            return Ok(());
        }
        if let Some(previous) = previous {
            let previous: GpuComputeInventory = serde_json::from_str(&previous)?;
            ensure!(
                !previous.complete || previous == *inventory,
                "GPU disk inventory changed after its complete witness; retain reservations for review"
            );
        }
        ensure!(
            !tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM ion_team_gpu_cleanup WHERE request_id=?)",
                [request.to_string()],
                |r| r.get::<_, bool>(0)
            )?,
            "GPU inventory is frozen for cleanup"
        );
        tx.execute("INSERT INTO ion_team_gpu_inventory(request_id,compute_id,body) VALUES(?,?,?) ON CONFLICT(request_id,compute_id) DO UPDATE SET body=excluded.body",
            params![request.to_string(),inventory.compute_id,body])?;
        audit(
            &tx,
            Uuid::parse_str(&run)?,
            Some(Uuid::parse_str(&work)?),
            "gpu_inventory",
            &format!(
                "GPU request {request}: resource inventory captured for {}",
                inventory.compute_id
            ),
            now,
        )?;
        bump(&tx, Uuid::parse_str(&run)?)?;
        tx.commit()?;
        Ok(())
    }
    /// Freeze all captured identities before the single DELETE. Missing startup/disk
    /// witnesses cannot be promoted into cleanup proof even when the job is terminal.
    pub(crate) fn claim_gpu_deletion(
        &self,
        request: Uuid,
        job: &str,
        now: u64,
    ) -> Result<Option<GpuCleanupRecord>> {
        resource_id(job)?;
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (run,work,state,ids):(String,String,String,String)=tx.query_row("SELECT run_id,work_id,state,compute_ids FROM ion_team_gpu_jobs WHERE request_id=? AND job_id=?",
            params![request.to_string(),job],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
        ensure!(
            state == "terminal",
            "Observe a terminal GPU job before deleting it"
        );
        if tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM ion_team_gpu_cleanup WHERE request_id=?)",
            [request.to_string()],
            |r| r.get::<_, bool>(0),
        )? {
            return Ok(None);
        }
        let ids: Vec<String> = serde_json::from_str(&ids)?;
        let inventory:Vec<GpuComputeInventory>=tx.prepare("SELECT body FROM ion_team_gpu_inventory WHERE request_id=? ORDER BY compute_id LIMIT 17")?
            .query_map([request.to_string()],|r|r.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?.into_iter()
            .map(|s|Ok(serde_json::from_str(&s)?)).collect::<Result<_>>()?;
        ensure!(
            !ids.is_empty()
                && inventory.len() == ids.len()
                && inventory
                    .iter()
                    .all(|i| i.complete && ids.contains(&i.compute_id)),
            "GPU resource inventory is incomplete; retain reservations and recover original resources"
        );
        let claim = GpuCleanupRecord {
            key: Uuid::new_v4(),
            operation: None,
            state: "claimed".into(),
            inventory,
        };
        tx.execute(
            "INSERT INTO ion_team_gpu_cleanup(request_id,body,state) VALUES(?,?,'claimed')",
            params![request.to_string(), serde_json::to_string(&claim)?],
        )?;
        audit(
            &tx,
            Uuid::parse_str(&run)?,
            Some(Uuid::parse_str(&work)?),
            "gpu_delete_requested",
            &format!("Deletion of original GPU job {job} persisted before DELETE"),
            now,
        )?;
        bump(&tx, Uuid::parse_str(&run)?)?;
        tx.commit()?;
        Ok(Some(claim))
    }
    pub(crate) fn bind_gpu_deletion(
        &self,
        request: Uuid,
        job: &str,
        operation: &str,
        now: u64,
    ) -> Result<()> {
        resource_id(operation)?;
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (run,work,create,cancel,body):(String,String,String,Option<String>,String)=tx.query_row(
            "SELECT j.run_id,j.work_id,j.creation_operation,j.cancellation_operation,c.body FROM ion_team_gpu_jobs j JOIN ion_team_gpu_cleanup c ON c.request_id=j.request_id WHERE j.request_id=? AND j.job_id=? AND c.state='claimed'",
            params![request.to_string(),job],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))?;
        ensure!(
            operation != job && operation != create && cancel.as_deref() != Some(operation),
            "GPU deletion reused another operation/resource identity"
        );
        let mut claim: GpuCleanupRecord = serde_json::from_str(&body)?;
        ensure!(
            claim.operation.is_none(),
            "GPU deletion acknowledgement already bound"
        );
        claim.operation = Some(operation.into());
        claim.state = "acknowledged".into();
        tx.execute(
            "UPDATE ion_team_gpu_cleanup SET body=?,state='acknowledged' WHERE request_id=?",
            params![serde_json::to_string(&claim)?, request.to_string()],
        )?;
        audit(
            &tx,
            Uuid::parse_str(&run)?,
            Some(Uuid::parse_str(&work)?),
            "gpu_delete_acknowledged",
            &format!("GPU request {request}: deletion operation {operation} bound"),
            now,
        )?;
        bump(&tx, Uuid::parse_str(&run)?)?;
        tx.commit()?;
        Ok(())
    }
    /// Trusted transport has verified the typed completed delete and absence of
    /// the job, every captured VM and every captured disk. Money and job results
    /// remain pending independently; this releases compute capacity only.
    pub(crate) fn verify_gpu_cleanup(
        &self,
        request: Uuid,
        operation: &str,
        proof: &GpuCleanupRecord,
        now: u64,
    ) -> Result<()> {
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (run,work,ids,body,state):(String,String,String,String,String)=tx.query_row(
            "SELECT j.run_id,j.work_id,j.compute_ids,c.body,c.state FROM ion_team_gpu_jobs j JOIN ion_team_gpu_cleanup c ON c.request_id=j.request_id WHERE j.request_id=?",
            [request.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))?;
        let mut claim: GpuCleanupRecord = serde_json::from_str(&body)?;
        ensure!(
            claim.operation.as_deref() == Some(operation)
                && proof.operation.as_deref() == Some(operation)
                && claim.key == proof.key
                && claim.inventory == proof.inventory,
            "GPU cleanup witness changed"
        );
        let ids: Vec<String> = serde_json::from_str(&ids)?;
        ensure!(
            ids.len() == claim.inventory.len()
                && claim
                    .inventory
                    .iter()
                    .all(|i| i.complete && ids.contains(&i.compute_id)),
            "GPU resources changed after deletion claim"
        );
        if state == "verified" {
            return Ok(());
        }
        ensure!(
            state == "acknowledged",
            "GPU deletion acknowledgement is unresolved"
        );
        claim.state = "verified".into();
        tx.execute(
            "UPDATE ion_team_gpu_cleanup SET body=?,state='verified' WHERE request_id=?",
            params![serde_json::to_string(&claim)?, request.to_string()],
        )?;
        audit(
            &tx,
            Uuid::parse_str(&run)?,
            Some(Uuid::parse_str(&work)?),
            "gpu_cleanup_verified",
            &format!(
                "GPU request {request}: captured compute/disks absent; spending and output reservations retained"
            ),
            now,
        )?;
        bump(&tx, Uuid::parse_str(&run)?)?;
        tx.commit()?;
        Ok(())
    }
}
