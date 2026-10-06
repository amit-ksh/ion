//! Bounded Kimi Code 2.1.1 native completion. Only Ion executes effects.
//! Uses the selected native direct coding account; OAuth needs its own transport.
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

const OUTPUT: u64 = 32_768;
const UPSTREAM: &str = "https://api.kimi.com/coding/v1";
struct Account {
    alias: String,
    id: String,
    input: u64,
    key: String,
}
fn account(model: &str) -> Result<Account> {
    let home = std::env::var_os("KIMI_CODE_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| dirs::home_dir().map(|p| p.join(".kimi-code")))
        .context("Native Kimi home is unavailable")?;
    let mut bytes = Vec::new();
    std::fs::File::open(home.join("config.toml"))
        .context("Configure the selected account in native Kimi Code first")?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 1024 * 1024,
        "Native Kimi configuration exceeds its bound"
    );
    let config: toml::Value = std::str::from_utf8(&bytes)
        .ok()
        .and_then(|text| toml::from_str(text).ok())
        .context("Native Kimi configuration is invalid; no completion was launched")?;
    from_config(&config, model, |name| std::env::var(name).ok())
}
fn from_config(
    config: &toml::Value,
    alias: &str,
    key_env: impl Fn(&str) -> Option<String>,
) -> Result<Account> {
    ensure!(
        !alias.is_empty() && alias.len() <= 160,
        "Choose an exact native Kimi model alias"
    );
    let model = config
        .get("models")
        .and_then(|v| v.get(alias))
        .context("Pinned Kimi model is absent from this host's native configuration")?;
    ensure!(
        ["overrides", "base_url", "protocol", "beta_api"]
            .iter()
            .all(|k| model.get(k).is_none()),
        "Kimi model overrides/custom routes need a separate verified Teams contract"
    );
    let id = model
        .get("model")
        .and_then(toml::Value::as_str)
        .context("Native Kimi model identity missing")?;
    ensure!(
        alias == format!("kimi-code/{id}"),
        "Kimi Teams requires an exact coding model identity; renamed aliases cannot change the pinned model"
    );
    // Published coding catalog defaults. Input is reserved conservatively in full;
    // the native Kimi provider is additionally clamped to OUTPUT before inference.
    let input = match id {
        "k3" => 1_048_576,
        "kimi-for-coding" | "kimi-for-coding-highspeed" => 262_144,
        _ => anyhow::bail!("This Kimi model needs a verified native Teams contract"),
    };
    ensure!(
        model
            .get("max_context_size")
            .and_then(toml::Value::as_integer)
            == Some(input as i64)
            && model.get("max_input_size").is_none()
            && model.get("max_output_size").is_none(),
        "Native Kimi context limits changed; no completion was launched"
    );
    let provider = model
        .get("provider")
        .and_then(toml::Value::as_str)
        .and_then(|name| config.get("providers").and_then(|v| v.get(name)))
        .context("Selected native Kimi provider missing")?;
    ensure!(
        provider.get("type").and_then(toml::Value::as_str) == Some("kimi")
            && provider
                .get("base_url")
                .and_then(toml::Value::as_str)
                .map(|s| s.trim_end_matches('/'))
                == Some(UPSTREAM),
        "Kimi Teams requires the selected native official coding route"
    );
    ensure!(
        provider.get("oauth").is_none()
            && provider.get("custom_headers").is_none()
            && provider.get("env").is_none(),
        "Kimi OAuth/custom authentication needs a separate verified Teams transport"
    );
    ensure!(
        provider.get("api_key").is_none_or(|v| v.as_str().is_some())
            && provider
                .get("api_key_env")
                .is_none_or(|v| v.as_str().is_some())
            && !(provider.get("api_key").is_some() && provider.get("api_key_env").is_some()),
        "Native Kimi authentication declarations are invalid or ambiguous"
    );
    let literal = provider
        .get("api_key")
        .and_then(toml::Value::as_str)
        .filter(|s| !s.is_empty());
    let env = provider.get("api_key_env").and_then(toml::Value::as_str);
    ensure!(
        literal.is_some() != env.is_some(),
        "Choose one native Kimi direct authentication source"
    );
    let key = if let Some(key) = literal {
        key.to_owned()
    } else {
        let name = env.context("Native Kimi direct API login missing")?;
        ensure!(
            !name.is_empty()
                && name.len() <= 160
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
            "Native Kimi key environment name is invalid"
        );
        key_env(name).context("Selected native Kimi key environment variable is unavailable")?
    };
    ensure!(
        !key.is_empty()
            && key.len() <= 16 * 1024
            && !key.starts_with(['!', '$'])
            && !key.chars().any(char::is_whitespace),
        "Kimi Teams requires a literal native direct API key"
    );
    Ok(Account {
        alias: alias.into(),
        id: id.into(),
        input,
        key,
    })
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
    cmd.current_dir(root.join("work"))
        .env("HOME", root.join("home"))
        .env("USERPROFILE", root.join("home"))
        .env("KIMI_CODE_HOME", root.join("home"));
    for (name, value) in [
        ("KIMI_DISABLE_TELEMETRY", "1"),
        ("KIMI_CODE_NO_AUTO_UPDATE", "1"),
        ("KIMI_LOG_LEVEL", "off"),
        ("KIMI_DISABLE_CRON", "1"),
        ("KIMI_CODE_WATCH", "0"),
        ("KIMI_CODE_SEARCH_WORKER", "0"),
        ("KIMI_CODE_PERSISTENCE_MINIDB_READMODEL", "0"),
        ("KIMI_CODE_INFINITE_RETRY", "0"),
        ("KIMI_CODE_BUILTIN_PRODUCT_SKILLS", "0"),
        ("KIMI_LOOP_MAX_STEPS_PER_TURN", "1"),
        ("KIMI_LOOP_MAX_ATTEMPTS_PER_STEP", "1"),
        ("KIMI_MODEL_MAX_COMPLETION_TOKENS", "32768"),
        ("KIMI_CODE_BACKGROUND_PRINT_BACKGROUND_MODE", "exit"),
    ] {
        cmd.env(name, value);
    }
    cmd
}
struct Session {
    root: EmptyDirectory,
}
impl Session {
    fn create(account: &Account, url: &str, task: &str) -> Result<Self> {
        ensure!(task.len() <= 128 * 1024, "Kimi context exceeds its bound");
        let root = EmptyDirectory::create("kimi")?;
        for name in ["home", "work", "skills"] {
            std::fs::create_dir(root.0.join(name))?;
        }
        // All native project-root/MCP discovery stops at this private marker,
        // even when the OS temp directory is under a user's Git checkout.
        std::fs::create_dir(root.0.join("work/.git"))?;
        let cfg = json!({"default_model":account.alias,"telemetry":false,"auto_session_title":false,
            "merge_all_available_skills":false,"builtin_product_skills":false,
            "providers":{"ion-selected":{"type":"kimi","base_url":url,"api_key_env":"ION_KIMI_NATIVE_KEY"}},
            "models":{account.alias.clone():{"provider":"ion-selected","model":account.id,"max_context_size":account.input,"capabilities":[]}},
            "thinking":{"enabled":false}, "loop_control":{"max_steps_per_turn":1,"max_attempts_per_step":1,"compaction_max_attempts":1},
            "background":{"keep_alive_on_exit":false,"print_background_mode":"exit"}, "watch":{"enabled":false}});
        let config = toml::to_string(&cfg)?;
        // JSON escapes prevent Kimi's prompt templating from interpreting any
        // dollar-prefixed input (including workspace variables) as live context.
        let task = task.replace('$', "\\u0024");
        ensure!(
            task.len() <= 128 * 1024,
            "Escaped Kimi context exceeds its bound"
        );
        let agent = format!(
            "---\nname: ion-teams\ndescription: Complete one bounded Ion Teams request\ntools: []\nsubagents: []\n---\n{}\n\nInput JSON:\n{task}\n",
            team_completion::INSTRUCTIONS
        );
        for (path, bytes) in [
            (root.0.join("home/config.toml"), config.as_bytes()),
            (root.0.join("agent.md"), agent.as_bytes()),
        ] {
            let mut opts = std::fs::OpenOptions::new();
            opts.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                opts.mode(0o600);
            }
            opts.open(path)?.write_all(bytes)?;
        }
        Ok(Self { root })
    }
    fn command(&self, binary: &Path) -> Command {
        isolated_command(binary, &self.root.0)
    }
    fn complete(
        &self,
        binary: &Path,
        account: &Account,
        cancelled: &AtomicBool,
    ) -> Result<Vec<u8>> {
        self.verify(binary, cancelled)?;
        let mut cmd = self.command(binary);
        // Only this isolated native process receives the selected host's account.
        cmd.env("ION_KIMI_NATIVE_KEY", &account.key)
            .args(["--model", &account.alias])
            .arg("--agent-file")
            .arg(self.root.0.join("agent.md"))
            .arg("--skills-dir")
            .arg(self.root.0.join("skills"))
            .args([
                "--prompt",
                "Complete the supplied Ion Teams input.",
                "--output-format",
                "stream-json",
            ]);
        bounded_process(&mut cmd, b"", cancelled, Duration::from_secs(120))
    }
    fn verify(&self, binary: &Path, cancelled: &AtomicBool) -> Result<()> {
        let mut cmd = self.command(binary);
        cmd.arg("--version");
        let bytes = bounded_process(&mut cmd, b"", cancelled, Duration::from_secs(30))?;
        ensure!(
            std::str::from_utf8(&bytes)?.trim() == "2.1.1",
            "Kimi Teams requires the verified native Code 2.1.1 runtime; this version needs a contract check"
        );
        let mut cmd = self.command(binary);
        cmd.args(["doctor", "config"]);
        // Native schema validation does not bootstrap a session or request a model.
        bounded_process(&mut cmd, b"", cancelled, Duration::from_secs(30))?;
        Ok(())
    }
}
pub(crate) fn ready(binary: &Path, model: &str) -> Result<()> {
    let account = account(model)?;
    Session::create(&account, "http://127.0.0.1:1/ion/probe", "{}")?
        .verify(binary, &AtomicBool::new(false))
}
fn allowance(model: &str) -> Result<ModelCallAllowance> {
    let account = account(model)?;
    Ok(ModelCallAllowance {
        model: model.into(),
        max_input_tokens: account.input,
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
        ProviderKind::Kimi,
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
    account: &Account,
    url: &str,
    system: &str,
    messages: &[Value],
    tools: &Value,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>> {
    let task = json!({"instructions":system,"messages":messages,"tools":tools}).to_string();
    let session = Session::create(account, url, &task)?;
    session.complete(binary, account, cancelled)
}
fn completion(
    binary: &Path,
    model: &str,
    system: &str,
    messages: &[Value],
    tools: &Value,
    cancelled: &AtomicBool,
) -> Result<super::nebius::StreamCompletion> {
    let account = account(model)?;
    let gate = team_gate::Gate::kimi(&account.id, OUTPUT)?;
    let result = launch(
        binary, &account, &gate.url, system, messages, tools, cancelled,
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
    let actual = allowance(model)?;
    ensure!(
        actual.max_input_tokens <= input_cap && OUTPUT <= output_cap,
        "Registered Kimi allowance is too small"
    );
    let binary = crate::model::provider_probe(ProviderKind::Kimi, None)
        .path
        .context("Kimi worker executable missing")?;
    completion(&binary, model, system, messages, tools, cancelled)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> toml::Value {
        toml::from_str(
            r#"[providers.native]
type = "kimi"
base_url = "https://api.kimi.com/coding/v1"
api_key = "fixture-only-not-real"
[models."kimi-code/k3"]
provider = "native"
model = "k3"
max_context_size = 1048576
"#,
        )
        .unwrap()
    }
    #[test]
    fn team_kimi_account_rejects_changed_identity_limits_routes_and_ambiguous_auth() {
        let cfg = config();
        let account = from_config(&cfg, "kimi-code/k3", |_| None).unwrap();
        assert_eq!(account.input, 1048576);
        assert_eq!(account.id, "k3");
        assert!(from_config(&cfg, "k3", |_| None).is_err());
        for (group, key, value) in [
            ("provider", "type", json!("openai")),
            ("provider", "base_url", json!("https://custom.example/v1")),
            (
                "provider",
                "oauth",
                json!({"storage":"file","key":"native"}),
            ),
            ("provider", "api_key_env", json!("ANOTHER_KEY")),
            ("provider", "custom_headers", json!({"X-Account":"other"})),
            ("provider", "env", json!({"KIMI_API_KEY":"other"})),
            ("provider", "api_key", json!("$DYNAMIC_KEY")),
            ("model", "model", json!("replacement")),
            ("model", "max_context_size", json!(999999)),
            ("model", "overrides", json!({"max_context_size":100})),
            ("model", "base_url", json!("https://custom.example/v1")),
        ] {
            let mut cfg = cfg.clone();
            let target = if group == "provider" {
                &mut cfg["providers"]["native"]
            } else {
                &mut cfg["models"]["kimi-code/k3"]
            };
            let v: toml::Value = serde_json::from_value(value).unwrap();
            target.as_table_mut().unwrap().insert(key.into(), v);
            assert!(
                from_config(&cfg, "kimi-code/k3", |_| Some("fixture-only".into())).is_err(),
                "accepted {group}/{key}"
            );
        }
        let mut renamed = cfg.clone();
        renamed["models"]["kimi-code/k3"]["model"] = toml::Value::String("kimi-for-coding".into());
        renamed["models"]["kimi-code/k3"]["max_context_size"] = toml::Value::Integer(262144);
        assert!(from_config(&renamed, "kimi-code/k3", |_| None).is_err());
        let mut env = cfg.clone();
        let provider = env["providers"]["native"].as_table_mut().unwrap();
        provider.remove("api_key");
        provider.insert(
            "api_key_env".into(),
            toml::Value::String("NATIVE_KIMI_TEST_KEY".into()),
        );
        assert!(from_config(&env, "kimi-code/k3", |_| None).is_err());
        assert!(
            from_config(&env, "kimi-code/k3", |name| (name
                == "NATIVE_KIMI_TEST_KEY")
                .then(|| "fixture-only".into()))
            .is_ok()
        );
    }
    #[test]
    fn team_kimi_session_is_private_tool_free_and_keeps_input_literal() {
        let account = from_config(&config(), "kimi-code/k3", |_| None).unwrap();
        let task = json!({"instructions":"keep ${cwd} and $HOME literal"}).to_string();
        let session = Session::create(&account, "http://127.0.0.1:1/ion/test", &task).unwrap();
        let root = session.root.0.clone();
        let cfg = std::fs::read_to_string(root.join("home/config.toml")).unwrap();
        assert!(!cfg.contains(&account.key));
        assert!(cfg.contains("api_key_env"));
        let agent = std::fs::read_to_string(root.join("agent.md")).unwrap();
        assert!(agent.contains("tools: []\nsubagents: []"));
        assert!(root.join("work/.git").is_dir());
        assert!(!agent.contains("${cwd}"));
        let restored: Value =
            serde_json::from_str(agent.split("Input JSON:\n").nth(1).unwrap().trim()).unwrap();
        assert_eq!(restored["instructions"], "keep ${cwd} and $HOME literal");
        let cmd = session.command(Path::new("fixture-kimi"));
        assert!(!cmd.get_envs().any(|(k, _)| matches!(
            k.to_string_lossy().as_ref(),
            "ION_KIMI_NATIVE_KEY"
                | "NODE_OPTIONS"
                | "KIMI_MODEL_NAME"
                | "KIMI_CODE_CUSTOM_HEADERS"
                | "HTTPS_PROXY"
        )));
        assert!(Session::create(&account, "unused", &"x".repeat(128 * 1024 + 1)).is_err());
        assert!(
            Session::create(
                &account,
                "unused",
                &json!({"instructions":"$".repeat(25000)}).to_string()
            )
            .is_err()
        );
        drop(session);
        assert!(!root.exists());
    }
    #[test]
    #[ignore = "Requires verified Kimi Code executable; loopback and fixture account only"]
    fn team_kimi_native_runtime_completes_one_metered_call_without_effects() {
        let _diagnostics = crate::team_process::fixture_diagnostics();
        let binary = std::env::var_os("ION_TEST_KIMI_BINARY")
            .map(std::path::PathBuf::from)
            .or_else(|| crate::model::provider_probe(ProviderKind::Kimi, None).path)
            .expect("Set ION_TEST_KIMI_BINARY");
        let server = super::super::team_test_http::Server::with_timeout(
            vec![(
                200,
                super::super::team_test_http::chat("k3", r#"{"text":"ready","calls":[]}"#),
            )],
            Duration::from_secs(120),
        );
        let gate = team_gate::Gate::start_with_output_limit(
            "k3",
            &format!("{}/chat/completions", server.url),
            Some(OUTPUT),
        )
        .unwrap();
        let account = from_config(&config(), "kimi-code/k3", |_| None).unwrap();
        let session = Session::create(
            &account,
            &gate.url,
            &json!({"instructions":"Return Ion JSON and retain ${cwd}","messages":[],"tools":[]})
                .to_string(),
        )
        .unwrap();
        std::fs::create_dir(session.root.0.join(".git")).unwrap();
        std::fs::create_dir(session.root.0.join(".kimi-code")).unwrap();
        // This invalid ancestor configuration would fail native bootstrap if
        // project discovery escaped the scratch workspace's .git marker.
        std::fs::write(session.root.0.join(".kimi-code/local.toml"), "[broken").unwrap();
        let result = session.complete(&binary, &account, &AtomicBool::new(false));
        let receipt = gate.receipt();
        result.unwrap();
        let receipt = receipt.unwrap();
        assert_eq!(receipt.usage.input_tokens, 100);
        assert_eq!(receipt.usage.output_tokens, 5);
        let requests = server.finish();
        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert_eq!(request.body["model"], "k3");
        assert_eq!(request.body["tools"], json!([]));
        assert_eq!(request.body["max_completion_tokens"], OUTPUT);
        assert!(
            request
                .headers
                .to_ascii_lowercase()
                .contains("authorization: bearer fixture-only-not-real")
        );
        assert!(
            team_completion::actions(&receipt.text, &json!([]), receipt.usage)
                .unwrap()
                .calls
                .is_empty()
        );
    }
}
