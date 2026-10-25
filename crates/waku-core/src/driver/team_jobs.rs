//! Explicit file bundles and actual commands in credential-free ConTree job images.
//! Explicit project approval is required for snapshots; no model credentials or local retry.
mod project;
use super::team_remote::{Client, read_json};
use crate::teams::{AssignmentLease, ClaimedAssignment, TeamCoordinator, jobs::*};
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
pub(crate) use project::{ProjectArgs, run_project};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use std::{
    collections::HashSet,
    io::Read,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    version: u16,
    target: Uuid,
    image: Uuid,
    credential_free: bool,
    max_commands: u16,
    max_seconds: u32,
    max_layer_bytes: u64,
    uid: u32,
    gid: u32,
    executables: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    resources: Option<ResourceLimits>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    project_snapshot: Option<crate::teams::project::ProjectPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    admission: Option<crate::teams::jobs::admission::AdmissionPolicy>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceLimits {
    version: u16,
    cpu_seconds: u32,
    address_space_bytes: u64,
    max_file_bytes: u64,
    processes: u32,
    open_files: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    group: Option<ResourceGroupLimits>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceGroupLimits {
    version: u16,
    cpu_millis: u32,
    memory_bytes: u64,
    processes: u32,
}

/// Prepared only from the reviewed managed registry; consumed after the native
/// chat asks the user to approve these exact arguments and image.
pub(super) struct PreparedSoloCommand {
    client: Client,
    policy: Policy,
    request: JobRequest,
}
pub(super) fn prepare_solo_command(argv: Vec<String>, id: Uuid) -> Result<PreparedSoloCommand> {
    let client = Client::configured()?;
    let target = client.default_command_target()?;
    let policy: Policy = serde_json::from_value(
        client
            .command_policy(target)?
            .context("Managed command policy unavailable")?,
    )?;
    validate_policy(&policy)?;
    ensure!(
        policy.resources.is_some(),
        "Command resource policy is unavailable"
    );
    let timeout = policy.max_seconds.min(120);
    let mut args = CommandArgs {
        argv,
        input_paths: vec![],
        output_paths: vec![],
        timeout_seconds: timeout,
    };
    admit_args(&policy, &mut args)?;
    let argv = args.argv;
    manifest(&client, &policy)?;
    Ok(PreparedSoloCommand {
        client,
        policy,
        request: JobRequest {
            version: 1,
            request_id: id,
            argv,
            timeout_seconds: timeout,
            inputs: vec![],
            output_paths: vec![],
            project_snapshot: None,
        },
    })
}
impl PreparedSoloCommand {
    pub(super) fn description(&self) -> Result<String> {
        Ok(format!(
            "Image: {}\nExact arguments: {}\nMaximum runtime: {} seconds\nNo local project files or credentials are uploaded.",
            self.policy.image,
            serde_json::to_string(&self.request.argv)?,
            self.request.timeout_seconds
        ))
    }
    pub(super) fn execute(self, cancelled: &AtomicBool) -> Result<String> {
        ensure!(
            !cancelled.load(Ordering::Acquire),
            "Stopped before sandbox submission"
        );
        let response=self.client.request(reqwest::Method::POST,"/instances")?.json(&body(&self.policy,&self.request)?).send()
            .with_context(||format!("Sandbox request {} has an uncertain acknowledgement; inspect it before further work",self.request.request_id))?;
        ensure!(
            response.status().as_u16() == 201,
            "Sandbox request {} was not acknowledged; no local fallback or resubmission",
            self.request.request_id
        );
        let value = read_json(response, 64 * 1024)?;
        let operation = Uuid::parse_str(
            value["uuid"]
                .as_str()
                .context("Sandbox operation acknowledgement missing")?,
        )?;
        let deadline =
            Instant::now() + Duration::from_secs(u64::from(self.request.timeout_seconds) + 15);
        let mut stopping = false;
        loop {
            if !stopping && (cancelled.load(Ordering::Acquire) || Instant::now() >= deadline) {
                self.client.cancel(operation)?;
                stopping = true;
            }
            let value =
                self.client
                    .operation_bounded(operation, self.policy.image, 3 * 1024 * 1024)?;
            if matches!(
                value["status"].as_str(),
                Some("SUCCESS" | "FAILED" | "CANCELLED")
            ) {
                ensure!(
                    !stopping,
                    "Sandbox operation {operation} is confirmed stopped after cancellation"
                );
                ensure!(
                    value["status"] == "SUCCESS",
                    "Sandbox operation {operation} stopped without a successful worker receipt"
                );
                let mut result = receipt(&value, &self.request)?.summary()?;
                result["operation_id"] = json!(operation);
                return Ok(result.to_string());
            }
            ensure!(
                Instant::now() < deadline + Duration::from_secs(15),
                "Sandbox operation {operation} stop is unconfirmed; its spending hold remains"
            );
            thread::sleep(Duration::from_millis(500));
        }
    }
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CommandArgs {
    pub argv: Vec<String>,
    pub input_paths: Vec<String>,
    pub output_paths: Vec<String>,
    pub timeout_seconds: u32,
}
pub(super) fn path(value: &str) -> Result<()> {
    crate::teams::jobs::path(value)
}

fn policy(target: Uuid) -> Result<Policy> {
    let file = std::env::var_os("ION_TEAM_COMMAND_IMAGES")
        .context("Register credential-free command images in ION_TEAM_COMMAND_IMAGES first")?;
    let mut data = Vec::new();
    std::fs::File::open(file)?
        .take(64 * 1024 + 1)
        .read_to_end(&mut data)?;
    ensure!(
        data.len() <= 64 * 1024,
        "Command image registry exceeds its bound"
    );
    let policies: Vec<Policy> = serde_json::from_slice(&data)
        .map_err(|_| anyhow::anyhow!("Command image registry is invalid"))?;
    ensure!(policies.len() <= 128, "Too many registered command images");
    let mut selected = policies.into_iter().filter(|p| p.target == target);
    let policy = selected
        .next()
        .context("No command image is registered for this agent's pinned sandbox target")?;
    ensure!(
        selected.next().is_none(),
        "Command image target is ambiguous"
    );
    validate_policy(&policy)?;
    Ok(policy)
}
fn validate_policy(p: &Policy) -> Result<()> {
    ensure!(
        p.version == 1
            && !p.target.is_nil()
            && !p.image.is_nil()
            && p.credential_free
            && (1..=128).contains(&p.max_commands)
            && (1..=120).contains(&p.max_seconds)
            && (1..=1024 * 1024 * 1024).contains(&p.max_layer_bytes)
            && p.uid > 0
            && p.gid > 0
            && !p.executables.is_empty()
            && p.executables.len() <= 64,
        "Invalid or credential-bearing command image policy"
    );
    if let Some(project) = &p.project_snapshot {
        project.validate()?;
        ensure!(
            2 * project.max_bytes + 2 * 1024 * 1024 + 12 <= p.max_layer_bytes,
            "Project archive exceeds image layer limit"
        );
    }
    if let Some(r) = &p.resources {
        ensure!(
            r.version == 1
                && (1..=p.max_seconds).contains(&r.cpu_seconds)
                && (64 * 1024 * 1024..=8 * 1024 * 1024 * 1024).contains(&r.address_space_bytes)
                && (64 * 1024..=p.max_layer_bytes).contains(&r.max_file_bytes)
                && (1..=256).contains(&r.processes)
                && (16..=256).contains(&r.open_files),
            "Command child resource policy exceeds its bound"
        );
    }
    if let Some(group) = p.resources.as_ref().and_then(|r| r.group.as_ref()) {
        ensure!(
            group.version == 1
                && (100..=32000).contains(&group.cpu_millis)
                && (64 * 1024 * 1024..=8 * 1024 * 1024 * 1024).contains(&group.memory_bytes)
                && (2..=256).contains(&group.processes)
                && group.processes <= p.resources.as_ref().unwrap().processes,
            "Sandbox aggregate resource policy exceeds its bound"
        );
    }
    crate::teams::jobs::admission::validate_policy(&serde_json::to_value(p)?)?;
    let mut seen = HashSet::new();
    for executable in &p.executables {
        ensure!(
            executable.starts_with('/')
                && executable.len() <= 512
                && !executable.chars().any(char::is_control)
                && !executable.contains(['\\', ':'])
                && executable
                    .split('/')
                    .skip(1)
                    .all(|s| !s.is_empty() && !matches!(s, "." | ".."))
                && seen.insert(executable),
            "Invalid or duplicate image executable"
        );
    }
    Ok(())
}
// Models usually name a program, not its path; resolve only to an executable
// the policy already allows, and name the allowed set when admission fails.
fn admit_args(p: &Policy, a: &mut CommandArgs) -> Result<()> {
    if let Some(path) = a.argv.first().filter(|name| !name.contains('/')).and_then(|name| {
        p.executables
            .iter()
            .find(|path| path.rsplit('/').next() == Some(name.as_str()))
    }) {
        a.argv[0] = path.clone();
    }
    validate_args(p, a).map_err(|error| {
        anyhow::anyhow!("{error}. Allowed executables: {}", p.executables.join(", "))
    })
}
fn validate_args(p: &Policy, a: &CommandArgs) -> Result<()> {
    ensure!(
        (1..=64).contains(&a.argv.len())
            && a.argv.iter().all(|v| v.len() <= 8192 && !v.contains('\0'))
            && a.argv.iter().map(String::len).sum::<usize>() <= 32 * 1024
            && p.executables.contains(&a.argv[0])
            && (1..=p.max_seconds).contains(&a.timeout_seconds),
        "Command executable, arguments or time exceed the registered policy"
    );
    for paths in [&a.input_paths, &a.output_paths] {
        ensure!(paths.len() <= 32, "Too many transferred files");
        let mut seen = HashSet::new();
        for file in paths {
            path(file)?;
            ensure!(
                seen.insert(file.to_ascii_lowercase()),
                "Duplicate transfer path"
            );
        }
    }
    Ok(())
}
fn manifest(client: &Client, p: &Policy) -> Result<Value> {
    let response = client
        .request(
            reqwest::Method::GET,
            &format!("/inspect/{}/download", p.image),
        )?
        .query(&[("path", "/ion/job-host.json")])
        .send()?;
    ensure!(
        response.status().is_success(),
        "Registered command image is unavailable before submission"
    );
    let host = read_json(response, 16 * 1024)?;
    let original = json!({"version":1,"worker":"ion-job-worker/1","credential_free":true,"network":"disabled","supervisor":"root","child":"nonroot","network_boundary":"linux-seccomp-socket-deny/v1","resource_boundary":"linux-rlimit/v1"});
    let mut extended = original.clone();
    extended["project_snapshot"] = json!("ion-project-archive/v1");
    let mut original_group = original.clone();
    original_group["resource_group_boundary"] = json!("linux-cgroup2-sandbox/v1");
    let mut extended_group = extended.clone();
    extended_group["resource_group_boundary"] = json!("linux-cgroup2-sandbox/v1");
    let group_approved = p.resources.as_ref().is_some_and(|r| r.group.is_some());
    ensure!(
        if group_approved {
            host == original_group || host == extended_group
        } else {
            host == original || host == extended || host == original_group || host == extended_group
        },
        "Command worker/image contract changed"
    );
    Ok(host)
}
fn bundle(request: Uuid, a: &CommandArgs, workspace: &Path) -> Result<JobRequest> {
    let mut inputs = Vec::new();
    let mut total = 0;
    for relative in &a.input_paths {
        path(relative)?;
        let file = super::nebius::checked_path(workspace, relative)?;
        let meta = std::fs::symlink_metadata(&file)?;
        ensure!(
            meta.is_file() && !meta.file_type().is_symlink() && meta.len() <= 64 * 1024,
            "Upload only selected bounded regular files"
        );
        let mut bytes = Vec::new();
        std::fs::File::open(file)?
            .take(64 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() <= 64 * 1024,
            "Input file changed beyond its bound"
        );
        total += bytes.len();
        ensure!(total <= 256 * 1024, "Job inputs exceed the transfer limit");
        inputs.push(JobFile {
            path: relative.clone(),
            content_base64: STANDARD.encode(bytes),
        });
    }
    Ok(JobRequest {
        project_snapshot: None,
        version: 1,
        request_id: request,
        argv: a.argv.clone(),
        timeout_seconds: a.timeout_seconds,
        inputs,
        output_paths: a.output_paths.clone(),
    })
}
fn body(policy: &Policy, request: &JobRequest) -> Result<Value> {
    let input = serde_json::to_vec(request)?;
    ensure!(input.len() <= 512 * 1024, "Job input exceeds its bound");
    let mut body = json!({"command":"/usr/local/bin/ion-job-worker","args":[],"image":policy.image,"shell":false,
        "hostname":format!("ion-job-{}",request.request_id),"env":{"PATH":"/usr/local/bin:/usr/bin:/bin","ION_JOB_ISOLATED":"1","ION_JOB_CHILD_UID":policy.uid.to_string(),"ION_JOB_CHILD_GID":policy.gid.to_string()},
        "cwd":"/ion","uid":0,"gid":0,"stdin":{"value":STANDARD.encode(input),"encoding":"base64","close":true},
        "timeout":request.timeout_seconds+10,"truncate_output_at":512*1024,"disposable":true,"preserve_env":false,
        "resources_limits":{"max_layer_bytes":policy.max_layer_bytes},"networking":{"enabled":false}});
    if let Some(resources) = &policy.resources {
        body["env"]["ION_JOB_RESOURCE_LIMITS"] = json!(serde_json::to_string(resources)?);
    }
    Ok(body)
}
fn receipt(value: &Value, request: &JobRequest) -> Result<JobResponse> {
    let result = &value["metadata"]["result"];
    ensure!(
        result["state"]["exit_code"] == 0
            && result["state"]["timed_out"] != true
            && result["state"]["core_dump"] != true
            && result["state"]["stopped"] != true
            && (result["state"]["signal"].is_null()
                || result["state"]["signal"]
                    .as_i64()
                    .is_some_and(|s| matches!(s, -1 | 0))),
        "Remote worker did not exit with a receipt"
    );
    let stdout = &result["stdout"];
    ensure!(
        stdout["truncated"] == false,
        "Remote job receipt was truncated"
    );
    let encoded = stdout["value"].as_str().context("Job receipt is missing")?;
    ensure!(encoded.len() <= 700 * 1024, "Job receipt exceeds its bound");
    // ConTree reports printable output as ascii and anything else as base64.
    let bytes = match stdout["encoding"].as_str() {
        Some("base64") => STANDARD.decode(encoded)?,
        Some("ascii") => encoded.as_bytes().to_vec(),
        _ => anyhow::bail!("Remote job receipt uses an unsupported encoding"),
    };
    ensure!(bytes.len() <= 512 * 1024, "Job receipt exceeds its bound");
    let response: JobResponse = serde_json::from_slice(&bytes)?;
    validate_receipt(&response, request)?;
    Ok(response)
}
fn validate_receipt(r: &JobResponse, request: &JobRequest) -> Result<()> {
    r.validate_against(request)
}

fn execute_with(
    client: &Client,
    coordinator: &TeamCoordinator,
    lease: &AssignmentLease,
    p: &Policy,
    request: &JobRequest,
    cancelled: &AtomicBool,
) -> Result<JobResponse> {
    if cancelled.load(Ordering::Acquire)
        || coordinator
            .authorize_remote_job(lease, crate::model::unix_time())
            .is_err()
    {
        coordinator.reject_job(request.request_id)?;
        anyhow::bail!("Stopped before command submission");
    }
    let response=client.request(reqwest::Method::POST,"/instances")?.json(&project::body_for(coordinator,p,request)?).send()
        .context("Command spawn acknowledgement lost; observe this request before continuing; no retry or local fallback")?;
    if matches!(response.status().as_u16(), 400 | 401 | 403) {
        coordinator.reject_job(request.request_id)?;
        anyhow::bail!("Remote command was rejected before launch");
    }
    ensure!(
        response.status().as_u16() == 201,
        "Command submission outcome is unresolved"
    );
    let operation = Uuid::parse_str(
        read_json(response, 64 * 1024)?["uuid"]
            .as_str()
            .context("Job acknowledgement has no operation ID")?,
    )?;
    coordinator.bind_job_operation(request.request_id, operation)?;
    let deadline = Instant::now() + Duration::from_secs(u64::from(request.timeout_seconds) + 15);
    let mut canceling = false;
    loop {
        if !canceling
            && (cancelled.load(Ordering::Acquire)
                || Instant::now() >= deadline
                || coordinator
                    .authorize_remote_job(lease, crate::model::unix_time())
                    .is_err())
        {
            client.cancel(operation)?;
            coordinator.job_canceling(request.request_id)?;
            canceling = true;
        }
        let value = client.operation_bounded(operation, p.image, 3 * 1024 * 1024)?;
        if matches!(
            value["status"].as_str(),
            Some("SUCCESS" | "FAILED" | "CANCELLED")
        ) {
            let output = if value["status"] == "SUCCESS" {
                receipt(&value, request)
            } else {
                Err(anyhow::anyhow!(
                    "Remote command worker stopped without a receipt"
                ))
            };
            coordinator.finish_job(request.request_id, output.as_ref().ok())?;
            ensure!(
                !canceling,
                "Remote command stopped after cancellation; its terminal evidence has been recorded"
            );
            return output;
        }
        ensure!(
            Instant::now() < deadline + Duration::from_secs(15),
            "Remote command stop is unconfirmed; its original operation remains reserved"
        );
        thread::sleep(Duration::from_millis(200));
    }
}
pub(crate) fn run(
    coordinator: &TeamCoordinator,
    claimed: &ClaimedAssignment,
    workspace: &Path,
    request_id: Uuid,
    args: CommandArgs,
    cancelled: &AtomicBool,
) -> Result<String> {
    run_with_host(
        coordinator,
        claimed,
        workspace,
        request_id,
        args,
        cancelled,
        |target| {
            let client = Client::configured()?;
            let policy = match client.command_policy(target)? {
                Some(p) => serde_json::from_value(p)?,
                None => policy(target)?,
            };
            Ok((policy, client))
        },
    )
}

// Only host configuration is injectable; authorization, file transfer, preflight,
// reservation, operation observation and artifact publication share this path.
#[allow(clippy::too_many_arguments)]
fn run_with_host(
    coordinator: &TeamCoordinator,
    claimed: &ClaimedAssignment,
    workspace: &Path,
    request_id: Uuid,
    mut args: CommandArgs,
    cancelled: &AtomicBool,
    configure: impl FnOnce(Uuid) -> Result<(Policy, Client)>,
) -> Result<String> {
    let profile = coordinator.authorize_remote_job(&claimed.lease, crate::model::unix_time())?;
    let target = profile
        .execution
        .sandbox_target
        .as_deref()
        .context("Pinned sandbox target missing")?;
    let target = Uuid::parse_str(target.rsplit('@').next().unwrap_or(target))?;
    let (policy, client) = configure(target)?;
    validate_policy(&policy)?;
    ensure!(
        policy.resources.is_some(),
        "Approve child resource limits in the command image registry before new commands"
    );
    ensure!(
        policy.target == target,
        "Command host differs from the pinned sandbox target"
    );
    admit_args(&policy, &mut args)?;
    let check_context =
        coordinator.command_check_context(&claimed.lease, workspace, crate::model::unix_time())?;
    let request = bundle(request_id, &args, workspace)?;
    // Validate the full wire request before creating a durable submitting record.
    body(&policy, &request)?;
    manifest(&client, &policy)?;
    let output = match coordinator.reserve_remote_job_with_context(
        &claimed.lease,
        &request,
        policy.image,
        &client.project,
        &serde_json::to_string(&policy)?,
        policy.max_commands,
        crate::model::unix_time(),
        check_context.as_ref(),
    )? {
        Some(receipt) => receipt,
        None => execute_with(
            &client,
            coordinator,
            &claimed.lease,
            &policy,
            &request,
            cancelled,
        )?,
    };
    // The receipt and artifact were committed together, including after Stop.
    let artifact = coordinator.publish_document(
        &claimed.lease,
        request_id,
        &format!("command-{request_id}"),
        "json",
        &output.summary()?.to_string(),
        crate::model::unix_time(),
    )?;
    output.tool_reply(artifact.id)
}
pub(crate) fn read_output(
    coordinator: &TeamCoordinator,
    lease: &AssignmentLease,
    request: Uuid,
    path: &str,
    offset: usize,
) -> Result<String> {
    let receipt = coordinator.job_receipt(lease, request, crate::model::unix_time())?;
    let output = receipt
        .outputs
        .iter()
        .find(|o| o.path == path)
        .context("This command did not return that file")?;
    let bytes = STANDARD.decode(&output.content_base64)?;
    ensure!(offset <= bytes.len(), "Output offset is outside the file");
    let end = (offset + 4096).min(bytes.len());
    Ok(json!({"path":path,"offset":offset,"next_offset":end,"total_bytes":bytes.len(),"content_base64":STANDARD.encode(&bytes[offset..end]),"sha256":output.sha256}).to_string())
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn bind_recovered_operation(
    coordinator: &TeamCoordinator,
    run: Uuid,
    work: Uuid,
    attempt: Uuid,
    revision: u64,
    request: Uuid,
    operation: Uuid,
) -> Result<()> {
    bind_recovered_with(
        &Client::configured()?,
        coordinator,
        run,
        work,
        attempt,
        revision,
        request,
        operation,
    )
}
#[allow(clippy::too_many_arguments)]
fn bind_recovered_with(
    client: &Client,
    coordinator: &TeamCoordinator,
    run: Uuid,
    work: Uuid,
    attempt: Uuid,
    revision: u64,
    request: Uuid,
    operation: Uuid,
) -> Result<()> {
    use waku_protocol::teams::{ExecutionEnvironment, TeamRecoveryConfirmation};
    if coordinator.recovery_confirmation_replayed(
        run,
        work,
        attempt,
        revision,
        &TeamRecoveryConfirmation::BindRemoteOperation {
            request_id: request,
            operation_id: operation,
        },
    )? {
        return Ok(());
    }
    ensure!(
        !operation.is_nil(),
        "Choose a non-empty command operation ID"
    );
    let snapshot = coordinator.snapshot(run)?;
    ensure!(
        snapshot.revision == revision,
        "Run changed; refresh command recovery"
    );
    let assignment = snapshot
        .work
        .iter()
        .find(|w| w.id == work)
        .context("Command assignment missing")?;
    ensure!(
        assignment.can_reconcile
            && assignment.attempts.last().is_some_and(
                |a| a.id == attempt && a.environment == ExecutionEnvironment::NebiusSandbox
            ),
        "Command recovery attempt/state changed"
    );
    let record = coordinator
        .unfinished_jobs(attempt)?
        .into_iter()
        .find(|j| j.id == request)
        .context("Original command request missing")?;
    ensure!(
        record.state == "submitting"
            && record.operation.is_none()
            && record.project == client.project,
        "Command request/project changed or already bound"
    );
    let policy: Policy = serde_json::from_str(&record.policy)?;
    validate_policy(&policy)?;
    ensure!(policy.image == record.image, "Stored command image changed");
    let expected = project::body_for(coordinator, &policy, &record.request)?;
    client.recover_operation(request, operation)?;
    let observed = client.operation_bounded(operation, record.image, 3 * 1024 * 1024)?;
    verify_original_command(&observed, &expected)?;
    coordinator.bind_recovered_job_operation(
        run,
        work,
        attempt,
        revision,
        request,
        operation,
        crate::model::unix_time(),
    )
}
fn verify_original_command(observed: &Value, expected: &Value) -> Result<()> {
    let metadata = &observed["metadata"];
    for (key, value) in expected.as_object().context("Saved command body invalid")? {
        if key == "stdin" {
            continue;
        }
        ensure!(
            metadata[key] == *value
                || (matches!(key.as_str(), "uid" | "gid")
                    && *value == 0
                    && metadata[key].is_null()),
            "Command operation differs from saved execution policy: {key}"
        );
    }
    ensure!(
        expected.get("files").is_some()
            || metadata
                .get("files")
                .is_none_or(|v| v.is_null() || *v == json!({})),
        "Command operation added unapproved files"
    );
    let stdin = &metadata["stdin"];
    ensure!(
        stdin["encoding"] == "base64" && stdin["close"] == true && stdin["truncated"] != true,
        "Original command input witness unavailable"
    );
    let input = stdin["value"]
        .as_str()
        .context("Original command input is missing")?;
    ensure!(
        input.len() <= 700 * 1024,
        "Original command input exceeds its bound"
    );
    let bytes = STANDARD.decode(input)?;
    ensure!(
        bytes.len() <= 512 * 1024
            && serde_json::from_slice::<Value>(&bytes)?
                == serde_json::from_slice::<Value>(
                    &STANDARD.decode(
                        expected["stdin"]["value"]
                            .as_str()
                            .context("Saved command input missing")?
                    )?
                )?,
        "Original command/files/arguments differ from saved request"
    );
    Ok(())
}
pub(crate) fn reconcile_attempt(coordinator: &TeamCoordinator, attempt: Uuid) -> Result<bool> {
    let jobs = coordinator.unfinished_jobs(attempt)?;
    if jobs.is_empty() {
        return Ok(true);
    }
    let client = Client::configured()?;
    reconcile_with(&client, coordinator, jobs)
}
fn reconcile_with(
    client: &Client,
    coordinator: &TeamCoordinator,
    jobs: Vec<JobRecord>,
) -> Result<bool> {
    let mut stopped = true;
    for job in jobs {
        ensure!(
            job.project == client.project,
            "Remote command project changed; use its original credential context"
        );
        if job.state == "staging" {
            // This state is persisted before any upload and cannot have dispatched
            // compute. Recovery stops it without a new upload or command.
            coordinator.reject_job(job.id)?;
            continue;
        }
        let Some(operation) = job.operation else {
            stopped = false;
            continue;
        };
        let value = client.operation_bounded(operation, job.image, 3 * 1024 * 1024)?;
        if matches!(
            value["status"].as_str(),
            Some("SUCCESS" | "FAILED" | "CANCELLED")
        ) {
            let output = if value["status"] == "SUCCESS" {
                receipt(&value, &job.request).ok()
            } else {
                None
            };
            coordinator.finish_job(job.id, output.as_ref())?;
        } else {
            if job.state != "canceling" {
                client.cancel(operation)?;
                coordinator.job_canceling(job.id)?;
            }
            stopped = false;
        }
    }
    Ok(stopped)
}

#[cfg(test)]
mod tests {
    use super::super::team_test_http::Server;
    use super::*;
    use crate::{
        control::ControlStore,
        teams::{DispatchHost, PrepareRun},
    };
    use sha2::{Digest, Sha256};
    use waku_protocol::{control::*, model::ProviderKind, teams::*};
    struct Fixture {
        dir: std::path::PathBuf,
        store: TeamCoordinator,
        claim: ClaimedAssignment,
        policy: Policy,
        request: JobRequest,
    }
    impl Fixture {
        fn new(remote: bool, shell: bool) -> Self {
            let dir = std::env::temp_dir().join(format!("ion-job-fixture-{}", Uuid::new_v4()));
            std::fs::create_dir(&dir).unwrap();
            let path = dir.join("control.db");
            let definitions = ControlStore::new(path.clone());
            let agent = Uuid::new_v4();
            let team = Uuid::new_v4();
            let target = Uuid::new_v4();
            let access = AgentAccess {
                shell,
                workspace_read: true,
                ..Default::default()
            };
            definitions
                .save(
                    definitions.load().unwrap().revision,
                    ControlRecord::Agent(AgentProfile {
                        id: agent,
                        name: "Job fixture".into(),
                        expertise: "Tests".into(),
                        instructions: "Run a check".into(),
                        provider: ProviderKind::Nebius,
                        model: Some("fixture".into()),
                        expected_output: "Receipt".into(),
                        execution: AgentExecution {
                            environment: if remote {
                                ExecutionEnvironment::NebiusSandbox
                            } else {
                                ExecutionEnvironment::Local
                            },
                            sandbox_target: if remote {
                                Some(target.to_string())
                            } else {
                                None
                            },
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
                        name: "Job team".into(),
                        objective: "Checks".into(),
                        limits: None,
                        members: vec![TeamMember {
                            agent_id: agent,
                            parent_id: None,
                        }],
                    }),
                )
                .unwrap();
            let store = TeamCoordinator::new(path);
            let now = crate::model::unix_time();
            let run = store
                .prepare(
                    PrepareRun {
                        account_scope: None,
                        id: Uuid::new_v4(),
                        root_task_id: Uuid::new_v4(),
                        project_id: Uuid::new_v4(),
                        team_id: team,
                        expected_definition_revision: definitions.load().unwrap().revision,
                        objective: "Execute check".into(),
                        limits: TeamRunLimits::default(),
                        access,
                        input_data: vec![],
                        source_attachments: vec![],
                        source_prompt: None,
                        origin: None,
                    },
                    now,
                )
                .unwrap();
            let claim = store
                .claim(
                    run.id,
                    run.work[0].id,
                    if remote {
                        DispatchHost::NebiusSandbox {
                            target: target.to_string(),
                        }
                    } else {
                        DispatchHost::Local
                    },
                    now,
                    180,
                )
                .unwrap();
            let policy = Policy {
                version: 1,
                target,
                image: Uuid::new_v4(),
                credential_free: true,
                max_commands: 1,
                max_seconds: 30,
                max_layer_bytes: 256 * 1024 * 1024,
                uid: 1000,
                gid: 1000,
                executables: vec!["/usr/bin/python3".into()],
                resources: None,
                project_snapshot: None,
                admission: None,
            };
            let request = JobRequest {
                project_snapshot: None,
                version: 1,
                request_id: Uuid::new_v4(),
                argv: vec!["/usr/bin/python3".into(), "check.py".into()],
                timeout_seconds: 10,
                inputs: vec![],
                output_paths: vec![],
            };
            Self {
                dir,
                store,
                claim,
                policy,
                request,
            }
        }
        fn reserve(&self) -> Result<Option<JobResponse>> {
            self.store.reserve_remote_job(
                &self.claim.lease,
                &self.request,
                self.policy.image,
                "fixture-project",
                &serde_json::to_string(&self.policy).unwrap(),
                self.policy.max_commands,
                crate::model::unix_time(),
            )
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(self.dir.join("input.bin"));
            let _ = std::fs::remove_file(self.dir.join("control.db"));
            let _ = std::fs::remove_dir(&self.dir);
        }
    }
    fn child_limits() -> ResourceLimits {
        ResourceLimits {
            version: 1,
            cpu_seconds: 5,
            address_space_bytes: 256 * 1024 * 1024,
            max_file_bytes: 16 * 1024 * 1024,
            processes: 256,
            open_files: 64,
            group: None,
        }
    }
    fn group_limits() -> ResourceGroupLimits {
        ResourceGroupLimits {
            version: 1,
            cpu_millis: 500,
            memory_bytes: 128 * 1024 * 1024,
            processes: 32,
        }
    }
    fn with_admission(f: &mut Fixture) {
        f.policy.resources = Some(child_limits());
        f.policy.resources.as_mut().unwrap().group = Some(group_limits());
        let cap = crate::teams::jobs::admission::CommandResourceVector {
            jobs: 1,
            cpu_millis: 500,
            memory_bytes: 128 * 1024 * 1024,
            processes: 32,
            storage_bytes: 1024 * 1024 * 1024,
        };
        f.policy.admission = Some(crate::teams::jobs::admission::AdmissionPolicy {
            version: 1,
            owner_limit: cap.clone(),
            project_limit: cap,
        });
    }
    #[test]
    fn team_job_admission_receipt_releases_compute_only_and_replay_has_one_hold() {
        let mut f = Fixture::new(true, true);
        with_admission(&mut f);
        f.reserve().unwrap();
        assert_eq!(f.store.command_resource_usage(None).unwrap().used.jobs, 1);
        f.store
            .bind_job_operation(f.request.request_id, Uuid::new_v4())
            .unwrap();
        f.store
            .finish_job(f.request.request_id, Some(&response(&f.request, 7)))
            .unwrap();
        let usage = f.store.command_resource_usage(None).unwrap();
        assert_eq!(usage.used.jobs, 0);
        assert_eq!(usage.used.storage_bytes, f.policy.max_layer_bytes);
        assert_eq!(f.reserve().unwrap().unwrap().exit_code, 7);
        let db = rusqlite::Connection::open(f.dir.join("control.db")).unwrap();
        assert_eq!(
            db.query_row("SELECT count(*) FROM ion_team_job_admission", [], |r| r
                .get::<_, u64>(0))
                .unwrap(),
            1
        );
    }
    #[test]
    fn team_job_admission_invalid_receipt_and_no_receipt_retain_compute() {
        let mut f = Fixture::new(true, true);
        with_admission(&mut f);
        f.reserve().unwrap();
        f.store
            .bind_job_operation(f.request.request_id, Uuid::new_v4())
            .unwrap();
        let mut invalid = response(&f.request, 0);
        invalid.elapsed_milliseconds = u64::MAX;
        assert!(
            f.store
                .finish_job(f.request.request_id, Some(&invalid))
                .is_err()
        );
        assert_eq!(f.store.command_resource_usage(None).unwrap().used.jobs, 1);
        f.store.finish_job(f.request.request_id, None).unwrap();
        assert_eq!(f.store.command_resource_usage(None).unwrap().used.jobs, 1);
    }
    #[test]
    fn team_job_admission_precreate_rejection_releases_capacity() {
        let mut f = Fixture::new(true, true);
        with_admission(&mut f);
        f.reserve().unwrap();
        f.store.reject_job(f.request.request_id).unwrap();
        let usage = f.store.command_resource_usage(None).unwrap();
        assert_eq!(usage.used.jobs, 0);
        assert_eq!(usage.used.storage_bytes, 0);
        assert!(usage.limit.is_none());
    }
    #[test]
    fn team_job_admission_blocks_actual_native_submission_before_post() {
        let mut f = Fixture::new(true, true);
        with_admission(&mut f);
        f.policy.max_commands = 128;
        f.reserve().unwrap();
        f.store
            .bind_job_operation(f.request.request_id, Uuid::new_v4())
            .unwrap();
        f.store.finish_job(f.request.request_id, None).unwrap();
        let mut host: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/job-worker/job-host.json"
        )))
        .unwrap();
        host["resource_group_boundary"] = json!("linux-cgroup2-sandbox/v1");
        let server = Server::new(vec![(200, serde_json::to_vec(&host).unwrap())]);
        let result = run_with_host(
            &f.store,
            &f.claim,
            &f.dir,
            Uuid::new_v4(),
            CommandArgs {
                argv: f.request.argv.clone(),
                input_paths: vec![],
                output_paths: vec![],
                timeout_seconds: 10,
            },
            &AtomicBool::new(false),
            |_| {
                Ok((
                    f.policy.clone(),
                    Client::fixture(&server.url, "fixture-project"),
                ))
            },
        );
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("aggregate resource reservation limit")
        );
        let requests = server.finish();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].line.starts_with("GET /inspect/"));
        assert_eq!(f.store.command_resource_usage(None).unwrap().used.jobs, 1);
    }
    #[test]
    fn team_job_group_resource_policy_is_owner_pinned_and_bounded() {
        let mut f = Fixture::new(true, true);
        let legacy = serde_json::to_value(&f.policy).unwrap();
        f.policy.resources = Some(child_limits());
        assert!(
            serde_json::to_value(f.policy.resources.as_ref().unwrap())
                .unwrap()
                .get("group")
                .is_none()
        );
        f.policy.resources.as_mut().unwrap().group = Some(group_limits());
        validate_policy(&f.policy).unwrap();
        let value = body(&f.policy, &f.request).unwrap();
        let encoded = value["env"]["ION_JOB_RESOURCE_LIMITS"].as_str().unwrap();
        let limits: Value = serde_json::from_str(encoded).unwrap();
        assert_eq!(
            limits["group"],
            json!({"version":1,"cpu_millis":500,"memory_bytes":134217728,"processes":32})
        );
        assert!(legacy.get("resources").is_none());
        for (field, invalid) in [
            ("version", 2),
            ("cpu_millis", 0),
            ("cpu_millis", 32001),
            ("memory_bytes", 1),
            ("memory_bytes", 8u64 * 1024 * 1024 * 1024 + 1),
            ("processes", 1),
            ("processes", 257),
        ] {
            let mut bad = serde_json::to_value(&f.policy).unwrap();
            bad["resources"]["group"][field] = invalid.into();
            let policy: Policy = serde_json::from_value(bad).unwrap();
            assert!(validate_policy(&policy).is_err(), "{field}");
        }
        f.policy.resources.as_mut().unwrap().processes = 16;
        assert!(validate_policy(&f.policy).is_err());
    }
    #[test]
    fn team_job_group_approval_requires_exact_image_manifest_before_reservation() {
        let mut f = Fixture::new(true, true);
        f.policy.resources = Some(child_limits());
        f.policy.resources.as_mut().unwrap().group = Some(group_limits());
        let shipped: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/job-worker/job-host.json"
        )))
        .unwrap();
        for accepted in [true, false] {
            let mut host = shipped.clone();
            if !accepted {
                host.as_object_mut()
                    .unwrap()
                    .remove("resource_group_boundary");
            }
            let server = Server::new(vec![(200, bytes(host))]);
            let client = Client::fixture(&server.url, "fixture-project");
            assert_eq!(manifest(&client, &f.policy).is_ok(), accepted);
            assert_eq!(server.finish().len(), 1);
            assert!(!f.store.has_pending_effect(f.claim.lease.run_id()).unwrap());
        }
    }
    #[test]
    fn team_job_child_resources_are_pinned_and_legacy_bodies_remain_unchanged() {
        let mut f = Fixture::new(true, true);
        let original_policy = serde_json::to_value(&f.policy).unwrap();
        assert!(original_policy.get("resources").is_none());
        let original_body = body(&f.policy, &f.request).unwrap();
        assert!(
            original_body["env"]
                .get("ION_JOB_RESOURCE_LIMITS")
                .is_none()
        );
        f.policy.resources = Some(child_limits());
        validate_policy(&f.policy).unwrap();
        let restricted = body(&f.policy, &f.request).unwrap();
        let limits: Value = serde_json::from_str(
            restricted["env"]["ION_JOB_RESOURCE_LIMITS"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            limits,
            serde_json::to_value(f.policy.resources.as_ref().unwrap()).unwrap()
        );
        let mut witness = restricted.clone();
        witness["env"]
            .as_object_mut()
            .unwrap()
            .remove("ION_JOB_RESOURCE_LIMITS");
        assert_eq!(witness, original_body);
        let observed = json!({"metadata": restricted});
        verify_original_command(&observed, &restricted).unwrap();
        let mut changed = observed.clone();
        changed["metadata"]["env"]
            .as_object_mut()
            .unwrap()
            .remove("ION_JOB_RESOURCE_LIMITS");
        assert!(verify_original_command(&changed, &restricted).is_err());
        for field in [
            "version",
            "cpu_seconds",
            "address_space_bytes",
            "max_file_bytes",
            "processes",
            "open_files",
        ] {
            let mut invalid = serde_json::to_value(&f.policy).unwrap();
            invalid["resources"][field] = json!(0);
            let policy: Policy = serde_json::from_value(invalid).unwrap();
            assert!(
                validate_policy(&policy).is_err(),
                "Accepted invalid child limit {field}"
            );
        }
        for (field, value) in [
            ("cpu_seconds", 31_u64),
            ("address_space_bytes", 8 * 1024 * 1024 * 1024 + 1),
            ("max_file_bytes", 256 * 1024 * 1024 + 1),
            ("processes", 257),
            ("open_files", 257),
        ] {
            let mut invalid = serde_json::to_value(&f.policy).unwrap();
            invalid["resources"][field] = json!(value);
            assert!(validate_policy(&serde_json::from_value(invalid).unwrap()).is_err());
        }
    }
    #[test]
    fn team_job_new_run_requires_explicit_child_resource_approval_before_reads() {
        let f = Fixture::new(true, true);
        let server = Server::new(vec![]);
        let result = run_with_host(
            &f.store,
            &f.claim,
            &f.dir,
            f.request.request_id,
            CommandArgs {
                argv: f.request.argv.clone(),
                input_paths: vec![],
                output_paths: vec![],
                timeout_seconds: 10,
            },
            &AtomicBool::new(false),
            |_| {
                Ok((
                    f.policy.clone(),
                    Client::fixture(&server.url, "fixture-project"),
                ))
            },
        );
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Approve child resource limits")
        );
        assert!(!f.store.has_pending_effect(f.claim.lease.run_id()).unwrap());
        assert!(server.finish().is_empty());
    }
    fn response(request: &JobRequest, exit: i32) -> JobResponse {
        JobResponse {
            project_sha256: None,
            version: 1,
            request_id: request.request_id,
            exit_code: exit,
            timed_out: false,
            canceled: false,
            elapsed_milliseconds: 100,
            stdout: JobStream {
                content_base64: STANDARD.encode(b"checked"),
                bytes: 7,
                truncated: false,
            },
            stderr: JobStream {
                content_base64: String::new(),
                bytes: 0,
                truncated: false,
            },
            outputs: vec![],
            output_errors: vec![],
        }
    }
    fn terminal(f: &Fixture, operation: Uuid, reply: &JobResponse) -> Value {
        json!({"uuid":operation,"kind":"instance","image_uuid":f.policy.image,"status":"SUCCESS","metadata":{"result":{"state":{"exit_code":0},"stdout":{"encoding":"base64","value":STANDARD.encode(serde_json::to_vec(reply).unwrap()),"truncated":false}}}})
    }
    fn bytes(value: Value) -> Vec<u8> {
        serde_json::to_vec(&value).unwrap()
    }
    #[test]
    fn team_job_shipped_image_manifest_accepts_exact_child_network_boundary() {
        let f = Fixture::new(true, true);
        // Serve the production Docker COPY asset, rather than a hand-built
        // fixture that can accidentally agree with the owner's wrong schema.
        let shipped: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/job-worker/job-host.json"
        )))
        .unwrap();
        let server = Server::new(vec![(200, bytes(shipped.clone()))]);
        let client = Client::fixture(&server.url, "fixture-project");
        manifest(&client, &f.policy).expect("The shipped command image must pass owner preflight");
        let requests = server.finish();
        assert_eq!(requests.len(), 1);
        assert!(
            requests[0]
                .line
                .starts_with(&format!("GET /inspect/{}/download?", f.policy.image))
        );
        assert!(requests[0].line.contains("path=%2Fion%2Fjob-host.json"));
        assert!(!f.store.has_pending_effect(f.claim.lease.run_id()).unwrap());

        for change in [
            "missing",
            "changed",
            "network",
            "supervisor",
            "project",
            "extra",
        ] {
            let mut invalid = shipped.clone();
            match change {
                "missing" => {
                    invalid.as_object_mut().unwrap().remove("network_boundary");
                }
                "changed" => invalid["network_boundary"] = json!("unverified/v1"),
                "network" => invalid["network"] = json!("enabled"),
                "supervisor" => invalid["supervisor"] = json!("user"),
                "project" => invalid["project_snapshot"] = json!("unverified/v2"),
                "extra" => invalid["native_credentials"] = json!(true),
                _ => unreachable!(),
            }
            let server = Server::new(vec![(200, bytes(invalid))]);
            let client = Client::fixture(&server.url, "fixture-project");
            assert!(
                manifest(&client, &f.policy).is_err(),
                "Accepted unapproved manifest change: {change}"
            );
            let requests = server.finish();
            assert_eq!(requests.len(), 1);
            assert!(requests[0].line.starts_with("GET "));
            assert!(!f.store.has_pending_effect(f.claim.lease.run_id()).unwrap());
        }
    }
    #[test]
    fn team_job_run_rejects_changed_manifest_before_reservation() {
        let mut f = Fixture::new(true, true);
        f.policy.resources = Some(child_limits());
        let server = Server::new(vec![(
            200,
            bytes(json!({
                "version":1,"worker":"ion-job-worker/1","credential_free":true,
                "network":"disabled","supervisor":"root","child":"nonroot"
            })),
        )]);
        let result = run_with_host(
            &f.store,
            &f.claim,
            &f.dir,
            f.request.request_id,
            CommandArgs {
                argv: f.request.argv.clone(),
                input_paths: vec![],
                output_paths: vec![],
                timeout_seconds: f.request.timeout_seconds,
            },
            &AtomicBool::new(false),
            |target| {
                assert_eq!(target, f.policy.target);
                Ok((
                    f.policy.clone(),
                    Client::fixture(&server.url, "fixture-project"),
                ))
            },
        );
        assert!(result.unwrap_err().to_string().contains("contract changed"));
        assert!(!f.store.has_pending_effect(f.claim.lease.run_id()).unwrap());
        assert!(
            f.store
                .unfinished_jobs(f.claim.lease.attempt_id())
                .unwrap()
                .is_empty()
        );
        let requests = server.finish();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].line.starts_with("GET /inspect/"));
    }

    #[test]
    #[ignore = "requires fixtures exported by the actual pinned Linux job-worker CLI"]
    fn team_job_native_linux_receipt_full_run_transfer_replay_and_artifact() {
        let root = std::env::var_os("ION_NATIVE_JOB_WORKER_FIXTURE")
            .expect("Export native CLI fixtures with packages/job-worker/test_linux_cli.py first");
        let root = std::path::PathBuf::from(root);
        let read = |name: &str, limit: u64| {
            let mut data = Vec::new();
            std::fs::File::open(root.join(name))
                .unwrap()
                .take(limit + 1)
                .read_to_end(&mut data)
                .unwrap();
            assert!(data.len() as u64 <= limit);
            data
        };
        let native_request: JobRequest =
            serde_json::from_slice(&read("request.json", 512 * 1024)).unwrap();
        let native_response: JobResponse =
            serde_json::from_slice(&read("receipt.json", 512 * 1024)).unwrap();
        let native_manifest: Value =
            serde_json::from_slice(&read("manifest.json", 16 * 1024)).unwrap();
        let shipped_manifest: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/job-worker/job-host.json"
        )))
        .unwrap();
        assert_eq!(native_manifest, shipped_manifest);
        native_response.validate_against(&native_request).unwrap();
        assert_eq!(native_request.argv[0], "/usr/local/bin/python3");
        assert_eq!(native_request.inputs.len(), 1);
        assert_eq!(native_request.inputs[0].path, "input.bin");
        assert_eq!(native_request.output_paths, ["output.bin"]);
        assert_eq!(
            native_response.exit_code, 7,
            "Native nonzero command outcome must be preserved"
        );
        assert!(!native_response.timed_out && !native_response.canceled);
        let payload: Vec<u8> = (0..256).map(|v| v as u8).collect::<Vec<_>>().repeat(200);
        assert_eq!(
            STANDARD
                .decode(&native_request.inputs[0].content_base64)
                .unwrap(),
            payload
        );
        let mut expected_output = payload.clone();
        expected_output.extend_from_slice(b"\x00\xffresult");
        assert_eq!(
            STANDARD
                .decode(&native_response.outputs[0].content_base64)
                .unwrap(),
            expected_output
        );

        let mut f = Fixture::new(true, true);
        f.request = native_request;
        f.policy.resources = Some(child_limits());
        f.policy.executables = vec![f.request.argv[0].clone()];
        let mut input = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(f.dir.join("input.bin"))
            .unwrap();
        std::io::Write::write_all(&mut input, &payload).unwrap();
        drop(input);
        let operation = Uuid::new_v4();
        let server = Server::new(vec![
            (200, bytes(native_manifest.clone())),
            (201, bytes(json!({"uuid":operation}))),
            (200, bytes(terminal(&f, operation, &native_response))),
            (200, bytes(native_manifest)),
        ]);
        let invoke = || {
            run_with_host(
                &f.store,
                &f.claim,
                &f.dir,
                f.request.request_id,
                CommandArgs {
                    argv: f.request.argv.clone(),
                    input_paths: vec!["input.bin".into()],
                    output_paths: f.request.output_paths.clone(),
                    timeout_seconds: f.request.timeout_seconds,
                },
                &AtomicBool::new(false),
                |target| {
                    assert_eq!(target, f.policy.target);
                    Ok((
                        f.policy.clone(),
                        Client::fixture(&server.url, "fixture-project"),
                    ))
                },
            )
        };
        let first: Value = serde_json::from_str(
            &invoke().expect("Actual Linux receipt must pass the complete owner run path"),
        )
        .unwrap();
        assert_eq!(first["exit_code"], 7);
        assert_eq!(first["request_id"], f.request.request_id.to_string());
        assert_eq!(first["output_count"], 1);
        let replay: Value = serde_json::from_str(&invoke().unwrap()).unwrap();
        assert_eq!(first, replay);
        assert!(!f.store.has_pending_effect(f.claim.lease.run_id()).unwrap());
        assert!(
            f.store
                .unfinished_jobs(f.claim.lease.attempt_id())
                .unwrap()
                .is_empty()
        );
        let artifact = Uuid::parse_str(first["artifact_id"].as_str().unwrap()).unwrap();
        assert!(!artifact.is_nil());
        let document: Value = serde_json::from_slice(
            &f.store
                .read_assignment_document(&f.claim.lease, artifact, crate::model::unix_time())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(document, native_response.summary().unwrap());
        assert!(document["outputs"][0].get("content_base64").is_none());
        let stored = f
            .store
            .job_receipt(
                &f.claim.lease,
                f.request.request_id,
                crate::model::unix_time(),
            )
            .unwrap();
        assert_eq!(
            serde_json::to_value(&stored).unwrap(),
            serde_json::to_value(&native_response).unwrap()
        );
        let mut received = Vec::new();
        loop {
            let page: Value = serde_json::from_str(
                &read_output(
                    &f.store,
                    &f.claim.lease,
                    f.request.request_id,
                    "output.bin",
                    received.len(),
                )
                .unwrap(),
            )
            .unwrap();
            assert_eq!(page["offset"], received.len());
            assert_eq!(page["total_bytes"], expected_output.len());
            assert_eq!(
                page["sha256"],
                format!("{:x}", Sha256::digest(&expected_output))
            );
            let data = STANDARD
                .decode(page["content_base64"].as_str().unwrap())
                .unwrap();
            assert!(data.len() <= 4096);
            received.extend_from_slice(&data);
            assert_eq!(page["next_offset"], received.len());
            if received.len() == expected_output.len() {
                break;
            }
            assert!(!data.is_empty());
        }
        assert_eq!(received, expected_output);
        assert!(read_output(&f.store, &f.claim.lease, f.request.request_id, "missing", 0).is_err());
        let foreign = Fixture::new(true, true);
        assert!(
            read_output(
                &f.store,
                &foreign.claim.lease,
                f.request.request_id,
                "output.bin",
                0
            )
            .is_err()
        );
        let requests = server.finish();
        assert_eq!(requests.len(), 4);
        assert_eq!(
            requests
                .iter()
                .filter(|r| r.line.starts_with("POST "))
                .count(),
            1
        );
        assert!(requests[0].line.starts_with("GET /inspect/"));
        assert!(requests[1].line.starts_with("POST /instances "));
        assert!(
            requests[2]
                .line
                .starts_with(&format!("GET /operations/{operation} "))
        );
        assert!(requests[3].line.starts_with("GET /inspect/"));
        let sent: JobRequest = serde_json::from_slice(
            &STANDARD
                .decode(requests[1].body["stdin"]["value"].as_str().unwrap())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(&sent).unwrap(),
            serde_json::to_value(&f.request).unwrap()
        );
        assert_eq!(requests[1].body, body(&f.policy, &f.request).unwrap());
    }

    #[test]
    fn team_job_pins_command_files_and_real_nonzero_receipt_without_a_second_post() {
        let f = Fixture::new(true, true);
        f.reserve().unwrap();
        let operation = Uuid::new_v4();
        let reply = response(&f.request, 7);
        let server = Server::new(vec![
            (201, bytes(json!({"uuid":operation}))),
            (200, bytes(terminal(&f, operation, &reply))),
        ]);
        let client = Client::fixture(&server.url, "fixture-project");
        let received = execute_with(
            &client,
            &f.store,
            &f.claim.lease,
            &f.policy,
            &f.request,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(received.exit_code, 7);
        assert_eq!(f.reserve().unwrap().unwrap().exit_code, 7);
        assert!(!f.store.has_pending_effect(f.claim.lease.run_id()).unwrap());
        let requests = server.finish();
        assert_eq!(requests.len(), 2);
        let body = &requests[0].body;
        assert_eq!(body["networking"]["enabled"], false);
        assert_eq!(body["shell"], false);
        assert_eq!(body["uid"], 0);
        assert_eq!(body["env"]["ION_JOB_CHILD_UID"], "1000");
        assert_eq!(body["disposable"], true);
        let uploaded: JobRequest = serde_json::from_slice(
            &STANDARD
                .decode(body["stdin"]["value"].as_str().unwrap())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(uploaded.argv, f.request.argv);
        assert_eq!(body["command"], "/usr/local/bin/ion-job-worker");
        assert!(body["env"].get("ION_NEBIUS_SANDBOX_IAM_TOKEN").is_none());
    }
    #[test]
    fn global_usage_remote_command_receipt_survives_replay_and_unknown_costs() {
        let f = Fixture::new(true, true);
        let control = ControlStore::new(f.dir.join("control.db"));
        f.reserve().unwrap();
        let page = control
            .usage_ledger(crate::usage_ledger::all_query())
            .unwrap();
        assert_eq!(page.summary.records, 1);
        assert_eq!(
            page.entries[0].kind,
            waku_protocol::usage_ledger::UsageLedgerKind::RemoteCommand
        );
        assert_eq!(
            page.entries[0].environment,
            Some(ExecutionEnvironment::NebiusSandbox)
        );
        assert_eq!(page.entries[0].host, "fixture-project");
        assert_eq!(page.entries[0].agent_name.as_deref(), Some("Job fixture"));
        assert!(page.entries[0].cost.is_none() && page.entries[0].elapsed_milliseconds.is_none());
        let response = response(&f.request, 7);
        f.store
            .bind_job_operation(f.request.request_id, Uuid::new_v4())
            .unwrap();
        f.store
            .finish_job(f.request.request_id, Some(&response))
            .unwrap();
        let page = control
            .usage_ledger(crate::usage_ledger::all_query())
            .unwrap();
        assert_eq!(page.summary.records, 1);
        assert_eq!(page.summary.model_calls, 0);
        assert_eq!(page.summary.elapsed_milliseconds, 100);
        assert_eq!(page.summary.unknown_cost_records, 1);
        assert!(page.summary.costs.is_empty());
        let revision = page.revision;
        f.reserve().unwrap();
        let reopened = ControlStore::new(f.dir.join("control.db"));
        assert_eq!(
            reopened
                .usage_ledger(crate::usage_ledger::all_query())
                .unwrap()
                .revision,
            revision
        );
        let mut changed = response.clone();
        changed.elapsed_milliseconds += 1;
        assert!(
            f.store
                .finish_job(f.request.request_id, Some(&changed))
                .is_err()
        );
        assert_eq!(
            reopened
                .usage_ledger(crate::usage_ledger::all_query())
                .unwrap()
                .revision,
            revision
        );
        let rejected = Fixture::new(false, true);
        assert!(rejected.reserve().is_err());
        assert_eq!(
            ControlStore::new(rejected.dir.join("control.db"))
                .usage_ledger(crate::usage_ledger::all_query())
                .unwrap()
                .summary
                .records,
            0
        );
    }

    #[test]
    fn team_job_denies_local_ungranted_and_changed_replay_or_lost_acknowledgement() {
        assert!(Fixture::new(false, true).reserve().is_err());
        assert!(Fixture::new(true, false).reserve().is_err());
        let mut f = Fixture::new(true, true);
        f.reserve().unwrap();
        assert!(f.store.has_pending_effect(f.claim.lease.run_id()).unwrap());
        assert!(
            !f.store
                .remote_attempt_stopped(f.claim.lease.attempt_id())
                .unwrap()
        );
        let server = Server::new(vec![(500, bytes(json!({})))]);
        let client = Client::fixture(&server.url, "fixture-project");
        assert!(
            execute_with(
                &client,
                &f.store,
                &f.claim.lease,
                &f.policy,
                &f.request,
                &AtomicBool::new(false)
            )
            .is_err()
        );
        assert!(f.reserve().is_err());
        f.request.argv.push("changed".into());
        assert!(f.reserve().is_err());
        assert_eq!(server.finish().len(), 1);
        assert_eq!(
            f.store
                .unfinished_jobs(f.claim.lease.attempt_id())
                .unwrap()
                .len(),
            1
        );
    }
    #[test]
    fn team_job_stop_cancels_the_bound_operation_and_retains_terminal_state() {
        let f = Fixture::new(true, true);
        f.reserve().unwrap();
        let operation = Uuid::new_v4();
        let stop = std::sync::Arc::new(AtomicBool::new(false));
        let hook = stop.clone();
        let server = Server::with_hook(
            vec![
                (201, bytes(json!({"uuid":operation}))),
                (202, bytes(json!({}))),
                (
                    200,
                    bytes(
                        json!({"uuid":operation,"kind":"instance","image_uuid":f.policy.image,"status":"CANCELLED"}),
                    ),
                ),
            ],
            move |r| {
                if r.line.starts_with("POST ") {
                    hook.store(true, Ordering::Release);
                }
            },
        );
        let client = Client::fixture(&server.url, "fixture-project");
        assert!(
            execute_with(
                &client,
                &f.store,
                &f.claim.lease,
                &f.policy,
                &f.request,
                &stop
            )
            .is_err()
        );
        assert!(
            f.store
                .unfinished_jobs(f.claim.lease.attempt_id())
                .unwrap()
                .is_empty()
        );
        assert!(!f.store.has_pending_effect(f.claim.lease.run_id()).unwrap());
        let requests = server.finish();
        assert_eq!(requests.len(), 3);
        assert!(requests[1].line.starts_with("DELETE "));
    }
    #[test]
    fn team_job_restart_observes_and_cancels_only_the_original_operation() {
        let f = Fixture::new(true, true);
        f.reserve().unwrap();
        let operation = Uuid::new_v4();
        f.store
            .bind_job_operation(f.request.request_id, operation)
            .unwrap();
        let server = Server::new(vec![
            (
                200,
                bytes(
                    json!({"uuid":operation,"kind":"instance","image_uuid":f.policy.image,"status":"EXECUTING"}),
                ),
            ),
            (202, bytes(json!({}))),
            (
                200,
                bytes(terminal(&f, operation, &response(&f.request, 0))),
            ),
        ]);
        let client = Client::fixture(&server.url, "fixture-project");
        assert!(
            !reconcile_with(
                &client,
                &f.store,
                f.store.unfinished_jobs(f.claim.lease.attempt_id()).unwrap()
            )
            .unwrap()
        );
        assert!(
            reconcile_with(
                &client,
                &f.store,
                f.store.unfinished_jobs(f.claim.lease.attempt_id()).unwrap()
            )
            .unwrap()
        );
        let requests = server.finish();
        assert_eq!(requests.len(), 3);
        assert!(requests.iter().all(|r| !r.line.starts_with("POST ")));
        assert!(requests[0].line.contains(&operation.to_string()));
        assert!(requests[1].line.starts_with("DELETE "));
        // Terminal evidence always includes the generated artifact, even on recovery.
        let snapshot = f.store.snapshot(f.claim.lease.run_id()).unwrap();
        let artifacts = ControlStore::new(f.dir.join("control.db")).load().unwrap();
        assert!(artifacts.records.iter().any(
            |r| matches!(r,ControlRecord::Artifact(a) if a.task_id==Some(snapshot.work[0].task_id))
        ));
    }
    #[test]
    fn team_job_policy_and_count_stay_pinned_and_output_hashes_are_verified() {
        let mut f = Fixture::new(true, true);
        f.reserve().unwrap();
        f.store
            .bind_job_operation(f.request.request_id, Uuid::new_v4())
            .unwrap();
        let mut reply = response(&f.request, 0);
        f.store
            .finish_job(f.request.request_id, Some(&reply))
            .unwrap();
        f.request.request_id = Uuid::new_v4();
        assert!(f.reserve().is_err()); // first command consumed the one-command cap
        f.policy.max_commands = 2;
        assert!(f.reserve().is_err()); // editing the registry cannot enlarge it
        f.request.output_paths = vec!["out.bin".into()];
        reply.request_id = f.request.request_id;
        reply.outputs = vec![JobOutput {
            path: "out.bin".into(),
            content_base64: STANDARD.encode([0, 1, 255]),
            sha256: format!("{:x}", Sha256::digest([0, 1, 255])),
            bytes: 3,
        }];
        assert!(validate_receipt(&reply, &f.request).is_ok());
        reply.outputs[0].sha256 = "0".repeat(64);
        assert!(validate_receipt(&reply, &f.request).is_err());
    }
    #[test]
    fn team_job_bundles_only_selected_bytes_and_never_accepts_unknown_operations() {
        let f = Fixture::new(true, true);
        std::fs::write(f.dir.join("selected.bin"), [0, 1, 255]).unwrap();
        std::fs::write(f.dir.join(".env"), "owner-secret").unwrap();
        let args = CommandArgs {
            argv: f.request.argv.clone(),
            input_paths: vec!["selected.bin".into()],
            output_paths: vec![],
            timeout_seconds: 10,
        };
        let bundled = bundle(f.request.request_id, &args, &f.dir).unwrap();
        assert_eq!(bundled.inputs.len(), 1);
        assert_eq!(
            STANDARD.decode(&bundled.inputs[0].content_base64).unwrap(),
            [0, 1, 255]
        );
        f.reserve().unwrap();
        let wrong_image = Uuid::new_v4();
        let operation = Uuid::new_v4();
        f.store
            .bind_job_operation(f.request.request_id, operation)
            .unwrap();
        let server = Server::new(vec![(
            200,
            bytes(
                json!({"uuid":operation,"kind":"instance","image_uuid":wrong_image,"status":"SUCCESS"}),
            ),
        )]);
        let client = Client::fixture(&server.url, "fixture-project");
        assert!(
            reconcile_with(
                &client,
                &f.store,
                f.store.unfinished_jobs(f.claim.lease.attempt_id()).unwrap()
            )
            .is_err()
        );
        assert!(f.store.has_pending_effect(f.claim.lease.run_id()).unwrap());
        assert_eq!(server.finish().len(), 1);
        std::fs::remove_file(f.dir.join("selected.bin")).unwrap();
        std::fs::remove_file(f.dir.join(".env")).unwrap();
    }
    #[test]
    fn team_job_recovery_matches_exact_tool_arguments_and_committed_artifact() {
        let f = Fixture::new(true, true);
        f.reserve().unwrap();
        f.store
            .bind_job_operation(f.request.request_id, Uuid::new_v4())
            .unwrap();
        f.store
            .finish_job(f.request.request_id, Some(&response(&f.request, 7)))
            .unwrap();
        let db = rusqlite::Connection::open(f.dir.join("control.db")).unwrap();
        let run = f.store.snapshot(f.claim.lease.run_id()).unwrap();
        let args =
            json!({"argv":f.request.argv,"input_paths":[],"output_paths":[],"timeout_seconds":10});
        let recovered = crate::teams::jobs::recover_receipt(
            &db,
            &run,
            &run.work[0],
            f.claim.lease.attempt_id(),
            f.request.request_id,
            &args,
        )
        .unwrap()
        .unwrap();
        let recovered: Value = serde_json::from_str(&recovered).unwrap();
        assert_eq!(recovered["exit_code"], 7);
        assert!(recovered["artifact_id"].is_string());
        let mut changed = args.clone();
        changed["timeout_seconds"] = json!(11);
        assert!(
            crate::teams::jobs::recover_receipt(
                &db,
                &run,
                &run.work[0],
                f.claim.lease.attempt_id(),
                f.request.request_id,
                &changed
            )
            .is_err()
        );
        assert!(
            crate::teams::jobs::recover_receipt(
                &db,
                &run,
                &run.work[0],
                Uuid::new_v4(),
                f.request.request_id,
                &args
            )
            .unwrap()
            .is_none()
        );
        db.execute(
            "DELETE FROM ion_control_records WHERE id=?",
            [recovered["artifact_id"].as_str().unwrap()],
        )
        .unwrap();
        assert!(
            crate::teams::jobs::recover_receipt(
                &db,
                &run,
                &run.work[0],
                f.claim.lease.attempt_id(),
                f.request.request_id,
                &args
            )
            .is_err()
        );
    }
    #[test]
    fn team_job_lost_ack_binding_verifies_original_input_and_recovers_without_post() {
        let f = Fixture::new(true, true);
        let now = crate::model::unix_time();
        let args =
            json!({"argv":f.request.argv,"input_paths":[],"output_paths":[],"timeout_seconds":10});
        let call = json!({"id":"command-recovery","name":"run_remote_command","arguments":args,
            "request_id":f.request.request_id,"activity_id":"command-recovery"});
        let journal=json!({"version":1,"steps":1,"messages":[{"role":"user","content":"Execute check"},
            {"role":"assistant","content":null,"tool_calls":[{"id":"command-recovery","type":"function",
            "function":{"name":"run_remote_command","arguments":args.to_string()}}]}],"pending":[call]}).to_string();
        f.store
            .save_runtime_checkpoint(&f.claim.lease, &journal, now)
            .unwrap();
        f.reserve().unwrap();
        f.store.reconcile_owner_restart(now + 1).unwrap();
        let run = f.claim.lease.run_id();
        let work = f.claim.lease.work_id();
        let attempt = f.claim.lease.attempt_id();
        let (review, snapshot) = f.store.recovery_review(run, work).unwrap();
        assert_eq!(review.remote_calls.len(), 1);
        assert!(matches!(
            review.remote_calls[0].purpose,
            TeamRemotePurpose::Command
        ));
        assert!(
            review.remote_calls[0].can_bind && !review.host_stopped && !snapshot.work[0].can_resume
        );
        let operation = Uuid::new_v4();
        let mut observed = terminal(&f, operation, &response(&f.request, 7));
        let result = observed["metadata"]["result"].clone();
        observed["metadata"] = body(&f.policy, &f.request).unwrap();
        observed["metadata"]["result"] = result;
        let server = Server::new(vec![(200, bytes(observed.clone())), (200, bytes(observed))]);
        let client = Client::fixture(&server.url, "fixture-project");
        bind_recovered_with(
            &client,
            &f.store,
            run,
            work,
            attempt,
            snapshot.revision,
            f.request.request_id,
            operation,
        )
        .unwrap();
        // Lost owner response is replayed from the durable confirmation without another GET.
        bind_recovered_with(
            &client,
            &f.store,
            run,
            work,
            attempt,
            snapshot.revision,
            f.request.request_id,
            operation,
        )
        .unwrap();
        assert!(
            reconcile_with(&client, &f.store, f.store.unfinished_jobs(attempt).unwrap()).unwrap()
        );
        assert!(f.store.remote_attempt_stopped(attempt).unwrap());
        let revision = f.store.snapshot(run).unwrap().revision;
        f.store
            .confirm_recovery(
                run,
                work,
                attempt,
                revision,
                TeamRecoveryConfirmation::VerifyRemoteStopped,
                now + 2,
            )
            .unwrap();
        let revision = f.store.snapshot(run).unwrap().revision;
        let (_, ready) = f
            .store
            .confirm_recovery(
                run,
                work,
                attempt,
                revision,
                TeamRecoveryConfirmation::PrepareContinuation,
                now + 3,
            )
            .unwrap();
        assert!(ready.work[0].can_resume);
        f.store
            .queue_checkpoint_resume(run, work, ready.revision, now + 4)
            .unwrap();
        let resumed = f
            .store
            .resume_after_children(run, work, now + 4, 180)
            .unwrap();
        let journal: Value = serde_json::from_str(&resumed.checkpoint).unwrap();
        assert_eq!(journal["pending"], json!([]));
        let tool = journal["messages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["role"] == "tool")
            .unwrap();
        let receipt: Value = serde_json::from_str(tool["content"].as_str().unwrap()).unwrap();
        assert_eq!(receipt["exit_code"], 7);
        assert!(receipt["artifact_id"].is_string());
        let requests = server.finish();
        assert_eq!(requests.len(), 2);
        assert!(requests.iter().all(|r| r.line.starts_with("GET ")));
    }
    #[test]
    fn team_job_binding_rejects_modified_command_files_limits_and_added_capabilities() {
        let f = Fixture::new(true, true);
        let expected = body(&f.policy, &f.request).unwrap();
        let observed = json!({"metadata":expected});
        assert!(verify_original_command(&observed, &expected).is_ok());
        for key in [
            "hostname",
            "command",
            "image",
            "args",
            "env",
            "networking",
            "resources_limits",
            "timeout",
            "uid",
        ] {
            let mut changed = observed.clone();
            changed["metadata"][key] = json!("different");
            assert!(
                verify_original_command(&changed, &expected).is_err(),
                "{key}"
            );
        }
        let mut changed = observed.clone();
        let mut request = f.request.clone();
        request.argv.push("changed".into());
        changed["metadata"]["stdin"]["value"] =
            json!(STANDARD.encode(serde_json::to_vec(&request).unwrap()));
        assert!(verify_original_command(&changed, &expected).is_err());
        changed = observed;
        changed["metadata"]["files"] = json!({"/ion/private":"extra"});
        assert!(verify_original_command(&changed, &expected).is_err());
    }
    #[test]
    fn personal_sandbox_policy_meets_the_command_contract() {
        let image = Uuid::new_v4();
        let policy: Policy =
            serde_json::from_value(super::super::team_remote::personal_policy(image)).unwrap();
        validate_policy(&policy).unwrap();
        assert!(policy.target == image && policy.image == image);
        assert!(policy.resources.is_some() && policy.project_snapshot.is_some());
        let args = |argv: &[&str]| CommandArgs {
            argv: argv.iter().map(|a| a.to_string()).collect(),
            input_paths: vec![],
            output_paths: vec![],
            timeout_seconds: 60,
        };
        validate_args(&policy, &args(&["/usr/local/bin/python3", "-c", "print(1)"])).unwrap();
        assert!(validate_args(&policy, &args(&["/usr/bin/curl", "https://example.com"])).is_err());
        let mut bare = args(&["python3", "-c", "print(1)"]);
        admit_args(&policy, &mut bare).unwrap();
        assert_eq!(bare.argv[0], "/usr/local/bin/python3");
        let error = admit_args(&policy, &mut args(&["curl", "https://example.com"])).unwrap_err();
        assert!(error.to_string().contains("Allowed executables: /bin/sh"));
    }
    #[test]
    fn team_job_rejects_paths_unregistered_commands_and_invented_output_receipts() {
        let f = Fixture::new(true, true);
        let mut args = CommandArgs {
            argv: f.request.argv.clone(),
            input_paths: vec![],
            output_paths: vec![],
            timeout_seconds: 10,
        };
        assert!(validate_args(&f.policy, &args).is_ok());
        args.argv[0] = "/bin/bash".into();
        assert!(validate_args(&f.policy, &args).is_err());
        for p in [
            "../escape",
            ".env",
            "dir/.ssh/key",
            "/root/private",
            "x\\y",
            "a//b",
            "x.pem",
        ] {
            assert!(path(p).is_err());
        }
        let mut reply = response(&f.request, 0);
        reply.outputs.push(JobOutput {
            path: "unrequested".into(),
            content_base64: "".into(),
            sha256: "0".repeat(64),
            bytes: 0,
        });
        assert!(validate_receipt(&reply, &f.request).is_err());
        reply.outputs.clear();
        reply.stdout.bytes = 1;
        assert!(validate_receipt(&reply, &f.request).is_err());
    }
    #[test]
    fn managed_solo_command_preserves_actual_exit_receipt_and_exact_argv() {
        for code in [0, 7] {
            let mut f = Fixture::new(true, true);
            f.policy.resources = Some(child_limits());
            let operation = Uuid::new_v4();
            let reply = response(&f.request, code);
            let server = Server::new(vec![
                (201, bytes(json!({"uuid":operation}))),
                (200, bytes(terminal(&f, operation, &reply))),
            ]);
            let command = PreparedSoloCommand {
                client: Client::fixture(&server.url, "fixture-project"),
                policy: f.policy.clone(),
                request: f.request.clone(),
            };
            assert!(
                command
                    .description()
                    .unwrap()
                    .contains(&f.policy.image.to_string())
            );
            let result: Value =
                serde_json::from_str(&command.execute(&AtomicBool::new(false)).unwrap()).unwrap();
            assert_eq!(result["exit_code"], code);
            assert_eq!(result["operation_id"], operation.to_string());
            let requests = server.finish();
            assert_eq!(requests.len(), 2);
            assert!(requests[0].line.starts_with("POST /instances "));
            let submitted = &requests[0].body;
            assert_eq!(submitted["networking"]["enabled"], false);
            assert_eq!(submitted["disposable"], true);
            let input = STANDARD
                .decode(submitted["stdin"]["value"].as_str().unwrap())
                .unwrap();
            let input: Value = serde_json::from_slice(&input).unwrap();
            assert_eq!(input["argv"], json!(f.request.argv));
            assert_eq!(input["inputs"], json!([]));
        }
    }
}

#[cfg(test)]
#[path = "team_job_checks_tests.rs"]
mod check_tests;
