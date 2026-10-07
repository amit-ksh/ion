//! Nebius Serverless AI Jobs lifecycle transport, separate from ConTree.
//! Agent submission is gated until accounting and resource/egress policy
//! integration is complete. Durable waits are available for approved bound jobs. Cleanup proofs are persisted independently. Launch/cancel requests persist their reservations first.
//! A job state is not a command/test receipt or cleanup proof.
use crate::teams::{
    AssignmentLease, TeamCoordinator,
    gpu_jobs::{GpuBudgetPolicy, GpuJobRecord},
    jobs::JobRequest,
};
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::{io::Read, time::Duration};
const BASE: &str = "https://api.nebius.cloud";
mod recovery;
mod resources;
mod results;
pub(crate) use recovery::bind_recovered_operation;

pub(crate) struct Launch {
    pub request: JobRequest,
    pub project: String,
    pub image: String,
    pub platform: String,
    pub preset: String,
    pub subnet: String,
    pub disk_bytes: u64,
    pub shm_bytes: u64,
    pub timeout_seconds: u32,
    pub result_volume: Option<crate::teams::gpu_jobs::GpuResultVolume>,
}
fn identifier(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= 160
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_')),
        "Invalid GPU resource identity"
    );
    Ok(())
}
impl Launch {
    fn name(&self) -> String {
        format!("ion-gpu-{}", self.request.request_id)
    }
    pub(super) fn body(&self) -> Result<Value> {
        for id in [&self.project, &self.platform, &self.preset, &self.subnet] {
            identifier(id)?;
        }
        let (repository, digest) = self
            .image
            .rsplit_once("@sha256:")
            .context("Pin the GPU image by SHA256 digest")?;
        ensure!(
            !repository.is_empty()
                && repository.len() <= 512
                && repository
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"/._-:".contains(&b))
                && digest.len() == 64
                && digest.bytes().all(|b| b.is_ascii_hexdigit()),
            "Invalid pinned GPU image"
        );
        // Nebius requires at least one hour; Ion retains its 24-hour policy cap.
        // Do not pad a short team deadline or silently change the child command limit.
        ensure!(
            (3600..=86400).contains(&self.timeout_seconds),
            "GPU job timeout must be 3600..86400 seconds"
        );
        ensure!(
            self.request.version == 1
                && !self.request.request_id.is_nil()
                && self.request.timeout_seconds > 0
                && self.request.timeout_seconds <= 3600
                && self.request.timeout_seconds <= self.timeout_seconds
                && (1024 * 1024 * 1024..=1024 * 1024 * 1024 * 1024).contains(&self.disk_bytes)
                && self.shm_bytes <= self.disk_bytes,
            "GPU resource/time limits are invalid"
        );
        ensure!(
            (1..=64).contains(&self.request.argv.len())
                && self.request.argv[0].starts_with('/')
                && self
                    .request
                    .argv
                    .iter()
                    .all(|a| a.len() <= 8192 && !a.contains('\0'))
                && self.request.argv.iter().map(String::len).sum::<usize>() <= 32768,
            "GPU command arguments exceed their bound"
        );
        ensure!(
            self.request.inputs.len() <= 32 && self.request.output_paths.len() <= 32,
            "Too many GPU transfer paths"
        );
        let mut seen = std::collections::HashSet::new();
        let mut transferred = 0;
        for input in &self.request.inputs {
            super::team_jobs::path(&input.path)?;
            ensure!(
                seen.insert(input.path.to_ascii_lowercase())
                    && input.content_base64.len() <= 88 * 1024,
                "Invalid GPU input path or size"
            );
            let bytes = STANDARD.decode(&input.content_base64)?;
            ensure!(bytes.len() <= 64 * 1024, "GPU input file exceeds its bound");
            transferred += bytes.len();
        }
        ensure!(transferred <= 256 * 1024, "GPU transfer exceeds its bound");
        seen.clear();
        for output in &self.request.output_paths {
            super::team_jobs::path(output)?;
            ensure!(
                seen.insert(output.to_ascii_lowercase()),
                "Duplicate GPU output path"
            );
        }
        let payload = serde_json::to_vec(&self.request)?;
        // Native FileInjection is for small configuration, not a dataset upload.
        ensure!(
            payload.len() <= 64 * 1024,
            "GPU injected configuration exceeds 64 KiB; use an approved dataset volume integration"
        );
        let mut body = json!({"metadata":{"parentId":self.project,"name":self.name(),
            "labels":{"ion-request":self.request.request_id.to_string()}},
            "spec":{"image":self.image,"containerCommand":"/usr/local/bin/ion-job-worker",
            "args":"--request-file /ion/job-request.json","workingDir":"/ion",
            "environmentVariables":[{"name":"ION_JOB_ISOLATED","value":"1"},{"name":"ION_JOB_CHILD_UID","value":"1000"},{"name":"ION_JOB_CHILD_GID","value":"1000"}],
            "injectedFiles":[{"containerPath":"/ion/job-request.json","content":STANDARD.encode(payload)}],
            "platform":self.platform,"preset":self.preset,"subnetId":self.subnet,
            "disk":{"type":"NETWORK_SSD","sizeBytes":self.disk_bytes.to_string()},
            "shmSizeBytes":self.shm_bytes.to_string(),"timeout":format!("{}s",self.timeout_seconds),
            "publicIp":false,"preemptible":false,"restartAttempts":"0","pricingModel":{"onDemand":{}}}});
        if let Some(volume) = &self.result_volume {
            volume.validate()?;
            body["spec"]["args"] =
                json!("--request-file /ion/job-request.json --result-volume /ion/results");
            body["spec"]["volumes"] = json!([volume.mount()]);
        }
        Ok(body)
    }
}
pub(super) struct Acknowledged {
    pub operation: String,
    pub job: String,
}
pub(super) struct Observation {
    pub state: String,
    pub terminal: bool,
    pub compute_ids: Vec<String>,
}
pub(super) struct Client {
    http: reqwest::blocking::Client,
    token: String,
    project: String,
    #[cfg(test)]
    base: String,
}
impl Client {
    pub(super) fn configured() -> Result<Self> {
        let token = std::env::var("ION_NEBIUS_GPU_IAM_TOKEN")
            .context("Configure owner GPU IAM credentials privately")?;
        let project = std::env::var("ION_NEBIUS_GPU_PROJECT")
            .context("Configure the GPU project on this owner")?;
        identifier(&project)?;
        ensure!(!token.is_empty(), "GPU IAM credentials missing");
        Ok(Self {
            http: reqwest::blocking::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(3))
                .timeout(Duration::from_secs(8))
                .build()?,
            token,
            project,
            #[cfg(test)]
            base: BASE.into(),
        })
    }
    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::blocking::RequestBuilder {
        #[cfg(test)]
        let base = &self.base;
        #[cfg(not(test))]
        let base = BASE;
        self.http
            .request(method, format!("{base}{path}"))
            .bearer_auth(&self.token)
    }
    fn response(response: reqwest::blocking::Response) -> Result<Value> {
        ensure!(
            response.status().is_success(),
            "GPU lifecycle request is unresolved (HTTP {})",
            response.status()
        );
        let mut bytes = Vec::new();
        response.take(256 * 1024 + 1).read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() <= 256 * 1024,
            "GPU lifecycle response exceeded its bound"
        );
        Ok(serde_json::from_slice(&bytes)?)
    }
    /// Call exactly once, after the coordinator has persisted its request.
    /// Any failure can have an uncertain outcome: never retry this POST or fall back.
    pub(super) fn submit(&self, launch: &Launch) -> Result<Acknowledged> {
        ensure!(
            launch.project == self.project,
            "GPU project differs from its owner context"
        );
        let body = launch.body()?;
        let response=self.request(reqwest::Method::POST,"/ai/v1/jobs").header("X-Idempotency-Key",launch.request.request_id.to_string()).json(&body).send()
            .context("GPU create acknowledgement lost; inspect the original named job, never resubmit or fall back")?;
        let receipt = Self::response(response)?;
        let operation = receipt["id"]
            .as_str()
            .context("GPU creation operation missing")?
            .to_string();
        let job = receipt["resourceId"]
            .as_str()
            .context("GPU job identity missing")?
            .to_string();
        identifier(&operation)?;
        identifier(&job)?;
        ensure!(
            operation != job,
            "GPU creation operation and job identities were conflated"
        );
        Ok(Acknowledged { operation, job })
    }
    pub(super) fn creation_finished(&self, ack: &Acknowledged) -> Result<bool> {
        identifier(&ack.operation)?;
        identifier(&ack.job)?;
        let value = Self::response(
            self.request(
                reqwest::Method::GET,
                &format!("/ai/v1/jobs/operations/{}", ack.operation),
            )
            .send()?,
        )?;
        ensure!(
            value["id"] == ack.operation && value["resourceId"] == ack.job,
            "GPU creation identity changed"
        );
        if value.get("status").is_none_or(Value::is_null) {
            return Ok(false);
        }
        ensure!(
            value["status"].is_object()
                && value["status"].get("code").is_none_or(|code| *code == 0),
            "GPU creation failed; retain its original resource identity for reconciliation"
        );
        Ok(true)
    }
    pub(super) fn observe(&self, launch: &Launch, ack: &Acknowledged) -> Result<Observation> {
        identifier(&ack.job)?;
        ensure!(launch.project == self.project, "GPU owner project changed");
        // SECRET is required to bind injected input bytes, not to forward credentials.
        let value = Self::response(
            self.request(reqwest::Method::GET, &format!("/ai/v1/jobs/{}", ack.job))
                .query(&[("view", "SECRET")])
                .send()?,
        )?;
        self.verify_observation(&launch.body()?, ack, &value)
    }
    fn observe_saved(&self, body: &Value, ack: &Acknowledged) -> Result<Observation> {
        identifier(&ack.job)?;
        let value = Self::response(
            self.request(reqwest::Method::GET, &format!("/ai/v1/jobs/{}", ack.job))
                .query(&[("view", "SECRET")])
                .send()?,
        )?;
        self.verify_observation(body, ack, &value)
    }
    fn verify_observation(
        &self,
        expected: &Value,
        ack: &Acknowledged,
        value: &Value,
    ) -> Result<Observation> {
        ensure!(
            expected["metadata"]["parentId"] == self.project
                && value["metadata"]["id"] == ack.job
                && value["metadata"]["parentId"] == self.project
                && value["metadata"]["name"] == expected["metadata"]["name"]
                && value["metadata"]["labels"]["ion-request"]
                    == expected["metadata"]["labels"]["ion-request"],
            "GPU job provenance changed"
        );
        for (key, expected) in expected["spec"].as_object().unwrap() {
            let actual = &value["spec"][key];
            // Protobuf default false/zero fields may be omitted by the service.
            let default_missing = actual.is_null()
                && (*expected == false
                    || (key == "restartAttempts" && *expected == "0")
                    || (key == "shmSizeBytes" && *expected == "0"));
            ensure!(
                actual == expected || default_missing,
                "GPU pinned specification changed: {key}"
            );
        }
        for key in ["registryCredentials", "sshAuthorizedKeys", "ports"] {
            let extra = &value["spec"][key];
            ensure!(
                extra.is_null()
                    || extra.as_array().is_some_and(Vec::is_empty)
                    || extra.as_object().is_some_and(serde_json::Map::is_empty),
                "Unapproved GPU capability appeared"
            );
        }
        if expected["spec"]["volumes"].is_null() {
            ensure!(
                value["spec"]["volumes"].is_null() || value["spec"]["volumes"] == json!([]),
                "Unapproved GPU volume appeared"
            );
        }
        let state = value["status"]["state"]
            .as_str()
            .context("GPU job has no state")?;
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
            "Unknown GPU job state"
        );
        let instances = value["status"]
            .get("instances")
            .filter(|v| !v.is_null())
            .map(|v| v.as_array().context("GPU instance status invalid"))
            .transpose()?;
        ensure!(
            instances.is_none_or(|v| v.len() <= 16),
            "GPU instance status exceeds its bound"
        );
        let mut compute_ids = Vec::new();
        for instance in instances.into_iter().flatten() {
            if let Some(id) = instance
                .get("computeInstanceId")
                .filter(|v| !v.is_null() && *v != "")
            {
                let id = id.as_str().context("GPU compute identity invalid")?;
                identifier(id)?;
                if !compute_ids.iter().any(|v| v == id) {
                    compute_ids.push(id.to_string());
                }
            }
            ensure!(
                instance
                    .get("publicIp")
                    .is_none_or(|v| v.is_null() || *v == ""),
                "Unapproved GPU public endpoint appeared"
            );
        }
        ensure!(
            value["status"]
                .get("publicEndpoints")
                .is_none_or(|v| v.is_null() || v.as_array().is_some_and(Vec::is_empty)),
            "Unapproved GPU public endpoints appeared"
        );
        Ok(Observation {
            state: state.into(),
            terminal: matches!(state, "COMPLETED" | "FAILED" | "CANCELLED" | "ERROR"),
            compute_ids,
        })
    }
    /// Cancellation acknowledgement is not terminal job or compute cleanup proof.
    pub(super) fn cancel(&self, ack: &Acknowledged, key: uuid::Uuid) -> Result<String> {
        identifier(&ack.job)?;
        ensure!(!key.is_nil(), "GPU cancellation identity missing");
        let value = Self::response(
            self.request(reqwest::Method::POST, "/ai/v1/jobs/cancel")
                .header("X-Idempotency-Key", key.to_string())
                .json(&json!({"id":ack.job}))
                .send()?,
        )?;
        ensure!(
            value["resourceId"] == ack.job,
            "GPU cancellation resource changed"
        );
        let operation = value["id"]
            .as_str()
            .context("GPU cancellation operation missing")?;
        identifier(operation)?;
        ensure!(
            operation != ack.operation && operation != ack.job,
            "GPU cancellation reused creation/job identity"
        );
        Ok(operation.into())
    }
}

