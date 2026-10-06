//! Bounded native DeepSeek Harness composition. Ion owns all effects.
//! Supports the verified 0.1.5-rc.1 runtime and direct official API login.
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
use std::{io::Write, path::Path, process::Command, sync::atomic::AtomicBool, time::Duration};
use waku_protocol::model::{DriverEvent, ProviderKind};

// Native release defaults; both are pinned explicitly in the private composition.
// Versions/models with unverified limits cannot inherit these values silently.
const INPUT: u64 = 1_000_000;
const OUTPUT: u64 = 256_000;
const PROFILE: &str = "ion-teams";
fn model_id(model: &str) -> Result<&str> {
    let id = model
        .strip_prefix("deepseek-official/")
        .context("Choose an exact DeepSeek official provider/model ID")?;
    ensure!(
        matches!(id, "deepseek-v4-flash" | "deepseek-v4-pro"),
        "DeepSeek Teams requires a verified native V4 text model; aliases/custom routes are unsupported"
    );
    Ok(id)
}
fn api_key() -> Result<String> {
    let base = std::env::var("DEEPSEEK_BASE_URL").unwrap_or_default();
    ensure!(
        base.is_empty()
            || matches!(
                base.trim_end_matches('/'),
                "https://api.deepseek.com" | "https://api.deepseek.com/v1"
            ),
        "DeepSeek Teams does not substitute a custom native endpoint"
    );
    let key = std::env::var("DEEPSEEK_API_KEY").context("Configure this host's native DEEPSEEK_API_KEY for Teams; Web-managed accounts need a separate verified transport")?;
    ensure!(
        !key.is_empty()
            && key.len() <= 16 * 1024
            && !key.starts_with(['!', '$'])
            && !key.chars().any(char::is_whitespace),
        "DeepSeek Teams requires a literal native API key"
    );
    Ok(key)
}
fn isolated_command(binary: &Path, root: &Path) -> Command {
    let mut cmd = crate::command_env::command(binary);
    let runtime: Vec<_> = cmd
        .get_envs()
        .filter_map(|(k, v)| v.map(|v| (k.to_owned(), v.to_owned())))
        .collect();
    let allowed = |k: &std::ffi::OsStr| {
        matches!(
            k.to_string_lossy().to_ascii_uppercase().as_str(),
            "PATH"
                | "SYSTEMROOT"
                | "WINDIR"
                | "COMSPEC"
                | "PATHEXT"
                | "TEMP"
                | "TMP"
                | "TMPDIR"
                | "LANG"
                | "LC_ALL"
        )
    };
    cmd.env_clear()
        .envs(std::env::vars_os().filter(|(k, _)| allowed(k)))
        .envs(runtime.into_iter().filter(|(k, _)| allowed(k)));
    cmd.env("HOME", root.join("home"))
        .env("USERPROFILE", root.join("home"))
        .env("DSH_HOME", root.join("home"))
        .current_dir(root.join("work"));
    cmd
}
struct Session {
    root: EmptyDirectory,
    rows: Value,
}
impl Session {
    fn create(model: &str, url: &str, task: &str) -> Result<Self> {
        let id = model_id(model)?;
        ensure!(
            task.len() <= 128 * 1024,
            "DeepSeek context exceeds its bound"
        );
        let root = EmptyDirectory::create("deepseek")?;
        let home = root.0.join("home");
        let profile = home.join("profiles").join(PROFILE);
        std::fs::create_dir_all(&profile)?;
        std::fs::create_dir(root.0.join("work"))?;
        let rows = json!([
            {"id":"timer","name":"@deepseek-ai/cordis-plugin-timer"},
            {"id":"llm","name":"@deepseek-ai/dsh-llm"},
            {"id":"session","name":"@deepseek-ai/dsh-session"},
            {"id":"session-projection","name":"@deepseek-ai/dsh-session-projection"},
            {"id":"session-title","name":"@deepseek-ai/dsh-session-title","config":{"fallbackMaxWords":5,"fallbackMaxBytes":40,"maxTitleBytes":80}},
            {"id":"tools","name":"@deepseek-ai/dsh-tools"},
            {"id":"agent","name":"@deepseek-ai/dsh-agent"},
            {"id":"jobs","name":"@deepseek-ai/dsh-jobs-local"},
            {"id":"system-prompt","name":"@deepseek-ai/dsh-system-prompt","config":{"includeHarnessIdentity":false,"includeRuntimeContext":false,"personaPrefix":team_completion::INSTRUCTIONS}},
            {"id":"agent-loop","name":"@deepseek-ai/dsh-agent-loop","config":{"agents":[]}},
            {"id":"llm-deepseek","name":"@deepseek-ai/dsh-llm-deepseek","config":{"apiKeyEnv":"DEEPSEEK_API_KEY","baseURL":url,"thinking":"disabled","maxTokens":OUTPUT,"defaultContextWindow":INPUT,"streamIdleTimeoutMs":90000,"retryPolicy":{"mode":"normal","maxRetries":0}}},
            {"id":"agent-default-model","name":"@deepseek-ai/dsh-agent-default-model","config":{"provider":"deepseek-official","model":id}},
            {"id":"sessions","name":"@deepseek-ai/dsh-session-persistence-jsonl","config":{"root":home.join("sessions"),"compression":"none"}},
            {"id":"headless-startup","name":"@deepseek-ai/dsh-headless/startup"},
            {"id":"headless-runner","name":"@deepseek-ai/dsh-headless","inject":["headlessStartup"],"config":{"task":task}}
        ]);
        for (name, data) in [
            (
                "package.json",
                json!({"name":"ion-teams-private","private":true,"dependencies":{},"dsh":{"profile":{"bundles":[],"patchReload":"startup"}}}),
            ),
            ("cordis.patch.yml", json!([{"insert":rows.clone()}])),
        ] {
            let mut opts = std::fs::OpenOptions::new();
            opts.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                opts.mode(0o600);
            }
            opts.open(profile.join(name))?
                .write_all(&serde_json::to_vec(&data)?)?;
        }
        Ok(Self { root, rows })
    }
    fn command(&self, binary: &Path) -> Command {
        let mut cmd = isolated_command(binary, &self.root.0);
        cmd.args(["--profile", PROFILE]);
        cmd
    }
    fn verify(&self, binary: &Path, cancelled: &AtomicBool) -> Result<()> {
        let mut cmd = isolated_command(binary, &self.root.0);
        cmd.arg("--version");
        let version = bounded_process(&mut cmd, b"", cancelled, Duration::from_secs(30))?;
        ensure!(
            std::str::from_utf8(&version)?.trim() == "0.1.5-rc.1",
            "DeepSeek Teams requires the verified native 0.1.5-rc.1 runtime; this version needs a contract check"
        );
        let mut cmd = self.command(binary);
        cmd.arg("--dump-config");
        // This native operation resolves the composition without booting it.
        // No prompt, native account or provider request reaches this probe.
        let dump = bounded_process(&mut cmd, b"", cancelled, Duration::from_secs(30))?;
        let actual: Value = serde_saphyr::from_str(std::str::from_utf8(&dump)?)?;
        ensure!(
            actual == self.rows,
            "DeepSeek native composition changed; no completion was launched"
        );
        Ok(())
    }
}
pub(crate) fn ready(binary: &Path, model: &str) -> Result<()> {
    api_key()?;
    Session::create(
        model,
        "http://127.0.0.1:1/ion/probe",
        "Read-only composition probe",
    )?
    .verify(binary, &AtomicBool::new(false))
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
        ProviderKind::DeepSeek,
        |model| {
            model_id(model)?;
            Ok(ModelCallAllowance {
                model: model.into(),
                max_input_tokens: INPUT,
                max_output_tokens: OUTPUT,
            })
        },
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
    model: &str,
    key: &str,
    url: &str,
    system: &str,
    messages: &[Value],
    tools: &Value,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>> {
    let task = json!({"instructions":system,"messages":messages,"tools":tools}).to_string();
    let session = Session::create(model, url, &task)?;
    session.verify(binary, cancelled)?;
    let mut cmd = session.command(binary);
    // Only the private native process receives its selected host-owned login.
    // The generated composition/task files contain no authentication material.
    cmd.env("DEEPSEEK_API_KEY", key).arg("Ion Teams completion");
    bounded_process(&mut cmd, b"", cancelled, Duration::from_secs(120))
}
fn completion(
    binary: &Path,
    model: &str,
    system: &str,
    messages: &[Value],
    tools: &Value,
    cancelled: &AtomicBool,
) -> Result<super::nebius::StreamCompletion> {
    let id = model_id(model)?;
    let key = api_key()?;
    let gate = team_gate::Gate::deepseek(id)?;
    let result = launch(
        binary, model, &key, &gate.url, system, messages, tools, cancelled,
    );
    let receipt = gate.receipt();
    result?;
    let receipt = receipt?;
    team_completion::actions(&receipt.text, tools, receipt.usage)
}
pub(super) fn worker_completion(
    model: &str,
    system: &str,
    messages: &[Value],
    tools: &Value,
    cancelled: &AtomicBool,
    input_cap: u64,
    output_cap: u64,
) -> Result<super::nebius::StreamCompletion> {
    model_id(model)?;
    ensure!(
        INPUT <= input_cap && OUTPUT <= output_cap,
        "Registered DeepSeek allowance is too small"
    );
    let binary = crate::model::provider_probe(ProviderKind::DeepSeek, None)
        .path
        .context("DeepSeek worker executable missing")?;
    completion(&binary, model, system, messages, tools, cancelled)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    #[test]
    fn team_deepseek_composition_is_private_and_has_no_native_effect_plugins() {
        let session = Session::create(
            "deepseek-official/deepseek-v4-flash",
            "http://127.0.0.1:1/ion/test",
            "fixture",
        )
        .unwrap();
        let root = session.root.0.clone();
        let names = session
            .rows
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(names.len(), 15);
        assert!(!names.iter().any(|n| n.contains("sandbox")
            || n.contains("subprocess")
            || n.contains("mcp")
            || n.contains("tool-")
            || n.contains("compaction")
            || n.contains("retry")
            || n.contains("credentials")
            || n.contains("settings")));
        let cmd = session.command(Path::new("fixture-dsh"));
        assert!(!cmd.get_envs().any(|(k, _)| matches!(
            k.to_string_lossy().as_ref(),
            "DEEPSEEK_API_KEY" | "DEEPSEEK_BASE_URL" | "NODE_OPTIONS" | "DSH_PROFILE"
        )));
        let cfg = &session
            .rows
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == "llm-deepseek")
            .unwrap()["config"];
        assert_eq!(cfg["maxTokens"], OUTPUT);
        assert_eq!(cfg["retryPolicy"]["maxRetries"], 0);
        drop(session);
        assert!(!root.exists());
    }
    #[test]
    fn team_deepseek_rejects_unverified_ids_and_oversized_context_before_a_process() {
        for model in [
            "deepseek-v4-flash",
            "deepseek-official/deepseek-chat",
            "deepseek-official/deepseek-flash",
            "custom/deepseek-v4-pro",
            "",
        ] {
            assert!(model_id(model).is_err());
        }
        assert!(
            Session::create(
                "deepseek-official/deepseek-v4-pro",
                "unused",
                &"x".repeat(128 * 1024 + 1)
            )
            .is_err()
        );
    }
    #[test]
    #[ignore = "Requires verified DeepSeek executable; loopback and fixture account only"]
    fn team_deepseek_native_runtime_completes_one_metered_call_with_no_effects() {
        let _diagnostics = crate::team_process::fixture_diagnostics();
        let binary = std::env::var_os("ION_TEST_DEEPSEEK_BINARY")
            .map(PathBuf::from)
            .or_else(|| crate::model::provider_probe(ProviderKind::DeepSeek, None).path)
            .expect("Set ION_TEST_DEEPSEEK_BINARY");
        let server = super::super::team_test_http::Server::with_timeout(
            vec![(
                200,
                super::super::team_test_http::chat(
                    "deepseek-v4-flash",
                    r#"{"text":"ready","calls":[]}"#,
                ),
            )],
            Duration::from_secs(120),
        );
        let gate = team_gate::Gate::start(
            "deepseek-v4-flash",
            &format!("{}/chat/completions", server.url),
        )
        .unwrap();
        let result = launch(
            &binary,
            "deepseek-official/deepseek-v4-flash",
            "sk-fixture-only",
            &gate.url,
            "Return Ion JSON",
            &[],
            &json!([]),
            &AtomicBool::new(false),
        );
        let receipt = gate.receipt();
        result.unwrap_or_else(|e| {
            panic!(
                "DeepSeek fixture failed: {e:#}; gate={}",
                receipt
                    .as_ref()
                    .err()
                    .map(|e| e.to_string())
                    .unwrap_or_default()
            )
        });
        let receipt = receipt.unwrap();
        assert_eq!(receipt.usage.input_tokens, 100);
        assert_eq!(receipt.usage.output_tokens, 5);
        let requests = server.finish();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].body["model"], "deepseek-v4-flash");
        assert_eq!(requests[0].body["tools"], json!([]));
        assert_eq!(requests[0].body["max_tokens"], OUTPUT);
        assert!(
            requests[0]
                .headers
                .to_ascii_lowercase()
                .contains("authorization: bearer sk-fixture-only")
        );
        assert!(
            team_completion::actions(&receipt.text, &json!([]), receipt.usage)
                .unwrap()
                .calls
                .is_empty()
        );
    }
}
