//! Fx 0.0.13's configured direct-provider ask contract (Linux/macOS).
//! The native account stays selected; Ion owns every effect and reservation.
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
#[derive(Clone, Debug, PartialEq, Eq)]
struct Metadata {
    provider: String,
    model: String,
    upstream: &'static str,
    key_env: String,
    input: u64,
    output: u64,
}
fn metadata(settings: &Value, selected: &str, provider_override: Option<&str>) -> Result<Metadata> {
    ensure!(
        !selected.is_empty()
            && selected.len() <= 1024
            && !selected.chars().any(char::is_whitespace)
            && !matches!(selected, "auto" | "default"),
        "Choose an exact native Fx model ID"
    );
    let provider = provider_override
        .filter(|s| !s.trim().is_empty())
        .or_else(|| settings["provider"].as_str())
        .context("Choose a native configured Fx provider first")?;
    ensure!(
        !provider.is_empty()
            && provider.len() <= 64
            && provider.as_bytes()[0].is_ascii_alphabetic()
            && provider
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'),
        "Native Fx provider ID is invalid"
    );
    ensure!(
        !["gateway", "codex", "grok"]
            .iter()
            .any(|p| provider.eq_ignore_ascii_case(p)),
        "Fx Gateway/subscription accounts require a separately verified Teams transport; the selected account is retained"
    );
    let config = &settings["providers"][provider];
    ensure!(
        config["protocol"] == "openai-chat-completions",
        "Native Fx provider protocol is unsupported"
    );
    let upstream = match config["base_url"].as_str().map(|s| s.trim_end_matches('/')) {
        Some("https://api.openai.com/v1") => "https://api.openai.com/v1",
        Some("https://api.tokenfactory.nebius.com/v1") => "https://api.tokenfactory.nebius.com/v1",
        _ => anyhow::bail!(
            "Native Fx endpoint requires a contract check; direct OpenAI and Nebius origins are supported"
        ),
    };
    ensure!(
        config["auth"]["type"] == "bearer",
        "Native Fx bearer account missing"
    );
    let key_env = config["auth"]["env"]
        .as_str()
        .context("Native Fx credential slot missing")?;
    ensure!(
        !key_env.is_empty()
            && key_env.len() <= 128
            && (key_env.as_bytes()[0].is_ascii_alphabetic() || key_env.starts_with('_'))
            && key_env
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_'),
        "Native Fx credential slot is invalid"
    );
    // The child uses a private credential slot so a native PATH/HOME alias cannot
    // replace its runtime settings. The secret value remains the same account's.
    let native = &config["model_metadata"][selected];
    let limit = |name| {
        native[name]
            .as_u64()
            .filter(|n| (1..=32_000_000).contains(n))
            .context("Native Fx model token metadata missing or invalid")
    };
    let input = limit("context_window")?;
    let output = limit("max_output_tokens")?;
    ensure!(
        output < input,
        "Native Fx model output limit exceeds its context"
    );
    Ok(Metadata {
        provider: provider.into(),
        model: selected.into(),
        upstream,
        key_env: key_env.into(),
        input,
        output: output.min(32_768),
    })
}
fn account(selected: &str) -> Result<(Metadata, String)> {
    let path = dirs::home_dir()
        .context("Native Fx home missing")?
        .join(".fx/settings.json");
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .context("Configure the native Fx direct provider first")?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 1024 * 1024,
        "Native Fx settings exceed their bound"
    );
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| anyhow::anyhow!("Native Fx settings are invalid"))?;
    let meta = metadata(
        &value,
        selected,
        std::env::var("FX_PROVIDER").ok().as_deref(),
    )?;
    let key = std::env::var(&meta.key_env)
        .context("Set the native Fx provider credential privately on this host first")?;
    ensure!(
        !key.is_empty()
            && key.len() <= 16 * 1024
            && !key.chars().any(char::is_whitespace)
            && !key.starts_with(['!', '$']),
        "Native Fx credential is invalid"
    );
    Ok((meta, key))
}
struct Session {
    root: EmptyDirectory,
}
impl Session {
    fn create(meta: &Metadata, url: &str) -> Result<Self> {
        let root = EmptyDirectory::create("fx")?;
        for dir in ["home", "home/.fx", "work", "tmp"] {
            std::fs::create_dir(root.0.join(dir))?;
        }
        let config = json!({"provider":meta.provider,"models":{&meta.provider:meta.model},"providers":{&meta.provider:{"protocol":"openai-chat-completions","base_url":url,"auth":{"type":"bearer","env":"ION_FX_NATIVE_KEY"},"tool_choice_mode":"send","model_metadata":{&meta.model:{"context_window":meta.input,"max_output_tokens":meta.output,"supports_tool_use":false,"supports_vision":false}}}},"max_agent_steps":1,"first_call_tool_choice":"none","context":false,"session_titles":false,"auto_upgrade":false,"prompt_history":{"enabled":false},"permission_mode":"ask","permission":{"*":"deny"}});
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        opts.open(root.0.join("home/.fx/settings.json"))?
            .write_all(&serde_json::to_vec(&config)?)?;
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
        cmd.env("FX_DISABLE_AUTO_UPGRADE", "1")
            .env("FX_MAX_AGENT_STEPS", "1");
        cmd
    }
    fn verify(&self, binary: &Path, cancelled: &AtomicBool) -> Result<()> {
        let bytes = bounded_process(
            self.command(binary).arg("--version"),
            b"",
            cancelled,
            Duration::from_secs(30),
        )?;
        ensure!(
            std::str::from_utf8(&bytes)?.trim() == "0.0.13",
            "Native Fx version needs a Teams contract check"
        );
        Ok(())
    }
}
pub(crate) fn ready(binary: &Path, selected: &str) -> Result<()> {
    ensure!(
        cfg!(unix),
        "Fx 0.0.13 has native Linux/macOS releases; select a registered Linux worker or use another local harness on Windows"
    );
    let (meta, _) = account(selected)?;
    Session::create(&meta, "http://127.0.0.1:1/ion/probe")?.verify(binary, &AtomicBool::new(false))
}
fn allowance(meta: &Metadata) -> ModelCallAllowance {
    ModelCallAllowance {
        model: meta.model.clone(),
        max_input_tokens: meta.input,
        max_output_tokens: meta.output,
    }
}
pub(crate) fn run(
    coordinator: &TeamCoordinator,
    start: NativeTeamStart,
    workspace: &Path,
    events: &DriverEventSender,
    cancelled: &AtomicBool,
    binary: &Path,
) -> Result<NativeTeamExit> {
    let selected = match &start {
        NativeTeamStart::Fresh(c) => &c.profile.model,
        NativeTeamStart::Resumed(c) => &c.claimed.profile.model,
    }
    .as_deref()
    .context("Pinned Fx model missing")?;
    let (meta, _) = account(selected)?;
    super::nebius::team::run_with_harness(
        coordinator,
        start,
        workspace,
        events,
        cancelled,
        ProviderKind::Fx,
        |selected| {
            ensure!(selected == meta.model, "Native Fx model changed");
            Ok(allowance(&meta))
        },
        |selected, system, messages, tools| {
            let reply = completion(binary, &meta, selected, system, messages, &tools, cancelled)?;
            if !reply.text.is_empty() {
                let _ = events.send(DriverEvent::TextDelta(reply.text.clone()));
            }
            Ok(reply)
        },
    )
}
fn launch(
    binary: &Path,
    meta: &Metadata,
    key: &str,
    url: &str,
    task: &str,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>> {
    let session = Session::create(meta, url)?;
    session.verify(binary, cancelled)?;
    bounded_process(
        session
            .command(binary)
            .env("ION_FX_NATIVE_KEY", key)
            .env("FX_PROVIDER", &meta.provider)
            .args([
                "ask",
                "--model",
                &meta.model,
                "--system",
                team_completion::INSTRUCTIONS,
                "--json",
                "--no-save",
                "--no-color",
                "--timeout",
                "90",
            ]),
        task.as_bytes(),
        cancelled,
        Duration::from_secs(120),
    )
}
fn completion(
    binary: &Path,
    expected: &Metadata,
    selected: &str,
    system: &str,
    messages: &[Value],
    tools: &Value,
    cancelled: &AtomicBool,
) -> Result<super::nebius::StreamCompletion> {
    let (meta, key) = account(selected)?;
    ensure!(
        &meta == expected,
        "Native Fx provider/model/limits changed after reservation"
    );
    let task = json!({"instructions":system,"messages":messages,"tools":tools}).to_string();
    let gate =
        team_gate::Gate::bounded_api(&meta.model, meta.upstream, meta.input, meta.output, &task)?;
    let result = launch(binary, &meta, &key, &gate.url, &task, cancelled);
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
    ensure!(cfg!(unix), "Native Fx worker requires Linux/macOS");
    let (meta, _) = account(selected)?;
    ensure!(
        meta.input <= input_cap && meta.output <= output_cap,
        "Registered Fx allowance is too small"
    );
    let binary = crate::model::provider_probe(ProviderKind::Fx, None)
        .path
        .context("Native Fx worker executable missing")?;
    completion(&binary, &meta, selected, system, messages, tools, cancelled)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    fn settings() -> Value {
        json!({"provider":"Personal","providers":{"Personal":{"protocol":"openai-chat-completions","base_url":"https://api.openai.com/v1","auth":{"type":"bearer","env":"MY_NATIVE_FX_KEY"},"model_metadata":{"gpt-5.4":{"context_window":272000,"max_output_tokens":128000}}}}})
    }
    #[test]
    fn team_fx_uses_selected_native_provider_metadata_without_aliases() {
        let value = settings();
        let meta = metadata(&value, "gpt-5.4", None).unwrap();
        assert_eq!(meta.output, 32768);
        assert_eq!(meta.key_env, "MY_NATIVE_FX_KEY");
        assert!(metadata(&value, "auto", None).is_err());
        let mut blank = settings();
        blank["provider"] = json!("");
        assert!(metadata(&blank, "gpt-5.4", None).is_err());
        assert!(metadata(&value, "gpt-5.5", None).is_err());
        assert!(metadata(&value, "gpt-5.4", Some("gateway")).is_err());
        for (field, replacement) in [
            ("base_url", json!("https://changed.example/v1")),
            ("auth", json!({"type":"none"})),
            (
                "model_metadata",
                json!({"gpt-5.4":{"context_window":100,"max_output_tokens":100}}),
            ),
        ] {
            let mut changed = settings();
            changed["providers"]["Personal"][field] = replacement;
            assert!(metadata(&changed, "gpt-5.4", None).is_err());
        }
    }
    #[test]
    fn team_fx_private_profile_keeps_native_account_and_has_no_credentials() {
        let meta = metadata(&settings(), "gpt-5.4", None).unwrap();
        let session = Session::create(&meta, "http://127.0.0.1:1/ion/private").unwrap();
        let bytes = std::fs::read(session.root.0.join("home/.fx/settings.json")).unwrap();
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["provider"], "Personal");
        assert_eq!(value["max_agent_steps"], 1);
        assert_eq!(
            value["providers"]["Personal"]["model_metadata"]["gpt-5.4"]["supports_tool_use"],
            false
        );
        assert_eq!(
            value["providers"]["Personal"]["auth"]["env"],
            "ION_FX_NATIVE_KEY"
        );
        assert!(value["providers"]["Personal"]["auth"].get("key").is_none());
    }
    #[test]
    #[ignore = "Requires Fx 0.0.13 on Linux/macOS; fake key and loopback only"]
    fn team_fx_installed_ask_one_request_is_bounded() {
        let binary = PathBuf::from(std::env::var_os("ION_TEST_FX_BINARY").unwrap());
        let meta = metadata(&settings(), "gpt-5.4", None).unwrap();
        let data=format!("data: {}\n\ndata: {}\n\ndata: [DONE]\n\n",json!({"model":"gpt-5.4","choices":[{"index":0,"delta":{"content":r#"{"text":"ready","calls":[]}"#},"finish_reason":null}]}),json!({"model":"gpt-5.4","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":100,"completion_tokens":5}})).into_bytes();
        let server = super::super::team_test_http::Server::with_timeout(
            vec![(200, data)],
            Duration::from_secs(100),
        );
        let task = r#"{"instructions":"fixture","messages":[],"tools":[]}"#;
        let gate = team_gate::Gate::start_bounded(
            &meta.model,
            &format!("{}/chat/completions", server.url),
            meta.input,
            meta.output,
            task,
        )
        .unwrap();
        let result = launch(
            &binary,
            &meta,
            "fixture-only",
            &gate.url,
            task,
            &AtomicBool::new(false),
        );
        let receipt = gate.receipt();
        result.unwrap();
        assert_eq!(receipt.unwrap().usage.input_tokens, 100);
        let requests = server.finish();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].body["max_tokens"], 32768);
        assert_eq!(requests[0].body["tools"], json!([]));
        assert_eq!(requests[0].body["messages"][1]["content"], task);
    }
}
