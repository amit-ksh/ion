//! Account-scoped services. All callers must use background workers.
use super::*;
/// Capture before dispatching a background operation; account replacement must
/// never lend another user's bearer token to an old app operation.
pub fn app_request(expected_workspace: &str, path: &str, body: Value) -> Result<Value> {
    account()?.app_request(expected_workspace, path, body)
}
/// Restricted sandbox transport; no provider project credential leaves the server.
pub fn sandbox_request(
    client: &reqwest::blocking::Client,
    method: reqwest::Method,
    path: &str,
    expected_workspace: &str,
) -> Result<reqwest::blocking::RequestBuilder> {
    ensure!(
        workspace(expected_workspace) && valid_sandbox_path(path),
        "Invalid managed sandbox route"
    );
    ensure!(
        [
            reqwest::Method::GET,
            reqwest::Method::POST,
            reqwest::Method::DELETE
        ]
        .contains(&method),
        "Invalid sandbox method"
    );
    let account = account()?;
    let _guard = account.lock()?;
    let mut state = account.read()?;
    ensure!(
        state
            .session
            .as_ref()
            .is_some_and(|s| s.workspace_id == expected_workspace),
        "The sandbox account changed; resume in the original Ion account"
    );
    account.authorize(&mut state)?;
    let session = state.session.as_ref().context("Sign in to Ion Cloud")?;
    ensure!(
        session.workspace_id == expected_workspace,
        "The sandbox account changed"
    );
    Ok(client
        .request(
            method,
            format!("{}/v1/sandbox{path}", account.config.origin),
        )
        .bearer_auth(&session.access_token))
}
fn valid_sandbox_path(path: &str) -> bool {
    if ["/config", "/instances", "/files", "/recover"].contains(&path) {
        return true;
    }
    if path.contains(['?', '#', '\\']) {
        return false;
    }
    let parts: Vec<_> = path.split('/').collect();
    match parts.as_slice() {
        ["", "operations" | "images", id] => device_id(id),
        ["", "inspect", id, "download"] => device_id(id),
        ["", "files", digest] => workspace(digest),
        _ => false,
    }
}
#[derive(Deserialize)]
pub struct SandboxTarget {
    pub target: String,
    pub image: String,
    pub label: String,
    pub kind: String,
    pub providers: Vec<String>,
}
#[derive(Deserialize)]
pub struct SandboxConfiguration {
    pub available: bool,
    pub reason: Option<String>,
    #[serde(default)]
    pub images: Vec<SandboxTarget>,
}
pub fn sandbox_configuration(expected_workspace: &str) -> Result<SandboxConfiguration> {
    let response = sandbox_request(
        &Network::client()?,
        reqwest::Method::GET,
        "/config",
        expected_workspace,
    )?
    .send()?;
    ensure!(
        response.status().is_success(),
        "Managed sandbox configuration is unavailable"
    );
    use std::io::Read;
    let mut bytes = Vec::new();
    response.take(64_001).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 64_000, "Sandbox registry exceeds its bound");
    let config: SandboxConfiguration = serde_json::from_slice(&bytes)?;
    ensure!(
        config.images.len() <= 16
            && config.reason.as_ref().is_none_or(|s| s.len() <= 256)
            && config.images.iter().all(|i| device_id(&i.target)
                && device_id(&i.image)
                && i.label.len() <= 160
                && ["harness", "command"].contains(&i.kind.as_str())
                && i.providers.len() <= 128
                && i.providers.iter().all(|p| opaque(p, 1, 64))),
        "Invalid sandbox registry"
    );
    Ok(config)
}
pub fn bootstrap() -> Result<Value> {
    let account = account()?;
    let value = account.http.json(
        "GET",
        &format!("{}/v1/bootstrap", account.config.origin),
        None,
        None,
    )?;
    ensure!(
        value["auth"]["issuer"] == ISSUER
            && value["auth"]["audience"] == AUDIENCE
            && value["auth"]["desktopClientId"] == DESKTOP_CLIENT_ID,
        "Cloud configuration changed"
    );
    Ok(value)
}
/// Capture the workspace when the service view/action is created. A queued
/// switch or usage read must not adopt another process's replacement login.
pub fn request(
    expected_workspace: &str,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> Result<Value> {
    account()?.service_request(expected_workspace, method, path, body)
}
/// Publishing is bound to the account which opened the review, including
/// delayed uploads and refreshes after a mutation.
pub fn hosting_request(
    expected_workspace: &str,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> Result<Value> {
    let account = account()?;
    let mut pending =
        account.hosting_request(&Network::client()?, method, path, expected_workspace)?;
    if let Some(body) = body {
        pending = pending.json(&body);
    }
    let response = pending
        .send()
        .map_err(|_| anyhow::anyhow!("Could not reach Ion Cloud"))?;
    decode_service_response(response)
}
fn decode_service_response(response: reqwest::blocking::Response) -> Result<Value> {
    ensure!(
        response.status().is_success(),
        "Cloud service request failed (HTTP {})",
        response.status()
    );
    use std::io::Read;
    let mut bytes = Vec::new();
    response.take(2_000_001).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 2_000_000,
        "Cloud service response exceeds its limit"
    );
    serde_json::from_slice(&bytes).map_err(|_| anyhow::anyhow!("Invalid cloud service response"))
}
pub(crate) fn speech() -> Result<Option<crate::speech::SpeechService>> {
    let status = super::status()?;
    if !status.configured || !status.signed_in {
        return Ok(None);
    }
    let value = bootstrap()?;
    if value["features"]["speech"] != true {
        return Ok(None);
    }
    let account = account()?;
    Ok(Some(crate::speech::SpeechService::managed(
        account.config.origin,
    )))
}
pub(crate) fn speech_ticket(seconds: u64) -> Result<crate::speech::SpeechService> {
    let workspace = super::status()?
        .workspace_id
        .context("Sign in to Ion Cloud")?;
    let value = request(
        &workspace,
        "POST",
        "/v1/speech/sessions",
        Some(json!({"maximumSeconds":seconds})),
    )?;
    let origin = account()?.config.origin;
    ensure!(value["baseUrl"] == origin, "Speech origin changed");
    crate::speech::SpeechService::new(
        origin,
        value["token"]
            .as_str()
            .context("Speech ticket missing")?
            .to_owned(),
    )
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceControls {
    pub cloud: bool,
    pub token_factory: bool,
    #[serde(default)]
    pub app_actions: bool,
    #[serde(default)]
    pub app_actions_available: bool,
    pub can_manage: bool,
    #[serde(default)]
    pub cloud_available: bool,
    #[serde(default)]
    pub token_factory_available: bool,
}
pub fn controls(expected_workspace: &str) -> Result<ServiceControls> {
    Ok(serde_json::from_value(request(
        expected_workspace,
        "GET",
        "/v1/services/controls",
        None,
    )?)?)
}
pub fn set_control(
    expected_workspace: &str,
    service: &str,
    enabled: bool,
) -> Result<ServiceControls> {
    ensure!(
        ["cloud", "tokenFactory", "appActions"].contains(&service),
        "Invalid shared service"
    );
    let mut body = serde_json::Map::new();
    body.insert(service.to_owned(), Value::Bool(enabled));
    Ok(serde_json::from_value(request(
        expected_workspace,
        "PUT",
        "/v1/services/controls",
        Some(Value::Object(body)),
    )?)?)
}

/// A managed transport cannot borrow a replacement account's token after logout.
pub(crate) fn inference_request(
    client: &reqwest::blocking::Client,
    method: reqwest::Method,
    path: &str,
    expected_workspace: &str,
) -> Result<reqwest::blocking::RequestBuilder> {
    ensure!(
        ["models", "chat/completions"].contains(&path),
        "Unsupported shared inference operation"
    );
    account()?.inference_request(client, method, path, expected_workspace)
}
impl<V: Vault, H: Http> Account<V, H> {
    pub(super) fn service_request(
        &self,
        expected_workspace: &str,
        method: &str,
        path: &str,
        body: Option<Value>,
    ) -> Result<Value> {
        ensure!(
            workspace(expected_workspace)
                && matches!(
                    (method, path),
                    ("GET" | "PUT", "/v1/services/controls")
                        | ("GET", "/v1/services/usage")
                        | ("POST", "/v1/speech/sessions")
                ),
            "Invalid account-bound service operation"
        );
        let _guard = self.lock()?;
        let mut state = self.read()?;
        ensure!(
            state
                .session
                .as_ref()
                .is_some_and(|s| s.workspace_id == expected_workspace),
            "The service account changed; reopen Settings in the intended account"
        );
        self.authorize(&mut state)?;
        let session = state.session.as_ref().context("Sign in to Ion Cloud")?;
        ensure!(
            session.workspace_id == expected_workspace,
            "The service account changed; reopen Settings in the intended account"
        );
        self.http.json(
            method,
            &format!("{}{path}", self.config.origin),
            Some(&session.access_token),
            body,
        )
    }
    pub(super) fn hosting_request(
        &self,
        client: &reqwest::blocking::Client,
        method: &str,
        path: &str,
        expected_workspace: &str,
    ) -> Result<reqwest::blocking::RequestBuilder> {
        ensure!(
            workspace(expected_workspace)
                && ["GET", "POST", "DELETE"].contains(&method)
                && path.starts_with("/v1/hosting/")
                && path.len() <= 240
                && path.split('/').skip(1).all(|part| !part.is_empty()
                    && part
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')),
            "Invalid account-bound publishing operation"
        );
        let _guard = self.lock()?;
        let mut state = self.read()?;
        ensure!(
            state
                .session
                .as_ref()
                .is_some_and(|s| s.workspace_id == expected_workspace),
            "The publishing account changed; reopen publishing in the intended account"
        );
        self.authorize(&mut state)?;
        let session = state.session.as_ref().context("Sign in to Ion Cloud")?;
        ensure!(
            session.workspace_id == expected_workspace,
            "The publishing account changed; reopen publishing in the intended account"
        );
        Ok(client
            .request(method.parse()?, format!("{}{path}", self.config.origin))
            .bearer_auth(&session.access_token))
    }
    pub(super) fn app_request(
        &self,
        expected_workspace: &str,
        path: &str,
        body: Value,
    ) -> Result<Value> {
        ensure!(
            workspace(expected_workspace)
                && [
                    "/v1/apps/catalog",
                    "/v1/apps/connect",
                    "/v1/apps/disconnect",
                    "/v1/apps/actions/tools",
                    "/v1/apps/actions/describe",
                    "/v1/apps/actions/prepare",
                    "/v1/apps/actions/approve",
                    "/v1/apps/actions/execute",
                    "/v1/apps/actions/status"
                ]
                .contains(&path),
            "Invalid managed app operation"
        );
        let _guard = self.lock()?;
        let mut state = self.read()?;
        // Verify identity before refresh as well as after it.
        ensure!(
            state
                .session
                .as_ref()
                .is_some_and(|s| s.workspace_id == expected_workspace),
            "The app account changed; refresh Apps in the intended account"
        );
        self.authorize(&mut state)?;
        let session = state.session.as_ref().context("Sign in to Ion Cloud")?;
        ensure!(
            session.workspace_id == expected_workspace,
            "The app account changed; refresh Apps in the intended account"
        );
        self.http.json(
            "POST",
            &format!("{}{path}", self.config.origin),
            Some(&session.access_token),
            Some(body),
        )
    }
    pub(super) fn inference_request(
        &self,
        client: &reqwest::blocking::Client,
        method: reqwest::Method,
        path: &str,
        expected_workspace: &str,
    ) -> Result<reqwest::blocking::RequestBuilder> {
        ensure!(
            ["models", "chat/completions"].contains(&path),
            "Unsupported shared inference operation"
        );
        let _guard = self.lock()?;
        let mut state = self.read()?;
        ensure!(
            state
                .session
                .as_ref()
                .is_some_and(|s| s.workspace_id == expected_workspace),
            "The inference account changed; restart this task in the intended account"
        );
        self.authorize(&mut state)?;
        let session = state.session.as_ref().context("Sign in to Ion Cloud")?;
        ensure!(
            session.workspace_id == expected_workspace,
            "The inference account changed; restart this task in the intended account"
        );
        Ok(client
            .request(
                method,
                format!("{}/v1/inference/{path}", self.config.origin),
            )
            .bearer_auth(&session.access_token))
    }
}

#[cfg(test)]
mod sandbox_path_tests {
    use super::*;
    #[test]
    fn managed_sandbox_route_cannot_escape_or_select_credential_endpoints() {
        for path in [
            "/config",
            "/instances",
            "/files",
            "/operations/12345678-1234-1234-1234-123456789abc",
            "/inspect/12345678-1234-1234-1234-123456789abc/download",
        ] {
            assert!(valid_sandbox_path(path));
        }
        for path in [
            "/whoami",
            "/images/tag:latest",
            "/operations/../whoami",
            "/instances?token=x",
            "//instances",
            "/inspect/12345678-1234-1234-1234-123456789abc/download/../../whoami",
        ] {
            assert!(!valid_sandbox_path(path));
        }
    }
}
