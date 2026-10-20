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
mod identity;
pub(crate) use identity::ExecutionIdentity;
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    execution: Option<ExecutionIdentity>,
}
struct RegisteredHost {
    capability: Capability,
    execution: Option<ExecutionIdentity>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkerRequest {
    pub version: u16,
    pub request_id: Uuid,
    pub capability: Capability,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution: Option<ExecutionIdentity>,
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
    /// Optional SDK price-table estimate, never an invoice or authoritative bill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_cost_usd_micros: Option<u64>,
}
pub(super) struct Client {
    http: reqwest::blocking::Client,
    token: String,
    managed_workspace: Option<String>,
    pub(super) project: String,
    #[cfg(test)]
    base: String,
}
impl Client {
    pub(super) fn configured() -> Result<Self> {
        let token = match std::env::var("ION_NEBIUS_SANDBOX_IAM_TOKEN") {
            Ok(token) => token,
            Err(_) => {
                let status = waku_credentials::cloud::status()?;
                let workspace = status
                    .workspace_id
                    .filter(|_| status.signed_in)
                    .context("Sign in to Ion to use managed sandboxes")?;
                let http = reqwest::blocking::Client::builder()
                    .redirect(reqwest::redirect::Policy::none())
                    .connect_timeout(Duration::from_secs(3))
                    .timeout(Duration::from_secs(20))
                    .build()?;
                let response = waku_credentials::cloud::services::sandbox_request(
                    &http,
                    reqwest::Method::GET,
                    "/config",
                    &workspace,
                )?
                .send()?;
                ensure!(
                    response.status().is_success(),
                    "Managed sandbox configuration is unavailable"
                );
                let config = read_json(response, 64 * 1024)?;
                ensure!(
                    config["available"] == true,
                    "Managed sandbox activation, permissions, reviewed images and pricing are required; Token Factory must be ON"
                );
                let project = config["project"]
                    .as_str()
                    .filter(|s| !s.is_empty() && s.len() <= 160)
                    .context("Managed sandbox project missing")?
                    .to_owned();
                return Ok(Self {
                    http,
                    token: String::new(),
                    managed_workspace: Some(workspace),
                    project,
                    #[cfg(test)]
                    base: BASE.into(),
                });
            }
        };
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
            managed_workspace: None,
            project,
            #[cfg(test)]
            base: BASE.into(),
        })
    }
    #[cfg(test)]
    pub(super) fn fixture(base: &str, project: &str) -> Self {
        Self {
            http: reqwest::blocking::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(3))
                .build()
                .unwrap(),
            token: "fixture-not-real".into(),
            managed_workspace: None,
            project: project.into(),
            base: base.into(),
        }
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
    pub(super) fn request(
        &self,
        method: reqwest::Method,
        path: &str,
    ) -> Result<reqwest::blocking::RequestBuilder> {
        if let Some(workspace) = &self.managed_workspace {
            return waku_credentials::cloud::services::sandbox_request(
                &self.http, method, path, workspace,
            );
        }
        Ok(self
            .http
            .request(method, format!("{}{path}", self.base()))
            .bearer_auth(&self.token)
            .header("Project", &self.project))
    }
    pub(super) fn ensure_enabled(&self) -> Result<()> {
        if self.managed_workspace.is_none() {
            return Ok(());
        }
        let response = self.request(reqwest::Method::GET, "/config")?.send()?;
        let config = read_json(response, 64 * 1024)?;
        ensure!(
            config["available"] == true && config["enabled"] == true,
            "Managed sandbox is unavailable or Token Factory is OFF"
        );
        Ok(())
    }
    pub(super) fn recover_operation(&self, request: Uuid, operation: Uuid) -> Result<()> {
        if self.managed_workspace.is_none() {
            return Ok(());
        }
        let response = self
            .request(reqwest::Method::POST, "/recover")?
            .json(&json!({"requestId":request,"operationId":operation}))
            .send()?;
        ensure!(
            response.status().is_success(),
            "The gateway could not verify this original sandbox operation"
        );
        Ok(())
    }
    pub(super) fn command_policy(&self, target: Uuid) -> Result<Option<Value>> {
        if self.managed_workspace.is_none() {
            return Ok(None);
        }
        let response = self
            .request(reqwest::Method::GET, &format!("/images/{target}"))?
            .send()?;
        ensure!(
            response.status().is_success(),
            "Managed command image is unavailable"
        );
        Ok(Some(read_json(response, 64 * 1024)?))
    }
    pub(super) fn default_command_target(&self) -> Result<Uuid> {
        ensure!(
            self.managed_workspace.is_some(),
            "Use your explicitly configured personal sandbox transport"
        );
        let response = self.request(reqwest::Method::GET, "/config")?.send()?;
        ensure!(
            response.status().is_success(),
            "Managed sandbox configuration is unavailable"
        );
        let config = read_json(response, 64 * 1024)?;
        ensure!(
            config["available"] == true && config["enabled"] == true,
            "Managed sandbox is unavailable or Token Factory is OFF"
        );
        let target = config["images"]
            .as_array()
            .and_then(|rows| rows.iter().find(|i| i["kind"] == "command"))
            .and_then(|i| i["target"].as_str())
            .context("No credential-free command image is registered by the operator")?;
        Ok(Uuid::parse_str(target)?)
    }
    fn operation(&self, id: Uuid, image: Uuid) -> Result<Value> {
        self.operation_bounded(id, image, 512 * 1024)
    }
    pub(super) fn operation_bounded(&self, id: Uuid, image: Uuid, limit: usize) -> Result<Value> {
        let response = self
            .request(reqwest::Method::GET, &format!("/operations/{id}"))?
            .send()
            .context("Sandbox operation status is unavailable; outcome remains unresolved")?;
        ensure!(
            response.status().is_success(),
            "Sandbox operation is unresolved (HTTP {})",
            response.status()
        );
        let value = read_json(response, limit)?;
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
    pub(super) fn cancel(&self, id: Uuid) -> Result<()> {
        let response = self
            .request(reqwest::Method::DELETE, &format!("/operations/{id}"))?
            .send()?;
        ensure!(
            matches!(response.status().as_u16(), 202 | 409),
            "Sandbox cancellation request was not acknowledged; inspect the operation"
        );
        Ok(())
    }
}
pub(super) fn read_json(response: reqwest::blocking::Response, limit: usize) -> Result<Value> {
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
    client.ensure_enabled()?;
    capability(&client, profile)?;
    Ok(())
}
fn capability(client: &Client, profile: &AgentProfile) -> Result<RegisteredHost> {
    ensure!(
        matches!(
            profile.provider,
            ProviderKind::Pi
                | ProviderKind::OhMyPi
                | ProviderKind::DeepSeek
                | ProviderKind::Kimi
                | ProviderKind::Antigravity
                | ProviderKind::OpenCode2
                | ProviderKind::Fx
                | ProviderKind::Grok
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
        .request(reqwest::Method::GET, &format!("/inspect/{image}/download"))?
        .query(&[("path", "/ion/host.json")])
        .send()
        .context("Registered sandbox image is unavailable before execution")?;
    ensure!(
        response.status().is_success(),
        "Register the Ion harness worker in this sandbox image (HTTP {})",
        response.status()
    );
    let value = read_json(response, 64 * 1024)?;
    registered_host(value, profile.provider, model)
}
fn registered_host(value: Value, provider: ProviderKind, model: &str) -> Result<RegisteredHost> {
    let manifest: Manifest = serde_json::from_value(value)?;
    ensure!(
        matches!(
            (manifest.version, manifest.execution.is_some()),
            (1, false) | (2, true)
        ) && manifest.capabilities.len() <= 128,
        "Sandbox worker protocol/manifest is unsupported"
    );
    if let Some(execution) = &manifest.execution {
        execution.validate()?;
    }
    let mut matches = manifest
        .capabilities
        .into_iter()
        .filter(|c| c.provider == provider && c.model == model);
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
    Ok(RegisteredHost {
        capability: selected,
        execution: manifest.execution,
    })
}
pub(crate) fn run(
    coordinator: &TeamCoordinator,
    start: NativeTeamStart,
    workspace: &Path,
    events: &DriverEventSender,
    cancelled: &AtomicBool,
) -> Result<NativeTeamExit> {
    let client = Client::configured()?;
    run_with_client(coordinator, start, workspace, events, cancelled, &client)
}
fn run_with_client(
    coordinator: &TeamCoordinator,
    start: NativeTeamStart,
    workspace: &Path,
    events: &DriverEventSender,
    cancelled: &AtomicBool,
    client: &Client,
) -> Result<NativeTeamExit> {
    let claim = match &start {
        NativeTeamStart::Fresh(c) => c,
        NativeTeamStart::Resumed(c) => &c.claimed,
    };
    let profile = claim.profile.clone();
    let lease = claim.lease.clone();
    let image = image(&profile)?;
    let host = capability(client, &profile)?;
    let cap = host.capability;
    super::nebius::team::run_with_remote_harness_bound(
        coordinator,
        start,
        workspace,
        events,
        cancelled,
        profile.provider,
        client.managed_workspace.as_deref(),
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
                execution: host.execution.clone(),
                system: system.into(),
                messages: messages.into(),
                tools: tools.clone(),
            };
            let output = execute(client, coordinator, image, &request, cancelled)?;
            ensure!(
                output.version == 1
                    && output.request_id == id
                    && output.provider == cap.provider
                    && output.model == cap.model,
                "Remote harness receipt identity changed"
            );
            let mut reply = team_completion::actions(&output.content, &tools, output.usage)?;
            reply.cost = output
                .estimated_cost_usd_micros
                .map(crate::usage_ledger::claude_sdk_estimate);
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
    if let Some(identity) = &request.execution {
        if let Err(error) = identity.validate() {
            coordinator.finish_remote_call(request.request_id, None, "rejected", None)?;
            return Err(error);
        }
    }
    coordinator.pin_remote_input(request.request_id, std::str::from_utf8(&input)?)?;
    let mut body = json!({"command":"/usr/local/bin/ion-harness-worker","args":[],"image":image,"shell":false,"hostname":format!("ion-{}",request.request_id),"env":{"PATH":"/usr/local/bin:/usr/bin:/bin"},"cwd":"/ion/work","stdin":{"value":STANDARD.encode(input),"encoding":"base64","close":true},"timeout":150,"truncate_output_at":64*1024,"disposable":true,"preserve_env":false,"resources_limits":{"max_layer_bytes":256*1024*1024},"networking":{"enabled":true}});
    if let Some(identity) = &request.execution {
        body["uid"] = json!(identity.uid);
        body["gid"] = json!(identity.gid);
        body["env"]["HOME"] = json!(identity.home);
    }
    // The image supplies a credential bootstrap outside the prompt. No owner
    // credentials, repository walk, env dump, app tokens or vault files are sent.
    let response=client.request(reqwest::Method::POST,"/instances")?.json(&body).send().context("Sandbox spawn acknowledgement lost; no local fallback or repeated submission is allowed")?;
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
    let output: WorkerResponse = serde_json::from_slice(&bytes)?;
    ensure!(
        output
            .estimated_cost_usd_micros
            .is_none_or(
                |micros| output.provider == ProviderKind::Claude && micros <= 1_000_000_000_000
            ),
        "Remote SDK estimate contract changed"
    );
    Ok(output)
}
/// User input is an operation ID, never proof. Only the actual API metadata
/// can establish that it belongs to the persisted original worker request.
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
    if coordinator.recovery_confirmation_replayed(
        run,
        work,
        attempt,
        revision,
        &waku_protocol::teams::TeamRecoveryConfirmation::BindRemoteOperation {
            request_id: request,
            operation_id: operation,
        },
    )? {
        return Ok(());
    }
    ensure!(
        !operation.is_nil(),
        "Choose a non-empty sandbox operation ID"
    );
    let snapshot = coordinator.snapshot(run)?;
    ensure!(
        snapshot.revision == revision,
        "Run changed; refresh before recovery"
    );
    let assignment = snapshot
        .work
        .iter()
        .find(|w| w.id == work)
        .context("Assignment missing")?;
    ensure!(
        assignment.can_reconcile
            && assignment.attempts.last().is_some_and(|a| a.id == attempt
                && a.environment == waku_protocol::teams::ExecutionEnvironment::NebiusSandbox),
        "Remote recovery attempt or state changed"
    );
    let calls = coordinator.remote_calls(attempt)?;
    let call = calls
        .iter()
        .find(|c| c.id == request)
        .context("Remote request missing")?;
    ensure!(
        call.state == "submitting" && call.operation.is_none() && call.project == client.project,
        "Remote request/project changed or is already bound"
    );
    let saved=coordinator.remote_input(request)?.context("Legacy request lacks an exact input witness; operator inspection cannot automatically bind it")?;
    let expected: Value = serde_json::from_str(&saved)?;
    client.recover_operation(request, operation)?;
    let value = client.operation(operation, call.image)?;
    verify_original_request(&value, call.image, request, &expected)?;
    coordinator.bind_recovered_remote_operation(
        run,
        work,
        attempt,
        revision,
        request,
        operation,
        crate::model::unix_time(),
    )
}
fn verify_original_request(
    value: &Value,
    image: Uuid,
    request: Uuid,
    expected: &Value,
) -> Result<()> {
    let original: WorkerRequest = serde_json::from_value(expected.clone())?;
    let identity = original.execution.as_ref();
    if let Some(identity) = identity {
        identity.validate()?;
    }
    let environment = match identity {
        Some(identity) => json!({"PATH":"/usr/local/bin:/usr/bin:/bin", "HOME":identity.home}),
        None => json!({"PATH":"/usr/local/bin:/usr/bin:/bin"}),
    };
    let m = &value["metadata"];
    ensure!(
        m["image"].as_str() == Some(&image.to_string())
            && m["hostname"] == format!("ion-{request}")
            && m["command"] == "/usr/local/bin/ion-harness-worker"
            && m["args"] == json!([])
            && m["shell"] == false
            && m["env"] == environment
            && m["cwd"] == "/ion/work"
            && m["timeout"] == 150
            && m["truncate_output_at"] == 64 * 1024
            && m["disposable"] == true
            && m["preserve_env"] == false
            && m["networking"] == json!({"enabled":true})
            && m["resources_limits"] == json!({"max_layer_bytes":256*1024*1024})
            && match identity {
                Some(identity) => m["uid"] == identity.uid && m["gid"] == identity.gid,
                None =>
                    m.get("uid").is_none_or(|v| *v == 0) && m.get("gid").is_none_or(|v| *v == 0),
            }
            && m.get("files")
                .is_none_or(|v| v.is_null() || *v == json!({})),
        "Sandbox metadata differs from the original bounded execution host"
    );
    let stdin = &m["stdin"];
    ensure!(
        stdin["encoding"] == "base64" && stdin["close"] == true && stdin["truncated"] != true,
        "Original sandbox input witness is unavailable"
    );
    let data = stdin["value"]
        .as_str()
        .context("Original sandbox input missing")?;
    ensure!(
        data.len() <= 176 * 1024,
        "Original sandbox input exceeds its bound"
    );
    let bytes = STANDARD.decode(data)?;
    ensure!(
        bytes.len() <= 128 * 1024
            && serde_json::from_slice::<Value>(&bytes)? == *expected
            && expected["request_id"] == request.to_string(),
        "Sandbox input/model/grants differ from the original request"
    );
    Ok(())
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
            estimated_cost_usd_micros: None,
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
            Self::new_at(100, true)
        }
        fn new_at(now: u64, reserve: bool) -> Self {
            Self::with_provider(now, reserve, ProviderKind::Codex)
        }
        fn with_provider(now: u64, reserve: bool, provider: ProviderKind) -> Self {
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
                        provider,
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
                        account_scope: None,
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
                    now,
                )
                .unwrap();
            let claim = owner
                .claim(
                    run.id,
                    run.work[0].id,
                    DispatchHost::NebiusSandbox {
                        target: image.to_string(),
                    },
                    now + 1,
                    100,
                )
                .unwrap();
            let id = if reserve {
                owner
                    .reserve_model_call(
                        &claim.lease,
                        1,
                        ModelCallAllowance {
                            model: "pinned".into(),
                            max_input_tokens: 1000,
                            max_output_tokens: 100,
                        },
                        now + 2,
                    )
                    .unwrap();
                owner
                    .begin_remote_call(&claim.lease, image, "fixture-project", now + 2)
                    .unwrap()
            } else {
                Uuid::new_v4()
            };
            let request = WorkerRequest {
                version: 1,
                request_id: id,
                capability: Capability {
                    provider,
                    model: "pinned".into(),
                    input_cap: 1000,
                    output_cap: 100,
                },
                execution: None,
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
            managed_workspace: None,
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
            estimated_cost_usd_micros: None,
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
    fn registered_identity() -> ExecutionIdentity {
        ExecutionIdentity {
            uid: 1000,
            gid: 1000,
            home: "/home/ion".into(),
        }
    }
    #[test]
    fn team_remote_manifest_versions_require_their_exact_registered_identity() {
        let cap = json!({"provider":"codex","model":"pinned","input_cap":1000,"output_cap":100});
        let legacy = json!({"version":1,"capabilities":[cap.clone()]});
        assert!(
            registered_host(legacy.clone(), ProviderKind::Codex, "pinned")
                .unwrap()
                .execution
                .is_none()
        );
        let current = json!({"version":2,"execution":registered_identity(),"capabilities":[cap]});
        assert_eq!(
            registered_host(current.clone(), ProviderKind::Codex, "pinned")
                .unwrap()
                .execution
                .unwrap()
                .uid,
            1000
        );
        for (version, execution) in [
            (1, Some(json!(registered_identity()))),
            (2, None),
            (3, Some(json!(registered_identity()))),
        ] {
            let mut value = legacy.clone();
            value["version"] = json!(version);
            if let Some(identity) = execution {
                value["execution"] = identity;
            }
            assert!(registered_host(value, ProviderKind::Codex, "pinned").is_err());
        }
        for (field, value) in [
            ("uid", json!(0)),
            ("gid", json!(0)),
            ("home", json!("/root")),
            ("extra", json!(true)),
        ] {
            let mut changed = current.clone();
            changed["execution"][field] = value;
            assert!(
                registered_host(changed, ProviderKind::Codex, "pinned").is_err(),
                "{field}"
            );
        }
        assert!(registered_host(current.clone(), ProviderKind::Claude, "pinned").is_err());
        assert!(registered_host(current, ProviderKind::Codex, "other-model").is_err());
    }
    #[test]
    fn team_remote_nonroot_submission_pins_and_recovers_exact_process_identity() {
        let mut f = Fixture::new();
        f.request.execution = Some(registered_identity());
        let metadata = lost_ack(&f);
        assert_eq!(metadata["uid"], 1000);
        assert_eq!(metadata["gid"], 1000);
        assert_eq!(
            metadata["env"],
            json!({"PATH":"/usr/local/bin:/usr/bin:/bin","HOME":"/home/ion"})
        );
        let saved = f.owner.remote_input(f.request.request_id).unwrap().unwrap();
        let expected: Value = serde_json::from_str(&saved).unwrap();
        assert_eq!(expected["execution"], json!(registered_identity()));
        let good = json!({"metadata":metadata});
        verify_original_request(&good, f.image, f.request.request_id, &expected).unwrap();
        for (key, value) in [
            ("uid", json!(0)),
            ("gid", json!(0)),
            ("uid", json!(null)),
            ("gid", json!(null)),
            ("cwd", json!("/ion")),
            ("env", json!({"PATH":"/usr/local/bin:/usr/bin:/bin"})),
            (
                "env",
                json!({"PATH":"/usr/local/bin:/usr/bin:/bin","HOME":"/root"}),
            ),
        ] {
            let mut changed = good.clone();
            changed["metadata"][key] = value;
            assert!(
                verify_original_request(&changed, f.image, f.request.request_id, &expected)
                    .is_err(),
                "{key}"
            );
        }
        let mut changed = good.clone();
        let mut stripped = expected.clone();
        stripped.as_object_mut().unwrap().remove("execution");
        changed["metadata"]["stdin"]["value"] =
            json!(STANDARD.encode(serde_json::to_vec(&stripped).unwrap()));
        assert!(
            verify_original_request(&changed, f.image, f.request.request_id, &expected).is_err()
        );
    }
    #[test]
    fn team_remote_invalid_identity_rejects_before_pinning_or_submission() {
        let mut f = Fixture::new();
        f.request.execution = Some(ExecutionIdentity {
            uid: 0,
            ..registered_identity()
        });
        let server = Server::new(vec![]);
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
                .remote_input(f.request.request_id)
                .unwrap()
                .is_none()
        );
        assert!(
            f.owner
                .remote_attempt_stopped(f.lease.attempt_id())
                .unwrap()
        );
        assert!(server.finish().is_empty());
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
    #[test]
    fn team_remote_assignment_runs_through_owner_loop_and_durable_tools() {
        let f = Fixture::new_at(crate::model::unix_time() - 2, false);
        let workspace =
            crate::driver::team_codex::EmptyDirectory::create("remote-owner-fixture").unwrap();
        let (events, _received) = crate::driver::test_event_channel();
        let claim = || {
            let run = f.owner.snapshot(f.lease.run_id()).unwrap();
            crate::teams::ClaimedAssignment {
                lease: f.lease.clone(),
                assignment: run.work[0].clone(),
                profile: run.profiles[0].clone(),
            }
        };
        // A local native adapter still cannot consume a sandbox assignment.
        let local = crate::driver::nebius::team::run_with_harness(
            &f.owner,
            NativeTeamStart::Fresh(claim()),
            &workspace.0,
            &events,
            &AtomicBool::new(false),
            ProviderKind::Codex,
            |_| panic!("No local reservation is allowed"),
            |_, _, _, _| panic!("No local inference is allowed"),
        );
        assert!(
            local
                .unwrap_err()
                .to_string()
                .contains("Selected execution host")
        );
        let operation = Uuid::new_v4();
        let image = f.image;
        let request = Arc::new(std::sync::Mutex::new(None::<WorkerRequest>));
        let captured = request.clone();
        let server = Server::with_response_handler(
            vec![
                (
                    200,
                    body(
                        json!({"version":1,"capabilities":[{"provider":"codex","model":"pinned","input_cap":1000,"output_cap":100}]}),
                    ),
                ),
                (201, body(json!({"uuid":operation}))),
                (200, vec![]),
            ],
            move |r, body_bytes| {
                if r.line.starts_with("POST ") {
                    let bytes = STANDARD
                        .decode(r.body["stdin"]["value"].as_str().unwrap())
                        .unwrap();
                    *captured.lock().unwrap() = Some(serde_json::from_slice(&bytes).unwrap());
                }
                if r.line.starts_with(&format!("GET /operations/{operation}")) {
                    let saved = captured.lock().unwrap();
                    let request = saved.as_ref().unwrap();
                    let output=WorkerResponse {version:1,request_id:request.request_id,provider:request.capability.provider,model:request.capability.model.clone(),
                    content:json!({"text":"Remote completion","calls":[{"name":"save_document","arguments":{"name":"Remote-report.md","format":"md","content":"Generated by the registered remote harness"}},{"name":"report_blocker","arguments":{"reason":"Fixture awaits owner review"}}]}).to_string(),
                    usage:ModelTokenUsage{input_tokens:10,output_tokens:5},estimated_cost_usd_micros:None};
                    return body(
                        json!({"uuid":operation,"image_uuid":image,"kind":"instance","status":"SUCCESS","metadata":{"result":{"state":{"exit_code":0},"stdout":{"value":STANDARD.encode(serde_json::to_vec(&output).unwrap()),"encoding":"base64","truncated":false}}}}),
                    );
                }
                body_bytes
            },
        );
        let exit = run_with_client(
            &f.owner,
            NativeTeamStart::Fresh(claim()),
            &workspace.0,
            &events,
            &AtomicBool::new(false),
            &client(&server),
        )
        .unwrap();
        assert!(matches!(exit, NativeTeamExit::Settled));
        let run = f.owner.snapshot(f.lease.run_id()).unwrap();
        assert_eq!(run.work[0].status, AssignmentStatus::Blocked);
        assert_eq!(
            run.work[0].attempts.last().unwrap().environment,
            ExecutionEnvironment::NebiusSandbox
        );
        let db = rusqlite::Connection::open(&f.path).unwrap();
        let content: Vec<u8> = db
            .query_row(
                "SELECT content FROM ion_team_document_receipts WHERE work_id=?",
                [f.lease.work_id().to_string()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(content, b"Generated by the registered remote harness");
        assert_eq!(run.usage.model_calls, 1);
        assert_eq!(run.usage.consumed_tokens, 15);
        assert_eq!(run.usage.reserved_tokens, 0);
        assert!(
            f.owner
                .remote_attempt_stopped(f.lease.attempt_id())
                .unwrap()
        );
        let requests = server.finish();
        assert_eq!(requests.len(), 3);
        assert_eq!(
            requests
                .iter()
                .filter(|r| r.line.starts_with("POST "))
                .count(),
            1
        );
    }
    fn recovery_checkpoint(f: &Fixture) {
        f.owner
            .save_runtime_checkpoint(
                &f.lease,
                &json!({"version":1,"steps":1,
            "messages":[{"role":"user","content":"Review"}],"pending":[]})
                .to_string(),
                102,
            )
            .unwrap();
    }
    fn lost_ack(f: &Fixture) -> Value {
        recovery_checkpoint(f);
        let server = Server::new(vec![(201, body(json!({})))]);
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
        let request = server.finish()[0].body.clone();
        f.owner.reconcile_owner_restart(103).unwrap();
        request
    }
    #[test]
    fn team_remote_recovery_binds_exact_original_request_and_continues_same_host_once() {
        let f = Fixture::new();
        let metadata = lost_ack(&f);
        let run = f.lease.run_id();
        let work = f.lease.work_id();
        let attempt = f.lease.attempt_id();
        let (review, unknown) = f.owner.recovery_review(run, work).unwrap();
        assert!(
            !review.host_stopped && review.remote_calls[0].can_bind && !unknown.work[0].can_resume
        );
        let public = serde_json::to_string(&review).unwrap();
        assert!(!public.contains("Review granted inputs") && !public.contains("stdin"));
        for confirmation in [
            TeamRecoveryConfirmation::HostStopped,
            TeamRecoveryConfirmation::VerifyRemoteStopped,
            TeamRecoveryConfirmation::PrepareContinuation,
        ] {
            assert!(
                f.owner
                    .confirm_recovery(run, work, attempt, unknown.revision, confirmation, 104)
                    .is_err()
            );
        }
        // An invalid operation image cannot bind, settle usage or release capacity.
        let id = Uuid::new_v4();
        let mut wrong = operation(&f, id, "EXECUTING");
        wrong["image_uuid"] = json!(Uuid::new_v4());
        let server = Server::new(vec![(200, body(wrong))]);
        assert!(
            bind_recovered_with(
                &client(&server),
                &f.owner,
                run,
                work,
                attempt,
                unknown.revision,
                f.request.request_id,
                id
            )
            .is_err()
        );
        assert_eq!(server.finish().len(), 1);
        assert!(
            f.owner.remote_calls(attempt).unwrap()[0]
                .operation
                .is_none()
        );
        let mut verified = operation(&f, id, "EXECUTING");
        verified["metadata"] = metadata;
        let server = Server::new(vec![(200, body(verified))]);
        let remote = client(&server);
        let reopened = TeamCoordinator::new(f.path.clone());
        bind_recovered_with(
            &remote,
            &reopened,
            run,
            work,
            attempt,
            unknown.revision,
            f.request.request_id,
            id,
        )
        .unwrap();
        let bound = reopened.snapshot(run).unwrap();
        bind_recovered_with(
            &remote,
            &reopened,
            run,
            work,
            attempt,
            unknown.revision,
            f.request.request_id,
            id,
        )
        .unwrap();
        assert_eq!(reopened.snapshot(run).unwrap().revision, bound.revision);
        assert!(
            bind_recovered_with(
                &remote,
                &reopened,
                run,
                work,
                attempt,
                unknown.revision,
                f.request.request_id,
                Uuid::new_v4()
            )
            .is_err()
        );
        assert_eq!(server.finish().len(), 1); // response replay sent no second GET or POST
        let server = Server::new(vec![
            (200, body(operation(&f, id, "EXECUTING"))),
            (202, body(json!({}))),
            (200, body(operation(&f, id, "CANCELLED"))),
        ]);
        let remote = client(&server);
        assert!(!reconcile_with(&remote, &reopened, attempt).unwrap());
        assert!(!reopened.remote_attempt_stopped(attempt).unwrap());
        assert!(!reopened.snapshot(run).unwrap().work[0].can_resume);
        assert!(reconcile_with(&remote, &reopened, attempt).unwrap());
        let revision = reopened.snapshot(run).unwrap().revision;
        let (review, blocked) = reopened
            .confirm_recovery(
                run,
                work,
                attempt,
                revision,
                TeamRecoveryConfirmation::VerifyRemoteStopped,
                104,
            )
            .unwrap();
        assert!(review.host_stopped && review.can_prepare_continuation);
        assert_eq!(blocked.work[0].status, AssignmentStatus::Blocked);
        assert_eq!(blocked.usage.reserved_tokens, 1100);
        assert_eq!(blocked.usage.consumed_tokens, 0);
        let (_, prepared) = reopened
            .confirm_recovery(
                run,
                work,
                attempt,
                blocked.revision,
                TeamRecoveryConfirmation::PrepareContinuation,
                104,
            )
            .unwrap();
        assert!(prepared.work[0].can_resume);
        let prepared_revision = prepared.revision;
        let queued = reopened
            .queue_checkpoint_resume(run, work, prepared_revision, 104)
            .unwrap();
        let again = reopened
            .queue_checkpoint_resume(run, work, prepared_revision, 104)
            .unwrap();
        assert_eq!(queued.revision, again.revision);
        let resumed = reopened.resume_after_children(run, work, 105, 100).unwrap();
        assert_eq!(resumed.claimed.lease.attempt_id(), attempt);
        assert_eq!(resumed.claimed.assignment.attempts.len(), 1);
        assert_eq!(
            resumed.claimed.assignment.attempts[0].environment,
            ExecutionEnvironment::NebiusSandbox
        );
        assert_eq!(resumed.claimed.profile.provider, ProviderKind::Codex);
        assert_eq!(resumed.claimed.profile.model.as_deref(), Some("pinned"));
        assert!(
            resumed.checkpoint.contains("remote_process_receipts")
                && resumed
                    .checkpoint
                    .contains("Cached model actions were not replayed")
        );
        assert!(reopened.renew(&f.lease, 105, 100).is_err());
        assert_eq!(reopened.snapshot(run).unwrap().usage.reserved_tokens, 1100);
        assert!(server.finish().iter().all(|r| !r.line.starts_with("POST ")));
    }
    #[test]
    fn team_remote_recovery_rejects_metadata_prompt_model_and_permission_changes() {
        let f = Fixture::new();
        let metadata = lost_ack(&f);
        let expected = serde_json::to_value(&f.request).unwrap();
        let good = json!({"metadata":metadata});
        verify_original_request(&good, f.image, f.request.request_id, &expected).unwrap();
        for (key, value) in [
            ("hostname", json!("another-request")),
            ("image", json!(Uuid::new_v4())),
            ("command", json!("/bin/sh")),
            ("shell", json!(true)),
            ("args", json!(["expanded"])),
            ("env", json!({"PATH":"/usr/bin","SECRET":"fixture"})),
            ("uid", json!(1000)),
            ("files", json!({"/escape":{"uuid":Uuid::new_v4()}})),
            ("timeout", json!(999)),
            ("preserve_env", json!(true)),
            ("networking", json!({"enabled":false})),
        ] {
            let mut wrong = good.clone();
            wrong["metadata"][key] = value;
            assert!(
                verify_original_request(&wrong, f.image, f.request.request_id, &expected).is_err(),
                "{key}"
            );
        }
        for (key, value) in [
            ("system", json!("Changed instructions")),
            ("tools", json!(["shell"])),
            ("messages", json!([{"role":"user","content":"other input"}])),
            ("request_id", json!(Uuid::new_v4())),
        ] {
            let mut changed = expected.clone();
            changed[key] = value;
            let mut wrong = good.clone();
            wrong["metadata"]["stdin"]["value"] =
                json!(STANDARD.encode(serde_json::to_vec(&changed).unwrap()));
            assert!(
                verify_original_request(&wrong, f.image, f.request.request_id, &expected).is_err(),
                "{key}"
            );
        }
        let mut changed = expected.clone();
        changed["capability"]["model"] = json!("other-model");
        let mut wrong = good.clone();
        wrong["metadata"]["stdin"]["value"] =
            json!(STANDARD.encode(serde_json::to_vec(&changed).unwrap()));
        assert!(verify_original_request(&wrong, f.image, f.request.request_id, &expected).is_err());
        let mut wrong = good;
        wrong["metadata"]["stdin"]["truncated"] = json!(true);
        assert!(verify_original_request(&wrong, f.image, f.request.request_id, &expected).is_err());
    }
    #[test]
    fn team_remote_recovery_legacy_input_and_repeated_post_are_refused() {
        let f = Fixture::new();
        recovery_checkpoint(&f);
        f.owner.reconcile_owner_restart(103).unwrap();
        let run = f.owner.snapshot(f.lease.run_id()).unwrap();
        let server = Server::new(vec![]);
        assert!(
            bind_recovered_with(
                &client(&server),
                &f.owner,
                run.id,
                f.lease.work_id(),
                f.lease.attempt_id(),
                run.revision,
                f.request.request_id,
                Uuid::new_v4()
            )
            .is_err()
        );
        assert!(
            !f.owner
                .recovery_review(run.id, f.lease.work_id())
                .unwrap()
                .0
                .remote_calls[0]
                .can_bind
        );
        assert_eq!(server.finish().len(), 0);
        let f = Fixture::new();
        let server = Server::new(vec![(201, body(json!({})))]);
        let remote = client(&server);
        assert!(
            execute(
                &remote,
                &f.owner,
                f.image,
                &f.request,
                &AtomicBool::new(false)
            )
            .is_err()
        );
        assert!(
            execute(
                &remote,
                &f.owner,
                f.image,
                &f.request,
                &AtomicBool::new(false)
            )
            .is_err()
        );
        assert_eq!(server.finish().len(), 1);
    }
    #[test]
    fn team_remote_recovery_canceled_run_and_changed_revision_cannot_continue() {
        let f = Fixture::new();
        let metadata = lost_ack(&f);
        let unknown = f.owner.snapshot(f.lease.run_id()).unwrap();
        let stopped = f.owner.stop(unknown.id, unknown.revision, 104).unwrap();
        let id = Uuid::new_v4();
        let mut value = operation(&f, id, "CANCELLED");
        value["metadata"] = metadata;
        let server = Server::new(vec![(200, body(value.clone()))]);
        let remote = client(&server);
        assert!(
            bind_recovered_with(
                &remote,
                &f.owner,
                unknown.id,
                f.lease.work_id(),
                f.lease.attempt_id(),
                unknown.revision,
                f.request.request_id,
                id
            )
            .is_err()
        );
        bind_recovered_with(
            &remote,
            &f.owner,
            unknown.id,
            f.lease.work_id(),
            f.lease.attempt_id(),
            stopped.revision,
            f.request.request_id,
            id,
        )
        .unwrap();
        assert_eq!(server.finish().len(), 1);
        let server = Server::new(vec![(200, body(value))]);
        assert!(reconcile_with(&client(&server), &f.owner, f.lease.attempt_id()).unwrap());
        let revision = f.owner.snapshot(unknown.id).unwrap().revision;
        let (review, canceled) = f
            .owner
            .confirm_recovery(
                unknown.id,
                f.lease.work_id(),
                f.lease.attempt_id(),
                revision,
                TeamRecoveryConfirmation::VerifyRemoteStopped,
                105,
            )
            .unwrap();
        assert!(
            review.host_stopped && !review.can_prepare_continuation && !canceled.work[0].can_resume
        );
        assert_eq!(canceled.work[0].status, AssignmentStatus::Canceled);
        assert_eq!(canceled.usage.reserved_tokens, 1100);
        assert!(
            f.owner
                .confirm_recovery(
                    unknown.id,
                    f.lease.work_id(),
                    f.lease.attempt_id(),
                    canceled.revision,
                    TeamRecoveryConfirmation::PrepareContinuation,
                    105
                )
                .is_err()
        );
        assert!(
            f.owner
                .queue_checkpoint_resume(unknown.id, f.lease.work_id(), canceled.revision, 105)
                .is_err()
        );
        assert_eq!(server.finish().len(), 1);
    }
    #[test]
    fn team_remote_recovery_resumed_owner_loop_uses_new_call_and_retains_original_unknown_budget() {
        let now = crate::model::unix_time();
        let f = Fixture::new_at(now, true);
        f.owner
            .save_runtime_checkpoint(
                &f.lease,
                &json!({"version":1,"steps":1,
            "messages":[{"role":"user","content":"Review"}],"pending":[]})
                .to_string(),
                now,
            )
            .unwrap();
        let server = Server::new(vec![(201, body(json!({})))]);
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
        let metadata = server.finish()[0].body.clone();
        f.owner.reconcile_owner_restart(now).unwrap();
        let run = f.lease.run_id();
        let work = f.lease.work_id();
        let attempt = f.lease.attempt_id();
        let old_operation = Uuid::new_v4();
        let mut value = operation(&f, old_operation, "CANCELLED");
        value["metadata"] = metadata;
        let server = Server::new(vec![(200, body(value.clone())), (200, body(value))]);
        let remote = client(&server);
        let revision = f.owner.snapshot(run).unwrap().revision;
        bind_recovered_with(
            &remote,
            &f.owner,
            run,
            work,
            attempt,
            revision,
            f.request.request_id,
            old_operation,
        )
        .unwrap();
        assert!(reconcile_with(&remote, &f.owner, attempt).unwrap());
        let revision = f.owner.snapshot(run).unwrap().revision;
        let (_, blocked) = f
            .owner
            .confirm_recovery(
                run,
                work,
                attempt,
                revision,
                TeamRecoveryConfirmation::VerifyRemoteStopped,
                now,
            )
            .unwrap();
        let (_, prepared) = f
            .owner
            .confirm_recovery(
                run,
                work,
                attempt,
                blocked.revision,
                TeamRecoveryConfirmation::PrepareContinuation,
                now,
            )
            .unwrap();
        f.owner
            .queue_checkpoint_resume(run, work, prepared.revision, now)
            .unwrap();
        let continuation = f.owner.resume_after_children(run, work, now, 180).unwrap();
        assert_eq!(server.finish().len(), 2);
        let operation = Uuid::new_v4();
        let image = f.image;
        let request = Arc::new(std::sync::Mutex::new(None::<WorkerRequest>));
        let captured = request.clone();
        let server = Server::with_response_handler(
            vec![
                (
                    200,
                    body(
                        json!({"version":1,"capabilities":[{"provider":"codex","model":"pinned","input_cap":1000,"output_cap":100}]}),
                    ),
                ),
                (201, body(json!({"uuid":operation}))),
                (200, vec![]),
            ],
            move |r, bytes| {
                if r.line.starts_with("POST ") {
                    *captured.lock().unwrap() = Some(
                        serde_json::from_slice(
                            &STANDARD
                                .decode(r.body["stdin"]["value"].as_str().unwrap())
                                .unwrap(),
                        )
                        .unwrap(),
                    );
                }
                if r.line.starts_with(&format!("GET /operations/{operation}")) {
                    let saved = captured.lock().unwrap();
                    let request = saved.as_ref().unwrap();
                    assert!(request.messages.iter().any(|m| {
                        m["content"]
                            .as_str()
                            .is_some_and(|s| s.contains("remote_process_receipts"))
                    }));
                    let output=WorkerResponse{version:1,request_id:request.request_id,provider:request.capability.provider,model:request.capability.model.clone(),
                        content:json!({"text":"Continued","calls":[{"name":"save_document","arguments":{"name":"Recovered.md","format":"md","content":"New completion after verified recovery"}},{"name":"report_blocker","arguments":{"reason":"Review recovered output"}}]}).to_string(),
                        usage:ModelTokenUsage{input_tokens:10,output_tokens:5},estimated_cost_usd_micros:None};
                    return body(
                        json!({"uuid":operation,"image_uuid":image,"kind":"instance","status":"SUCCESS","metadata":{"result":{"state":{"exit_code":0},"stdout":{"value":STANDARD.encode(serde_json::to_vec(&output).unwrap()),"encoding":"base64","truncated":false}}}}),
                    );
                }
                bytes
            },
        );
        let workspace =
            crate::driver::team_codex::EmptyDirectory::create("remote-resume-fixture").unwrap();
        let (events, _) = crate::driver::test_event_channel();
        let exit = run_with_client(
            &f.owner,
            NativeTeamStart::Resumed(continuation),
            &workspace.0,
            &events,
            &AtomicBool::new(false),
            &client(&server),
        )
        .unwrap();
        assert_eq!(exit, NativeTeamExit::Settled);
        let resumed = f.owner.snapshot(run).unwrap();
        assert_eq!(resumed.work[0].attempts.len(), 1);
        assert_eq!(resumed.work[0].attempts[0].id, attempt);
        assert_eq!(
            resumed.work[0].attempts[0].environment,
            ExecutionEnvironment::NebiusSandbox
        );
        assert_eq!(resumed.usage.model_calls, 2);
        assert_eq!(resumed.usage.consumed_tokens, 15);
        assert_eq!(resumed.usage.reserved_tokens, 1100);
        assert_ne!(
            request.lock().unwrap().as_ref().unwrap().request_id,
            f.request.request_id
        );
        assert_eq!(f.owner.remote_calls(attempt).unwrap().len(), 2);
        let db = rusqlite::Connection::open(&f.path).unwrap();
        let content: Vec<u8> = db
            .query_row(
                "SELECT content FROM ion_team_document_receipts WHERE work_id=?",
                [work.to_string()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(content, b"New completion after verified recovery");
        assert_eq!(
            server
                .finish()
                .iter()
                .filter(|r| r.line.starts_with("POST "))
                .count(),
            1
        );
    }
    #[test]
    fn global_usage_remote_sdk_estimate_and_tokens_commit_with_terminal_receipt_after_stop() {
        let f = Fixture::with_provider(100, true, ProviderKind::Claude);
        let operation = Uuid::new_v4();
        f.owner
            .bind_remote_operation(f.request.request_id, operation)
            .unwrap();
        let run = f.owner.snapshot(f.lease.run_id()).unwrap();
        f.owner.stop(run.id, run.revision, 103).unwrap();
        let mut output = response(&f);
        output.provider = ProviderKind::Claude;
        output.estimated_cost_usd_micros = Some(12345);
        let receipt = serde_json::to_string(&output).unwrap();
        // Simulate exit immediately after the trusted terminal observation,
        // before the transport's ordinary token-accounting callback can run.
        f.owner
            .finish_remote_call(
                f.request.request_id,
                Some(operation),
                "SUCCESS",
                Some(&receipt),
            )
            .unwrap();
        let reopened = TeamCoordinator::new(f.path.clone());
        let run = reopened.snapshot(f.lease.run_id()).unwrap();
        assert_eq!(run.usage.consumed_tokens, 15);
        assert_eq!(run.usage.reserved_tokens, 0);
        assert_eq!(run.work[0].status, AssignmentStatus::Canceling);
        assert!(run.work[0].result.is_none());
        let control = ControlStore::new(f.path.clone());
        let page = control
            .usage_ledger(crate::usage_ledger::all_query())
            .unwrap();
        assert_eq!(page.summary.records, 1);
        assert_eq!(page.summary.unknown_cost_records, 0);
        let cost = page.entries[0].cost.as_ref().unwrap();
        assert_eq!(cost.amount.micros, 12345);
        assert_eq!(cost.amount.currency, "USD");
        assert_eq!(
            cost.quality,
            waku_protocol::usage_ledger::UsageCostQuality::Estimated
        );
        assert_eq!(
            cost.finality,
            waku_protocol::usage_ledger::UsageCostFinality::Provisional
        );
        let revision = page.revision;
        reopened
            .record_remote_model_usage(
                f.request.request_id,
                ProviderKind::Claude,
                "pinned",
                output.usage,
                104,
            )
            .unwrap();
        assert_eq!(
            control
                .usage_ledger(crate::usage_ledger::all_query())
                .unwrap()
                .revision,
            revision
        );
        let server = Server::new(vec![]);
        assert!(reconcile_with(&client(&server), &reopened, f.lease.attempt_id()).unwrap());
        assert!(server.finish().is_empty());
    }
    #[test]
    fn global_usage_remote_invalid_cost_or_identity_does_not_partly_release_tokens() {
        for changed in 0..6 {
            let f = Fixture::with_provider(100, true, ProviderKind::Claude);
            let operation = Uuid::new_v4();
            f.owner
                .bind_remote_operation(f.request.request_id, operation)
                .unwrap();
            let mut output = response(&f);
            output.provider = ProviderKind::Claude;
            output.estimated_cost_usd_micros = Some(50);
            match changed {
                0 => output.estimated_cost_usd_micros = Some(1_000_000_000_001),
                1 => output.provider = ProviderKind::Codex,
                2 => output.model = "other-model".into(),
                3 => output.request_id = Uuid::new_v4(),
                4 => output.version = 2,
                _ => output.usage.input_tokens = u64::MAX,
            }
            let control = ControlStore::new(f.path.clone());
            let before = control
                .usage_ledger(crate::usage_ledger::all_query())
                .unwrap()
                .revision;
            f.owner
                .finish_remote_call(
                    f.request.request_id,
                    Some(operation),
                    "SUCCESS",
                    Some(&serde_json::to_string(&output).unwrap()),
                )
                .unwrap();
            let run = f.owner.snapshot(f.lease.run_id()).unwrap();
            assert_eq!(run.usage.reserved_tokens, 1100);
            assert_eq!(run.usage.consumed_tokens, 0);
            assert!(
                f.owner
                    .remote_attempt_stopped(f.lease.attempt_id())
                    .unwrap()
            );
            let page = control
                .usage_ledger(crate::usage_ledger::all_query())
                .unwrap();
            assert_eq!(page.revision, before);
            assert_eq!(page.summary.unknown_cost_records, 1);
            assert!(page.entries[0].cost.is_none());
        }
    }
    #[test]
    fn global_usage_remote_estimate_wire_accepts_legacy_and_zero_but_no_invoice_claim() {
        let f = Fixture::new();
        let operation_id = Uuid::new_v4();
        let encode = |output: &WorkerResponse| {
            let mut value = operation(&f, operation_id, "SUCCESS");
            value["metadata"] = json!({"result":{"state":{"exit_code":0,"signal":null},"stdout":{"value":STANDARD.encode(serde_json::to_vec(output).unwrap()),"encoding":"base64","truncated":false}}});
            value
        };
        let mut output = response(&f);
        assert!(
            !serde_json::to_string(&output)
                .unwrap()
                .contains("estimated_cost_usd_micros")
        );
        assert!(
            worker_output(&encode(&output))
                .unwrap()
                .estimated_cost_usd_micros
                .is_none()
        );
        output.provider = ProviderKind::Claude;
        output.estimated_cost_usd_micros = Some(0);
        assert_eq!(
            worker_output(&encode(&output))
                .unwrap()
                .estimated_cost_usd_micros,
            Some(0)
        );
        output.provider = ProviderKind::Codex;
        assert!(worker_output(&encode(&output)).is_err());
        output.provider = ProviderKind::Claude;
        output.estimated_cost_usd_micros = Some(1_000_000_000_001);
        assert!(worker_output(&encode(&output)).is_err());
    }
}