/// Trusted owner entrypoint, not an agent-facing tool. All preflight failures happen
/// before reservation; everything after it retains the exact launch and budget.
pub(crate) fn launch_once(
    coordinator: &TeamCoordinator,
    lease: &AssignmentLease,
    launch: &Launch,
    policy: &GpuBudgetPolicy,
) -> Result<GpuJobRecord> {
    launch_once_with(&Client::configured()?, coordinator, lease, launch, policy)
}
fn launch_once_with(
    client: &Client,
    coordinator: &TeamCoordinator,
    lease: &AssignmentLease,
    launch: &Launch,
    policy: &GpuBudgetPolicy,
) -> Result<GpuJobRecord> {
    ensure!(
        launch.project == client.project,
        "GPU owner project changed before launch"
    );
    ensure!(
        launch.result_volume == policy.result_volume,
        "GPU output volume differs from the approved policy"
    );
    let body = launch.body()?;
    let now = crate::model::unix_time();
    coordinator.authorize_gpu_launch(lease, policy, now)?;
    // Exact recovery replays use the saved launch; no provider read/POST is needed.
    let replay = coordinator
        .gpu_jobs(lease.attempt_id())?
        .iter()
        .any(|r| r.request_id == launch.request.request_id);
    if !replay {
        if let Some(volume) = &launch.result_volume {
            client.verify_result_bucket(volume)?;
        }
    }
    if coordinator.reserve_gpu_launch(
        lease,
        &launch.request,
        &body,
        policy,
        crate::model::unix_time(),
    )? {
        let ack = client.submit(launch)?;
        coordinator.bind_gpu_acknowledgement(
            launch.request.request_id,
            &ack.operation,
            &ack.job,
            crate::model::unix_time(),
        )?;
    }
    coordinator
        .gpu_jobs(lease.attempt_id())?
        .into_iter()
        .find(|v| v.request_id == launch.request.request_id)
        .context("Saved GPU launch missing")
}
/// Collect approved GPU receipts on an owner background worker, independent of Stop.
pub(crate) fn collect_results_attempt(
    coordinator: &TeamCoordinator,
    attempt: uuid::Uuid,
) -> Result<bool> {
    results::collect_attempt(coordinator, attempt)
}
pub(crate) fn read_gpu_output(
    coordinator: &TeamCoordinator,
    lease: &AssignmentLease,
    request: uuid::Uuid,
    path: &str,
    offset: usize,
) -> Result<String> {
    results::read_output(coordinator, lease, request, path, offset)
}
/// One owner/background wait observation. Original identity only; never create/fallback.
pub(crate) fn progress_wait(coordinator: &TeamCoordinator, work: uuid::Uuid) -> Result<bool> {
    let Some((run, attempt, request)) = coordinator.gpu_wait_request(work)? else {
        return Ok(true);
    };
    if coordinator.settle_gpu_result(request, crate::model::unix_time())? {
        return Ok(true);
    };
    let record = coordinator
        .gpu_jobs(attempt)?
        .into_iter()
        .find(|r| r.request_id == request)
        .context("GPU wait request missing")?;
    let state = coordinator.snapshot(run)?;
    let cancel = state.status != waku_protocol::teams::TeamRunStatus::Active
        || crate::model::unix_time() >= record.deadline_at;
    let client = Client::configured()?;
    let lifecycle = if resources_closed(&record) {
        Ok(true)
    } else {
        advance_record(&client, coordinator, &record, cancel)
    };
    // Try results independently even if lifecycle/cleanup observation is unavailable.
    let outputs = results::collect_attempt(coordinator, attempt);
    lifecycle?;
    outputs?;
    coordinator.settle_gpu_result(request, crate::model::unix_time())
}
/// Owner maintenance for a saved job after the model/task no longer dispatches.
/// The original mutation claims still fence cancel/delete; a missing receipt is pending.
pub(crate) fn progress_saved_job(
    coordinator: &TeamCoordinator,
    attempt: uuid::Uuid,
    request: uuid::Uuid,
) -> Result<bool> {
    // Check the original attempt before settling; a stale/foreign callback must
    // not mutate another assignment even if its receipt is already complete.
    let record = coordinator
        .gpu_jobs(attempt)?
        .into_iter()
        .find(|r| r.request_id == request)
        .context("Saved GPU maintenance request missing")?;
    if coordinator.settle_gpu_result(request, crate::model::unix_time())? {
        return Ok(true);
    };
    progress_saved_with(
        &Client::configured()?,
        coordinator,
        &record,
        attempt,
        results::collect_saved,
    )
}
fn progress_saved_with(
    client: &Client,
    coordinator: &TeamCoordinator,
    record: &GpuJobRecord,
    attempt: uuid::Uuid,
    collect: impl FnOnce(&Client, &TeamCoordinator, &GpuJobRecord, uuid::Uuid) -> Result<bool>,
) -> Result<bool> {
    let lifecycle = if resources_closed(record) {
        Ok(true)
    } else {
        advance_record(client, coordinator, record, true)
    };
    // Result retrieval remains independent of cleanup permissions, including after Stop.
    let output = collect(client, coordinator, record, attempt);
    lifecycle?;
    output?;
    if record.result_volume.is_none() {
        return Ok(coordinator
            .gpu_jobs(attempt)?
            .iter()
            .find(|r| r.request_id == record.request_id)
            .is_some_and(resources_closed));
    }
    coordinator.settle_gpu_result(record.request_id, crate::model::unix_time())
}

