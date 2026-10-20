pub mod bundle;
pub mod services;
// Native account boundary. Call only on a background worker.
mod oauth;
mod store;
#[cfg(test)]
mod tests;

use anyhow::{Context as _, Result, bail, ensure};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use store::{AccountLock, OsVault, Store, Vault};

pub const DEFAULT_ORIGIN: &str = "https://api.build-with-ion.tech";
pub const ISSUER: &str = "https://dev-amit-ksh.au.auth0.com/";
pub const AUDIENCE: &str = "https://ion-cloud-api";
pub const DESKTOP_CLIENT_ID: &str = "M7iCZjpo5bzOVO2E5nrvKzDJ00p3ZWy0";
pub const CALLBACK: &str = "http://127.0.0.1:43891/ion/callback";
const VAULT_KEY: &str = "cloud-account-v1";

#[derive(Clone)]
struct Config {
    origin: String,
}
impl Config {
    fn new(origin: &str) -> Result<Self> {
        let url =
            Url::parse(origin).map_err(|_| anyhow::anyhow!("Ion Cloud hostname is invalid"))?;
        ensure!(
            url.scheme() == "https"
                && origin == url.origin().ascii_serialization()
                && url.username().is_empty()
                && url.password().is_none(),
            "Ion Cloud requires an HTTPS origin"
        );
        Ok(Self {
            origin: origin.to_owned(),
        })
    }
}
fn operator_config() -> Result<Option<Config>> {
    let value = std::env::var("ION_CLOUD_PUBLIC_ORIGIN")
        .ok()
        .or_else(|| option_env!("ION_CLOUD_PUBLIC_ORIGIN").map(str::to_owned))
        .or_else(|| Some(DEFAULT_ORIGIN.to_owned()));
    value
        .filter(|v| !v.trim().is_empty())
        .map(|v| Config::new(v.trim()))
        .transpose()
}
fn now_ms() -> Result<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .try_into()?)
}
fn random() -> Result<String> {
    let mut value = [0u8; 32];
    getrandom::fill(&mut value).map_err(|_| anyhow::anyhow!("Secure randomness is unavailable"))?;
    Ok(URL_SAFE_NO_PAD.encode(value))
}
fn workspace(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn opaque(value: &str, min: usize, max: usize) -> bool {
    (min..=max).contains(&value.len())
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
}
fn credential_token(value: &str) -> bool {
    !value.is_empty() && value.len() <= 16_384 && value.bytes().all(|b| b.is_ascii_graphic())
}
fn device_id(value: &str) -> bool {
    value.len() == 36
        && value != "00000000-0000-0000-0000-000000000000"
        && value.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
            }
        })
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Device {
    id: String,
    token: String,
    expires_at: u64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Session {
    origin: String,
    issuer: String,
    audience: String,
    client_id: String,
    workspace_id: String,
    access_token: String,
    refresh_token: String,
    expires_at: u64,
    refreshing: bool,
    device: Option<Device>,
}
impl Session {
    fn valid(&self, config: &Config) -> bool {
        self.origin == config.origin
            && self.issuer == ISSUER
            && self.audience == AUDIENCE
            && self.client_id == DESKTOP_CLIENT_ID
            && workspace(&self.workspace_id)
            && credential_token(&self.access_token)
            && credential_token(&self.refresh_token)
            && self
                .device
                .as_ref()
                .is_none_or(|d| device_id(&d.id) && opaque(&d.token, 20, 256))
    }
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct State {
    version: u8,
    epoch: String,
    session: Option<Session>,
}
impl State {
    fn empty() -> Result<Self> {
        Ok(Self {
            version: 1,
            epoch: random()?,
            session: None,
        })
    }
}

#[derive(Clone, Default)]
pub struct AccountStatus {
    pub configured: bool,
    pub signed_in: bool,
    pub connected: bool,
    pub workspace_id: Option<String>,
}
#[derive(Clone)]
pub(crate) struct PublisherBinding {
    pub(super) origin: String,
    pub(super) workspace_id: String,
    pub(super) device_id: String,
    pub(super) device_token: String,
}
impl PublisherBinding {
    pub(super) fn matches(&self, other: &Self) -> bool {
        self.origin == other.origin
            && self.workspace_id == other.workspace_id
            && self.device_id == other.device_id
            && self.device_token == other.device_token
    }
}

trait Http: Send + Sync {
    fn json(
        &self,
        method: &str,
        url: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> Result<Value>;
    fn form(&self, url: &str, fields: &[(&str, &str)]) -> Result<Value>;
    fn revoke(&self, refresh: &str) -> bool;
}
struct Network;
impl Network {
    fn client() -> Result<reqwest::blocking::Client> {
        Ok(reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(12))
            .redirect(reqwest::redirect::Policy::none())
            .build()?)
    }
    fn decode(response: reqwest::blocking::Response) -> Result<Value> {
        use std::io::Read;
        ensure!(
            response.status().is_success(),
            "Cloud request was rejected (HTTP {})",
            response.status()
        );
        let mut bytes = Vec::new();
        response
            .take(70_001)
            .read_to_end(&mut bytes)
            .map_err(|_| anyhow::anyhow!("Cloud response could not be read"))?;
        ensure!(bytes.len() <= 70_000, "Cloud response exceeds its limit");
        serde_json::from_slice(&bytes).map_err(|_| anyhow::anyhow!("Cloud returned invalid JSON"))
    }
}
impl Http for Network {
    fn json(
        &self,
        method: &str,
        url: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> Result<Value> {
        let mut request = Self::client()?.request(method.parse()?, url);
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        if let Some(body) = body {
            request = request.json(&body);
        }
        Self::decode(
            request
                .send()
                .map_err(|_| anyhow::anyhow!("Could not reach Ion Cloud"))?,
        )
    }
    fn form(&self, url: &str, fields: &[(&str, &str)]) -> Result<Value> {
        let encoded: String = Url::parse_with_params("https://unused.invalid", fields)?
            .query()
            .unwrap_or_default()
            .to_owned();
        Self::decode(
            Self::client()?
                .post(url)
                .header("Content-Type", "application/x-www-form-urlencoded")
                .body(encoded)
                .send()
                .map_err(|_| anyhow::anyhow!("Could not reach account sign-in"))?,
        )
    }
    fn revoke(&self, refresh: &str) -> bool {
        Self::client()
            .and_then(|client| {
                Ok(client
                    .post(format!("{ISSUER}oauth/revoke"))
                    .header("Content-Type", "application/x-www-form-urlencoded")
                    .body(
                        Url::parse_with_params(
                            "https://unused.invalid",
                            &[
                                ("client_id", DESKTOP_CLIENT_ID),
                                ("token", refresh),
                                ("token_type_hint", "refresh_token"),
                            ],
                        )?
                        .query()
                        .unwrap_or_default()
                        .to_owned(),
                    )
                    .send()?
                    .status()
                    .is_success())
            })
            .unwrap_or(false)
    }
}
struct Account<V: Vault, H: Http> {
    config: Config,
    vault: V,
    http: H,
    lock_dir: PathBuf,
}
impl<V: Vault, H: Http> Account<V, H> {
    fn store(&self) -> Store<'_, V> {
        Store::new(&self.vault, VAULT_KEY)
    }
    fn lock(&self) -> Result<AccountLock> {
        AccountLock::acquire(&self.lock_dir, Duration::from_secs(4))
    }
    fn read(&self) -> Result<State> {
        let mut state = self
            .store()
            .read::<State>()?
            .map(Ok)
            .unwrap_or_else(State::empty)?;
        ensure!(
            state.version == 1 && opaque(&state.epoch, 43, 43),
            "Cloud account state is invalid"
        );
        if state
            .session
            .as_ref()
            .is_some_and(|s| !s.valid(&self.config) || s.refreshing)
        {
            state.session = None;
            self.store().write(&state)?;
        }
        Ok(state)
    }
    fn status(&self) -> Result<AccountStatus> {
        let _guard = self.lock()?;
        let state = self.read()?;
        let now = now_ms()?;
        Ok(AccountStatus {
            configured: true,
            workspace_id: state.session.as_ref().map(|s| s.workspace_id.clone()),
            signed_in: state.session.is_some(),
            connected: state
                .session
                .as_ref()
                .and_then(|s| s.device.as_ref())
                .is_some_and(|d| d.expires_at > now),
        })
    }
    fn verified_status(&self) -> Result<AccountStatus> {
        let _guard = self.lock()?;
        let mut state = self.read()?;
        if state.session.is_none() {
            return Ok(AccountStatus {
                configured: true,
                ..AccountStatus::default()
            });
        }
        self.authorize(&mut state)?;
        let session = state.session.as_ref().context("Sign in to Ion Cloud")?;
        let identity = self.http.json(
            "GET",
            &format!("{}/v1/session", self.config.origin),
            Some(&session.access_token),
            None,
        )?;
        ensure!(
            identity["workspaceId"].as_str() == Some(session.workspace_id.as_str()),
            "Account identity could not be verified"
        );
        let now = now_ms()?;
        Ok(AccountStatus {
            configured: true,
            signed_in: true,
            connected: session.device.as_ref().is_some_and(|d| d.expires_at > now),
            workspace_id: Some(session.workspace_id.clone()),
        })
    }
    fn tokens(&self, value: &Value, expected: Option<&str>) -> Result<Session> {
        let access = value["access_token"].as_str().unwrap_or_default();
        let refresh = value["refresh_token"].as_str().unwrap_or_default();
        let lifetime = value["expires_in"].as_u64().unwrap_or_default();
        ensure!(
            value["token_type"] == "Bearer"
                && credential_token(access)
                && credential_token(refresh)
                && (1..=31_536_000).contains(&lifetime),
            "Sign-in did not return a renewable account session"
        );
        let identity = self.http.json(
            "GET",
            &format!("{}/v1/session", self.config.origin),
            Some(access),
            None,
        )?;
        let id = identity["workspaceId"].as_str().unwrap_or_default();
        ensure!(
            workspace(id) && expected.is_none_or(|v| v == id),
            "Account identity could not be verified"
        );
        Ok(Session {
            origin: self.config.origin.clone(),
            issuer: ISSUER.into(),
            audience: AUDIENCE.into(),
            client_id: DESKTOP_CLIENT_ID.into(),
            workspace_id: id.into(),
            access_token: access.into(),
            refresh_token: refresh.into(),
            expires_at: now_ms()?
                .checked_add(lifetime * 1000)
                .context("Invalid token lifetime")?,
            refreshing: false,
            device: None,
        })
    }
    fn authorize(&self, state: &mut State) -> Result<()> {
        let session = state.session.as_mut().context("Sign in to Ion Cloud")?;
        if session.expires_at > now_ms()?.saturating_add(30_000) {
            return Ok(());
        }
        session.refreshing = true;
        self.store().write(state)?; // Commit before consuming a rotating refresh token.
        let previous = state.session.as_ref().unwrap();
        let result = self
            .http
            .form(
                &format!("{ISSUER}oauth/token"),
                &[
                    ("grant_type", "refresh_token"),
                    ("client_id", DESKTOP_CLIENT_ID),
                    ("refresh_token", &previous.refresh_token),
                ],
            )
            .and_then(|value| self.tokens(&value, Some(&previous.workspace_id)));
        match result {
            Ok(mut session) => {
                session.device = state.session.as_ref().and_then(|s| s.device.clone());
                state.session = Some(session);
                self.store().write(state)?;
                Ok(())
            }
            Err(_) => {
                state.session = None;
                self.store().write(state)?;
                bail!("Sign in again; account refresh could not be confirmed")
            }
        }
    }
    fn binding(&self) -> Result<PublisherBinding> {
        let _guard = self.lock()?;
        let state = self.read()?;
        let session = state.session.context("Sign in to Ion Cloud")?;
        let device = session
            .device
            .context("Connect this computer to Ion Cloud")?;
        ensure!(
            device.expires_at > now_ms()?,
            "Cloud device grant expired; reconnect this computer"
        );
        Ok(PublisherBinding {
            origin: session.origin,
            workspace_id: session.workspace_id,
            device_id: device.id,
            device_token: device.token,
        })
    }
    fn with_publisher<T>(
        &self,
        binding: &PublisherBinding,
        operation: impl FnOnce(&str) -> Result<T>,
    ) -> Result<T> {
        let _guard = self.lock()?;
        let mut state = self.read()?;
        let session = state.session.as_ref().context("Sign in to Ion Cloud")?;
        let device = session
            .device
            .as_ref()
            .context("Cloud computer connection was removed")?;
        ensure!(
            binding.origin == self.config.origin
                && binding.workspace_id == session.workspace_id
                && binding.device_id == device.id
                && binding.device_token == device.token
                && device.expires_at > now_ms()?,
            "Cloud account or computer connection changed"
        );
        self.authorize(&mut state)?;
        // Keep the OS lock for dispatch, including decoding: sign-out cannot
        // overtake a cached publisher after its authorization check.
        operation(&state.session.as_ref().unwrap().access_token)
    }
    fn connect(&self) -> Result<()> {
        let _guard = self.lock()?;
        let mut state = self.read()?;
        self.authorize(&mut state)?;
        let session = state.session.as_mut().unwrap();
        if let Some(device) = &session.device {
            self.http.json(
                "POST",
                &format!(
                    "{}/v1/gateway/v1/owner/devices/{}/revoke",
                    self.config.origin, device.id
                ),
                Some(&session.access_token),
                Some(json!({})),
            )?;
        }
        let invite = self.http.json(
            "POST",
            &format!("{}/v1/gateway/v1/owner/invites", self.config.origin),
            Some(&session.access_token),
            Some(json!({"grants":["publish"]})),
        )?;
        let paired = self.http.json(
            "POST",
            &format!("{}/v1/gateway/v1/pair", self.config.origin),
            Some(&session.access_token),
            Some(json!({"invite":invite["invite"],"name":"ion desktop"})),
        )?;
        let device = Device {
            id: paired["id"].as_str().unwrap_or_default().into(),
            token: paired["token"].as_str().unwrap_or_default().into(),
            expires_at: paired["expiresAt"].as_u64().unwrap_or_default(),
        };
        ensure!(
            device_id(&device.id)
                && opaque(&device.token, 20, 256)
                && device.expires_at > now_ms()?
                && paired["grants"]
                    .as_array()
                    .is_some_and(|grants| grants.len() == 1 && grants[0] == "publish"),
            "Cloud returned an invalid computer grant"
        );
        session.device = Some(device);
        self.store().write(&state)
    }
    fn sign_out(&self) -> Result<bool> {
        let _guard = self.lock()?;
        // Do not claim remote revocation when corrupt/recovered state leaves
        // the old device unknown. Never send foreign-configuration credentials.
        let (previous, uncertain) = match self.store().read::<State>() {
            Ok(None) => (None, false),
            Ok(Some(state)) if state.version == 1 && opaque(&state.epoch, 43, 43) => {
                match state.session {
                    None => (None, false),
                    Some(session) if session.valid(&self.config) => {
                        let uncertain = session.refreshing;
                        (Some(session), uncertain)
                    }
                    Some(_) => (None, true),
                }
            }
            _ => (None, true),
        };
        self.store().reset(&State::empty()?)?; // All processes stop before remote revocation.
        let Some(session) = previous else {
            return Ok(!uncertain);
        };
        let device_revoked = session.device.as_ref().is_none_or(|device| {
            self.http
                .json(
                    "POST",
                    &format!(
                        "{}/v1/gateway/v1/owner/devices/{}/revoke",
                        self.config.origin, device.id
                    ),
                    Some(&session.access_token),
                    Some(json!({})),
                )
                .is_ok()
        });
        Ok(self.http.revoke(&session.refresh_token) && device_revoked && !uncertain)
    }
}
fn account() -> Result<Account<OsVault, Network>> {
    let config = operator_config()?.context("Ion Cloud is not configured in this build")?;
    let lock_dir = dirs::data_local_dir()
        .context("User application directory is unavailable")?
        .join("ion")
        .join("cloud-account");
    Ok(Account {
        config,
        vault: OsVault,
        http: Network,
        lock_dir,
    })
}

pub fn status() -> Result<AccountStatus> {
    if operator_config()?.is_none() {
        return Ok(AccountStatus::default());
    }
    account()?.status()
}
/// Background-only entry check. The IDE must stay locked on verification errors.
pub fn verified_status() -> Result<AccountStatus> {
    if operator_config()?.is_none() {
        return Ok(AccountStatus::default());
    }
    account()?.verified_status()
}
pub fn connect_computer() -> Result<()> {
    // Personal/self-hosted state is preserved until the owner explicitly removes it.
    ensure!(
        crate::CredentialService::global().status(crate::CredentialKind::GatewayPairingToken)
            == crate::CredentialStatus::Missing,
        "Remove the personal gateway pairing before connecting Ion Cloud"
    );
    account()?.connect()
}
pub fn sign_out() -> Result<bool> {
    account()?.sign_out()
}
pub(crate) fn publisher_binding() -> Result<PublisherBinding> {
    account()?.binding()
}
pub(crate) fn with_publisher_authorization<T>(
    binding: &PublisherBinding,
    operation: impl FnOnce(&str) -> Result<T>,
) -> Result<T> {
    account()?.with_publisher(binding, operation)
}
pub use oauth::LoginAttempt;
pub fn begin_sign_in() -> Result<LoginAttempt> {
    oauth::begin(account()?)
}

pub fn disconnect_computer() -> Result<bool> {
    let account = account()?;
    let _guard = account.lock()?;
    let mut state = account.read()?;
    let session = state.session.as_mut().context("Sign in to Ion Cloud")?;
    let device = session.device.take();
    account.store().write(&state)?;
    let Some(device) = device else {
        return Ok(true);
    };
    Ok(account
        .http
        .json(
            "POST",
            &format!(
                "{}/v1/gateway/v1/owner/devices/{}/revoke",
                account.config.origin, device.id
            ),
            Some(&state.session.as_ref().unwrap().access_token),
            Some(json!({})),
        )
        .is_ok())
}
pub fn verify_connection() -> Result<bool> {
    if !status()?.connected {
        return Ok(false);
    }
    let api = crate::GatewayApi::managed(publisher_binding()?)?;
    ensure!(
        api.has_publish_grant()?,
        "Cloud computer grant is unavailable"
    );
    Ok(true)
}
