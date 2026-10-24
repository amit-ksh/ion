//! Pure personal-project binding validation. No vault, network or account I/O.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

pub(crate) const ACCOUNT_PREFIX: &str = "nebius-personal-project-v1";

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Binding {
    pub(super) version: u8,
    pub(super) project_id: String,
    pub(super) workspace_id: String,
    pub(super) key_digest: String,
}

fn project_id(value: &str) -> bool {
    value.len() <= 160
        && value.strip_prefix("aiproject-").is_some_and(|id| {
            !id.is_empty()
                && id
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}
fn workspace_id(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
fn digest(key: &str) -> String {
    format!("{:x}", Sha256::digest(key.as_bytes()))
}
impl Binding {
    pub(super) fn new(project: &str, workspace: &str, key: &str) -> Result<Self> {
        ensure!(
            project_id(project),
            "Enter a Token Factory AI project ID beginning with aiproject-"
        );
        ensure!(
            workspace_id(workspace),
            "Sign in to Ion before saving a personal project"
        );
        ensure!(
            !key.is_empty() && key.len() <= 16_384 && !key.contains(['\r', '\n']),
            "Save a valid personal Token Factory key first"
        );
        Ok(Self {
            version: 1,
            project_id: project.into(),
            workspace_id: workspace.into(),
            key_digest: digest(key),
        })
    }
    pub(super) fn matches(&self, workspace: Option<&str>, key: &str) -> bool {
        self.version == 1
            && project_id(&self.project_id)
            && workspace_id(&self.workspace_id)
            && workspace == Some(self.workspace_id.as_str())
            && self.key_digest == digest(key)
    }
}

pub(super) fn account(workspace: &str) -> Result<String> {
    ensure!(workspace_id(workspace), "Invalid Ion account binding");
    Ok(format!("{}-{workspace}", ACCOUNT_PREFIX))
}

#[cfg(test)]
mod tests {
    use super::*;
    const PROJECT: &str = "aiproject-e00h49rtswkv9bz46s";
    #[test]
    fn project_binding_does_not_serialize_the_key() {
        let binding = Binding::new(PROJECT, &"a".repeat(64), "fixture-personal-key").unwrap();
        let encoded = serde_json::to_string(&binding).unwrap();
        assert!(!encoded.contains("fixture-personal-key"));
        let decoded: Binding = serde_json::from_str(&encoded).unwrap();
        assert!(decoded.matches(Some(&"a".repeat(64)), "fixture-personal-key"));
    }
    #[test]
    fn another_account_signed_out_session_and_rotated_key_do_not_match() {
        let binding = Binding::new(PROJECT, &"a".repeat(64), "fixture-personal-key").unwrap();
        assert!(!binding.matches(Some(&"b".repeat(64)), "fixture-personal-key"));
        assert!(!binding.matches(None, "fixture-personal-key"));
        assert!(!binding.matches(Some(&"a".repeat(64)), "rotated-key"));
    }
    #[test]
    fn compute_projects_urls_and_header_injection_are_rejected() {
        for value in [
            "project-example",
            "https://example.com",
            "aiproject-",
            "aiproject-x\r\nProject: other",
            "aiproject-UPPER",
        ] {
            assert!(Binding::new(value, &"a".repeat(64), "fixture-key").is_err());
        }
        assert!(
            Binding::new(
                &format!("aiproject-{}", "x".repeat(160)),
                &"a".repeat(64),
                "fixture-key"
            )
            .is_err()
        );
    }
    #[test]
    fn invalid_scope_key_and_future_binding_version_fail_closed() {
        assert!(Binding::new(PROJECT, "wrong-scope", "fixture-key").is_err());
        assert!(Binding::new(PROJECT, &"a".repeat(64), "").is_err());
        assert!(Binding::new(PROJECT, &"a".repeat(64), "key\nsecret").is_err());
        let mut binding = Binding::new(PROJECT, &"a".repeat(64), "fixture-key").unwrap();
        binding.version = 2;
        assert!(!binding.matches(Some(&"a".repeat(64)), "fixture-key"));
    }
    #[test]
    fn project_storage_is_namespaced_by_validated_ion_account() {
        assert_ne!(
            account(&"a".repeat(64)).unwrap(),
            account(&"b".repeat(64)).unwrap()
        );
        assert!(account("../another-account").is_err());
        assert!(account(&"A".repeat(64)).is_err());
    }
}
