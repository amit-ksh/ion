//! Verified Antigravity 1.3.0/1.3.1 completion using its selected native Gemini account.
//! Ion owns effects and blocks auxiliary model calls at the loopback gate.
use super::{
    DriverEventSender, NativeTeamExit, NativeTeamStart, team_codex::EmptyDirectory,
    team_completion, team_gate,
};
use crate::{
    team_process::bounded_process,
    teams::{ModelCallAllowance, TeamCoordinator},
};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    path::Path,
    process::Command,
    sync::atomic::AtomicBool,
    time::Duration,
};
use waku_protocol::model::{DriverEvent, ProviderKind};
const INPUT: u64 = 1_048_576;
const OUTPUT: u64 = 32_768;
const API_MODEL: &str = "gemini-3.1-pro-preview";
fn model(model: &str) -> Result<()> {
    ensure!(
        matches!(model, "gemini-3.1-pro-low" | "gemini-3.1-pro-high"),
        "Antigravity Teams requires an exact verified Gemini 3.1 Pro low/high native slug; defaults and other models need a contract check"
    );
    Ok(())
}
fn from_settings(settings: &Value, key: String) -> Result<String> {
    ensure!(
        settings["modelProvider"] == "gemini",
        "Antigravity Teams requires this host's native Gemini profile; signed-in Google accounts need a separate verified transport"
    );
    ensure!(
        [
            "customModels",
            "customModel",
            "customModelConfig",
            "enterprise",
            "apiKey",
            "modelConfigs"
        ]
        .iter()
        .all(|k| settings.get(k).is_none()),
        "Native Antigravity custom models/accounts need a contract check"
    );
    ensure!(
        !key.is_empty()
            && key.len() <= 16 * 1024
            && !key.chars().any(char::is_whitespace)
            && !key.starts_with(['!', '$']),
        "Native Gemini key is invalid"
    );
    Ok(key)
}
fn account(selected: &str) -> Result<String> {
    model(selected)?;
    ensure!(
        std::env::var("GOOGLE_GEMINI_BASE_URL")
            .ok()
            .filter(|v| !v.is_empty())
            .is_none_or(|v| v.trim_end_matches('/') == "https://generativelanguage.googleapis.com"),
        "Native Antigravity endpoint changed"
    );
    let path = dirs::home_dir()
        .context("Native Antigravity home missing")?
        .join(".gemini/antigravity-cli/settings.json");
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .context("Configure the native Antigravity Gemini profile first")?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 1024 * 1024,
        "Native Antigravity settings exceed their bound"
    );
    let settings: Value = serde_json::from_slice(&bytes)
        .map_err(|_| anyhow::anyhow!("Native Antigravity settings are invalid"))?;
    from_settings(
        &settings,
        std::env::var("GEMINI_API_KEY")
            .context("Set this host's native GEMINI_API_KEY privately first")?,
    )
}
struct Session {
    root: EmptyDirectory,
}
impl Session {
    fn create() -> Result<Self> {
        let root = EmptyDirectory::create("antigravity")?;
        for dir in [
            "home",
            "home/.gemini",
            "home/.gemini/antigravity-cli",
            "work",
            "work/.agents",
            "work/.agents/agents",
            "tmp",
        ] {
            std::fs::create_dir(root.0.join(dir))?;
        }
        let settings = json!({"modelProvider":"gemini","enableTelemetry":false,"notifications":false,"toolPermission":"strict","showTips":false,"enableTerminalSandbox":false});
        let agent = format!(
            "---\nname: ion-teams\ndescription: Complete one bounded Ion Teams request\ntools: []\nmainAgent: true\nsubagent: false\nexcludeDefaultComponents: true\nmcpServers: []\nskills: []\nplugins: []\n---\n{}\n",
            team_completion::INSTRUCTIONS
        );
        for (path, data) in [
            (
                "home/.gemini/antigravity-cli/settings.json",
                serde_json::to_vec(&settings)?,
            ),
            ("work/.agents/agents/ion-teams.md", agent.into_bytes()),
        ] {
            let mut opts = std::fs::OpenOptions::new();
            opts.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                opts.mode(0o600);
            }
            opts.open(root.0.join(path))?.write_all(&data)?;
        }
        Ok(Self { root })
    }
    fn command(&self, binary: &Path) -> Command {
        let mut cmd = crate::command_env::command(binary);
        let runtime: Vec<_> = cmd
            .get_envs()
            .filter_map(|(k, v)| v.map(|v| (k.to_owned(), v.to_owned())))
            .collect();
        let allowed = |k: &std::ffi::OsStr| {
            matches!(
                k.to_string_lossy().to_ascii_uppercase().as_str(),
                "PATH" | "SYSTEMROOT" | "WINDIR" | "COMSPEC" | "PATHEXT" | "LANG" | "LC_ALL"
            )
        };
        cmd.env_clear()
            .envs(std::env::vars_os().filter(|(k, _)| allowed(k)))
            .envs(runtime.into_iter().filter(|(k, _)| allowed(k)))
            .current_dir(self.root.0.join("work"));
        for (name, dir) in [
            ("HOME", "home"),
            ("USERPROFILE", "home"),
            ("LOCALAPPDATA", "home"),
            ("APPDATA", "home"),
            ("TEMP", "tmp"),
            ("TMP", "tmp"),
            ("TMPDIR", "tmp"),
        ] {
            cmd.env(name, self.root.0.join(dir));
        }
        cmd.env("AGY_CLI_DISABLE_AUTO_UPDATE", "true");
        cmd
    }
    fn verify(&self, binary: &Path, cancelled: &AtomicBool) -> Result<()> {
        let out = bounded_process(
            self.command(binary).arg("--version"),
            b"",
            cancelled,
            Duration::from_secs(30),
        )?;
        ensure!(
            matches!(std::str::from_utf8(&out)?.trim(), "1.3.0" | "1.3.1"),
            "Antigravity native version needs a Teams contract check"
        );
        Ok(())
    }
}
pub(crate) fn ready(binary: &Path, selected: &str) -> Result<()> {
    account(selected)?;
    Session::create()?.verify(binary, &AtomicBool::new(false))
}
fn allowance(selected: &str) -> Result<ModelCallAllowance> {
    model(selected)?;
    Ok(ModelCallAllowance {
        model: selected.into(),
        max_input_tokens: INPUT,
        max_output_tokens: OUTPUT,
    })
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
        ProviderKind::Antigravity,
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
fn launch(
    binary: &Path,
    selected: &str,
    key: &str,
    url: &str,
    task: &str,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>> {
    model(selected)?;
    let session = Session::create()?;
    session.verify(binary, cancelled)?;
    let input = serde_json::to_vec(&json!({"event":"user","message":{"content":task}}))?;
    let mut cmd = session.command(binary);
    cmd.env("GEMINI_API_KEY", key)
        .env("GOOGLE_GEMINI_BASE_URL", url)
        .args([
            "--agent",
            "ion-teams",
            "--model",
            selected,
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
            "--disable-slash-commands",
            "--print-timeout",
            "90s",
        ]);
    bounded_process(&mut cmd, &input, cancelled, Duration::from_secs(120))
}
fn completion(
    binary: &Path,
    selected: &str,
    system: &str,
    messages: &[Value],
    tools: &Value,
    cancelled: &AtomicBool,
) -> Result<super::nebius::StreamCompletion> {
    let key = account(selected)?;
    let task = json!({"instructions":system,"messages":messages,"tools":tools}).to_string();
    let gate = team_gate::Gate::gemini(API_MODEL, INPUT, OUTPUT, &task)?;
    let result = launch(binary, selected, &key, &gate.url, &task, cancelled);
    let receipt = gate.receipt();
    result?;
    let receipt = receipt?;
    team_completion::actions(&receipt.text, tools, receipt.usage)
}
pub(super) fn worker_completion(
    selected: &str,
    system: &str,
    messages: &[Value],
    tools: &Value,
    cancelled: &AtomicBool,
    input_cap: u64,
    output_cap: u64,
) -> Result<super::nebius::StreamCompletion> {
    allowance(selected)?;
    ensure!(
        INPUT <= input_cap && OUTPUT <= output_cap,
        "Registered Antigravity allowance is too small"
    );
    let binary = crate::model::provider_probe(ProviderKind::Antigravity, None)
        .path
        .context("Antigravity worker executable missing")?;
    completion(&binary, selected, system, messages, tools, cancelled)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    #[test]
    fn team_antigravity_preserves_native_account_and_exact_slug() {
        assert!(model("default").is_err());
        assert!(model("gemini-3.1-pro").is_err());
        assert!(model("gemini-3.1-pro-low").is_ok());
        assert!(from_settings(&json!({}), "fixture".into()).is_err());
        assert!(
            from_settings(
                &json!({"modelProvider":"gemini","customModels":[]}),
                "fixture".into()
            )
            .is_err()
        );
    }
    #[test]
    #[ignore = "Requires Antigravity 1.3.0/1.3.1; fake key and loopback only"]
    fn team_antigravity_installed_one_request_blocks_auxiliary_and_tools() {
        let _diagnostics = crate::team_process::fixture_diagnostics();
        let binary = PathBuf::from(std::env::var_os("ION_TEST_ANTIGRAVITY_BINARY").unwrap());
        let data=format!("data: {}\n\n",json!({"modelVersion":API_MODEL,"candidates":[{"index":0,"content":{"role":"model","parts":[{"text":r#"{"text":"ready","calls":[]}"#}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":100,"candidatesTokenCount":5,"thoughtsTokenCount":2,"totalTokenCount":107}})).into_bytes();
        let server = super::super::team_test_http::Server::with_timeout(
            vec![(200, data)],
            Duration::from_secs(100),
        );
        let task = r#"{"instructions":"fixture","messages":[],"tools":[]}"#;
        let gate = team_gate::Gate::start_bounded(
            API_MODEL,
            &format!(
                "{}/v1beta/models/{API_MODEL}:streamGenerateContent?alt=sse",
                server.url
            ),
            INPUT,
            OUTPUT,
            task,
        )
        .unwrap();
        let result = launch(
            &binary,
            "gemini-3.1-pro-low",
            "fixture-only",
            &gate.url,
            task,
            &AtomicBool::new(false),
        );
        let receipt = gate.receipt();
        result.unwrap();
        assert_eq!(receipt.unwrap().usage.output_tokens, 7);
        let requests = server.finish();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].body.get("tools").is_none());
        assert_eq!(
            requests[0].body["generationConfig"]["maxOutputTokens"],
            OUTPUT
        );
        assert_eq!(requests[0].body["contents"][0]["parts"][0]["text"], task);
    }
}
