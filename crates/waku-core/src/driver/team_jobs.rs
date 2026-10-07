//! Explicit file bundles and actual commands in credential-free ConTree job images.
//! No automatic upload, owner command, model credential or local retry is allowed.
use super::team_remote::{Client, read_json};
use crate::teams::{AssignmentLease, ClaimedAssignment, TeamCoordinator, jobs::*};
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
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
fn manifest(client: &Client, p: &Policy) -> Result<()> {
    let response = client
        .request(
            reqwest::Method::GET,
            &format!("/inspect/{}/download", p.image),
        )
        .query(&[("path", "/ion/job-host.json")])
        .send()?;
    ensure!(
        response.status().is_success(),
        "Registered command image is unavailable before submission"
    );
    let host = read_json(response, 16 * 1024)?;
    ensure!(
        host == json!({"version":1,"worker":"ion-job-worker/1","credential_free":true,"network":"disabled","supervisor":"root","child":"nonroot"}),
        "Command worker/image contract changed"
    );
    Ok(())
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
    Ok(
        json!({"command":"/usr/local/bin/ion-job-worker","args":[],"image":policy.image,"shell":false,
        "hostname":format!("ion-job-{}",request.request_id),"env":{"PATH":"/usr/local/bin:/usr/bin:/bin","ION_JOB_ISOLATED":"1","ION_JOB_CHILD_UID":policy.uid.to_string(),"ION_JOB_CHILD_GID":policy.gid.to_string()},
        "cwd":"/ion","uid":0,"gid":0,"stdin":{"value":STANDARD.encode(input),"encoding":"base64","close":true},
        "timeout":request.timeout_seconds+10,"truncate_output_at":512*1024,"disposable":true,"preserve_env":false,
        "resources_limits":{"max_layer_bytes":policy.max_layer_bytes},"networking":{"enabled":false}}),
    )
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
        stdout["truncated"] == false && stdout["encoding"] == "base64",
        "Remote job receipt was truncated or uses an unsupported encoding"
    );
    let encoded = stdout["value"].as_str().context("Job receipt is missing")?;
    ensure!(encoded.len() <= 700 * 1024, "Job receipt exceeds its bound");
    let bytes = STANDARD.decode(encoded)?;
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
    let response=client.request(reqwest::Method::POST,"/instances").json(&body(p,request)?).send()
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
    let profile = coordinator.authorize_remote_job(&claimed.lease, crate::model::unix_time())?;
    let target = profile
        .execution
        .sandbox_target
        .as_deref()
        .context("Pinned sandbox target missing")?;
    let target = Uuid::parse_str(target.rsplit('@').next().unwrap_or(target))?;
    let policy = policy(target)?;
    validate_args(&policy, &args)?;
    let check_context =
        coordinator.command_check_context(&claimed.lease, workspace, crate::model::unix_time())?;
    let request = bundle(request_id, &args, workspace)?;
    // Validate the full wire request before creating a durable submitting record.
    body(&policy, &request)?;
    let client = Client::configured()?;
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
    let expected = body(&policy, &record.request)?;
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
        metadata
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
            };
            let request = JobRequest {
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
            let _ = std::fs::remove_file(self.dir.join("control.db"));
            let _ = std::fs::remove_dir(&self.dir);
        }
    }
    fn response(request: &JobRequest, exit: i32) -> JobResponse {
        JobResponse {
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
}

#[cfg(test)]
#[path = "team_job_checks_tests.rs"]
mod check_tests;
