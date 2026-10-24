//! Personal sandbox project metadata is tied to an Ion account and key digest.
//! No worker is started here; whoami is the only supported provider operation.
use crate::{CredentialKind, CredentialService, CredentialStatus, cloud};
use anyhow::{Context as _, Result, ensure};
use std::io::Read as _;

mod binding;
pub(crate) use binding::ACCOUNT_PREFIX;
use binding::{Binding, account};

#[derive(Clone, Debug)]
pub struct ProjectSummary {
    pub project_id: String,
    pub workspace_id: String,
}

pub struct ProjectAccess {
    pub project_id: String,
    pub can_execute: bool,
    pub can_list: bool,
    pub can_cancel: bool,
}

pub(super) fn signed_in_workspace() -> Result<String> {
    let status = cloud::status()?;
    status
        .workspace_id
        .filter(|_| status.signed_in)
        .context("Sign in to Ion before configuring a personal project")
}

fn encode_for(service: &CredentialService, project: &str, workspace: &str) -> Result<String> {
    ensure!(
        signed_in_workspace()? == workspace,
        "The Ion account changed; save the project again"
    );
    let key = service
        .secret(CredentialKind::NebiusTokenFactoryApiKey)
        .context("Save your personal Token Factory API key first")?;
    serde_json::to_string(&Binding::new(project, &workspace, &key)?).map_err(Into::into)
}

fn current(service: &CredentialService) -> Result<Option<(Binding, String)>> {
    let status = cloud::status()?;
    let Some(workspace) = status.workspace_id.filter(|_| status.signed_in) else {
        return Ok(None);
    };
    let memory = service
        .personal_projects
        .lock()
        .unwrap()
        .get(&workspace)
        .cloned();
    let raw = match memory {
        Some(raw) => Some(raw),
        None => crate::vault::get(&account(&workspace)?)?,
    };
    let Some(raw) = raw else { return Ok(None) };
    let binding: Binding =
        serde_json::from_str(&raw).context("Saved personal project is invalid; save it again")?;
    let Some(key) = service.optional_secret(CredentialKind::NebiusTokenFactoryApiKey)? else {
        return Ok(None);
    };
    Ok(binding
        .matches(Some(&workspace), &key)
        .then_some((binding, key)))
}

impl CredentialService {
    /// Pin the UI action's account even if native login changes before this worker runs.
    pub fn save_nebius_project(&self, project: &str, workspace: &str) -> Result<CredentialStatus> {
        let encoded = encode_for(self, project.trim(), workspace)?;
        // Persist to the pinned account's namespace, never the account present later.
        match crate::vault::set(&account(workspace)?, &encoded) {
            Ok(()) => {
                self.personal_projects.lock().unwrap().remove(workspace);
                Ok(CredentialStatus::StoredInVault)
            }
            Err(_) => {
                self.personal_projects
                    .lock()
                    .unwrap()
                    .insert(workspace.into(), encoded);
                Ok(CredentialStatus::MemoryOnly)
            }
        }
    }

    pub fn delete_nebius_project_for(&self, workspace: &str) -> Result<()> {
        ensure!(
            signed_in_workspace()? == workspace,
            "The Ion account changed; remove the project again"
        );
        crate::vault::delete(&account(workspace)?)
            .context("Could not confirm personal project removal")?;
        self.personal_projects.lock().unwrap().remove(workspace);
        Ok(())
    }

    pub(super) fn nebius_project_status(&self) -> CredentialStatus {
        match self.nebius_project_summary().ok().flatten() {
            None => CredentialStatus::Missing,
            Some(summary)
                if self
                    .personal_projects
                    .lock()
                    .unwrap()
                    .contains_key(&summary.workspace_id) =>
            {
                CredentialStatus::MemoryOnly
            }
            Some(_) => CredentialStatus::StoredInVault,
        }
    }

    /// Non-secret summary only. Call on a worker, not during rendering.
    pub fn nebius_project_summary(&self) -> Result<Option<ProjectSummary>> {
        Ok(current(self)?.map(|(binding, _)| ProjectSummary {
            project_id: binding.project_id,
            workspace_id: binding.workspace_id,
        }))
    }

    pub fn test_nebius_project(&self) -> Result<ProjectAccess> {
        self.test_nebius_project_for(&signed_in_workspace()?)
    }

    pub fn test_nebius_project_for(&self, workspace: &str) -> Result<ProjectAccess> {
        let (binding, key) = current(self)?
            .context("Save a personal AI project for this Ion account and key first")?;
        ensure!(
            binding.workspace_id == workspace,
            "The Ion account changed; test the project again"
        );
        let response = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(12))
            .redirect(reqwest::redirect::Policy::none())
            .build()?
            .get("https://api.tokenfactory.nebius.com/sandboxes/v1/whoami")
            .header("Project", &binding.project_id)
            .bearer_auth(&key)
            .send()
            .context("Could not reach Token Factory project access")?;
        ensure!(
            response.status().is_success(),
            "Project access check failed (HTTP {}). Check the key and AI project ID.",
            response.status()
        );
        let mut body = Vec::new();
        response.take(24_001).read_to_end(&mut body)?;
        ensure!(
            body.len() <= 24_000,
            "Project access response exceeded its size limit"
        );
        let value: serde_json::Value = serde_json::from_slice(&body)
            .context("Token Factory returned invalid project access data")?;
        let permissions = value
            .get("permissions")
            .and_then(serde_json::Value::as_object)
            .context("Token Factory did not return project permissions")?;
        // A sign-out, account change or key replacement invalidates this result.
        let (latest, _) =
            current(self)?.context("The Ion account or personal key changed; test again")?;
        ensure!(
            latest.project_id == binding.project_id
                && latest.workspace_id == binding.workspace_id
                && latest.key_digest == binding.key_digest,
            "Personal project changed; test again"
        );
        Ok(ProjectAccess {
            project_id: binding.project_id,
            can_execute: permissions
                .get("spawn_disposable")
                .and_then(serde_json::Value::as_bool)
                == Some(true),
            can_list: permissions.get("list").and_then(serde_json::Value::as_bool) == Some(true),
            can_cancel: permissions
                .get("cancel")
                .and_then(serde_json::Value::as_bool)
                == Some(true),
        })
    }
}
