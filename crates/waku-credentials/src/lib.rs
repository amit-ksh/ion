//! Desktop credential boundary. Values never enter settings, SQLite, logs, or model context.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};

pub const TOKEN_FACTORY_BASE_URL: &str = "https://api.tokenfactory.nebius.com/v1";
const COMPOSIO_TOOLS_URL: &str = "https://backend.composio.dev/api/v3.1/tools";

#[cfg_attr(target_os = "windows", path = "credentials/windows.rs")]
#[cfg_attr(target_os = "macos", path = "credentials/macos.rs")]
#[cfg_attr(target_os = "linux", path = "credentials/linux.rs")]
mod vault;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CredentialKind {
    NebiusTokenFactoryApiKey,
    ComposioScopedProjectKey,
    GatewayPairingToken,
}

impl CredentialKind {
    pub fn account(self) -> &'static str {
        match self {
            Self::NebiusTokenFactoryApiKey => "nebius-token-factory",
            Self::ComposioScopedProjectKey => "composio-project",
            Self::GatewayPairingToken => "gateway-pairing",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialStatus {
    Missing,
    StoredInVault,
    MemoryOnly,
}

pub enum CredentialTest {
    NebiusModels(Vec<String>),
    ComposioVerified,
    GatewayConnected(GatewaySession),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayPairing {
    pub base_url: String,
    pub device_id: String,
    token: String,
    pub expires_at: u64,
}

#[cfg(test)]
mod gateway_url_tests {
    use super::gateway_base_url;

    #[test]
    fn requires_https_outside_loopback_and_rejects_url_secrets() {
        assert!(gateway_base_url("https://gateway.example/").is_ok());
        assert!(gateway_base_url("http://127.0.0.1:8787/").is_ok());
        assert!(gateway_base_url("http://gateway.example/").is_err());
        assert!(gateway_base_url("https://token@gateway.example/").is_err());
        assert!(gateway_base_url("https://gateway.example/path").is_err());
        assert!(gateway_base_url("https://gateway.example/?token=secret").is_err());
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct GatewaySession {
    pub id: String,
    pub name: String,
    #[serde(rename = "expiresAt")]
    pub expires_at: u64,
    pub grants: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GatewayAccount {
    pub id: String,
    pub toolkit: String,
    pub status: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayEffect {
    pub toolkit: String,
    pub tool: String,
    pub version: String,
    pub account_id: String,
    pub target: String,
    pub body: String,
    pub scopes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GatewayProposal {
    pub id: String,
    pub status: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRelayTask {
    pub id: String,
    pub project_id: String,
    pub project_name: String,
    pub title: String,
    pub provider: String,
    pub status: String,
    pub runtime_id: Option<String>,
    pub updated_at: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRelayCommand {
    pub id: String,
    pub task_id: String,
    pub runtime_id: String,
    pub kind: String,
    pub payload: HashMap<String, String>,
    pub status: String,
}

/// A short-lived client for the desktop's outbound task bridge. It never
/// exposes its bearer token or sends it through a redirect.
pub struct GatewayApi {
    url: reqwest::Url,
    device_id: String,
    token: String,
    client: reqwest::blocking::Client,
}

impl GatewayApi {
    pub fn identity(&self) -> (&str, &str) {
        (self.url.as_str(), &self.device_id)
    }

    fn request<T: serde::de::DeserializeOwned>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<T> {
        let mut request = self
            .client
            .request(method, self.url.join(path)?)
            .bearer_auth(&self.token)
            .header("x-waku-device-id", &self.device_id);
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request
            .send()
            .context("Could not reach the personal gateway")?;
        if !response.status().is_success() {
            bail!(
                "Personal gateway request failed (HTTP {})",
                response.status()
            );
        }
        response
            .json()
            .context("Gateway returned an invalid response")
    }

    pub fn has_publish_grant(&self) -> Result<bool> {
        #[derive(Deserialize)]
        struct Session {
            device: GatewaySession,
        }
        let session: Session = self.request(reqwest::Method::GET, "v1/device/session", None)?;
        Ok(session.device.id == self.device_id
            && session.device.grants.iter().any(|grant| grant == "publish"))
    }

    pub fn publish_task(&self, task: &GatewayRelayTask) -> Result<()> {
        let path = format!("v1/device/tasks/{}", task.id);
        let _: serde_json::Value = self.request(
            reqwest::Method::PUT,
            &path,
            Some(serde_json::json!({ "task": task })),
        )?;
        Ok(())
    }

    pub fn reconcile_tasks(&self, active_ids: &[String]) -> Result<()> {
        let _: serde_json::Value = self.request(
            reqwest::Method::POST,
            "v1/device/tasks/reconcile",
            Some(serde_json::json!({ "activeIds": active_ids })),
        )?;
        Ok(())
    }

    pub fn heartbeat(&self) -> Result<()> {
        let _: serde_json::Value = self.request(
            reqwest::Method::POST,
            "v1/device/heartbeat",
            Some(serde_json::json!({})),
        )?;
        Ok(())
    }

    pub fn publish_event(
        &self,
        event_id: &str,
        task_id: &str,
        kind: &str,
        text: &str,
        at: u64,
    ) -> Result<()> {
        let _: serde_json::Value = self.request(
            reqwest::Method::POST,
            "v1/device/events",
            Some(serde_json::json!({ "eventId": event_id, "taskId": task_id,
                "kind": kind, "text": text, "at": at })),
        )?;
        Ok(())
    }

    pub fn pending_commands(&self) -> Result<Vec<GatewayRelayCommand>> {
        #[derive(Deserialize)]
        struct Commands {
            commands: Vec<GatewayRelayCommand>,
        }
        Ok(self
            .request::<Commands>(reqwest::Method::GET, "v1/device/commands", None)?
            .commands)
    }

    pub fn claim_command(&self, id: &str) -> Result<GatewayRelayCommand> {
        self.request(
            reqwest::Method::POST,
            &format!("v1/device/commands/{id}/claim"),
            Some(serde_json::json!({})),
        )
    }

    pub fn settle_command(&self, id: &str, applied: bool) -> Result<GatewayRelayCommand> {
        self.request(
            reqwest::Method::POST,
            &format!("v1/device/commands/{id}/settle"),
            Some(serde_json::json!({ "applied": applied })),
        )
    }
}

fn gateway_base_url(value: &str) -> Result<reqwest::Url> {
    let url = reqwest::Url::parse(value.trim()).context("Enter a valid gateway URL")?;
    let loopback = matches!(url.host_str(), Some("127.0.0.1" | "::1" | "localhost"));
    if !(url.scheme() == "https" || (url.scheme() == "http" && loopback))
        || url.username() != ""
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
    {
        bail!("Use an HTTPS gateway URL, or HTTP on localhost, without a path or credentials");
    }
    Ok(url)
}

fn gateway_client() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .redirect(reqwest::redirect::Policy::none())
        .build()?)
}

pub struct CredentialService {
    volatile: Mutex<HashMap<CredentialKind, String>>,
}

impl CredentialService {
    pub fn global() -> &'static Self {
        static SERVICE: OnceLock<CredentialService> = OnceLock::new();
        SERVICE.get_or_init(|| Self {
            volatile: Mutex::new(HashMap::new()),
        })
    }

    /// Returns only a storage status. Run on the background executor.
    pub fn set(&self, kind: CredentialKind, value: String) -> Result<CredentialStatus> {
        let value = value.trim();
        if value.is_empty() {
            bail!("credential must not be empty")
        }
        match vault::set(kind.account(), value) {
            Ok(()) => {
                self.volatile.lock().unwrap().remove(&kind);
                Ok(CredentialStatus::StoredInVault)
            }
            Err(_) => {
                self.volatile.lock().unwrap().insert(kind, value.to_owned());
                Ok(CredentialStatus::MemoryOnly)
            }
        }
    }

    pub fn status(&self, kind: CredentialKind) -> CredentialStatus {
        if self.volatile.lock().unwrap().contains_key(&kind) {
            return CredentialStatus::MemoryOnly;
        }
        match vault::get(kind.account()) {
            Ok(Some(_)) => CredentialStatus::StoredInVault,
            _ => CredentialStatus::Missing,
        }
    }

    pub fn delete(&self, kind: CredentialKind) -> Result<()> {
        self.volatile.lock().unwrap().remove(&kind);
        vault::delete(kind.account()).context("could not confirm OS vault deletion")?;
        Ok(())
    }

    fn secret(&self, kind: CredentialKind) -> Result<String> {
        if let Some(value) = self.volatile.lock().unwrap().get(&kind) {
            return Ok(value.clone());
        }
        vault::get(kind.account())?.context("credential has not been configured")
    }

    /// The daemon reads the same user vault entry at runtime. A memory-only
    /// desktop key cannot cross the process boundary and must be saved again
    /// after the OS credential store becomes available.
    pub fn nebius_runtime_key(&self) -> Result<String> {
        vault::get(CredentialKind::NebiusTokenFactoryApiKey.account())?
            .context("Save a Nebius key in the OS vault before starting a task")
    }

    /// Consume a one-use invite. The returned device token is saved only in
    /// the OS vault (or this process's memory when the vault is unavailable).
    /// Call on the background executor, never from a GPUI frame.
    pub fn pair_gateway(
        &self,
        base_url: &str,
        invite: &str,
        name: &str,
    ) -> Result<CredentialStatus> {
        let url = gateway_base_url(base_url)?;
        if invite.trim().is_empty() || name.trim().is_empty() {
            bail!("Enter a pairing invite and device name");
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct PairResponse {
            id: String,
            token: String,
            expires_at: u64,
        }
        let response = gateway_client()?
            .post(url.join("v1/pair")?)
            .json(&serde_json::json!({ "invite": invite.trim(), "name": name.trim() }))
            .send()
            .context("Could not reach the personal gateway")?;
        if !response.status().is_success() {
            bail!(
                "Gateway pairing failed (HTTP {}). Check the invite and gateway URL.",
                response.status()
            );
        }
        let paired: PairResponse = response
            .json()
            .context("Gateway returned an invalid pairing response")?;
        if paired.id.is_empty() || paired.token.len() < 32 {
            bail!("Gateway returned an invalid pairing token");
        }
        let pairing = GatewayPairing {
            base_url: url.to_string(),
            device_id: paired.id,
            token: paired.token,
            expires_at: paired.expires_at,
        };
        self.set(
            CredentialKind::GatewayPairingToken,
            serde_json::to_string(&pairing)?,
        )
    }

    fn gateway_pairing(&self) -> Result<GatewayPairing> {
        let pairing: GatewayPairing =
            serde_json::from_str(&self.secret(CredentialKind::GatewayPairingToken)?)
                .context("Pair this desktop from the personal gateway owner console")?;
        gateway_base_url(&pairing.base_url)?;
        Ok(pairing)
    }

    pub fn gateway_api(&self) -> Result<GatewayApi> {
        let pairing = self.gateway_pairing()?;
        Ok(GatewayApi {
            url: gateway_base_url(&pairing.base_url)?,
            device_id: pairing.device_id,
            token: pairing.token,
            client: gateway_client()?,
        })
    }

    pub fn gateway_session(&self) -> Result<GatewaySession> {
        let pairing = self.gateway_pairing()?;
        let url = gateway_base_url(&pairing.base_url)?;
        let response = gateway_client()?
            .get(url.join("v1/device/session")?)
            .bearer_auth(&pairing.token)
            .header("x-waku-device-id", &pairing.device_id)
            .send()
            .context("Could not reach the personal gateway")?;
        if !response.status().is_success() {
            bail!(
                "Gateway pairing is no longer valid (HTTP {}). Pair again from the owner console.",
                response.status()
            );
        }
        #[derive(Deserialize)]
        struct SessionResponse {
            device: GatewaySession,
        }
        let session: SessionResponse = response
            .json()
            .context("Gateway returned an invalid session")?;
        if session.device.id != pairing.device_id {
            bail!("Gateway returned a mismatched device session");
        }
        Ok(session.device)
    }

    pub fn gateway_accounts(&self) -> Result<Vec<GatewayAccount>> {
        let pairing = self.gateway_pairing()?;
        let url = gateway_base_url(&pairing.base_url)?;
        let response = gateway_client()?
            .get(url.join("v1/device/connected-accounts")?)
            .bearer_auth(&pairing.token)
            .header("x-waku-device-id", &pairing.device_id)
            .send()
            .context("Could not reach the personal gateway")?;
        if !response.status().is_success() {
            bail!(
                "Could not list gateway accounts (HTTP {}).",
                response.status()
            );
        }
        #[derive(Deserialize)]
        struct AccountPage {
            items: Vec<GatewayAccount>,
        }
        let page: AccountPage = response
            .json()
            .context("Gateway returned an invalid account list")?;
        Ok(page.items)
    }

    /// Submit an exact action to the gateway's durable owner-approval queue.
    /// This never executes the action; only the owner can decide and execute.
    pub fn gateway_propose(&self, effect: GatewayEffect) -> Result<GatewayProposal> {
        let pairing = self.gateway_pairing()?;
        let url = gateway_base_url(&pairing.base_url)?;
        let response = gateway_client()?
            .post(url.join("v1/device/proposals")?)
            .bearer_auth(&pairing.token)
            .header("x-waku-device-id", &pairing.device_id)
            .json(&serde_json::json!({ "effect": effect }))
            .send()
            .context("Could not reach the personal gateway")?;
        if !response.status().is_success() {
            bail!(
                "Gateway rejected the proposed action (HTTP {}). Check the tool, account, version, and scopes against its allowlist.",
                response.status()
            );
        }
        let proposal: GatewayProposal = response
            .json()
            .context("Gateway returned an invalid proposal")?;
        if proposal.id.len() != 36 || proposal.status != "pending" {
            bail!("Gateway returned an invalid proposal state");
        }
        Ok(proposal)
    }

    pub fn gateway_inspect(&self, proposal_id: &str) -> Result<GatewayProposal> {
        if proposal_id.len() != 36
            || !proposal_id
                .chars()
                .all(|ch| ch.is_ascii_hexdigit() || ch == '-')
        {
            bail!("Invalid gateway proposal ID");
        }
        let pairing = self.gateway_pairing()?;
        let url = gateway_base_url(&pairing.base_url)?;
        let response = gateway_client()?
            .get(url.join(&format!("v1/device/proposals/{proposal_id}"))?)
            .bearer_auth(&pairing.token)
            .header("x-waku-device-id", &pairing.device_id)
            .send()
            .context("Could not reach the personal gateway")?;
        if !response.status().is_success() {
            bail!(
                "Could not inspect gateway proposal (HTTP {}).",
                response.status()
            );
        }
        let proposal: GatewayProposal = response
            .json()
            .context("Gateway returned an invalid proposal")?;
        if proposal.id != proposal_id {
            bail!("Gateway returned a mismatched proposal");
        }
        Ok(proposal)
    }

    pub fn nebius_model_ids(&self) -> Result<Vec<String>> {
        let secret = self.secret(CredentialKind::NebiusTokenFactoryApiKey)?;
        #[derive(Deserialize)]
        struct Model {
            id: String,
        }
        #[derive(Deserialize)]
        struct Catalog {
            data: Vec<Model>,
        }
        let response = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(12))
            .build()?
            .get(format!("{TOKEN_FACTORY_BASE_URL}/models"))
            .bearer_auth(secret)
            .send()
            .context("could not reach Nebius Token Factory")?;
        if !response.status().is_success() {
            bail!(
                "Nebius rejected the key or catalog request (HTTP {})",
                response.status()
            );
        }
        let catalog: Catalog = response
            .json()
            .context("Nebius returned an invalid model catalog")?;
        Ok(catalog.data.into_iter().map(|model| model.id).collect())
    }

    /// Only catalog IDs leave this boundary; the key remains private.
    pub fn test(&self, kind: CredentialKind) -> Result<CredentialTest> {
        match kind {
            CredentialKind::NebiusTokenFactoryApiKey => {
                Ok(CredentialTest::NebiusModels(self.nebius_model_ids()?))
            }
            CredentialKind::ComposioScopedProjectKey => {
                let secret = self.secret(kind)?;
                let response = reqwest::blocking::Client::builder()
                    .timeout(std::time::Duration::from_secs(12))
                    .build()?
                    .get(COMPOSIO_TOOLS_URL)
                    .query(&[("limit", "1")])
                    .header("x-api-key", secret)
                    .send()
                    .context("Could not reach Composio")?;
                if !response.status().is_success() {
                    bail!(
                        "Composio rejected the scoped key or Tools read permission (HTTP {})",
                        response.status()
                    );
                }
                Ok(CredentialTest::ComposioVerified)
            }
            CredentialKind::GatewayPairingToken => {
                Ok(CredentialTest::GatewayConnected(self.gateway_session()?))
            }
        }
    }
}
