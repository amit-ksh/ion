//! ConTree REST transport using the official asynchronous instance/operation
//! contract. The selected immutable image runs only a bounded harness request;
//! the owner retains all tool capabilities, files, approvals and delegation.
use super::{DriverEventSender, NativeTeamExit, NativeTeamStart, team_completion};
use crate::{
    settings::DaemonSettings,
    teams::{ModelCallAllowance, ModelTokenUsage, TeamCoordinator},
};
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::Read,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;
use waku_protocol::{
    control::AgentProfile,
    model::{DriverEvent, ProviderKind},
};
const BASE: &str = "https://api.tokenfactory.nebius.com/sandboxes/v1";
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Capability {
    pub provider: ProviderKind,
    pub model: String,
    pub input_cap: u64,
    pub output_cap: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u16,
    capabilities: Vec<Capability>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkerRequest {
    pub version: u16,
    pub request_id: Uuid,
    pub capability: Capability,
    pub system: String,
    pub messages: Vec<Value>,
    pub tools: Value,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkerResponse {
    pub version: u16,
    pub request_id: Uuid,
    pub provider: ProviderKind,
    pub model: String,
    pub content: String,
    pub usage: ModelTokenUsage,
}
struct Client {
    http: reqwest::blocking::Client,
    token: String,
    project: String,
    #[cfg(test)]
    base: String,
}
impl Client {
    fn configured() -> Result<Self> {
        let token=std::env::var("ION_NEBIUS_SANDBOX_IAM_TOKEN").context("Configure the sandbox IAM token on this owner; the inference key is a separate credential")?;
        let project = std::env::var("ION_NEBIUS_SANDBOX_PROJECT")
            .context("Configure the sandbox's Nebius project on this owner")?;
        ensure!(
            !token.is_empty() && !project.is_empty() && project.len() <= 160,
            "Sandbox authentication/project is missing"
        );
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
    fn base(&self) -> &str {
        #[cfg(test)]
        {
            &self.base
        }
        #[cfg(not(test))]
        {
            BASE
        }
    }
    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::blocking::RequestBuilder {
        self.http
            .request(method, format!("{}{path}", self.base()))
            .bearer_auth(&self.token)
            .header("Project", &self.project)
    }
    fn operation(&self, id: Uuid, image: Uuid) -> Result<Value> {
        let response = self
            .request(reqwest::Method::GET, &format!("/operations/{id}"))
            .send()
            .context("Sandbox operation status is unavailable; outcome remains unresolved")?;
        ensure!(
            response.status().is_success(),
            "Sandbox operation is unresolved (HTTP {})",
            response.status()
        );
        let value = read_json(response, 256 * 1024)?;
        ensure!(
            value["uuid"].as_str() == Some(&id.to_string())
                && value["kind"] == "instance"
                && value["image_uuid"].as_str() == Some(&image.to_string()),
            "Sandbox operation identity/image changed"
        );
        ensure!(
            matches!(
                value["status"].as_str(),
                Some("PENDING" | "ASSIGNED" | "EXECUTING" | "SUCCESS" | "FAILED" | "CANCELLED")
            ),
            "Unknown sandbox operation state"
        );
        Ok(value)
    }
    fn cancel(&self, id: Uuid) -> Result<()> {
        let response = self
            .request(reqwest::Method::DELETE, &format!("/operations/{id}"))
            .send()?;
        ensure!(
            matches!(response.status().as_u16(), 202 | 409),
            "Sandbox cancellation request was not acknowledged; inspect the operation"
        );
        Ok(())
    }
}
fn read_json(response: reqwest::blocking::Response, limit: usize) -> Result<Value> {
    let mut bytes = Vec::new();
    response.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= limit, "Sandbox response exceeded its limit");
    Ok(serde_json::from_slice(&bytes)?)
}
fn image(profile: &AgentProfile) -> Result<Uuid> {
    let target = profile
        .execution
        .sandbox_target
        .as_deref()
        .context("Set this agent's registered sandbox image UUID")?;
    let id = target.rsplit('@').next().unwrap_or(target);
    Uuid::parse_str(id)
        .context("Sandbox target must be an immutable ConTree image UUID, optionally name@UUID")
}
pub(crate) fn ready(settings: &DaemonSettings, profile: &AgentProfile) -> Result<()> {
    ensure!(
        settings
            .extra
            .get("nebius_sandbox_enabled")
            .and_then(Value::as_bool)
            == Some(true),
        "Token Factory sandbox is off in Settings"
    );
    let client = Client::configured()?;
    capability(&client, profile)?;
    Ok(())
}
fn capability(client: &Client, profile: &AgentProfile) -> Result<Capability> {
    ensure!(
        matches!(
            profile.provider,
            ProviderKind::Pi
                | ProviderKind::Codex
                | ProviderKind::Claude
                | ProviderKind::OpenCode
                | ProviderKind::Nebius
        ),
        "The selected harness has no bounded sandbox worker adapter yet"
    );
    let model = profile
        .model
        .as_deref()
        .context("Pinned harness model missing")?;
    let image = image(profile)?;
    let response = client
        .request(reqwest::Method::GET, &format!("/inspect/{image}/download"))
        .query(&[("path", "/ion/host.json")])
        .send()
        .context("Registered sandbox image is unavailable before execution")?;
    ensure!(
        response.status().is_success(),
        "Register the Ion harness worker in this sandbox image (HTTP {})",
        response.status()
    );
    let value = read_json(response, 64 * 1024)?;
    let manifest: Manifest = serde_json::from_value(value)?;
    ensure!(
        manifest.version == 1 && manifest.capabilities.len() <= 128,
        "Sandbox worker protocol/manifest is unsupported"
    );
    let mut matches = manifest
        .capabilities
        .into_iter()
        .filter(|c| c.provider == profile.provider && c.model == model);
    let selected = matches
        .next()
        .context("Selected harness/model is unavailable in this sandbox image")?;
    ensure!(
        matches.next().is_none()
            && (1..=32_000_000).contains(&selected.input_cap)
            && (1..=32_000_000).contains(&selected.output_cap),
        "Sandbox model capability is invalid or ambiguous"
    );
    // These are registered host capabilities, not model-produced metadata. The
    // worker verifies them against its actual native catalog before inference.
    Ok(selected)
}
pub(crate) fn run(
    coordinator: &TeamCoordinator,
    start: NativeTeamStart,
    workspace: &Path,
    events: &DriverEventSender,
    cancelled: &AtomicBool,
) -> Result<NativeTeamExit> {
    let claim = match &start {
        NativeTeamStart::Fresh(c) => c,
        NativeTeamStart::Resumed(c) => &c.claimed,
    };
    let profile = claim.profile.clone();
    let lease = claim.lease.clone();
    let image = image(&profile)?;
    let client = Client::configured()?;
    let cap = capability(&client, &profile)?;
    super::nebius::team::run_with_harness(
        coordinator,
        start,
        workspace,
        events,
        cancelled,
        profile.provider,
        |model| {
            Ok(ModelCallAllowance {
                model: model.into(),
                max_input_tokens: cap.input_cap,
                max_output_tokens: cap.output_cap,
            })
        },
        |model, system, messages, tools| {
            ensure!(model == cap.model, "Pinned remote model changed");
            let id = coordinator.begin_remote_call(
                &lease,
                image,
                &client.project,
                crate::model::unix_time(),
            )?;
            let request = WorkerRequest {
                version: 1,
                request_id: id,
                capability: cap.clone(),
                system: system.into(),
                messages: messages.into(),
                tools: tools.clone(),
            };
            let output = execute(&client, coordinator, image, &request, cancelled)?;
            ensure!(
                output.version == 1
                    && output.request_id == id
                    && output.provider == cap.provider
                    && output.model == cap.model,
                "Remote harness receipt identity changed"
            );
            let reply = team_completion::actions(&output.content, &tools, output.usage)?;
            if !reply.text.is_empty() {
                let _ = events.send(DriverEvent::TextDelta(reply.text.clone()));
            }
            Ok(reply)
        },
    )
}
fn execute(
    client: &Client,
    coordinator: &TeamCoordinator,
    image: Uuid,
    request: &WorkerRequest,
    cancelled: &AtomicBool,
) -> Result<WorkerResponse> {
    let input = serde_json::to_vec(request)?;
    if input.len() > 128 * 1024 {
        coordinator.finish_remote_call(request.request_id, None, "rejected", None)?;
        anyhow::bail!("Remote request exceeds its input bound");
    }
    if cancelled.load(Ordering::Acquire) {
        coordinator.finish_remote_call(request.request_id, None, "rejected", None)?;
        anyhow::bail!("Stopped before sandbox submission");
    }
    let body = json!({"command":"/usr/local/bin/ion-harness-worker","args":[],"image":image,"shell":false,"hostname":format!("ion-{}",request.request_id),"env":{"PATH":"/usr/local/bin:/usr/bin:/bin"},"cwd":"/ion/work","stdin":{"value":STANDARD.encode(input),"encoding":"base64","close":true},"timeout":150,"truncate_output_at":64*1024,"disposable":true,"preserve_env":false,"resources_limits":{"max_layer_bytes":256*1024*1024},"networking":{"enabled":true}});
    // The image supplies a credential bootstrap outside the prompt. No owner
    // credentials, repository walk, env dump, app tokens or vault files are sent.
    let response=client.request(reqwest::Method::POST,"/instances").json(&body).send().context("Sandbox spawn acknowledgement lost; no local fallback or repeated submission is allowed")?;
    if matches!(response.status().as_u16(), 400 | 401 | 403) {
        coordinator.finish_remote_call(request.request_id, None, "rejected", None)?;
        anyhow::bail!(
            "Sandbox rejected the request before launch (HTTP {})",
            response.status()
        );
    }
    ensure!(
        response.status().as_u16() == 201,
        "Sandbox submission outcome is unresolved (HTTP {})",
        response.status()
    );
    let value = read_json(response, 256 * 1024)?;
    let operation = Uuid::parse_str(
        value["uuid"]
            .as_str()
            .context("Sandbox spawn operation ID missing; outcome is unresolved")?,
    )?;
    coordinator.bind_remote_operation(request.request_id, operation)?;
    let deadline = Instant::now() + Duration::from_secs(155);
    let mut cancellation_requested = false;
    loop {
        if (cancelled.load(Ordering::Acquire) || Instant::now() >= deadline)
            && !cancellation_requested
        {
            client.cancel(operation)?;
            cancellation_requested = true;
        }
        let value = client.operation(operation, image)?;
        if let Some(state) = terminal(&value) {
            let output = if state == "SUCCESS" {
                worker_output(&value).and_then(|output| {
                    ensure!(
                        output.version == 1
                            && output.request_id == request.request_id
                            && output.provider == request.capability.provider
                            && output.model == request.capability.model,
                        "Remote harness receipt identity changed"
                    );
                    Ok(output)
                })
            } else {
                Err(anyhow::anyhow!(
                    "Sandbox harness stopped without a successful result"
                ))
            };
            let receipt = output
                .as_ref()
                .ok()
                .map(serde_json::to_string)
                .transpose()?;
            coordinator.finish_remote_call(
                request.request_id,
                Some(operation),
                state,
                receipt.as_deref(),
            )?;
            if let Ok(output) = &output {
                coordinator.record_remote_model_usage(
                    request.request_id,
                    output.provider,
                    &output.model,
                    output.usage,
                    crate::model::unix_time(),
                )?;
            }
            ensure!(
                !cancellation_requested,
                "Sandbox process stopped after cancellation; its reserved usage remains committed until a receipt is available"
            );
            return output;
        }
        ensure!(
            Instant::now() < deadline + Duration::from_secs(15),
            "Sandbox stop is unconfirmed; inspect this operation before continuing"
        );
        // Poll at a bounded cadence even after Stop; cancellation acknowledgement
        // is not a terminal receipt and must not create a busy loop.
        thread::sleep(Duration::from_millis(100));
        for _ in 0..4 {
            if cancelled.load(Ordering::Acquire) {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
}
fn terminal(value: &Value) -> Option<&str> {
    value["status"]
        .as_str()
        .filter(|s| matches!(*s, "SUCCESS" | "FAILED" | "CANCELLED"))
}
fn worker_output(value: &Value) -> Result<WorkerResponse> {
    let result = &value["metadata"]["result"];
    ensure!(
        result["state"]["exit_code"] == 0
            && result["state"]["timed_out"] != true
            && result["state"]["stopped"] != true
            && result["state"]["core_dump"] != true
            && (result["state"]["signal"].is_null()
                || result["state"]["signal"]
                    .as_i64()
                    .is_some_and(|s| matches!(s, -1 | 0))),
        "Remote harness did not exit successfully"
    );
    let stdout = &result["stdout"];
    ensure!(
        stdout["truncated"] == false,
        "Remote harness output was truncated"
    );
    let data = stdout["value"]
        .as_str()
        .context("Remote harness stdout missing")?;
    ensure!(
        data.len() <= 96 * 1024,
        "Remote harness output is too large"
    );
    let bytes = match stdout["encoding"].as_str() {
        Some("base64") => STANDARD.decode(data)?,
        Some("ascii" | "utf-8") => data.as_bytes().to_vec(),
        _ => anyhow::bail!("Remote harness output encoding unsupported"),
    };
    ensure!(
        bytes.len() <= 64 * 1024,
        "Remote harness output is too large"
    );
    Ok(serde_json::from_slice(&bytes)?)
}
/// Restart/Stop reconciles existing operation IDs only; this never submits work.
pub(crate) fn reconcile(coordinator: &TeamCoordinator, attempt: Uuid) -> Result<bool> {
    let calls = coordinator.remote_calls(attempt)?;
    if calls.iter().all(|c| terminal_state(&c.state)) {
        return Ok(true);
    }
    let client = Client::configured()?;
    reconcile_with(&client, coordinator, attempt)
}
fn reconcile_with(client: &Client, coordinator: &TeamCoordinator, attempt: Uuid) -> Result<bool> {
    let calls = coordinator.remote_calls(attempt)?;
    for call in calls.iter().filter(|c| !terminal_state(&c.state)) {
        ensure!(
            call.project == client.project,
            "Sandbox project differs from the pinned remote operation"
        );
        let operation=call.operation.context("Spawn acknowledgement was lost; inspect Nebius operations manually. Ion will not resubmit or fall back")?;
        let value = client.operation(operation, call.image)?;
        if let Some(state) = terminal(&value) {
            let output = if state == "SUCCESS" {
                worker_output(&value).ok()
            } else {
                None
            };
            // Process stop is authoritative even if a model receipt is absent or
            // invalid. Accounting may settle only its pinned request identity.
            let receipt = output
                .as_ref()
                .filter(|r| r.request_id == call.id && r.version == 1)
                .map(serde_json::to_string)
                .transpose()?;
            coordinator.finish_remote_call(call.id, Some(operation), state, receipt.as_deref())?;
            if let Some(output) = output.filter(|r| r.request_id == call.id && r.version == 1) {
                let _ = coordinator.record_remote_model_usage(
                    call.id,
                    output.provider,
                    &output.model,
                    output.usage,
                    crate::model::unix_time(),
                );
            }
        } else {
            client.cancel(operation)?;
        }
    }
    coordinator.remote_attempt_stopped(attempt)
}
fn terminal_state(state: &str) -> bool {
    matches!(state, "SUCCESS" | "FAILED" | "CANCELLED" | "rejected")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn team_remote_terminal_receipt_requires_process_and_untruncated_output() {
        let id = Uuid::new_v4();
        let response = WorkerResponse {
            version: 1,
            request_id: id,
            provider: ProviderKind::Codex,
            model: "pinned".into(),
            content: "{}".into(),
            usage: ModelTokenUsage {
                input_tokens: 1,
                output_tokens: 2,
            },
        };
        let mut value = json!({"status":"SUCCESS","metadata":{"result":{"state":{"exit_code":0,"signal":null},"stdout":{"value":STANDARD.encode(serde_json::to_vec(&response).unwrap()),"encoding":"base64","truncated":false}}}});
        assert_eq!(worker_output(&value).unwrap().request_id, id);
        // The official schema uses an optional integer signal, not a nullable
        // enum. A normal exit may encode no signal as zero or minus one.
        for signal in [0, -1] {
            value["metadata"]["result"]["state"]["signal"] = json!(signal);
            assert!(worker_output(&value).is_ok());
        }
        value["metadata"]["result"]["state"]["signal"] = json!(9);
        assert!(worker_output(&value).is_err());
        value["metadata"]["result"]["state"]["signal"] = Value::Null;
        value["metadata"]["result"]["state"]["timed_out"] = json!(true);
        assert!(worker_output(&value).is_err());
        value["metadata"]["result"]["state"]["timed_out"] = json!(false);
        value["metadata"]["result"]["stdout"]["truncated"] = json!(true);
        assert!(worker_output(&value).is_err());
        assert!(terminal(&json!({"status":"EXECUTING"})).is_none());
        assert!(terminal(&json!({"status":"CANCELLED"})).is_some());
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use crate::driver::team_test_http::Server;
    use crate::{
        control::ControlStore,
        teams::{DispatchHost, PrepareRun},
    };
    use std::sync::Arc;
    use waku_protocol::{control::*, teams::*};
    struct Fixture {
        path: std::path::PathBuf,
        owner: TeamCoordinator,
        lease: crate::teams::AssignmentLease,
        request: WorkerRequest,
        image: Uuid,
    }
    impl Fixture {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("ion-remote-fixture-{}.db", Uuid::new_v4()));
            let definitions = ControlStore::new(path.clone());
            let agent = Uuid::new_v4();
            let team = Uuid::new_v4();
            let image = Uuid::new_v4();
            definitions
                .save(
                    0,
                    ControlRecord::Agent(AgentProfile {
                        id: agent,
                        name: "Fixture".into(),
                        expertise: "Review".into(),
                        instructions: "Review".into(),
                        expected_output: "Report".into(),
                        provider: ProviderKind::Codex,
                        model: Some("pinned".into()),
                        execution: AgentExecution {
                            environment: ExecutionEnvironment::NebiusSandbox,
                            sandbox_target: Some(image.to_string()),
                            local_fallback: true,
                        },
                        access: AgentAccess::default(),
                        max_parallel_tasks: 1,
                    }),
                )
                .unwrap();
            definitions
                .save(
                    definitions.load().unwrap().revision,
                    ControlRecord::Team(Team {
                        id: team,
                        name: "Fixture".into(),
                        objective: "Report".into(),
                        limits: None,
                        members: vec![TeamMember {
                            agent_id: agent,
                            parent_id: None,
                        }],
                    }),
                )
                .unwrap();
            let owner = TeamCoordinator::new(path.clone());
            let run = owner
                .prepare(
                    PrepareRun {
                        id: Uuid::new_v4(),
                        root_task_id: Uuid::new_v4(),
                        project_id: Uuid::new_v4(),
                        team_id: team,
                        expected_definition_revision: definitions.load().unwrap().revision,
                        objective: "Review".into(),
                        input_data: vec![],
                        source_attachments: vec![],
                        source_prompt: None,
                        origin: None,
                        limits: TeamRunLimits::default(),
                        access: AgentAccess::default(),
                    },
                    100,
                )
                .unwrap();
            let claim = owner
                .claim(
                    run.id,
                    run.work[0].id,
                    DispatchHost::NebiusSandbox {
                        target: image.to_string(),
                    },
                    101,
                    100,
                )
                .unwrap();
            owner
                .reserve_model_call(
                    &claim.lease,
                    1,
                    ModelCallAllowance {
                        model: "pinned".into(),
                        max_input_tokens: 1000,
                        max_output_tokens: 100,
                    },
                    102,
                )
                .unwrap();
            let id = owner
                .begin_remote_call(&claim.lease, image, "fixture-project", 102)
                .unwrap();
            let request = WorkerRequest {
                version: 1,
                request_id: id,
                capability: Capability {
                    provider: ProviderKind::Codex,
                    model: "pinned".into(),
                    input_cap: 1000,
                    output_cap: 100,
                },
                system: "Review granted inputs".into(),
                messages: vec![],
                tools: json!([]),
            };
            Self {
                path,
                owner,
                lease: claim.lease,
                request,
                image,
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
    fn client(server: &Server) -> Client {
        Client {
            http: reqwest::blocking::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(3))
                .build()
                .unwrap(),
            token: "fixture-IAM-not-real".into(),
            project: "fixture-project".into(),
            base: server.url.clone(),
        }
    }
    fn operation(f: &Fixture, id: Uuid, state: &str) -> Value {
        json!({"uuid":id,"image_uuid":f.image,"kind":"instance","status":state})
    }
    fn response(f: &Fixture) -> WorkerResponse {
        WorkerResponse {
            version: 1,
            request_id: f.request.request_id,
            provider: ProviderKind::Codex,
            model: "pinned".into(),
            content: r#"{"text":"finished","calls":[]}"#.into(),
            usage: ModelTokenUsage {
                input_tokens: 10,
                output_tokens: 5,
            },
        }
    }
    fn body(v: Value) -> Vec<u8> {
        serde_json::to_vec(&v).unwrap()
    }
    #[test]
    fn team_remote_spawn_pins_image_and_transfers_only_bounded_worker_inputs() {
        let f = Fixture::new();
        let id = Uuid::new_v4();
        let mut done = operation(&f, id, "SUCCESS");
        done["metadata"] = json!({"result":{"state":{"exit_code":0,"signal":null},"stdout":{"value":STANDARD.encode(serde_json::to_vec(&response(&f)).unwrap()),"encoding":"base64","truncated":false}}});
        let server = Server::new(vec![(201, body(json!({"uuid":id}))), (200, body(done))]);
        let result = execute(
            &client(&server),
            &f.owner,
            f.image,
            &f.request,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(result.model, "pinned");
        assert!(
            f.owner
                .remote_attempt_stopped(f.lease.attempt_id())
                .unwrap()
        );
        let requests = server.finish();
        assert_eq!(requests.len(), 2);
        assert!(requests[0].line.starts_with("POST /instances "));
        assert_eq!(requests[0].body["image"], f.image.to_string());
        assert_eq!(
            requests[0].body["env"],
            json!({"PATH":"/usr/local/bin:/usr/bin:/bin"})
        );
        let stdin = STANDARD
            .decode(requests[0].body["stdin"]["value"].as_str().unwrap())
            .unwrap();
        let request: WorkerRequest = serde_json::from_slice(&stdin).unwrap();
        assert_eq!(request.request_id, f.request.request_id);
        assert!(!String::from_utf8(stdin).unwrap().contains("fixture-IAM"));
    }
    #[test]
    fn team_remote_cancel_ack_keeps_capacity_until_terminal_operation() {
        let f = Fixture::new();
        let id = Uuid::new_v4();
        let cancelled = Arc::new(AtomicBool::new(false));
        let stop = cancelled.clone();
        let server = Server::with_hook(
            vec![
                (201, body(json!({"uuid":id}))),
                (202, body(json!({}))),
                (200, body(operation(&f, id, "EXECUTING"))),
                (200, body(operation(&f, id, "CANCELLED"))),
            ],
            move |r| {
                if r.line.starts_with("POST ") {
                    stop.store(true, Ordering::Release)
                }
            },
        );
        assert!(execute(&client(&server), &f.owner, f.image, &f.request, &cancelled).is_err());
        assert!(
            f.owner
                .remote_attempt_stopped(f.lease.attempt_id())
                .unwrap()
        );
        let requests = server.finish();
        assert_eq!(requests.len(), 4);
        assert!(requests[1].line.starts_with("DELETE "));
        assert_eq!(
            requests
                .iter()
                .filter(|r| r.line.starts_with("POST "))
                .count(),
            1
        );
        assert!(
            f.owner
                .snapshot(f.lease.run_id())
                .unwrap()
                .usage
                .reserved_tokens
                > 0
        );
    }
    #[test]
    fn team_remote_lost_spawn_acknowledgement_is_not_repeated_or_assumed_stopped() {
        let f = Fixture::new();
        let server = Server::new(vec![(201, b"{}".to_vec())]);
        assert!(
            execute(
                &client(&server),
                &f.owner,
                f.image,
                &f.request,
                &AtomicBool::new(false)
            )
            .is_err()
        );
        assert!(
            !f.owner
                .remote_attempt_stopped(f.lease.attempt_id())
                .unwrap()
        );
        assert!(
            f.owner.remote_calls(f.lease.attempt_id()).unwrap()[0]
                .operation
                .is_none()
        );
        assert!(reconcile_with(&client(&server), &f.owner, f.lease.attempt_id()).is_err());
        assert_eq!(server.finish().len(), 1);
    }
    #[test]
    fn team_remote_restart_reconciliation_only_checks_and_cancels_existing_identity() {
        let f = Fixture::new();
        let id = Uuid::new_v4();
        f.owner
            .bind_remote_operation(f.request.request_id, id)
            .unwrap();
        let server = Server::new(vec![
            (200, body(operation(&f, id, "EXECUTING"))),
            (202, body(json!({}))),
            (200, body(operation(&f, id, "CANCELLED"))),
        ]);
        let client = client(&server);
        let reopened = TeamCoordinator::new(f.path.clone());
        assert!(!reconcile_with(&client, &reopened, f.lease.attempt_id()).unwrap());
        assert!(reconcile_with(&client, &reopened, f.lease.attempt_id()).unwrap());
        let requests = server.finish();
        assert!(requests.iter().all(|r| !r.line.starts_with("POST ")));
    }
    #[test]
    fn team_remote_late_success_settles_usage_after_stop_without_resuming_actions() {
        let f = Fixture::new();
        let id = Uuid::new_v4();
        f.owner
            .bind_remote_operation(f.request.request_id, id)
            .unwrap();
        let before = f.owner.snapshot(f.lease.run_id()).unwrap();
        f.owner.stop(before.id, before.revision, 103).unwrap();
        let mut done = operation(&f, id, "SUCCESS");
        done["metadata"] = json!({"result":{"state":{"exit_code":0,"signal":null},"stdout":{"value":STANDARD.encode(serde_json::to_vec(&response(&f)).unwrap()),"encoding":"base64","truncated":false}}});
        let server = Server::new(vec![(200, body(done))]);
        let reopened = TeamCoordinator::new(f.path.clone());
        assert!(reconcile_with(&client(&server), &reopened, f.lease.attempt_id()).unwrap());
        let settled = reopened.snapshot(f.lease.run_id()).unwrap();
        assert_eq!(settled.usage.consumed_tokens, 15);
        assert_eq!(settled.usage.reserved_tokens, 0);
        assert_eq!(settled.work[0].status, AssignmentStatus::Canceling);
        assert!(settled.work[0].result.is_none());
        // Duplicate accounting is harmless; another request or a changed receipt is not.
        reopened
            .record_remote_model_usage(
                f.request.request_id,
                ProviderKind::Codex,
                "pinned",
                response(&f).usage,
                104,
            )
            .unwrap();
        assert_eq!(
            reopened
                .snapshot(f.lease.run_id())
                .unwrap()
                .usage
                .consumed_tokens,
            15
        );
        assert!(
            reopened
                .reserve_model_call(
                    &f.lease,
                    2,
                    ModelCallAllowance {
                        model: "pinned".into(),
                        max_input_tokens: 1000,
                        max_output_tokens: 100
                    },
                    104
                )
                .is_err()
        );
        assert_eq!(server.finish().len(), 1);
    }
    #[test]
    fn team_remote_late_receipt_cannot_settle_another_harness_or_model() {
        for change_provider in [true, false] {
            let f = Fixture::new();
            let id = Uuid::new_v4();
            f.owner
                .bind_remote_operation(f.request.request_id, id)
                .unwrap();
            let mut output = response(&f);
            if change_provider {
                output.provider = ProviderKind::Pi;
            } else {
                output.model = "other-model".into();
            }
            let mut done = operation(&f, id, "SUCCESS");
            done["metadata"] = json!({"result":{"state":{"exit_code":0,"signal":null},"stdout":{"value":STANDARD.encode(serde_json::to_vec(&output).unwrap()),"encoding":"base64","truncated":false}}});
            let server = Server::new(vec![(200, body(done))]);
            assert!(reconcile_with(&client(&server), &f.owner, f.lease.attempt_id()).unwrap());
            let settled = f.owner.snapshot(f.lease.run_id()).unwrap();
            assert_eq!(settled.usage.consumed_tokens, 0);
            assert_eq!(settled.usage.reserved_tokens, 1100);
            assert!(settled.work[0].result.is_none());
            assert_eq!(server.finish().len(), 1);
        }
    }
    #[test]
    fn team_remote_wrong_image_and_malformed_terminal_output_do_not_publish_actions() {
        let f = Fixture::new();
        let id = Uuid::new_v4();
        let mut wrong = operation(&f, id, "SUCCESS");
        wrong["image_uuid"] = json!(Uuid::new_v4());
        let server = Server::new(vec![(201, body(json!({"uuid":id}))), (200, body(wrong))]);
        assert!(
            execute(
                &client(&server),
                &f.owner,
                f.image,
                &f.request,
                &AtomicBool::new(false)
            )
            .is_err()
        );
        assert!(
            !f.owner
                .remote_attempt_stopped(f.lease.attempt_id())
                .unwrap()
        );
        server.finish();
        let f = Fixture::new();
        let server = Server::new(vec![
            (201, body(json!({"uuid":id}))),
            (200, body(operation(&f, id, "SUCCESS"))),
        ]);
        assert!(
            execute(
                &client(&server),
                &f.owner,
                f.image,
                &f.request,
                &AtomicBool::new(false)
            )
            .is_err()
        );
        assert!(
            f.owner
                .remote_attempt_stopped(f.lease.attempt_id())
                .unwrap()
        );
        server.finish();
    }
}
