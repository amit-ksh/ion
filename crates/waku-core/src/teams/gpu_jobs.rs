//! Owner-side GPU launch reservations. Service state never releases money or proves cleanup.
use super::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
mod maintenance;
mod price_policy;
pub(crate) use price_policy::GpuPricePolicy;
#[cfg(test)]
pub(crate) use price_policy::{GpuEstimateMethod, GpuPriceComponent, GpuPriceSku};
mod resource_policy;
#[cfg(test)]
pub(crate) use resource_policy::{
    GpuChildNetworkPolicy, GpuQuotaPolicy, GpuQuotaScope, GpuResourceVector,
};
pub(crate) use resource_policy::{
    GpuGroupPolicy, GpuQuotaKind, GpuResourcePolicy, GpuRuntimePolicy,
};
pub(in crate::teams) mod recovery;
mod resources;
mod results;
pub(super) mod waits;
pub(crate) use recovery::GpuRecoveryWitness;
pub(crate) use resources::{GpuCleanupRecord, GpuComputeInventory, GpuDiskInventory};
pub(crate) use results::GpuResultVolume;

/// Owner-approved, immutable spending allowance. These are reservations, not invoice totals.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GpuBudgetPolicy {
    pub version: u16,
    pub target: String,
    pub currency: String,
    pub quote_id: String,
    pub valid_until: u64,
    pub reservation_micros: u64,
    pub run_budget_micros: u64,
    pub max_jobs: u16,
    pub max_parallel: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_volume: Option<GpuResultVolume>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<GpuResourcePolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pricing: Option<GpuPricePolicy>,
}
impl GpuBudgetPolicy {
    fn validate(&self, now: u64) -> Result<()> {
        ensure!(
            self.version == 1 && !self.target.is_empty() && self.target.len() <= 160,
            "Invalid GPU target policy"
        );
        ensure!(
            self.currency.len() == 3
                && self.currency.bytes().all(|b| b.is_ascii_uppercase())
                && !self.quote_id.is_empty()
                && self.quote_id.len() <= 160
                && self
                    .quote_id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b)),
            "GPU currency/quote identity missing"
        );
        ensure!(
            now < self.valid_until
                && self.valid_until <= now.saturating_add(86400)
                && (1..=1_000_000_000_000).contains(&self.run_budget_micros)
                && (1..=self.run_budget_micros).contains(&self.reservation_micros)
                && (1..=128).contains(&self.max_jobs)
                && (1..=16).contains(&self.max_parallel),
            "GPU spending policy expired or exceeds its bound"
        );
        if let Some(pricing) = &self.pricing {
            pricing.validate()?;
        }
        if let Some(resources) = &self.resources {
            resources.validate()?;
        }
        if let Some(volume) = &self.result_volume {
            volume.validate()?;
        }
        Ok(())
    }
}
#[derive(Clone)]
pub(crate) struct GpuJobRecord {
    pub request_id: Uuid,
    pub project: String,
    pub body: Value,
    pub creation_operation: Option<String>,
    pub job_id: Option<String>,
    pub cancellation_operation: Option<String>,
    pub cancel_requested: bool,
    pub state: String,
    pub compute_ids: Vec<String>,
    pub deadline_at: u64,
    pub cleanup: Option<GpuCleanupRecord>,
    pub result_volume: Option<GpuResultVolume>,
    pub result_receipt: Option<jobs::JobResponse>,
    pub result_artifact: Option<Uuid>,
}
pub(crate) fn resource_id(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= 160
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b)),
        "Invalid GPU resource identity"
    );
    Ok(())
}
impl TeamCoordinator {
    pub(crate) fn authorize_gpu_launch(
        &self,
        lease: &AssignmentLease,
        policy: &GpuBudgetPolicy,
        now: u64,
    ) -> Result<()> {
        policy.validate(now)?;
        let db = self.connect()?;
        let run = load(&db, lease.run_id)?;
        active(&run, now)?;
        let work = checked_lease(&run, lease, now, false)?;
        let profile = run
            .profiles
            .iter()
            .find(|p| p.id == work.agent_id)
            .context("GPU profile missing")?;
        ensure!(
            work.access.shell
                && work.access.workspace_read
                && work.access.network
                && work
                    .attempts
                    .last()
                    .is_some_and(|a| a.environment == ExecutionEnvironment::NebiusSandbox)
                && profile.execution.sandbox_target.as_deref() == Some(&policy.target),
            "GPU launch requires its original Nebius target and command/read/network grants"
        );
        Ok(())
    }
    pub(crate) fn authorize_shared_gpu_budget(
        &self,
        project: &str,
        policy: &GpuBudgetPolicy,
    ) -> Result<()> {
        crate::cloud_spending::authorize(
            &self.connect()?,
            project,
            &policy.target,
            &policy.currency,
            policy.reservation_micros,
        )
    }
    pub(crate) fn verify_gpu_launch_replay(
        &self,
        lease: &AssignmentLease,
        request: &jobs::JobRequest,
        body: &Value,
        policy: &GpuBudgetPolicy,
        now: u64,
    ) -> Result<()> {
        policy.validate(now)?;
        let db = self.connect()?;
        let run = load(&db, lease.run_id)?;
        active(&run, now)?;
        checked_lease(&run, lease, now, false)?;
        let signature = json!({"request":request,"body":body,"policy":policy}).to_string();
        let old:String=db.query_row("SELECT signature FROM ion_team_gpu_jobs WHERE request_id=? AND run_id=? AND work_id=? AND attempt_id=?",
            params![request.request_id.to_string(),lease.run_id.to_string(),lease.work_id.to_string(),lease.attempt_id.to_string()],|r|r.get(0)).context("GPU saved replay missing")?;
        ensure!(
            old == signature,
            "GPU replay request, resource policy or spending allowance changed"
        );
        Ok(())
    }
    /// Returns false for the exact existing request; the caller must return its saved
    /// state, never POST again. Uncertain acknowledgements retain the whole reservation.
    #[cfg(test)]
    pub(crate) fn reserve_gpu_launch(
        &self,
        lease: &AssignmentLease,
        request: &jobs::JobRequest,
        body: &Value,
        policy: &GpuBudgetPolicy,
        now: u64,
    ) -> Result<bool> {
        self.reserve_gpu_launch_inner(lease, request, body, policy, None, None, now)
    }
    #[cfg(test)]
    pub(crate) fn reserve_resource_gpu_launch_for_test(
        &self,
        lease: &AssignmentLease,
        request: &jobs::JobRequest,
        body: &Value,
        policy: &GpuBudgetPolicy,
        receipt: &crate::driver::team_gpu::GpuPreflight,
        now: u64,
    ) -> Result<bool> {
        self.reserve_gpu_launch_inner(lease, request, body, policy, Some(receipt), None, now)
    }
    pub(crate) fn reserve_verified_gpu_launch(
        &self,
        lease: &AssignmentLease,
        request: &jobs::JobRequest,
        body: &Value,
        policy: &GpuBudgetPolicy,
        receipt: &crate::driver::team_gpu::GpuPreflight,
        price: &crate::driver::team_gpu::GpuPriceReceipt,
        now: u64,
    ) -> Result<bool> {
        self.reserve_gpu_launch_inner(
            lease,
            request,
            body,
            policy,
            Some(receipt),
            Some(price),
            now,
        )
    }
    fn reserve_gpu_launch_inner(
        &self,
        lease: &AssignmentLease,
        request: &jobs::JobRequest,
        body: &Value,
        policy: &GpuBudgetPolicy,
        receipt: Option<&crate::driver::team_gpu::GpuPreflight>,
        price: Option<&crate::driver::team_gpu::GpuPriceReceipt>,
        now: u64,
    ) -> Result<bool> {
        policy.validate(now)?;
        let project = body["metadata"]["parentId"]
            .as_str()
            .context("GPU project missing")?;
        resource_id(project)?;
        if let Some(volume) = &policy.result_volume {
            ensure!(
                body["spec"]["volumes"] == json!([volume.mount()]),
                "GPU result volume differs from its owner policy"
            );
            ensure!(
                body["spec"]["args"]
                    == "--request-file /ion/job-request.json --result-volume /ion/results",
                "GPU result collector changed"
            );
        } else {
            ensure!(
                body["spec"]["volumes"].is_null() || body["spec"]["volumes"] == json!([]),
                "Unapproved GPU result volume"
            );
        }
        let signature = json!({"request":request,"body":body,"policy":policy}).to_string();
        ensure!(
            signature.len() <= 256 * 1024
                && !request.request_id.is_nil()
                && body["metadata"]["name"] == format!("ion-gpu-{}", request.request_id)
                && body["metadata"]["labels"]["ion-request"] == request.request_id.to_string(),
            "GPU request/provenance invalid"
        );
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = load(&tx, lease.run_id)?;
        active(&run, now)?;
        let work = checked_lease(&run, lease, now, false)?;
        ensure!(
            work.access.shell && work.access.workspace_read && work.access.network,
            "GPU launch requires command, read and network grants"
        );
        let attempt = work.attempts.last().context("GPU attempt missing")?;
        let profile = run
            .profiles
            .iter()
            .find(|p| p.id == work.agent_id)
            .context("GPU profile missing")?;
        ensure!(
            attempt.environment == ExecutionEnvironment::NebiusSandbox
                && profile.execution.sandbox_target.as_deref() == Some(&policy.target),
            "GPU launch cannot change the selected environment/target or run from Local fallback"
        );
        if let Some(old) = tx.query_row("SELECT signature FROM ion_team_gpu_jobs WHERE request_id=? AND run_id=? AND work_id=? AND attempt_id=?",
            params![request.request_id.to_string(),run.id.to_string(),work.id.to_string(),lease.attempt_id.to_string()],
            |r|r.get::<_,String>(0)).optional()? {
            ensure!(old == signature,"GPU replay request, resource policy or spending allowance changed");
            return Ok(false);
        }
        let witnesses: Vec<(String, u64)> = tx
            .prepare(
                "SELECT policy,reserved_micros FROM ion_team_gpu_jobs WHERE run_id=? LIMIT 129",
            )?
            .query_map([run.id.to_string()], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        ensure!(witnesses.len() < 128, "GPU job count exceeded");
        let mut budget = policy.run_budget_micros;
        let mut maximum = policy.max_jobs;
        let mut committed = 0u64;
        for (old, reserved) in &witnesses {
            let old: GpuBudgetPolicy = serde_json::from_str(old)?;
            ensure!(
                old.currency == policy.currency,
                "GPU run currency cannot change"
            );
            if old.target == policy.target {
                ensure!(
                    old == *policy,
                    "GPU target spending policy changed during the run"
                );
            }
            budget = budget.min(old.run_budget_micros);
            maximum = maximum.min(old.max_jobs);
            committed = committed
                .checked_add(*reserved)
                .context("GPU spending overflow")?;
        }
        ensure!(
            witnesses.len() < usize::from(maximum),
            "GPU run job count limit reached"
        );
        ensure!(
            committed
                .checked_add(policy.reservation_micros)
                .is_some_and(|v| v <= budget),
            "GPU run reservation budget reached; unknown and terminal costs remain reserved"
        );
        let active_policies: Vec<String> = tx.prepare("SELECT policy FROM ion_team_gpu_jobs j WHERE project=? AND state!='cleaned' AND NOT EXISTS(SELECT 1 FROM ion_team_gpu_cleanup c WHERE c.request_id=j.request_id AND c.state='verified') LIMIT 129")?
            .query_map([project],|r|r.get(0))?.collect::<rusqlite::Result<_>>()?;
        let mut parallel = policy.max_parallel;
        for old in &active_policies {
            parallel = parallel.min(serde_json::from_str::<GpuBudgetPolicy>(old)?.max_parallel);
        }
        ensure!(
            active_policies.len() < usize::from(parallel),
            "GPU project concurrency is still reserved by an unfinished job"
        );
        if let Some(resources) = &policy.resources {
            let checked =
                receipt.context("New GPU resources require a fresh native preflight receipt")?;
            checked.validate(body, request, resources, now)?;
            resource_policy::reserve_resources(&tx, resources, checked)?;
        } else {
            ensure!(
                cfg!(test) && receipt.is_none(),
                "New GPU launch requires exact owner resource approval"
            );
        }
        if let Some(price) = price {
            price.validate(body, request, policy, now)?;
        } else {
            ensure!(
                cfg!(test),
                "New GPU launch requires a trusted native price receipt"
            );
        }
        workspace::require_settled_effects(&tx, run.id)?;
        let seconds: u64 = body["spec"]["timeout"]
            .as_str()
            .and_then(|v| v.strip_suffix('s'))
            .and_then(|v| v.parse().ok())
            .context("GPU timeout missing")?;
        ensure!(
            (3600..=86400).contains(&seconds),
            "GPU job timeout must be 3600..86400 seconds"
        );
        let deadline = now.checked_add(seconds).context("GPU deadline overflow")?;
        ensure!(
            deadline
                <= run
                    .created_at
                    .saturating_add(u64::from(run.limits.timeout_seconds)),
            "GPU deadline exceeds the original team deadline"
        );
        tx.execute("INSERT INTO ion_team_gpu_jobs(request_id,run_id,work_id,attempt_id,project,signature,body,policy,reserved_micros,currency,deadline_at,state,compute_ids) VALUES(?,?,?,?,?,?,?,?,?,?,?,'submitting','[]')",
            params![request.request_id.to_string(),run.id.to_string(),work.id.to_string(),lease.attempt_id.to_string(),project,signature,body.to_string(),serde_json::to_string(policy)?,policy.reservation_micros,policy.currency,deadline])?;
        if let Some(checked) = receipt {
            tx.execute(
                "INSERT INTO ion_team_gpu_preflight(request_id,body) VALUES(?,?)",
                params![request.request_id.to_string(), checked.persisted()?],
            )?;
        }
        if let Some(price) = price {
            tx.execute(
                "INSERT INTO ion_team_gpu_prices(request_id,body) VALUES(?,?)",
                params![request.request_id.to_string(), price.persisted()?],
            )?;
        }
        crate::cloud_spending::hold(
            &tx,
            request.request_id,
            run.id,
            project,
            &policy.target,
            &policy.currency,
            policy.reservation_micros,
        )?;
        audit(
            &tx,
            run.id,
            Some(work.id),
            "gpu_launch_reserved",
            &format!(
                "GPU request {}: {} {} micro-units reserved before submission",
                request.request_id, policy.reservation_micros, policy.currency
            ),
            now,
        )?;
        bump(&tx, run.id)?;
        tx.commit()?;
        Ok(true)
    }
    pub(crate) fn gpu_jobs(&self, attempt: Uuid) -> Result<Vec<GpuJobRecord>> {
        let db = self.connect()?;
        let rows:Vec<(String,String,String,Option<String>,Option<String>,Option<String>,bool,String,String,u64,Option<String>,String,Option<String>,Option<String>)> = db.prepare(
            "SELECT j.request_id,j.project,j.body,j.creation_operation,j.job_id,j.cancellation_operation,j.cancel_requested,j.state,j.compute_ids,j.deadline_at,c.body,j.policy,g.receipt,g.artifact_id FROM ion_team_gpu_jobs j LEFT JOIN ion_team_gpu_cleanup c ON c.request_id=j.request_id LEFT JOIN ion_team_gpu_results g ON g.request_id=j.request_id WHERE j.attempt_id=? ORDER BY j.rowid LIMIT 129")?
            .query_map([attempt.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?,r.get(9)?,r.get(10)?,r.get(11)?,r.get(12)?,r.get(13)?)))?.collect::<rusqlite::Result<_>>()?;
        ensure!(rows.len() <= 128, "GPU recovery count exceeded");
        rows.into_iter()
            .map(|r| {
                Ok(GpuJobRecord {
                    request_id: Uuid::parse_str(&r.0)?,
                    project: r.1,
                    body: serde_json::from_str(&r.2)?,
                    creation_operation: r.3,
                    job_id: r.4,
                    cancellation_operation: r.5,
                    cancel_requested: r.6,
                    state: r.7,
                    compute_ids: serde_json::from_str(&r.8)?,
                    deadline_at: r.9,
                    cleanup: r.10.map(|s| serde_json::from_str(&s)).transpose()?,
                    result_volume: serde_json::from_str::<GpuBudgetPolicy>(&r.11)?.result_volume,
                    result_receipt: r.12.map(|s| serde_json::from_str(&s)).transpose()?,
                    result_artifact: r.13.map(|s| Uuid::parse_str(&s)).transpose()?,
                })
            })
            .collect()
    }
    pub(crate) fn bind_gpu_acknowledgement(
        &self,
        request: Uuid,
        operation: &str,
        job: &str,
        now: u64,
    ) -> Result<()> {
        resource_id(operation)?;
        resource_id(job)?;
        ensure!(operation != job, "GPU operation is not its job resource");
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (run, work): (String, String) = tx.query_row(
            "SELECT run_id,work_id FROM ion_team_gpu_jobs WHERE request_id=?",
            [request.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        ensure!(tx.execute("UPDATE ion_team_gpu_jobs SET creation_operation=?,job_id=?,state='running' WHERE request_id=? AND state='submitting' AND creation_operation IS NULL AND job_id IS NULL",
            params![operation,job,request.to_string()])?==1,"GPU acknowledgement already bound or changed");
        audit(
            &tx,
            Uuid::parse_str(&run)?,
            Some(Uuid::parse_str(&work)?),
            "gpu_launch_acknowledged",
            &format!("GPU request {request} bound to {job}"),
            now,
        )?;
        bump(&tx, Uuid::parse_str(&run)?)?;
        tx.commit()?;
        Ok(())
    }
    /// Trusted transport observations only. A terminal state is still an unfinished
    /// resource; no cost or concurrency reservation is released without cleanup.
    pub(crate) fn record_gpu_observation(
        &self,
        request: Uuid,
        job: &str,
        state: &str,
        compute: &[String],
        now: u64,
    ) -> Result<()> {
        resource_id(job)?;
        ensure!(
            matches!(
                state,
                "PROVISIONING"
                    | "STARTING"
                    | "IMAGE_PULLING"
                    | "RUNNING"
                    | "CANCELLING"
                    | "DELETING"
                    | "COMPLETED"
                    | "FAILED"
                    | "CANCELLED"
                    | "ERROR"
            ),
            "Unknown GPU service state"
        );
        ensure!(compute.len() <= 16, "GPU instance count exceeded");
        for id in compute {
            resource_id(id)?;
        }
        let terminal = matches!(state, "COMPLETED" | "FAILED" | "CANCELLED" | "ERROR");
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (run,work,old,remote,stored_ids):(String,String,String,Option<String>,String)=tx.query_row("SELECT run_id,work_id,state,remote_state,compute_ids FROM ion_team_gpu_jobs WHERE request_id=? AND job_id=?",
            params![request.to_string(),job],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))?;
        ensure!(
            !matches!(old.as_str(), "cleaned" | "settled")
                && (old != "terminal" || (terminal && remote.as_deref() == Some(state))),
            "GPU state regressed after terminal observation"
        );
        let mut ids: Vec<String> = serde_json::from_str(&stored_ids)?;
        for id in compute {
            if !ids.contains(id) {
                ids.push(id.clone());
            }
        }
        ensure!(ids.len() <= 16, "GPU observed resource count exceeded");
        let next = if terminal {
            "terminal"
        } else if old == "canceling" {
            "canceling"
        } else {
            "running"
        };
        let updated_ids = serde_json::to_string(&ids)?;
        if old == next && remote.as_deref() == Some(state) && stored_ids == updated_ids {
            return Ok(());
        }
        tx.execute(
            "UPDATE ion_team_gpu_jobs SET state=?,remote_state=?,compute_ids=? WHERE request_id=?",
            params![next, state, updated_ids, request.to_string()],
        )?;
        audit(
            &tx,
            Uuid::parse_str(&run)?,
            Some(Uuid::parse_str(&work)?),
            "gpu_observed",
            &format!("GPU request {request}: {state}; reservations retained"),
            now,
        )?;
        bump(&tx, Uuid::parse_str(&run)?)?;
        tx.commit()?;
        Ok(())
    }
    /// Claim cancellation before its one POST. Lost acknowledgement remains claimed.
    pub(crate) fn claim_gpu_cancellation(&self, request: Uuid, job: &str) -> Result<Option<Uuid>> {
        resource_id(job)?;
        let key = Uuid::new_v4();
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed=tx.execute("UPDATE ion_team_gpu_jobs SET cancel_requested=1,state='canceling',cancellation_key=? WHERE request_id=? AND job_id=? AND state='running' AND cancel_requested=0",
            params![key.to_string(),request.to_string(),job])?;
        if changed == 0 {
            return Ok(None);
        }
        let (run, work): (String, String) = tx.query_row(
            "SELECT run_id,work_id FROM ion_team_gpu_jobs WHERE request_id=?",
            [request.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        audit(
            &tx,
            Uuid::parse_str(&run)?,
            Some(Uuid::parse_str(&work)?),
            "gpu_cancel_requested",
            &format!("Cancellation of original GPU job {job} persisted before POST"),
            crate::model::unix_time(),
        )?;
        bump(&tx, Uuid::parse_str(&run)?)?;
        tx.commit()?;
        Ok(Some(key))
    }
    pub(crate) fn bind_gpu_cancellation(
        &self,
        request: Uuid,
        job: &str,
        operation: &str,
    ) -> Result<()> {
        resource_id(operation)?;
        ensure!(
            operation != job,
            "GPU cancellation operation is not its job"
        );
        ensure!(self.connect()?.execute("UPDATE ion_team_gpu_jobs SET cancellation_operation=? WHERE request_id=? AND job_id=? AND cancel_requested=1 AND cancellation_operation IS NULL",
            params![operation,request.to_string(),job])?==1,"GPU cancellation acknowledgement changed");
        Ok(())
    }
}