/// Stop/restart observes and cancels bound jobs. No create POST or Local execution.
/// Terminal states retain resource reservations until cleanup is independently verified.
pub(crate) fn reconcile_attempt(
    coordinator: &TeamCoordinator,
    attempt: uuid::Uuid,
) -> Result<bool> {
    let records = coordinator.gpu_jobs(attempt)?;
    if records.iter().all(resources_closed) {
        return Ok(true);
    }
    reconcile_with(&Client::configured()?, coordinator, records)
}
fn resources_closed(record: &GpuJobRecord) -> bool {
    record.state == "cleaned"
        || record
            .cleanup
            .as_ref()
            .is_some_and(|c| c.state == "verified")
}
fn reconcile_with(
    client: &Client,
    coordinator: &TeamCoordinator,
    records: Vec<GpuJobRecord>,
) -> Result<bool> {
    let mut closed = true;
    let mut first_error = None;
    // A broken witness or missing permission for one job must not skip the others.
    for record in records.iter().filter(|r| !resources_closed(r)) {
        match reconcile_record(client, coordinator, record) {
            Ok(done) => closed &= done,
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }
    if let Some(error) = first_error {
        return Err(error);
    }
    Ok(closed)
}
fn reconcile_record(
    client: &Client,
    coordinator: &TeamCoordinator,
    record: &GpuJobRecord,
) -> Result<bool> {
    advance_record(client, coordinator, record, true)
}
/// Active waits observe and capture inventory; Stop/deadline alone requests cancellation.
fn advance_record(
    client: &Client,
    coordinator: &TeamCoordinator,
    record: &GpuJobRecord,
    cancel: bool,
) -> Result<bool> {
    ensure!(
        record.project == client.project,
        "Use the GPU job's original owner project/account"
    );
    let (Some(operation), Some(job)) = (&record.creation_operation, &record.job_id) else {
        return Ok(false);
    };
    let ack = Acknowledged {
        operation: operation.clone(),
        job: job.clone(),
    };
    if let Some(claim) = &record.cleanup {
        if claim.operation.is_none() || !client.cleanup_proven(&ack, claim)? {
            return Ok(false);
        }
        coordinator.verify_gpu_cleanup(
            record.request_id,
            claim.operation.as_deref().unwrap(),
            claim,
            crate::model::unix_time(),
        )?;
        return Ok(true);
    }
    let observed = client.observe_saved(&record.body, &ack)?;
    coordinator.record_gpu_observation(
        record.request_id,
        job,
        &observed.state,
        &observed.compute_ids,
        crate::model::unix_time(),
    )?;
    let mut compute = record.compute_ids.clone();
    for id in observed.compute_ids {
        if !compute.contains(&id) {
            compute.push(id);
        }
    }
    let inventory_result = (|| -> Result<()> {
        for id in &compute {
            if let Some(inventory) = client.inventory(&record.body, id)? {
                coordinator.record_gpu_inventory(
                    record.request_id,
                    job,
                    &inventory,
                    crate::model::unix_time(),
                )?;
            }
        }
        Ok(())
    })();
    // An unavailable inventory permission must not prevent stopping the original job.
    if cancel && !observed.terminal {
        if let Some(key) = coordinator.claim_gpu_cancellation(record.request_id, job)? {
            let cancel = client.cancel(&ack, key)?;
            coordinator.bind_gpu_cancellation(record.request_id, job, &cancel)?;
        }
    }
    inventory_result?;
    if observed.terminal && !compute.is_empty() {
        let inventory = coordinator.gpu_inventory(record.request_id)?;
        if inventory.len() == compute.len() && inventory.iter().all(|i| i.complete) {
            if let Some(claim) =
                coordinator.claim_gpu_deletion(record.request_id, job, crate::model::unix_time())?
            {
                let operation = client.delete_job(&ack, claim.key)?;
                coordinator.bind_gpu_deletion(
                    record.request_id,
                    job,
                    &operation,
                    crate::model::unix_time(),
                )?;
            }
        }
    }
    Ok(false)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::super::team_test_http::Server;
    use super::*;
    use uuid::Uuid;
    pub(crate) fn launch_body(launch: &Launch) -> Value {
        launch.body().unwrap()
    }
    pub(crate) fn launch() -> Launch {
        Launch {
            request: JobRequest {
                version: 1,
                request_id: Uuid::new_v4(),
                argv: vec![
                    "/usr/local/bin/python3".into(),
                    "-c".into(),
                    "print('gpu fixture')".into(),
                ],
                timeout_seconds: 30,
                inputs: vec![],
                output_paths: vec![],
            },
            project: "project-fixture".into(),
            image: format!("registry.example/ion-gpu@sha256:{}", "a".repeat(64)),
            platform: "gpu-l40s-a".into(),
            preset: "1gpu-8vcpu-32gb".into(),
            subnet: "vpcsubnet-fixture".into(),
            disk_bytes: 16 * 1024 * 1024 * 1024,
            shm_bytes: 0,
            timeout_seconds: 3600,
            result_volume: None,
        }
    }
    pub(crate) struct Fixture {
        pub(crate) dir: std::path::PathBuf,
        pub(crate) coordinator: TeamCoordinator,
        pub(crate) lease: AssignmentLease,
        pub(crate) policy: GpuBudgetPolicy,
    }
    impl Fixture {
        pub(crate) fn new(remote: bool, network: bool) -> Self {
            Self::new_in(
                std::env::temp_dir().join(format!("ion-gpu-test-{}", Uuid::new_v4())),
                remote,
                network,
            )
        }
        pub(crate) fn new_in(dir: std::path::PathBuf, remote: bool, network: bool) -> Self {
            use crate::{
                control::ControlStore,
                teams::{DispatchHost, PrepareRun},
            };
            use waku_protocol::{control::*, model::ProviderKind, teams::*};
            let path = dir.join("control.db");
            let definitions = ControlStore::new(path.clone());
            let agent = Uuid::new_v4();
            let team = Uuid::new_v4();
            let target = Uuid::new_v4().to_string();
            let access = AgentAccess {
                shell: true,
                workspace_read: true,
                network,
                ..Default::default()
            };
            definitions
                .save(
                    definitions.load().unwrap().revision,
                    ControlRecord::Agent(AgentProfile {
                        id: agent,
                        name: "GPU fixture".into(),
                        expertise: "Training".into(),
                        instructions: "Run one reviewed job".into(),
                        provider: ProviderKind::Nebius,
                        model: Some("fixture".into()),
                        expected_output: "Receipt".into(),
                        execution: AgentExecution {
                            environment: if remote {
                                ExecutionEnvironment::NebiusSandbox
                            } else {
                                ExecutionEnvironment::Local
                            },
                            sandbox_target: remote.then(|| target.clone()),
                            local_fallback: true,
                        },
                        access: access.clone(),
                        max_parallel_tasks: 1,
                    }),
                )
                .unwrap();
            definitions
                .save(
                    definitions.load().unwrap().revision,
                    ControlRecord::Team(Team {
                        id: team,
                        name: "GPU team".into(),
                        objective: "Bounded training".into(),
                        limits: None,
                        members: vec![TeamMember {
                            agent_id: agent,
                            parent_id: None,
                        }],
                    }),
                )
                .unwrap();
            let coordinator = TeamCoordinator::new(path);
            let now = crate::model::unix_time();
            let run = coordinator
                .prepare(
                    PrepareRun {
                        id: Uuid::new_v4(),
                        root_task_id: Uuid::new_v4(),
                        project_id: Uuid::new_v4(),
                        team_id: team,
                        expected_definition_revision: definitions.load().unwrap().revision,
                        objective: "GPU fixture".into(),
                        limits: TeamRunLimits {
                            timeout_seconds: 7200,
                            ..Default::default()
                        },
                        access,
                        input_data: vec![],
                        source_attachments: vec![],
                        source_prompt: None,
                        origin: None,
                    },
                    now,
                )
                .unwrap();
            let claimed = coordinator
                .claim(
                    run.id,
                    run.work[0].id,
                    if remote {
                        DispatchHost::NebiusSandbox {
                            target: target.clone(),
                        }
                    } else {
                        DispatchHost::Local
                    },
                    now,
                    180,
                )
                .unwrap();
            Self {
                dir,
                coordinator,
                lease: claimed.lease,
                policy: GpuBudgetPolicy {
                    version: 1,
                    target,
                    currency: "USD".into(),
                    quote_id: "approved-fixture-bound".into(),
                    valid_until: now + 3600,
                    reservation_micros: 10000,
                    run_budget_micros: 10000,
                    max_jobs: 4,
                    max_parallel: 2,
                    result_volume: None,
                },
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            for name in ["control.db", "control.db-wal", "control.db-shm"] {
                let _ = std::fs::remove_file(self.dir.join(name));
            }
            let _ = std::fs::remove_dir(&self.dir);
        }
    }
    pub(crate) fn bytes(v: Value) -> Vec<u8> {
        serde_json::to_vec(&v).unwrap()
    }
    pub(super) fn client(s: &Server) -> Client {
        Client {
            http: reqwest::blocking::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(3))
                .build()
                .unwrap(),
            token: "fixture-only".into(),
            project: "project-fixture".into(),
            base: s.url.clone(),
        }
    }
    pub(crate) fn job(l: &Launch, state: &str) -> Value {
        let mut v = l.body().unwrap();
        v["metadata"]["id"] = json!("aijob-fixture");
        v["status"] = json!({"state":state});
        v
    }
    #[test]
    fn team_gpu_provider_timeout_minimum_is_separate_from_command_and_original_team_deadlines() {
        let f = Fixture::new(true, true);
        let now = crate::model::unix_time();
        let mut l = launch();
        for seconds in [0, 1, 60, 3599, 86401, 604800] {
            l.timeout_seconds = seconds;
            assert!(
                l.body()
                    .unwrap_err()
                    .to_string()
                    .contains("GPU job timeout")
            );
        }
        l.timeout_seconds = 3600;
        let body = l.body().unwrap();
        assert_eq!(body["spec"]["timeout"], "3600s");
        assert_eq!(l.request.timeout_seconds, 30);
        for seconds in [0, 60, 3599, 86401] {
            let mut invalid = body.clone();
            invalid["spec"]["timeout"] = json!(format!("{seconds}s"));
            assert!(
                f.coordinator
                    .reserve_gpu_launch(&f.lease, &l.request, &invalid, &f.policy, now)
                    .is_err()
            );
            assert!(
                f.coordinator
                    .gpu_jobs(f.lease.attempt_id())
                    .unwrap()
                    .is_empty()
            );
        }
        // Valid provider timeout still cannot expand the original run deadline.
        let mut long = body.clone();
        long["spec"]["timeout"] = json!("86400s");
        assert!(
            f.coordinator
                .reserve_gpu_launch(&f.lease, &l.request, &long, &f.policy, now)
                .unwrap_err()
                .to_string()
                .contains("original team deadline")
        );
        assert!(
            f.coordinator
                .gpu_jobs(f.lease.attempt_id())
                .unwrap()
                .is_empty()
        );
        assert!(
            f.coordinator
                .reserve_gpu_launch(&f.lease, &l.request, &body, &f.policy, now)
                .unwrap()
        );
        let saved = f
            .coordinator
            .gpu_jobs(f.lease.attempt_id())
            .unwrap()
            .remove(0);
        assert_eq!(saved.deadline_at, now + 3600);
        assert_eq!(saved.body, body);
        l.timeout_seconds = 86400;
        assert!(l.body().is_ok());
    }
    #[test]
    fn team_gpu_real_api_contract_keeps_creation_job_and_cancel_identities_separate() {
        let l = launch();
        let server = Server::new(vec![
            (
                200,
                bytes(json!({"id":"operation-create","resourceId":"aijob-fixture"})),
            ),
            (
                200,
                bytes(json!({"id":"operation-create","resourceId":"aijob-fixture"})),
            ),
            (200, bytes(job(&l, "RUNNING"))),
            (
                200,
                bytes(
                    json!({"id":"operation-cancel","resourceId":"aijob-fixture","status":{"code":0}}),
                ),
            ),
            (200, bytes(job(&l, "CANCELLED"))),
        ]);
        let c = client(&server);
        let ack = c.submit(&l).unwrap();
        assert!(!c.creation_finished(&ack).unwrap());
        let observed = c.observe(&l, &ack).unwrap();
        assert_eq!(observed.state, "RUNNING");
        assert!(!observed.terminal);
        assert_eq!(c.cancel(&ack, Uuid::new_v4()).unwrap(), "operation-cancel");
        assert!(c.observe(&l, &ack).unwrap().terminal);
        let requests = server.finish();
        assert_eq!(requests.len(), 5);
        assert!(requests[0].line.starts_with("POST /ai/v1/jobs "));
        assert_eq!(
            requests[0].body["spec"]["args"],
            "--request-file /ion/job-request.json"
        );
        assert_eq!(requests[0].body["spec"]["publicIp"], false);
        assert!(requests[2].line.contains("view=SECRET"));
        assert_eq!(requests[3].body, json!({"id":"aijob-fixture"}));
    }
    #[test]
    fn team_gpu_durable_launch_replay_stop_and_terminal_keep_original_reservations() {
        let f = Fixture::new(true, true);
        let l = launch();
        let mut running = job(&l, "RUNNING");
        running["status"]["instances"] =
            json!([{"computeInstanceId":"compute-fixture","state":"RUNNING"}]);
        let terminal = job(&l, "CANCELLED");
        let vm = json!({"metadata":{"id":"compute-fixture","parentId":"project-fixture"},
            "spec":{"resources":{"platform":l.platform,"preset":l.preset},"bootDisk":{"managedDisk":{"name":"gpu-main","spec":{}}}},
            "status":{"diskAttachments":[{"id":"disk-fixture","name":"gpu-main","isManaged":true}]}});
        let disk = json!({"metadata":{"id":"disk-fixture","parentId":"project-fixture"}});
        let server = Server::new(vec![
            (
                200,
                bytes(json!({"id":"operation-create","resourceId":"aijob-fixture"})),
            ),
            (200, bytes(running)),
            (200, bytes(vm)),
            (200, bytes(disk)),
            (
                200,
                bytes(json!({"id":"operation-cancel","resourceId":"aijob-fixture"})),
            ),
            (200, bytes(terminal)),
            (404, bytes(json!({"code":5}))),
            (
                200,
                bytes(json!({"id":"operation-delete","resourceId":"aijob-fixture"})),
            ),
        ]);
        let c = client(&server);
        let record = launch_once_with(&c, &f.coordinator, &f.lease, &l, &f.policy).unwrap();
        assert_eq!(record.job_id.as_deref(), Some("aijob-fixture"));
        assert_eq!(
            launch_once_with(&c, &f.coordinator, &f.lease, &l, &f.policy)
                .unwrap()
                .creation_operation,
            record.creation_operation
        );
        let reopened = TeamCoordinator::new(f.dir.join("control.db"));
        assert!(
            !reconcile_with(
                &c,
                &reopened,
                reopened.gpu_jobs(f.lease.attempt_id()).unwrap()
            )
            .unwrap()
        );
        assert!(
            !reconcile_with(
                &c,
                &reopened,
                reopened.gpu_jobs(f.lease.attempt_id()).unwrap()
            )
            .unwrap()
        );
        let record = reopened.gpu_jobs(f.lease.attempt_id()).unwrap().remove(0);
        assert_eq!(record.state, "terminal");
        assert_eq!(record.compute_ids, vec!["compute-fixture"]);
        assert_eq!(
            record.cancellation_operation.as_deref(),
            Some("operation-cancel")
        );
        assert!(record.cancel_requested);
        assert!(reopened.has_pending_effect(f.lease.run_id()).unwrap());
        assert!(
            !reopened
                .remote_attempt_stopped(f.lease.attempt_id())
                .unwrap()
        );
        let mut second = launch();
        second.request.request_id = Uuid::new_v4();
        let error = reopened
            .reserve_gpu_launch(
                &f.lease,
                &second.request,
                &second.body().unwrap(),
                &f.policy,
                crate::model::unix_time(),
            )
            .unwrap_err();
        assert!(error.to_string().contains("reservation budget reached"));
        let requests = server.finish();
        assert_eq!(requests.len(), 8);
        assert_eq!(
            requests
                .iter()
                .filter(|r| r.line.starts_with("POST /ai/v1/jobs "))
                .count(),
            1
        );
        assert_eq!(
            requests
                .iter()
                .filter(|r| r.line.starts_with("POST /ai/v1/jobs/cancel "))
                .count(),
            1
        );
    }
    #[test]
    fn team_gpu_lost_create_ack_replays_saved_state_without_second_submission_or_fallback() {
        let f = Fixture::new(true, true);
        let l = launch();
        let server = Server::new(vec![(500, bytes(json!({})))]);
        let c = client(&server);
        assert!(launch_once_with(&c, &f.coordinator, &f.lease, &l, &f.policy).is_err());
        let record = launch_once_with(&c, &f.coordinator, &f.lease, &l, &f.policy).unwrap();
        assert_eq!(record.state, "submitting");
        assert!(record.job_id.is_none());
        assert!(!reconcile_with(&c, &f.coordinator, vec![record]).unwrap());
        let mut changed = launch();
        changed.request.request_id = l.request.request_id;
        changed.request.argv.push("changed".into());
        assert!(launch_once_with(&c, &f.coordinator, &f.lease, &changed, &f.policy).is_err());
        assert!(
            !f.coordinator
                .remote_attempt_stopped(f.lease.attempt_id())
                .unwrap()
        );
        assert!(f.coordinator.has_pending_effect(f.lease.run_id()).unwrap());
        assert_eq!(server.finish().len(), 1);
    }
    #[test]
    fn team_gpu_lost_cancel_ack_does_not_duplicate_cancellation_or_release_capacity() {
        let f = Fixture::new(true, true);
        let l = launch();
        let server = Server::new(vec![
            (
                200,
                bytes(json!({"id":"operation-create","resourceId":"aijob-fixture"})),
            ),
            (200, bytes(job(&l, "RUNNING"))),
            (500, bytes(json!({}))),
            (200, bytes(job(&l, "RUNNING"))),
        ]);
        let c = client(&server);
        launch_once_with(&c, &f.coordinator, &f.lease, &l, &f.policy).unwrap();
        assert!(
            reconcile_with(
                &c,
                &f.coordinator,
                f.coordinator.gpu_jobs(f.lease.attempt_id()).unwrap()
            )
            .is_err()
        );
        let before = f.coordinator.snapshot(f.lease.run_id()).unwrap().revision;
        assert!(
            !reconcile_with(
                &c,
                &f.coordinator,
                f.coordinator.gpu_jobs(f.lease.attempt_id()).unwrap()
            )
            .unwrap()
        );
        let record = f
            .coordinator
            .gpu_jobs(f.lease.attempt_id())
            .unwrap()
            .remove(0);
        assert_eq!(
            f.coordinator.snapshot(f.lease.run_id()).unwrap().revision,
            before
        );
        assert_eq!(record.state, "canceling");
        assert!(record.cancel_requested);
        assert!(record.cancellation_operation.is_none());
        let requests = server.finish();
        assert_eq!(requests.len(), 4);
        assert_eq!(
            requests
                .iter()
                .filter(|r| r.line.starts_with("POST /ai/v1/jobs/cancel "))
                .count(),
            1
        );
    }
    #[test]
    fn team_gpu_authorization_currency_quote_and_policy_fail_before_network() {
        let l = launch();
        let server = Server::new(vec![]);
        let c = client(&server);
        let local = Fixture::new(false, true);
        assert!(launch_once_with(&c, &local.coordinator, &local.lease, &l, &local.policy).is_err());
        let no_network = Fixture::new(true, false);
        assert!(
            launch_once_with(
                &c,
                &no_network.coordinator,
                &no_network.lease,
                &l,
                &no_network.policy
            )
            .is_err()
        );
        let f = Fixture::new(true, true);
        for mutate in 0..4 {
            let mut policy = f.policy.clone();
            match mutate {
                0 => policy.valid_until = crate::model::unix_time(),
                1 => policy.currency = "usd".into(),
                2 => policy.target = "wrong-target".into(),
                _ => policy.reservation_micros = policy.run_budget_micros + 1,
            }
            assert!(launch_once_with(&c, &f.coordinator, &f.lease, &l, &policy).is_err());
        }
        assert!(
            f.coordinator
                .gpu_jobs(f.lease.attempt_id())
                .unwrap()
                .is_empty()
        );
        assert!(server.finish().is_empty());
    }
    #[test]
    fn team_gpu_rejects_mutable_images_oversized_configs_and_changed_job_provenance() {
        let mut l = launch();
        let original = l.image.clone();
        l.image = "registry.example/image:latest".into();
        assert!(l.body().is_err());
        l.image = original;
        l.request.inputs.push(crate::teams::jobs::JobFile {
            path: "data.bin".into(),
            content_base64: STANDARD.encode(vec![0; 64 * 1024]),
        });
        assert!(l.body().is_err());
        l.request.inputs.clear();
        let mut wrong = job(&l, "COMPLETED");
        wrong["spec"]["image"] = json!("unapproved:latest");
        let server = Server::new(vec![(200, bytes(wrong))]);
        let c = client(&server);
        assert!(
            c.observe(
                &l,
                &Acknowledged {
                    operation: "operation-create".into(),
                    job: "aijob-fixture".into()
                }
            )
            .is_err()
        );
        assert_eq!(server.finish().len(), 1);
        let server = Server::new(vec![(500, bytes(json!({})))]);
        let c = client(&server);
        assert!(c.submit(&l).is_err());
        assert_eq!(server.finish().len(), 1);
        let server = Server::new(vec![(
            200,
            bytes(json!({"id":"operation-create","resourceId":"aijob-fixture"})),
        )]);
        let c = client(&server);
        assert!(
            c.cancel(
                &Acknowledged {
                    operation: "operation-create".into(),
                    job: "aijob-fixture".into()
                },
                Uuid::new_v4()
            )
            .is_err()
        );
        assert_eq!(server.finish().len(), 1);
    }
}
