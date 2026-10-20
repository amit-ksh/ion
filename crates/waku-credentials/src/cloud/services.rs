//! Account-scoped services. All callers must use background workers.
use super::*;
/// Capture before dispatching a background operation; account replacement must
/// never lend another user's bearer token to an old app operation.
pub fn app_request(expected_workspace: &str, path: &str, body: Value) -> Result<Value> {
    account()?.app_request(expected_workspace, path, body)
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
pub fn request(method: &str, path: &str, body: Option<Value>) -> Result<Value> {
    ensure!(
        path.starts_with("/v1/hosting/")
            || path == "/v1/speech/sessions"
            || path == "/v1/services/controls"
            || path == "/v1/services/usage",
        "Unsupported managed service route"
    );
    ensure!(
        !path.contains(['?', '#', '\\']) && !path.split('/').any(|v| v == "." || v == ".."),
        "Invalid managed service path"
    );
    let account = account()?;
    let _guard = account.lock()?;
    let mut state = account.read()?;
    account.authorize(&mut state)?;
    let token = &state
        .session
        .as_ref()
        .context("Sign in to Ion Cloud")?
        .access_token;
    let mut pending = Network::client()?
        .request(
            method.parse()?,
            format!("{}{}", account.config.origin, path),
        )
        .bearer_auth(token);
    if let Some(body) = body {
        pending = pending.json(&body);
    }
    let response = pending
        .send()
        .map_err(|_| anyhow::anyhow!("Could not reach Ion Cloud"))?;
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
    let value = request(
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
    pub can_manage: bool,
    #[serde(default)]
    pub cloud_available: bool,
    #[serde(default)]
    pub token_factory_available: bool,
}
pub fn controls() -> Result<ServiceControls> {
    Ok(serde_json::from_value(request(
        "GET",
        "/v1/services/controls",
        None,
    )?)?)
}
pub fn set_control(service: &str, enabled: bool) -> Result<ServiceControls> {
    ensure!(
        ["cloud", "tokenFactory"].contains(&service),
        "Invalid shared service"
    );
    let mut body = serde_json::Map::new();
    body.insert(service.to_owned(), Value::Bool(enabled));
    Ok(serde_json::from_value(request(
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
    pub(super) fn app_request(&self, expected_workspace: &str, path: &str, body: Value) -> Result<Value> {
        ensure!(workspace(expected_workspace) && ["/v1/apps/catalog", "/v1/apps/connect", "/v1/apps/disconnect"].contains(&path), "Invalid managed app operation");
        let _guard = self.lock()?;
        let mut state = self.read()?;
        // Verify identity before refresh as well as after it.
        ensure!(state.session.as_ref().is_some_and(|s| s.workspace_id == expected_workspace), "The app account changed; refresh Apps in the intended account");
        self.authorize(&mut state)?;
        let session = state.session.as_ref().context("Sign in to Ion Cloud")?;
        ensure!(session.workspace_id == expected_workspace, "The app account changed; refresh Apps in the intended account");
        self.http.json("POST", &format!("{}{path}", self.config.origin), Some(&session.access_token), Some(body))
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
