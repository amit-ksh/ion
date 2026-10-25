//! Daemon-only provider discovery layered over shared protocol models.

use std::path::Path;

pub use waku_protocol::model::*;

mod codex_binary;

pub fn provider_probe(provider: ProviderKind, binary_override: Option<&str>) -> ProviderProbe {
    if provider == ProviderKind::Nebius {
        return ProviderProbe {
            provider,
            installed: true,
            path: None,
            models: crate::model_catalog::fallback_models(provider),
            agent_presets: Vec::new(),
        };
    }
    let path = if provider == ProviderKind::Codex {
        codex_binary::resolve(binary_override)
    } else {
        match binary_override {
            Some(binary) => crate::command_env::resolve_binary_override(binary),
            None => crate::command_env::find_executable(provider.command()),
        }
    };
    ProviderProbe {
        provider,
        installed: path.is_some(),
        path,
        models: crate::model_catalog::fallback_models(provider),
        agent_presets: crate::model_catalog::fallback_agent_presets(provider),
    }
}

/// Detect a provider and hydrate its catalog from the daemon-owned cache.
///
/// This is the fast half of stale-while-revalidate: clients can render the
/// last successful catalog immediately, then request live discovery to replace
/// it. Cache I/O stays in the daemon instead of leaking host filesystem access
/// into desktop or Web clients.
pub fn cached_provider_probe(
    provider: ProviderKind,
    binary_override: Option<&str>,
) -> ProviderProbe {
    let cached = crate::model_catalog::cached_models(provider);
    apply_cached_models(provider_probe(provider, binary_override), cached)
}

fn apply_cached_models(
    mut probe: ProviderProbe,
    cached_models: Option<Vec<ProviderModel>>,
) -> ProviderProbe {
    if probe.provider.supports_model_discovery()
        && let Some(models) = cached_models
    {
        probe.models = models;
    }
    probe
}

/// A Token Factory `org/model` id reads as the model, with the org as the picker subtitle.
fn nebius_model(id: String) -> ProviderModel {
    let model = match id.split_once('/') {
        Some((org, name)) if !org.is_empty() && !name.is_empty() => ProviderModel::new(&id, name)
            .sub_provider(crate::model_catalog::display_name_from_slug(org)),
        _ => ProviderModel::new(&id, &id),
    };
    if id == "nvidia/nemotron-3-super-120b-a12b" {
        model.default()
    } else {
        model
    }
}

pub fn discover_provider_models(mut probe: ProviderProbe) -> ProviderProbe {
    if probe.provider == ProviderKind::Nebius {
        if let Ok(ids) = crate::credentials::CredentialService::global().nebius_model_ids() {
            probe.models = ids.into_iter().map(nebius_model).collect();
        }
        return probe;
    }
    if probe.provider.supports_model_discovery()
        && let Some(path) = probe.path.as_deref()
    {
        let (models, agent_presets) = crate::model_catalog::discover_catalog(probe.provider, path);
        probe.models = models;
        probe.agent_presets = agent_presets;
    }
    probe
}

/// Run `<cli> --version` on the daemon host and extract its first version-like
/// token. Provider CLIs decorate this output differently, so clients receive a
/// normalized value rather than subprocess output.
pub fn probe_provider_version(binary: &Path) -> Option<String> {
    let mut command = crate::command_env::command(binary);
    let command = command.arg("--version").stdin(std::process::Stdio::null());
    let output = crate::command_env::output(command).ok()?;
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    parse_cli_version(&combined)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_catalog_replaces_fallback_before_live_discovery() {
        let probe = ProviderProbe {
            provider: ProviderKind::Codex,
            installed: true,
            path: Some("/usr/bin/codex".into()),
            models: crate::model_catalog::fallback_models(ProviderKind::Codex),
            agent_presets: Vec::new(),
        };
        let cached = vec![ProviderModel::new("cached-model", "Cached model").default()];

        let probe = apply_cached_models(probe, Some(cached));

        assert_eq!(probe.models.len(), 1);
        assert_eq!(probe.models[0].id, "cached-model");
    }

    #[test]
    fn nebius_models_show_the_name_with_the_org_as_subtitle() {
        let model = nebius_model("moonshotai/Kimi-K3".into());
        assert_eq!(model.id, "moonshotai/Kimi-K3");
        assert_eq!(model.name, "Kimi-K3");
        assert_eq!(model.sub_provider.as_deref(), Some("Moonshot AI"));
        let nemotron = nebius_model("nvidia/nemotron-3-super-120b-a12b".into());
        assert!(nemotron.is_default);
        assert_eq!(nemotron.sub_provider.as_deref(), Some("NVIDIA"));
        assert_eq!(nebius_model("plain".into()).name, "plain");
    }
}
