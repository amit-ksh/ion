//! Desktop credential boundary. Values never enter settings, SQLite, logs, or model context.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use anyhow::{Context as _, Result, bail};
use serde::Deserialize;

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
    Stored,
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
                self.secret(kind)?;
                Ok(CredentialTest::Stored)
            }
        }
    }
}
