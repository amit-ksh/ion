//! OpenCode 2's verified private standalone completion contract.
//! Native credentials are read only; adopted services are never changed.
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
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::AtomicBool,
    time::Duration,
};
use waku_protocol::model::{DriverEvent, ProviderKind};
const VERSION: &str = "opencode2 v0.0.0-beta-17823";
const INPUT: u64 = 272_000;
const OUTPUT: u64 = 32_768;
const UPSTREAM: &str = "https://api.openai.com/v1";
fn model_id(model: &str) -> Result<&str> {
    ensure!(
        model == "openai/gpt-5.4",
        "OpenCode 2 Teams requires the verified exact openai/gpt-5.4 model; another model needs a native contract check"
    );
    Ok("gpt-5.4")
}
fn key(value: Value) -> Result<String> {
    ensure!(
        value["type"] == "key",
        "OpenCode 2 OAuth accounts need a separate verified Teams transport"
    );
    let key = value["key"]
        .as_str()
        .context("Native OpenCode 2 API key missing")?;
    ensure!(
        !key.is_empty()
            && key.len() <= 16 * 1024
            && !key.chars().any(char::is_whitespace)
            && !key.starts_with(['!', '$']),
        "Native OpenCode 2 API key is invalid"
    );
    Ok(key.into())
}
fn account(model: &str) -> Result<String> {
    model_id(model)?;
    ensure!(
        [
            "OPENCODE_CONFIG",
            "OPENCODE_CONFIG_CONTENT",
            "OPENCODE_DB",
            "OPENCODE_AUTH_CONTENT",
            "OPENCODE_MODELS_URL",
            "OPENCODE_MODELS_PATH"
        ]
        .iter()
        .all(|n| std::env::var_os(n).is_none_or(|v| v.is_empty())),
        "OpenCode 2 custom profiles/catalogs need a separate native contract check"
    );
    let home = dirs::home_dir().context("Native home unavailable")?;
    let config = std::env::var_os("OPENCODE_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".config"))
                .join("opencode")
        });
    // Do not silently bypass an account/endpoint declared in global config.
    for name in ["opencode.json", "opencode.jsonc"] {
        let path = config.join(name);
        if !path.exists() {
            continue;
        }
        let mut bytes = Vec::new();
        std::fs::File::open(path)?
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() <= 1024 * 1024,
            "Native config exceeds its bound"
        );
        let value: Value = serde_json::from_slice(&bytes).map_err(|_| {
            anyhow::anyhow!("OpenCode 2 JSONC/custom config requires a native contract check")
        })?;
        ensure!(
            value.get("provider").is_none()
                && value.get("providers").is_none()
                && value.get("enterprise").is_none(),
            "OpenCode 2 configured providers/enterprise routes require a native contract check"
        );
    }
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share"))
        .join("opencode");
    let db = data.join("opencode.db");
    let mut stored = None;
    if db.exists() {
        let connection = rusqlite::Connection::open_with_flags(
            db,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .context("Native OpenCode 2 credential store unavailable")?;
        connection.busy_timeout(Duration::from_secs(2))?;
        let mut statement = connection
            .prepare("SELECT value FROM credential WHERE integration_id = 'openai' LIMIT 2")
            .context("Native OpenCode 2 credential schema changed")?;
        let rows = statement
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        ensure!(
            rows.len() <= 1,
            "Native OpenCode 2 account selection is ambiguous"
        );
        if let Some(value) = rows.first() {
            ensure!(
                value.len() <= 32 * 1024,
                "Native credential exceeds its bound"
            );
            stored = Some(key(serde_json::from_str(value)
                .map_err(|_| anyhow::anyhow!("Native credential is invalid"))?)?);
        }
    }
    // Native environment credentials are supported only when the stored selection
    // agrees, so a cached OAuth account cannot be replaced by an unrelated key.
    let env = std::env::var("OPENAI_API_KEY")
        .ok()
        .filter(|k| !k.is_empty());
    if let (Some(stored), Some(env)) = (&stored, &env) {
        ensure!(
            stored == env,
            "OpenCode 2 environment and saved accounts differ"
        );
    }
    let chosen = stored
        .or(env)
        .context("Configure this host's native OpenCode 2 direct OpenAI login first")?;
    key(json!({"type":"key","key":chosen}))
}
struct Session {
    root: EmptyDirectory,
}
impl Session {
    fn create(url: &str) -> Result<Self> {
        let root = EmptyDirectory::create("opencode2")?;
        for dir in [
            "home",
            "work",
            "work/.git",
            "tmp",
            "config",
            "data",
            "cache",
            "state",
        ] {
            std::fs::create_dir(root.0.join(dir))?;
        }
        // No user/worktree discovery and no global process registration.
        let config = json!({"share":"disabled","autoupdate":false,"plugin":[],"mcp":{},"permission":{"*":"deny"},"compaction":{"auto":false,"prune":false},"agent":{"title":{"disable":true},"summary":{"disable":true},"compaction":{"disable":true},"ion-team":{"mode":"primary","prompt":team_completion::INSTRUCTIONS,"model":"openai/gpt-5.4","steps":1,"permission":{"*":"deny"}}},"provider":{"openai":{"options":{"baseURL":url,"apiKey":"{env:ION_OPENCODE2_NATIVE_KEY}"},"models":{"gpt-5.4":{"id":"gpt-5.4","limit":{"context":INPUT,"output":OUTPUT}}}}}});
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options
            .open(root.0.join("config/opencode.json"))?
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
            ("OPENCODE_TEST_HOME", "home"),
            ("XDG_CONFIG_HOME", "config"),
            ("XDG_DATA_HOME", "data"),
            ("XDG_CACHE_HOME", "cache"),
            ("XDG_STATE_HOME", "state"),
            ("OPENCODE_CONFIG_DIR", "config"),
            ("TEMP", "tmp"),
            ("TMP", "tmp"),
            ("TMPDIR", "tmp"),
        ] {
            cmd.env(name, self.root.0.join(dir));
        }
        for name in [
            "OPENCODE_DISABLE_AUTOUPDATE",
            "OPENCODE_DISABLE_MODELS_FETCH",
            "OPENCODE_DISABLE_PROJECT_CONFIG",
            "OPENCODE_DISABLE_FILEWATCHER",
            "DO_NOT_TRACK",
        ] {
            cmd.env(name, "true");
        }
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
            std::str::from_utf8(&out)?.trim() == VERSION,
            "OpenCode 2 native version needs a Teams contract check"
        );
        let out = bounded_process(
            self.command(binary).args(["run", "--help"]),
            b"",
            cancelled,
            Duration::from_secs(30),
        )?;
        ensure!(
            std::str::from_utf8(&out)?.contains("--standalone"),
            "OpenCode 2 standalone execution is unavailable"
        );
        Ok(())
    }
}
pub(crate) fn ready(binary: &Path, model: &str) -> Result<()> {
    account(model)?;
    Session::create("http://127.0.0.1:1/ion/probe")?.verify(binary, &AtomicBool::new(false))
}
fn allowance(model: &str) -> Result<ModelCallAllowance> {
    model_id(model)?;
    Ok(ModelCallAllowance {
        model: model.into(),
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
        ProviderKind::OpenCode2,
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
    key: &str,
    url: &str,
    task: &str,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>> {
    let session = Session::create(url)?;
    session.verify(binary, cancelled)?;
    bounded_process(
        session
            .command(binary)
            .env("ION_OPENCODE2_NATIVE_KEY", key)
            .env("OPENAI_API_KEY", key)
            .args([
                "run",
                "--standalone",
                "--format",
                "json",
                "--model",
                "openai/gpt-5.4",
                "--agent",
                "ion-team",
                "--title",
                "Ion Teams completion",
            ]),
        task.as_bytes(),
        cancelled,
        Duration::from_secs(120),
    )
}
fn completion(
    binary: &Path,
    model: &str,
    system: &str,
    messages: &[Value],
    tools: &Value,
    cancelled: &AtomicBool,
) -> Result<super::nebius::StreamCompletion> {
    let key = account(model)?;
    let task = json!({"instructions":system,"messages":messages,"tools":tools}).to_string();
    let gate = team_gate::Gate::bounded_api(model_id(model)?, UPSTREAM, INPUT, OUTPUT, &task)?;
    let result = launch(binary, &key, &gate.url, &task, cancelled);
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
    allowance(model)?;
    ensure!(
        INPUT <= input_cap && OUTPUT <= output_cap,
        "Registered OpenCode 2 allowance is too small"
    );
    let binary = crate::model::provider_probe(ProviderKind::OpenCode2, None)
        .path
        .context("OpenCode 2 worker executable missing")?;
    completion(&binary, model, system, messages, tools, cancelled)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn team_opencode2_rejects_alias_and_oauth_accounts() {
        assert!(model_id("openai/gpt-5").is_err());
        assert!(key(json!({"type":"oauth","access":"fixture"})).is_err());
        assert!(key(json!({"type":"key","key":"a b"})).is_err());
        assert_eq!(
            key(json!({"type":"key","key":"fixture-only"})).unwrap(),
            "fixture-only"
        );
    }
    #[test]
    #[ignore = "Requires verified OpenCode 2 binary; only fake key and localhost provider"]
    fn team_opencode2_installed_standalone_has_one_bounded_request() {
        let _diagnostics = crate::team_process::fixture_diagnostics();
        let binary = std::env::var_os("ION_TEST_OPENCODE2_BINARY")
            .map(PathBuf::from)
            .unwrap();
        let server = super::super::team_test_http::Server::with_timeout(
            vec![(
                200,
                super::super::team_test_http::responses(
                    "gpt-5.4",
                    r#"{"text":"ready","calls":[]}"#,
                ),
            )],
            Duration::from_secs(100),
        );
        let task = r#"{"instructions":"fixture","messages":[],"tools":[]}"#;
        let gate = team_gate::Gate::start_bounded(
            "gpt-5.4",
            &format!("{}/responses", server.url),
            INPUT,
            OUTPUT,
            task,
        )
        .unwrap();
        let result = launch(
            &binary,
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
        assert_eq!(requests[0].body["max_output_tokens"], OUTPUT);
        assert_eq!(requests[0].body["tools"], json!([]));
        assert_eq!(requests[0].body["input"][1]["content"], task);
        assert_eq!(requests[0].body["input"].as_array().unwrap().len(), 2);
    }
}
