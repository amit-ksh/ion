//! Oh My Pi's own native catalog, SQLite login and YAML config contract.
//! Runs tool-free in a private home; the gate permits one metered provider request.
use super::{
    DriverEventSender, NativeTeamExit, NativeTeamStart, team_codex::EmptyDirectory,
    team_completion, team_gate,
};
use crate::{
    team_process::{bounded_process, bounded_rpc},
    teams::{ModelCallAllowance, TeamCoordinator},
};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OpenFlags};
use serde_json::{Value, json};
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::AtomicBool,
    time::Duration,
};
use waku_protocol::model::{DriverEvent, ProviderKind};

const ISOLATION: &[&str] = &[
    "--no-session",
    "--no-tools",
    "--no-lsp",
    "--no-pty",
    "--no-extensions",
    "--no-skills",
    "--no-rules",
    "--no-title",
    "--no-prewalk",
];
struct Metadata {
    provider: String,
    id: String,
    api: String,
    upstream: &'static str,
    input: u64,
    output: u64,
}
fn selector(model: &str) -> Result<(&str, &str, &'static str)> {
    let (provider, id) = model
        .split_once('/')
        .context("Choose an exact Oh My Pi provider/model ID")?;
    ensure!(
        !id.is_empty() && model.len() <= 160,
        "Invalid Oh My Pi model ID"
    );
    let upstream = match provider {
        "openai" => "https://api.openai.com/v1",
        "anthropic" => "https://api.anthropic.com/v1",
        "nebius" => "https://api.tokenfactory.nebius.com/v1",
        _ => anyhow::bail!(
            "Oh My Pi Teams currently supports direct OpenAI, Anthropic and Nebius models"
        ),
    };
    Ok((provider, id, upstream))
}
fn from_catalog(models: &Value, model: &str) -> Result<Metadata> {
    let (provider, id, upstream) = selector(model)?;
    let mut entries = models
        .as_array()
        .context("Oh My Pi catalog missing")?
        .iter()
        .filter(|v| v["provider"].as_str() == Some(provider) && v["id"].as_str() == Some(id));
    let entry = entries
        .next()
        .context("Pinned Oh My Pi model is unavailable in the native catalog")?;
    ensure!(
        entries.next().is_none() && entry["baseUrl"].as_str() == Some(upstream),
        "Oh My Pi model identity or endpoint is ambiguous/custom"
    );
    let api = entry["api"]
        .as_str()
        .context("Oh My Pi model protocol missing")?;
    ensure!(
        if provider == "anthropic" {
            api == "anthropic-messages"
        } else {
            matches!(api, "openai-responses" | "openai-completions")
        },
        "Selected Oh My Pi API is unsupported"
    );
    let limit = |key: &str| {
        entry[key]
            .as_u64()
            .filter(|n| (1..=32_000_000).contains(n))
            .context("Oh My Pi token allowance missing")
    };
    Ok(Metadata {
        provider: provider.into(),
        id: id.into(),
        api: api.into(),
        upstream,
        input: limit("contextWindow")?,
        output: limit("maxTokens")?,
    })
}
// No borrowed HOME/config, .env, extension roots, auth brokers, provider-key
// overrides, NODE_OPTIONS, proxies or runtime hooks reach the isolated CLI.
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
    cmd.env_clear();
    cmd.envs(std::env::vars_os().filter(|(k, _)| allowed(k)));
    cmd.envs(runtime.into_iter().filter(|(k, _)| allowed(k)));
    cmd.env("HOME", root.join("home"))
        .env("USERPROFILE", root.join("home"))
        .env("PI_CODING_AGENT_DIR", root.join("agent"))
        .env("PI_CONFIG_DIR", ".omp")
        .current_dir(root.join("work"))
        .args(ISOLATION)
        .args(["--config"])
        .arg(root.join("agent/config.yml"));
    cmd
}
struct Session {
    root: EmptyDirectory,
    files: Vec<PrivateFile>,
}
impl Session {
    fn create(model: &str, key: &str, meta: Option<(&Metadata, &str)>) -> Result<Self> {
        let (provider, _, _) = selector(model)?;
        let root = EmptyDirectory::create("omp")?;
        for name in ["home", "agent", "work"] {
            std::fs::create_dir(root.0.join(name))?;
        }
        let mut provider_config = json!({"apiKey":key});
        if let Some((meta, url)) = meta {
            // Keep provider discovery on its native origin; only this exact
            // model's inference route receives the one-use capability URL.
            provider_config["baseUrl"] = json!(meta.upstream);
            provider_config["api"] = json!(meta.api);
            provider_config["models"] = json!([{"id":meta.id,"baseUrl":url,"api":meta.api,"contextWindow":meta.input,"maxTokens":meta.output,"input":["text"],"reasoning":false}]);
        }
        // JSON is a YAML subset and is accepted by OMP's native YAML parser.
        let config = json!({"startup":{"checkUpdate":false,"setupWizard":false,"quiet":true},"retry":{"enabled":false,"modelFallback":false,"usageAwareFallback":false},"compaction":{"enabled":false,"midTurnEnabled":false,"asyncEnabled":false},"mcp":{"enableProjectConfig":false},"enabledProviders":[provider],"enabledModels":[model],"modelRoles":{"default":model,"smol":model,"slow":model,"plan":model},"telemetry":{"otlp":{"export":{"enabled":false}}}});
        let mut files = Vec::new();
        for (name, value) in [
            ("config.yml", config),
            (
                "models.yml",
                json!({"providers":{provider:provider_config}}),
            ),
        ] {
            let file = PrivateFile(root.0.join("agent").join(name));
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            options
                .open(&file.0)?
                .write_all(&serde_json::to_vec(&value)?)?;
            files.push(file);
        }
        Ok(Self { root, files })
    }
    fn command(&self, binary: &Path) -> Command {
        isolated_command(binary, &self.root.0)
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        // Remove the selected secret explicitly before the directory guard.
        self.files.clear();
    }
}
struct PrivateFile(PathBuf);
impl Drop for PrivateFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
fn metadata(binary: &Path, model: &str) -> Result<Metadata> {
    let session = Session::create(model, "ion-catalog-placeholder", None)?;
    let mut cmd = session.command(binary);
    cmd.arg("--help");
    let help = bounded_process(
        &mut cmd,
        b"",
        &AtomicBool::new(false),
        Duration::from_secs(30),
    )?;
    let help = std::str::from_utf8(&help)?;
    ensure!(
        ISOLATION.iter().all(|f| help.contains(f)),
        "Update Oh My Pi: verified Teams isolation flags are required"
    );
    let mut cmd = session.command(binary);
    cmd.args(["--mode", "rpc", "--model", model]);
    // Catalog lookup has no prompt and only a placeholder credential. Never
    // opens/migrates the owner's SQLite database or executes stored commands.
    let bytes = bounded_rpc(
        &mut cmd,
        b"{\"id\":\"ion-models\",\"type\":\"get_available_models\"}\n",
        "ion-models",
        Duration::from_secs(30),
    )?;
    let response = std::str::from_utf8(&bytes)?
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find(|v| v["id"] == "ion-models" && v["type"] == "response")
        .context("Oh My Pi catalog response missing")?;
    ensure!(
        response["success"] == true && response["command"] == "get_available_models",
        "Oh My Pi model catalog unavailable"
    );
    from_catalog(&response["data"]["models"], model)
}
fn credential(database: &Path, provider: &str) -> Result<String> {
    let db = Connection::open_with_flags(
        database,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .context("Sign in to the selected Oh My Pi provider with a direct API key")?;
    db.busy_timeout(Duration::from_millis(200))?;
    let mut query=db.prepare("SELECT credential_type, CASE WHEN length(data)<=262144 THEN data ELSE NULL END FROM auth_credentials WHERE provider=?1 AND disabled_cause IS NULL LIMIT 2")
        .context("Update Oh My Pi: native credential schema is required")?;
    let rows = query
        .query_map([provider], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    ensure!(
        rows.len() == 1 && rows[0].0 == "api_key",
        "Oh My Pi Teams requires one active direct API login; OAuth and account pools are not replaced"
    );
    let data: Value = serde_json::from_str(
        rows[0]
            .1
            .as_deref()
            .context("Oh My Pi credential exceeds its bound")?,
    )?;
    let key = data["key"]
        .as_str()
        .filter(|k| !k.is_empty() && k.len() <= 16 * 1024 && !k.starts_with(['!', '$']))
        .context("Oh My Pi Teams requires a stored literal API key")?;
    Ok(key.into())
}
fn validate_native_config(input: &str) -> Result<()> {
    let input = input.to_ascii_lowercase();
    ensure!(
        !input.contains("broker") && !input.contains("accountpolicies"),
        "Oh My Pi configured auth routing is not supported by this Teams adapter"
    );
    Ok(())
}
fn auth(provider: &str) -> Result<String> {
    ensure!(
        ![
            "OMP_AUTH_BROKER_URL",
            "OMP_AUTH_BROKER_TOKEN",
            "OMP_AUTH_BROKER_ACCOUNT_POOL_FILE",
            "PI_CONFIG_FILES",
            "OMP_PROFILE",
            "PI_PROFILE"
        ]
        .iter()
        .any(|name| std::env::var_os(name).is_some_and(|v| !v.is_empty())),
        "Oh My Pi Teams requires its default local SQLite login; broker/profile/overlay selection needs a separate verified transport"
    );
    let home = dirs::home_dir().context("Oh My Pi home unavailable")?;
    let config_name = std::env::var_os("PI_CONFIG_DIR")
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| ".omp".into());
    // Native Node path.join treats this as a name under HOME, unlike Rust's
    // absolute-path replacement. Reject that ambiguous override.
    ensure!(
        !Path::new(&config_name).is_absolute(),
        "Oh My Pi PI_CONFIG_DIR must be relative to the native home"
    );
    let config = home.join(config_name);
    let agent = std::env::var_os("PI_CODING_AGENT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| config.join("agent"));
    ensure!(
        !config.join("auth-broker.token").exists(),
        "Oh My Pi auth broker is not supported by this Teams adapter"
    );
    // Conservatively reject auth-routing YAML, rather than interpret it with a
    // different parser or select a local account behind an active broker policy.
    for root in [&config, &agent] {
        for name in ["config.yml", "config.yaml", ".env"] {
            let path = root.join(name);
            if path.exists() {
                let mut input = Vec::new();
                use std::io::Read;
                std::fs::File::open(path)?
                    .take(256 * 1024 + 1)
                    .read_to_end(&mut input)?;
                ensure!(
                    input.len() <= 256 * 1024,
                    "Oh My Pi config exceeds its bound"
                );
                let input = std::str::from_utf8(&input)?;
                validate_native_config(input)?;
            }
        }
    }
    ensure!(
        !["models.yml", "models.yaml", "models.json"]
            .iter()
            .any(|name| agent.join(name).exists()),
        "Oh My Pi custom model configuration requires a verified Teams transport"
    );
    let database = agent.join("agent.db");
    #[cfg(unix)]
    let mut database = database;
    #[cfg(unix)]
    {
        if agent == config.join("agent") {
            if let Some(data) = std::env::var_os("XDG_DATA_HOME") {
                let xdg = PathBuf::from(data).join("omp");
                if xdg.is_dir() {
                    database = xdg.join("agent.db");
                }
            }
        }
    }
    credential(&database, provider)
}
pub(crate) fn ready(binary: &Path, model: &str) -> Result<()> {
    let (provider, _, _) = selector(model)?;
    auth(provider)?;
    metadata(binary, model)?;
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
    let model = match &start {
        NativeTeamStart::Fresh(c) => &c.profile.model,
        NativeTeamStart::Resumed(c) => &c.claimed.profile.model,
    }
    .as_deref()
    .context("Pinned Oh My Pi model missing")?;
    let meta = metadata(binary, model)?;
    super::nebius::team::run_with_harness(
        coordinator,
        start,
        workspace,
        events,
        cancelled,
        ProviderKind::OhMyPi,
        |model| {
            Ok(ModelCallAllowance {
                model: model.into(),
                max_input_tokens: meta.input,
                max_output_tokens: meta.output,
            })
        },
        |model, system, messages, tools| {
            let reply = completion(binary, model, &meta, system, messages, &tools, cancelled)?;
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
    system: &str,
    messages: &[Value],
    tools: &Value,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>> {
    let model = format!("{}/{}", meta.provider, meta.id);
    let session = Session::create(&model, key, Some((meta, url)))?;
    let input =
        serde_json::to_vec(&json!({"instructions":system,"messages":messages,"tools":tools}))?;
    ensure!(
        input.len() <= 128 * 1024,
        "Oh My Pi context exceeds its bound"
    );
    let mut cmd = session.command(binary);
    cmd.args([
        "--mode",
        "json",
        "--print",
        "--model",
        &model,
        "--thinking",
        "off",
        "--system-prompt",
        team_completion::INSTRUCTIONS,
    ]);
    bounded_process(&mut cmd, &input, cancelled, Duration::from_secs(120))
}
fn completion(
    binary: &Path,
    model: &str,
    meta: &Metadata,
    system: &str,
    messages: &[Value],
    tools: &Value,
    cancelled: &AtomicBool,
) -> Result<super::nebius::StreamCompletion> {
    ensure!(
        model == format!("{}/{}", meta.provider, meta.id),
        "Pinned Oh My Pi model changed"
    );
    let key = auth(&meta.provider)?;
    let gate = team_gate::Gate::api(&meta.id, meta.upstream)?;
    let result = launch(
        binary, meta, &key, &gate.url, system, messages, tools, cancelled,
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
    let binary = crate::model::provider_probe(ProviderKind::OhMyPi, None)
        .path
        .context("Oh My Pi worker executable missing")?;
    let meta = metadata(&binary, model)?;
    ensure!(
        meta.input <= input_cap && meta.output <= output_cap,
        "Registered Oh My Pi allowance is too small"
    );
    completion(&binary, model, &meta, system, messages, tools, cancelled)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn team_omp_catalog_rejects_aliases_duplicates_custom_routes_and_missing_caps() {
        let entry = json!({"provider":"openai","id":"pinned","api":"openai-responses","baseUrl":"https://api.openai.com/v1","contextWindow":1000000,"maxTokens":128000});
        assert_eq!(
            from_catalog(&json!([entry.clone()]), "openai/pinned")
                .unwrap()
                .output,
            128000
        );
        for data in [json!([entry.clone(), entry.clone()]), json!([])] {
            assert!(from_catalog(&data, "openai/pinned").is_err());
        }
        for (field, value) in [
            ("baseUrl", json!("https://custom.example/v1")),
            ("api", json!("openai-codex-responses")),
            ("maxTokens", json!(null)),
            ("contextWindow", json!(0)),
        ] {
            let mut entry = entry.clone();
            entry[field] = value;
            assert!(from_catalog(&json!([entry]), "openai/pinned").is_err());
        }
        for model in ["pinned", "openai/", "openai-codex/pinned"] {
            assert!(selector(model).is_err());
        }
    }
    #[test]
    fn team_omp_sqlite_login_is_readonly_selected_and_unambiguous() {
        let root = EmptyDirectory::create("omp-auth-fixture").unwrap();
        let path = root.0.join("agent.db");
        let db = Connection::open(&path).unwrap();
        db.execute_batch("CREATE TABLE auth_credentials (provider TEXT,credential_type TEXT,data TEXT,disabled_cause TEXT);").unwrap();
        db.execute(
            "INSERT INTO auth_credentials VALUES ('openai','api_key',?1,NULL)",
            [json!({"key":"sk-fixture-only"}).to_string()],
        )
        .unwrap();
        db.execute(
            "INSERT INTO auth_credentials VALUES ('anthropic','api_key',?1,NULL)",
            [json!({"key":"other-fixture"}).to_string()],
        )
        .unwrap();
        db.execute(
            "INSERT INTO auth_credentials VALUES ('openai','oauth','{}','disabled')",
            [],
        )
        .unwrap();
        let before = std::fs::read(&path).unwrap();
        assert_eq!(credential(&path, "openai").unwrap(), "sk-fixture-only");
        assert_eq!(std::fs::read(&path).unwrap(), before);
        db.execute(
            "INSERT INTO auth_credentials VALUES ('openai','oauth','{}',NULL)",
            [],
        )
        .unwrap();
        assert!(credential(&path, "openai").is_err());
        db.execute("DELETE FROM auth_credentials WHERE provider='openai'", [])
            .unwrap();
        for key in ["!run-auth", "$SECRET", ""] {
            db.execute(
                "INSERT INTO auth_credentials VALUES ('openai','api_key',?1,NULL)",
                [json!({"key":key}).to_string()],
            )
            .unwrap();
            assert!(credential(&path, "openai").is_err());
            db.execute("DELETE FROM auth_credentials WHERE provider='openai'", [])
                .unwrap();
        }
        drop(db);
    }
    #[test]
    fn team_omp_rejects_native_auth_routing_in_yaml_and_dotenv() {
        for config in [
            "auth:\n  broker:\n    url: https://fixture.invalid",
            "'auth.broker.url': https://fixture.invalid",
            "OMP_AUTH_BROKER_URL=https://fixture.invalid",
            "auth:\n  accountPolicies: []",
        ] {
            assert!(validate_native_config(config).is_err());
        }
        assert!(validate_native_config("startup:\n  quiet: true").is_ok());
    }
    #[test]
    fn team_omp_session_has_no_native_resources_or_inherited_auth_and_removes_private_files() {
        let session = Session::create("openai/pinned", "sk-fixture-only", None).unwrap();
        let root = session.root.0.clone();
        let cmd = session.command(Path::new("fixture-omp"));
        let env = cmd
            .get_envs()
            .filter_map(|(k, v)| {
                v.map(|v| {
                    (
                        k.to_string_lossy().into_owned(),
                        v.to_string_lossy().into_owned(),
                    )
                })
            })
            .collect::<Vec<_>>();
        assert!(!env.iter().any(|(k, _)| k == "OPENAI_API_KEY"
            || k == "OMP_AUTH_BROKER_TOKEN"
            || k == "NODE_OPTIONS"));
        assert!(env.iter().any(|(k, v)| k == "PI_CODING_AGENT_DIR"
            && v.as_str() == root.join("agent").to_string_lossy()));
        let args = cmd
            .get_args()
            .map(|a| a.to_string_lossy())
            .collect::<Vec<_>>();
        assert!(ISOLATION.iter().all(|flag| args.iter().any(|a| a == flag)));
        drop(session);
        assert!(!root.exists());
    }
    #[test]
    #[ignore = "Requires current Oh My Pi executable; uses only loopback and fixture credentials"]
    fn team_omp_native_catalog_and_completion_use_one_metered_request_without_tools() {
        let _diagnostics = crate::team_process::fixture_diagnostics();
        let binary = std::env::var_os("ION_TEST_OMP_BINARY")
            .map(PathBuf::from)
            .or_else(|| crate::model::provider_probe(ProviderKind::OhMyPi, None).path)
            .expect("Set ION_TEST_OMP_BINARY");
        let meta = metadata(&binary, "openai/gpt-5.4").unwrap();
        let server = crate::driver::team_test_http::Server::with_timeout(
            vec![(
                200,
                crate::driver::team_test_http::responses(
                    "gpt-5.4",
                    r#"{"text":"ready","calls":[]}"#,
                ),
            )],
            Duration::from_secs(120),
        );
        let gate = team_gate::Gate::start("gpt-5.4", &format!("{}/responses", server.url)).unwrap();
        let result = launch(
            &binary,
            &meta,
            "sk-fixture-only",
            &gate.url,
            "Return the Ion action contract",
            &[],
            &json!([]),
            &AtomicBool::new(false),
        );
        let receipt = gate.receipt();
        result.unwrap_or_else(|e| {
            panic!(
                "Oh My Pi fixture failed: {e:#}; gate={}",
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
        assert_eq!(requests[0].body["model"], "gpt-5.4");
        assert_eq!(requests[0].body["tools"], json!([]));
        assert!(requests[0].headers.lines().any(|l| {
            l.split_once(':').is_some_and(|(name, value)| {
                name.eq_ignore_ascii_case("authorization")
                    && value.trim() == "Bearer sk-fixture-only"
            })
        }));
        let reply = team_completion::actions(&receipt.text, &json!([]), receipt.usage).unwrap();
        assert_eq!(reply.text, "ready");
        assert!(reply.calls.is_empty());
    }
}
