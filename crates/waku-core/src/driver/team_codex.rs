//! Codex CLI owns authentication and prompt preparation. A one-use loopback
//! transport removes native tools and prevents additional provider requests.
use super::{DriverEventSender, NativeTeamExit, NativeTeamStart, team_completion, team_gate};
use crate::{
    team_process::bounded_process,
    teams::{ModelCallAllowance, TeamCoordinator},
};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::{Duration, SystemTime},
};
use uuid::Uuid;
use waku_protocol::model::{DriverEvent, ProviderKind};

fn auth(binary: &Path) -> Result<&'static str> {
    let cancel = AtomicBool::new(false);
    let mut command = crate::command_env::command(binary);
    command.args(["login", "status"]);
    // Codex writes its nonsecret login status to stderr. Capture it explicitly
    // with the same process limits, without reading/exporting auth.json.
    let output = super::team_codex::login_status(&mut command, &cancel)?;
    if output.contains("Logged in using ChatGPT") {
        Ok("https://chatgpt.com/backend-api/codex/responses")
    } else if output.contains("Logged in using an API key") {
        Ok("https://api.openai.com/v1/responses")
    } else {
        anyhow::bail!("Sign in with codex login before running this agent")
    }
}
fn login_status(command: &mut std::process::Command, cancel: &AtomicBool) -> Result<String> {
    // The CLI's status includes masked key material for API auth. It remains
    // private; only the auth mode survives this readiness check.
    let output = crate::team_process::bounded_process_with_stderr(
        command,
        b"",
        cancel,
        Duration::from_secs(8),
    )?;
    Ok(String::from_utf8(output)?)
}
fn catalog_path() -> Result<PathBuf> {
    Ok(std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|p| p.join(".codex")))
        .context("Codex home is unavailable")?
        .join("models_cache.json"))
}
pub(crate) fn allowance(model: &str) -> Result<ModelCallAllowance> {
    let path = catalog_path()?;
    let metadata = std::fs::metadata(&path)
        .context("Run Codex once to refresh its model catalog before using Teams")?;
    ensure!(
        metadata.len() <= 2 * 1024 * 1024
            && SystemTime::now()
                .duration_since(metadata.modified()?)
                .unwrap_or_default()
                < Duration::from_secs(24 * 3600),
        "Refresh the Codex model catalog; cached capabilities are missing or stale"
    );
    let data: Value = serde_json::from_slice(&std::fs::read(path)?)?;
    let models = data["models"]
        .as_array()
        .context("Invalid Codex model catalog")?;
    let mut selected = models
        .iter()
        .filter(|entry| entry["slug"].as_str() == Some(model));
    let entry = selected
        .next()
        .context("Pinned Codex model is unavailable; refresh the model catalog")?;
    ensure!(selected.next().is_none(), "Ambiguous Codex model identity");
    let context = entry["context_window"]
        .as_u64()
        .filter(|n| (1..=32_000_000).contains(n))
        .context("Codex model context limit unavailable")?;
    // Context bounds both the input and generated output. Reserving each full
    // context is conservative and includes reasoning/cache tokens; no byte estimate.
    Ok(ModelCallAllowance {
        model: model.into(),
        max_input_tokens: context,
        max_output_tokens: context,
    })
}
pub(crate) fn ready(binary: &Path, model: &str) -> Result<()> {
    allowance(model)?;
    let mut command = crate::command_env::command(binary);
    command.args(["exec", "--help"]);
    let help = bounded_process(
        &mut command,
        b"",
        &AtomicBool::new(false),
        Duration::from_secs(8),
    )?;
    let help = std::str::from_utf8(&help)?;
    ensure!(
        [
            "--ignore-user-config",
            "--ignore-rules",
            "--ephemeral",
            "--json"
        ]
        .iter()
        .all(|flag| help.contains(flag)),
        "Update Codex CLI: required Teams isolation flags are unavailable"
    );
    auth(binary)?;
    Ok(())
}
pub(crate) fn run(
    coordinator: &TeamCoordinator,
    start: NativeTeamStart,
    workspace: &Path,
    events: &DriverEventSender,
    cancelled: &AtomicBool,
    binary: &Path,
) -> Result<NativeTeamExit> {
    super::nebius::team::run_with_harness(
        coordinator,
        start,
        workspace,
        events,
        cancelled,
        ProviderKind::Codex,
        allowance,
        |model, system, messages, tools| {
            let reply = completion(binary, model, system, messages, &tools, cancelled)?;
            if !reply.text.is_empty() {
                let _ = events.send(DriverEvent::TextDelta(reply.text.clone()));
            }
            Ok(reply)
        },
    )
}
pub(super) fn completion(
    binary: &Path,
    model: &str,
    system: &str,
    messages: &[Value],
    tools: &Value,
    cancelled: &AtomicBool,
) -> Result<super::nebius::StreamCompletion> {
    let upstream = auth(binary)?;
    let gate = team_gate::Gate::responses(model, upstream)?;
    let cwd = EmptyDirectory::create("codex")?;
    let mut command = command(binary, model, &gate.url, &cwd.0)?;
    let input = serde_json::to_vec(
        &json!({"adapter_instructions":team_completion::INSTRUCTIONS,"instructions":system,"messages":messages,"tools":tools}),
    )?;
    ensure!(
        input.len() <= 128 * 1024,
        "Codex Teams context exceeds its limit"
    );
    let outcome = bounded_process(&mut command, &input, cancelled, Duration::from_secs(120));
    let receipt = gate.receipt();
    outcome?;
    let receipt = receipt?;
    team_completion::actions(&receipt.text, tools, receipt.usage)
}
fn command(binary: &Path, model: &str, url: &str, cwd: &Path) -> Result<std::process::Command> {
    let mut command = crate::command_env::command(binary);
    command.args([
        "exec",
        "--ignore-user-config",
        "--ignore-rules",
        "--ephemeral",
        "--skip-git-repo-check",
        "--json",
        "--color",
        "never",
        "--sandbox",
        "read-only",
        "--model",
        model,
    ]);
    for config in [
        "model_provider=\"ion_team\"".to_owned(),
        format!(
            "model_providers.ion_team={{name=\"Ion Teams transport\",base_url={},wire_api=\"responses\",requires_openai_auth=true,request_max_retries=0,stream_max_retries=0}}",
            serde_json::to_string(url)?
        ),
        "approval_policy=\"never\"".into(),
        "features.shell_tool=false".into(),
        "features.unified_exec=false".into(),
        "features.multi_agent=false".into(),
        "features.apps=false".into(),
        "features.plugins=false".into(),
        "web_search=\"disabled\"".into(),
        "project_doc_max_bytes=0".into(),
        "disable_paste_burst=true".into(),
    ] {
        command.args(["-c", &config]);
    }
    command.current_dir(cwd).arg("-");
    Ok(command)
}
pub(super) struct EmptyDirectory(pub PathBuf, PathBuf);
impl EmptyDirectory {
    pub fn create(kind: &str) -> Result<Self> {
        let path = std::env::temp_dir().join(format!("ion-team-{kind}-{}", Uuid::new_v4()));
        std::fs::create_dir(&path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
        }
        let resolved = path.canonicalize()?;
        ensure!(
            resolved.parent() == Some(std::env::temp_dir().canonicalize()?.as_path()),
            "Private temporary directory escaped its root"
        );
        Ok(Self(path, resolved))
    }
}
impl Drop for EmptyDirectory {
    fn drop(&mut self) {
        // Verify the final absolute target before recursive cleanup. Rust removes
        // child symlinks without following them; never follow a replaced root.
        if std::fs::symlink_metadata(&self.0)
            .is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
            && self.0.canonicalize().is_ok_and(|p| p == self.1)
            && std::env::temp_dir()
                .canonicalize()
                .is_ok_and(|p| self.1.parent() == Some(p.as_path()))
        {
            let _ = std::fs::remove_dir_all(&self.1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "Requires installed Codex; uses fake authentication and loopback provider only"]
    fn team_codex_installed_cli_uses_one_request_with_tools_disabled() {
        let _diagnostics = crate::team_process::fixture_diagnostics();
        let binary = crate::model::provider_probe(ProviderKind::Codex, None)
            .path
            .unwrap();
        let server = crate::driver::team_test_http::Server::new(vec![(
            200,
            crate::driver::team_test_http::responses(
                "gpt-5.4",
                r#"{"text":"fixture complete","calls":[]}"#,
            ),
        )]);
        let gate = team_gate::Gate::start("gpt-5.4", &format!("{}/responses", server.url)).unwrap();
        let cwd = EmptyDirectory::create("codex-fixture").unwrap();
        let home = cwd.0.join("home");
        std::fs::create_dir(&home).unwrap();
        let mut cmd = command(&binary, "gpt-5.4", &gate.url, &cwd.0).unwrap();
        cmd.env("CODEX_HOME", &home)
            .env("OPENAI_API_KEY", "sk-fixture-only")
            .env("ION_FIXTURE_API_KEY", "sk-fixture-only")
            .args([
                "-c",
                "model_providers.ion_team.env_key=\"ION_FIXTURE_API_KEY\"",
            ]);
        let result = bounded_process(
            &mut cmd,
            b"Return {\"text\":\"fixture complete\",\"calls\":[]}",
            &AtomicBool::new(false),
            Duration::from_secs(20),
        );
        let receipt = gate.receipt();
        if let Err(e) = result {
            panic!("Installed CLI failed: {e:#}");
        }
        let requests = server.finish();
        assert_eq!(receipt.unwrap().usage.input_tokens, 100);
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].body["model"], "gpt-5.4");
        assert_eq!(requests[0].body["tools"], json!([]));
    }
}
