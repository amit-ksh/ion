//! Inference credentials stay inside this module; managed requests are bound to
//! the account that started the task and refresh native access tokens per call.
use crate::{CredentialKind, CredentialService, TOKEN_FACTORY_BASE_URL};
use anyhow::{Context as _, Result, ensure};
#[derive(Clone)]
pub struct Inference {
    source: Source,
}
#[derive(Clone)]
enum Source {
    Personal(String),
    Managed(String),
}
impl Inference {
    /// Non-secret identity used to bind persisted continuations to their payer.
    pub fn workspace_id(&self) -> Option<&str> {
        match &self.source {
            Source::Managed(workspace) => Some(workspace),
            Source::Personal(_) => None,
        }
    }
    pub fn personal(key: String) -> Result<Self> {
        ensure!(
            !key.is_empty() && key.len() <= 16384 && !key.contains(['\r', '\n']),
            "Invalid inference credential"
        );
        Ok(Self {
            source: Source::Personal(key),
        })
    }
    pub fn runtime() -> Result<Self> {
        #[cfg(target_os = "linux")]
        if let Some(path) = std::env::var_os("WAKU_NEBIUS_KEY_FILE") {
            return Self::personal(crate::read_private_nebius_key_file(std::path::Path::new(
                &path,
            ))?);
        }
        if let Some(key) = crate::vault::get(CredentialKind::NebiusTokenFactoryApiKey.account())? {
            return Self::personal(key);
        }
        Self::managed()
    }
    pub(crate) fn catalog(service: &CredentialService) -> Result<Self> {
        if let Some(key) = service
            .volatile
            .lock()
            .unwrap()
            .get(&CredentialKind::NebiusTokenFactoryApiKey)
            .cloned()
        {
            return Self::personal(key);
        }
        Self::runtime()
    }
    fn managed() -> Result<Self> {
        let status = crate::cloud::status()?;
        ensure!(
            status.configured && status.signed_in,
            "Sign in to Ion Cloud or save your own Nebius key"
        );
        Ok(Self {
            source: Source::Managed(
                status
                    .workspace_id
                    .context("Cloud account is unavailable")?,
            ),
        })
    }
    pub fn request(
        &self,
        client: &reqwest::blocking::Client,
        path: &str,
    ) -> Result<reqwest::blocking::RequestBuilder> {
        ensure!(
            ["models", "chat/completions"].contains(&path),
            "Unsupported inference operation"
        );
        let method = if path == "models" {
            reqwest::Method::GET
        } else {
            reqwest::Method::POST
        };
        match &self.source {
            Source::Personal(key) => Ok(client
                .request(method, format!("{TOKEN_FACTORY_BASE_URL}/{path}"))
                .bearer_auth(key)),
            Source::Managed(workspace) => {
                crate::cloud::services::inference_request(client, method, path, workspace)
            }
        }
    }
}
