//! Native Grok completion with a prompt-bound, one-use Responses gate.
//! Ion owns effects. Direct native API-key accounts only; no OAuth substitution.
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
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::AtomicBool,
    time::Duration,
};
use waku_protocol::model::{DriverEvent, ProviderKind};
const INPUT: u64 = 500_000;
const OUTPUT: u64 = 32_768;
const VERSION: &str = "1.0.46";
const UPSTREAM: &str = "https://api.x.ai/v1";
fn model_id(model: &str) -> Result<&str> {
    ensure!(
        matches!(model, "grok-4.6" | "grok-4.5"),
        "Grok Teams requires an exact verified native model; other models/aliases need a contract check"
    );
    Ok(model)
}
fn home() -> Result<PathBuf> {
    std::env::var_os("GROK_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".grok")))
        .context("Native Grok home unavailable")
}
fn from_config(config: &toml::Value, model: &str, key: String) -> Result<String> {
    model_id(model)?;
    ensure!(
        config.get("model").is_none()
            && config.get("auth_providers").is_none()
            && config.get("auth_provider").is_none()
            && config.get("auth").is_none()
            && config.get("grok_com_config").is_none(),
        "Grok native custom model/authentication needs a separate verified Teams transport"
    );
    if let Some(models) = config.get("models") {
        ensure!(
            models.get("extra_headers").is_none()
                && models.get("allowed_models").is_none()
                && models.get("disabled_models").is_none(),
            "Grok model headers/catalog restrictions need a contract check"
        );
        ensure!(
            models
                .get("max_completion_tokens")
                .is_none_or(|v| v.as_integer() == Some(OUTPUT as i64)),
            "Grok native output limit differs from the verified Teams contract"
        );
    }
    if let Some(endpoints) = config.get("endpoints") {
        let fields = endpoints
            .as_table()
            .context("Native Grok endpoint configuration invalid")?;
        ensure!(
            fields.iter().all(|(k, v)| k == "xai_api_base_url"
                && v.as_str().map(|s| s.trim_end_matches('/')) == Some(UPSTREAM)),
            "Grok Teams does not substitute a native endpoint/account"
        );
    }
    ensure!(
        !key.is_empty()
            && key.len() <= 16 * 1024
            && !key.starts_with(['!', '$'])
            && !key.chars().any(char::is_whitespace),
        "Grok Teams requires a literal native API key"
    );
    Ok(key)
}
fn account_overlay(name: &str) -> bool {
    let name = name.to_ascii_uppercase();
    name.starts_with("GROK_AUTH")
        || name.starts_with("GROK_OAUTH2_")
        || name.starts_with("GROK_OIDC_")
        || name.starts_with("GROK_WS_")
        || matches!(
            name.as_str(),
            "GROK_CONFIG"
                | "GROK_CONFIG_PATH"
                | "GROK_MODELS_BASE_URL"
                | "GROK_MODELS_LIST_URL"
                | "GROK_DEPLOYMENT_KEY"
                | "GROK_CODE_XAI_API_KEY"
                | "GROK_EXTRA_AUTH_KEY"
                | "GROK_DISABLE_API_KEY_AUTH"
                | "GROK_FORCE_LOGIN_TEAM_ID"
                | "GROK_LOCAL_AUTH"
                | "GROK_CAMPAIGNS_OVERRIDE"
                | "GROK_CLI_CHAT_PROXY_BASE_URL"
        )
}
fn account(model: &str) -> Result<String> {
    model_id(model)?;
    for (name, value) in std::env::vars_os() {
        ensure!(
            value.is_empty() || !account_overlay(&name.to_string_lossy()),
            "Native Grok overlays, custom catalogs or managed authentication need a verified Teams transport"
        );
    }
    ensure!(
        std::env::var("GROK_XAI_API_BASE_URL")
            .ok()
            .filter(|v| !v.is_empty())
            .is_none_or(|v| v.trim_end_matches('/') == UPSTREAM),
        "Grok native endpoint changed"
    );
    let home = home()?;
    for name in ["managed_config.toml", "requirements.toml", "auth.json"] {
        // A native cached login can take precedence over XAI_API_KEY. Do not
        // silently replace it with another account or bypass its managed policy.
        ensure!(
            !home.join(name).exists(),
            "Grok cached/managed accounts need a separate verified Teams transport; use a direct-key host profile"
        );
    }
    let path = home.join("config.toml");
    let config = if path.exists() {
        ensure!(
            std::fs::metadata(&path)?.len() <= 1024 * 1024,
            "Native Grok config exceeds its bound"
        );
        let bytes = std::fs::read_to_string(path)?;
        toml::from_str(&bytes).map_err(|_| anyhow::anyhow!("Native Grok config is invalid"))?
    } else {
        toml::Value::Table(Default::default())
    };
    let key = std::env::var("XAI_API_KEY").context("Configure this host's native XAI_API_KEY privately for Grok Teams; OAuth needs a separate verified transport")?;
    from_config(&config, model, key)
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
        .env("GROK_HOME", root.join("home"));
    for (name, value) in [
        // Native pooling prewarms the origin with an unauthenticated GET before
        // inference. Keep it from consuming the one-use completion capability.
        ("GROK_SAMPLER_SHARED_CLIENT", "0"),
        ("GROK_DISABLE_AUTOUPDATER", "1"),
        ("GROK_TELEMETRY_ENABLED", "0"),
        ("GROK_TELEMETRY_TRACE_UPLOAD", "0"),
        ("GROK_WORKSPACE_UPLOAD_QUEUE_ENABLED", "false"),
        ("GROK_MANAGED_CONFIG", "0"),
        ("GROK_CRASH_HANDLER", "0"),
        ("GROK_TURN_SUMMARY", "0"),
        ("GROK_TITLE_REFRESH", "0"),
        ("GROK_GOAL_SUMMARY", "0"),
        ("GROK_PROMPT_SUGGESTIONS", "0"),
        ("GROK_MEMORY", "0"),
        ("GROK_SUBAGENTS", "0"),
        ("GROK_WEB_FETCH", "0"),
        ("GROK_WRITE_FILE", "0"),
        ("GROK_TOOL_SEARCH", "0"),
        ("GROK_LSP_TOOLS", "0"),
        ("GROK_MANAGED_MCPS_ENABLED", "0"),
        ("GROK_MANAGED_MCP_GATEWAY_TOOLS_ENABLED", "0"),
        ("GROK_OFFICIAL_MARKETPLACE_AUTO_REGISTER", "0"),
        ("GROK_AGENT_DASHBOARD", "0"),
        ("OTEL_TRACES_EXPORTER", "none"),
        ("RUST_LOG", "off"),
    ] {
        cmd.env(name, value);
    }
    for vendor in ["CURSOR", "CLAUDE", "CODEX"] {
        for surface in ["SKILLS", "RULES", "AGENTS", "MCPS", "HOOKS", "SESSIONS"] {
            cmd.env(format!("GROK_{vendor}_{surface}_ENABLED"), "0");
        }
    }
    // Ignore system Git configuration/hooks while inspecting the private repo.
    cmd.env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", root.join("empty-git-config"));
    cmd
}
struct Session {
    root: EmptyDirectory,
}
impl Session {
    fn create(model: &str, url: &str, task: &str) -> Result<Self> {
        model_id(model)?;
        ensure!(task.len() <= 128 * 1024, "Grok context exceeds its bound");
        let root = EmptyDirectory::create("grok")?;
        for name in [
            "home",
            "work",
            "work/.git",
            "work/.git/objects",
            "work/.git/refs",
        ] {
            std::fs::create_dir(root.0.join(name))?;
        }
        // An empty .git is not a repository to Grok's Git-based root discovery.
        // This minimal private repo has no remotes, hooks, objects or user files.
        let config = json!({"models":{"default":model,"max_completion_tokens":OUTPUT,"max_retries":0},
            "model":{model:{"model":model,"base_url":url,"env_key":"ION_GROK_NATIVE_KEY","api_backend":"responses","context_window":INPUT,"max_completion_tokens":OUTPUT,"max_retries":0,"supports_backend_search":false}},
            "features":{"remote_fetch":false,"managed_config":false,"telemetry":"off","codebase_indexing":false,"title_refresh":false},
            "telemetry":{"trace_upload":false},"cli":{"auto_update":false},
            "endpoints":{"cli_chat_proxy_base_url":"http://127.0.0.1:1","xai_api_base_url":"http://127.0.0.1:1"}});
        let config = toml::to_string(&config)?;
        let agent = "---\nname: ion-teams\ndescription: Complete one Ion request\ntool_config:\n  tools: []\ninject_default_tools: false\ndiscover_skills: false\ninherit_skills: false\nagents_md: false\nmax_turns: 1\n---\nComplete the supplied Ion input.\n";
        for (path, bytes) in [
            ("home/config.toml", config.as_bytes()),
            ("input.txt", task.as_bytes()),
            ("agent.md", agent.as_bytes()),
            ("empty-git-config", b""),
            ("work/.git/HEAD", b"ref: refs/heads/main\n"),
            (
                "work/.git/config",
                b"[core]\nrepositoryformatversion=0\nbare=false\n",
            ),
        ] {
            let mut opts = std::fs::OpenOptions::new();
            opts.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                opts.mode(0o600);
            }
            opts.open(root.0.join(path))?.write_all(bytes)?;
        }
        Ok(Self { root })
    }
    fn command(&self, binary: &Path) -> Command {
        isolated_command(binary, &self.root.0)
    }
    fn verify(&self, binary: &Path, cancelled: &AtomicBool) -> Result<()> {
        let mut cmd = self.command(binary);
        cmd.arg("--version");
        let bytes = bounded_process(&mut cmd, b"", cancelled, Duration::from_secs(30))?;
        let version = std::str::from_utf8(&bytes)?.trim();
        ensure!(
            version
                .strip_prefix("grok ")
                .and_then(|v| v.split_whitespace().next())
                == Some(VERSION),
            "Grok Teams requires the verified native 1.0.46 runtime; this version needs a contract check"
        );
        let mut cmd = self.command(binary);
        cmd.args(["inspect", "--json"]);
        let bytes = bounded_process(&mut cmd, b"", cancelled, Duration::from_secs(30))?;
        let report: Value = serde_json::from_slice(&bytes)
            .map_err(|_| anyhow::anyhow!("Native Grok inspection is invalid"))?;
        inspect(&report, &self.root.0)
    }
    fn complete(
        &self,
        binary: &Path,
        key: &str,
        cancelled: &AtomicBool,
        model: &str,
    ) -> Result<Vec<u8>> {
        self.verify(binary, cancelled)?;
        let mut cmd = self.command(binary);
        cmd.env("ION_GROK_NATIVE_KEY", key)
            .arg("--prompt-file")
            .arg(self.root.0.join("input.txt"))
            .arg("--agent")
            .arg(self.root.0.join("agent.md"))
            .args([
                "--model",
                model,
                "--tools",
                "ion-no-native-tools",
                "--verbatim",
                "--system-prompt-override",
                team_completion::INSTRUCTIONS,
                "--max-turns",
                "1",
                "--no-memory",
                "--no-subagents",
                "--disable-web-search",
                "--permission-mode",
                "plan",
                "--no-wait-for-background",
                "--output-format",
                "json",
            ]);
        bounded_process(&mut cmd, b"", cancelled, Duration::from_secs(120))
    }
}
fn inspect(report: &Value, root: &Path) -> Result<()> {
    ensure!(
        report["grokVersion"] == VERSION,
        "Grok inspection version changed"
    );
    let work = std::fs::canonicalize(root.join("work"))?;
    let actual = report["projectRoot"]
        .as_str()
        .context("Grok private repository not resolved")?;
    ensure!(
        std::fs::canonicalize(actual)? == work,
        "Grok discovered a repository outside the private workspace"
    );
    ensure!(
        report["cwd"]
            .as_str()
            .is_some_and(|p| std::fs::canonicalize(p).ok().as_ref() == Some(&work)),
        "Grok inspection workspace changed"
    );
    for key in [
        "projectInstructions",
        "hooks",
        "skills",
        "plugins",
        "marketplaces",
        "mcpServers",
        "lspServers",
    ] {
        ensure!(
            report[key].as_array().is_some_and(|v| v.is_empty()),
            "Grok native resources were discovered; no completion was launched"
        );
    }
    ensure!(
        report
            .get("configWarnings")
            .is_none_or(|v| v.as_array().is_some_and(|v| v.is_empty()))
            && report
                .get("mcpConfigProblems")
                .is_none_or(|v| v.as_array().is_some_and(|v| v.is_empty())),
        "Grok private configuration was ignored or invalid"
    );
    ensure!(
        report["loginPolicy"]["apiKeyAuthDisabled"] == false
            && report["permissions"]["managedSettingsExists"] == false
            && report["permissions"]
                .get("enforced")
                .is_none_or(|v| v.as_array().is_some_and(|v| v.is_empty())),
        "Grok managed policy requires a separate verified Teams transport"
    );
    let layers = report["configSources"]["layers"]
        .as_array()
        .context("Grok config sources missing")?;
    let expected = std::fs::canonicalize(root.join("home/config.toml"))?;
    ensure!(
        layers.len() == 1
            && layers[0]["role"] == "user"
            && layers[0].get("note").is_none()
            && layers[0]["path"]
                .as_str()
                .is_some_and(|p| std::fs::canonicalize(p).ok().as_ref() == Some(&expected)),
        "Grok loaded an unexpected native configuration layer"
    );
    ensure!(
        report["agents"]
            .as_array()
            .is_some_and(|rows| rows.iter().all(|r| r["source"]["type"] == "builtin")),
        "Grok discovered external agents"
    );
    Ok(())
}
pub(crate) fn ready(binary: &Path, model: &str) -> Result<()> {
    account(model)?;
    Session::create(model, "http://127.0.0.1:1/ion/probe", "{}")?
        .verify(binary, &AtomicBool::new(false))
}
fn allowance(model: &str) -> Result<ModelCallAllowance> {
    account(model)?;
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
        ProviderKind::Grok,
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
    ensure!(task.len() <= 128 * 1024, "Grok context exceeds its bound");
    let gate = team_gate::Gate::grok(model, OUTPUT, &task)?;
    let session = Session::create(model, &gate.url, &task)?;
    let result = session.complete(binary, &key, cancelled, model);
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
        "Registered Grok allowance is too small"
    );
    let binary = crate::model::provider_probe(ProviderKind::Grok, None)
        .path
        .context("Grok worker executable missing")?;
    completion(&binary, model, system, messages, tools, cancelled)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn team_grok_refuses_model_endpoint_auth_and_limit_substitution() {
        let config = toml::Value::Table(Default::default());
        assert!(from_config(&config, "grok-4.6", "fixture-only".into()).is_ok());
        for id in ["grok-build", "grok-4.7", "xai/grok-4.6", "custom"] {
            assert!(from_config(&config, id, "fixture-only".into()).is_err());
        }
        for s in [
            r#"[model."grok-4.6"]
model="replacement""#,
            r#"[endpoints]
xai_api_base_url="https://other.example/v1""#,
            r#"[models]
max_completion_tokens=100"#,
            r#"[models]
extra_headers={Authorization="secret"}"#,
            r#"[auth_providers.native]
command="external""#,
            r#"[auth_provider.native]
command="external""#,
            r#"[auth]
preferred_method="oidc""#,
        ] {
            let cfg = toml::from_str(s).unwrap();
            assert!(from_config(&cfg, "grok-4.6", "fixture-only".into()).is_err());
        }
        for name in [
            "GROK_AUTH",
            "GROK_AUTH_PROVIDER_COMMAND",
            "GROK_DISABLE_API_KEY_AUTH",
            "GROK_FORCE_LOGIN_TEAM_ID",
            "GROK_OAUTH2_ISSUER",
            "GROK_OIDC_CLIENT_ID",
            "GROK_CLI_CHAT_PROXY_BASE_URL",
        ] {
            assert!(account_overlay(name));
        }
        assert!(!account_overlay("XAI_API_KEY"));
        assert!(!account_overlay("GROK_HOME"));
        for key in ["", "$KEY", "not literal"] {
            assert!(from_config(&config, "grok-4.6", key.into()).is_err());
        }
    }
    #[test]
    fn team_grok_private_inputs_are_bounded_and_credentials_are_child_only() {
        let task = json!({"instructions":"Keep @secret, $HOME and ${cwd} literal"}).to_string();
        let session = Session::create("grok-4.6", "http://127.0.0.1:1/ion/probe", &task).unwrap();
        let root = session.root.0.clone();
        assert_eq!(
            std::fs::read_to_string(root.join("input.txt")).unwrap(),
            task
        );
        let config = std::fs::read_to_string(root.join("home/config.toml")).unwrap();
        assert!(!config.contains("fixture-only"));
        assert!(config.contains("remote_fetch = false"));
        assert!(
            std::fs::read_to_string(root.join("agent.md"))
                .unwrap()
                .contains("inject_default_tools: false")
        );
        let cmd = session.command(Path::new("fixture-grok"));
        assert!(!cmd.get_envs().any(|(k, _)| matches!(
            k.to_string_lossy().as_ref(),
            "ION_GROK_NATIVE_KEY"
                | "XAI_API_KEY"
                | "GROK_CONFIG"
                | "GROK_AUTH_PATH"
                | "HTTPS_PROXY"
        )));
        assert!(Session::create("grok-4.6", "unused", &"x".repeat(128 * 1024 + 1)).is_err());
        drop(session);
        assert!(!root.exists());
    }
    #[test]
    #[ignore = "Requires verified Grok executable; fixture key and loopback only"]
    fn team_grok_native_one_metered_prompt_no_effects_or_auxiliary_billing() {
        let _diagnostics = crate::team_process::fixture_diagnostics();
        let binary = std::env::var_os("ION_TEST_GROK_BINARY")
            .map(PathBuf::from)
            .or_else(|| crate::model::provider_probe(ProviderKind::Grok, None).path)
            .expect("Set ION_TEST_GROK_BINARY");
        let bytes =
            super::super::team_test_http::responses("grok-4.6", r#"{"text":"ready","calls":[]}"#);
        // Native Responses deserialization requires a sequence_number on events.
        let mut sequence = 0;
        let mut body = std::str::from_utf8(&bytes)
            .unwrap()
            .lines()
            .map(|l| {
                if let Some(data) = l.strip_prefix("data: ") {
                    let mut v: Value = serde_json::from_str(data).unwrap();
                    if v["type"] == "response.created" {
                        v["response"] = json!({"id":"resp_fixture","object":"response","created_at":1,
                            "status":"in_progress","model":"grok-4.6","output":[],"usage":null});
                    }
                    if v["type"] == "response.output_item.added" {
                        v["item"]["status"] = json!("in_progress");
                    }
                    v["sequence_number"] = json!(sequence);
                    sequence += 1;
                    format!("data: {v}")
                } else {
                    l.into()
                }
            })
            .collect::<Vec<String>>()
            .join("\n")
            .into_bytes();
        // The final SSE event needs a blank line; EOF alone does not dispatch it.
        body.extend_from_slice(b"\n\n");
        let server = super::super::team_test_http::Server::with_timeout(
            vec![(200, body)],
            Duration::from_secs(120),
        );
        let task=json!({"instructions":"Return Ion JSON; retain @secret, $HOME and ${cwd}","messages":[],"tools":[]}).to_string();
        let gate = team_gate::Gate::start_with_prompt(
            "grok-4.6",
            &format!("{}/responses", server.url),
            Some(OUTPUT),
            Some(task.clone()),
        )
        .unwrap();
        let session = Session::create("grok-4.6", &gate.url, &task).unwrap();
        let result = session.complete(
            &binary,
            "fixture-only-not-real",
            &AtomicBool::new(false),
            "grok-4.6",
        );
        let receipt = gate.receipt();
        if let Err(error) = result {
            panic!(
                "Native fixture failed: {error:#}; gate: {}",
                receipt
                    .err()
                    .map(|e| format!("{e:#}"))
                    .unwrap_or_else(|| "completed".into())
            );
        }
        let receipt = receipt.unwrap();
        assert_eq!(receipt.usage.input_tokens, 100);
        assert_eq!(receipt.usage.output_tokens, 5);
        let requests = server.finish();
        assert_eq!(requests.len(), 1);
        let b = &requests[0].body;
        assert_eq!(b["model"], "grok-4.6");
        assert_eq!(b["max_output_tokens"], OUTPUT);
        assert_eq!(b["tools"], json!([]));
        assert_eq!(b["store"], false);
        assert_eq!(
            b["input"],
            json!([
                {"role":"system","content":team_completion::INSTRUCTIONS},
                {"role":"user","content":task}
            ])
        );
        assert!(
            team_completion::actions(&receipt.text, &json!([]), receipt.usage)
                .unwrap()
                .calls
                .is_empty()
        );
    }
}
