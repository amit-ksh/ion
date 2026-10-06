//! Pi print mode with native model metadata, isolated direct-provider auth,
//! no tools/resources and one billable request through the shared transport gate.
use super::{
    DriverEventSender, NativeTeamExit, NativeTeamStart, team_codex::EmptyDirectory,
    team_completion, team_gate,
};
use crate::{
    team_process::{bounded_process, bounded_rpc},
    teams::{ModelCallAllowance, TeamCoordinator},
};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::Duration,
};
use waku_protocol::model::{DriverEvent, ProviderKind};
struct Metadata {
    provider: String,
    id: String,
    api: String,
    upstream: &'static str,
    input: u64,
    output: u64,
}
const ISOLATION: &[&str] = &[
    "--offline",
    "--no-session",
    "--no-tools",
    "--no-extensions",
    "--no-skills",
    "--no-prompt-templates",
    "--no-themes",
    "--no-context-files",
    "--no-approve",
];
fn flags(binary: &Path) -> Result<()> {
    let mut cmd = crate::command_env::command(binary);
    cmd.args(ISOLATION)
        .arg("--help")
        .env("PI_SKIP_VERSION_CHECK", "1");
    let help = bounded_process(
        &mut cmd,
        b"",
        &AtomicBool::new(false),
        Duration::from_secs(30),
    )?;
    let help = std::str::from_utf8(&help)?;
    ensure!(
        ISOLATION.iter().all(|flag| help.contains(flag)),
        "Update Pi: verified Teams isolation flags are required"
    );
    Ok(())
}
fn metadata(binary: &Path, model: &str) -> Result<Metadata> {
    flags(binary)?;
    let cwd = EmptyDirectory::create("pi-catalog")?;
    let mut cmd = crate::command_env::command(binary);
    cmd.args(ISOLATION)
        .args(["--mode", "rpc"])
        .current_dir(&cwd.0)
        .env("PI_SKIP_VERSION_CHECK", "1");
    let bytes = bounded_rpc(
        &mut cmd,
        b"{\"id\":\"ion-models\",\"type\":\"get_available_models\"}\n",
        "ion-models",
        Duration::from_secs(25),
    )?;
    let response = std::str::from_utf8(&bytes)?
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find(|v| v["id"] == "ion-models" && v["type"] == "response")
        .context("Pi model response missing")?;
    ensure!(
        response["success"] == true && response["command"] == "get_available_models",
        "Pi model catalog unavailable"
    );
    from_catalog(&response["data"]["models"], model)
}
fn from_catalog(models: &Value, model: &str) -> Result<Metadata> {
    let (provider, id) = model
        .split_once('/')
        .context("Choose an exact Pi provider/model ID")?;
    let upstream = match provider {
        "openai" => "https://api.openai.com/v1",
        "anthropic" => "https://api.anthropic.com/v1",
        "nebius" => "https://api.tokenfactory.nebius.com/v1",
        _ => anyhow::bail!(
            "This Pi provider requires a verified Teams transport; direct OpenAI, Anthropic and Nebius routes are supported"
        ),
    };
    let mut entries = models
        .as_array()
        .context("Pi catalog missing")?
        .iter()
        .filter(|v| v["provider"].as_str() == Some(provider) && v["id"].as_str() == Some(id));
    let entry = entries.next().context("Pinned Pi model is unavailable")?;
    ensure!(
        entries.next().is_none() && entry["baseUrl"].as_str() == Some(upstream),
        "Pi model identity or endpoint is ambiguous/custom"
    );
    let api = entry["api"].as_str().context("Pi protocol missing")?;
    ensure!(
        if provider == "anthropic" {
            api == "anthropic-messages"
        } else {
            matches!(api, "openai-responses" | "openai-completions")
        },
        "Selected Pi API is unsupported"
    );
    let limit = |key: &str| {
        entry[key]
            .as_u64()
            .filter(|n| (1..=32_000_000).contains(n))
            .context("Pi token allowance missing")
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
fn auth(provider: &str) -> Result<Value> {
    let root = std::env::var_os("PI_CODING_AGENT_DIR")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|p| p.join(".pi/agent")))
        .context("Pi home unavailable")?;
    let path = root.join("auth.json");
    ensure!(
        std::fs::metadata(&path)
            .context("Sign in to the selected Pi provider with a direct API key")?
            .len()
            <= 256 * 1024,
        "Pi auth store exceeds its bound"
    );
    let auth: Value = serde_json::from_slice(&std::fs::read(path)?)?;
    let credential = &auth[provider];
    ensure!(
        credential["type"] == "api_key"
            && credential["key"]
                .as_str()
                .is_some_and(|k| !k.is_empty() && !k.starts_with(['!', '$'])),
        "Pi Teams requires a direct literal API login; OAuth and command-based auth are not replaced"
    );
    Ok(json!({provider:credential}))
}
pub(crate) fn ready(binary: &Path, model: &str) -> Result<()> {
    let meta = metadata(binary, model)?;
    auth(&meta.provider)?;
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
    .context("Pinned Pi model missing")?;
    let meta = metadata(binary, model)?;
    super::nebius::team::run_with_harness(
        coordinator,
        start,
        workspace,
        events,
        cancelled,
        ProviderKind::Pi,
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
fn completion(
    binary: &Path,
    model: &str,
    meta: &Metadata,
    system: &str,
    messages: &[Value],
    tools: &Value,
    cancelled: &AtomicBool,
) -> Result<super::nebius::StreamCompletion> {
    let gate = team_gate::Gate::api(&meta.id, meta.upstream)?;
    let result = launch(
        binary,
        meta,
        &auth(&meta.provider)?,
        &gate.url,
        system,
        messages,
        tools,
        cancelled,
    );
    let receipt = gate.receipt();
    result?;
    let receipt = receipt?;
    ensure!(
        model == format!("{}/{}", meta.provider, meta.id),
        "Pinned Pi model changed"
    );
    team_completion::actions(&receipt.text, tools, receipt.usage)
}
fn launch(
    binary: &Path,
    meta: &Metadata,
    auth: &Value,
    url: &str,
    system: &str,
    messages: &[Value],
    tools: &Value,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>> {
    let cwd = EmptyDirectory::create("pi")?;
    let files =
        ["auth.json", "models.json", "settings.json"].map(|name| PrivateFile(cwd.0.join(name)));
    let models = json!({"providers":{meta.provider.clone():{"baseUrl":url,"api":meta.api,"models":[{"id":meta.id,"contextWindow":meta.input,"maxTokens":meta.output,"input":["text"],"reasoning":false}]}}});
    for (file,value) in files.iter().zip([auth.clone(),models,json!({"retry":{"enabled":false},"compaction":{"enabled":false},"enableVersionCheck":false})]) {
        use std::io::Write;
        let mut options=std::fs::OpenOptions::new(); options.write(true).create_new(true);
        #[cfg(unix)] {use std::os::unix::fs::OpenOptionsExt; options.mode(0o600);}
        options.open(&file.0)?.write_all(&serde_json::to_vec(&value)?)?;
    }
    let input =
        serde_json::to_vec(&json!({"instructions":system,"messages":messages,"tools":tools}))?;
    ensure!(
        input.len() <= 128 * 1024,
        "Pi Teams context exceeds its bound"
    );
    let mut cmd = crate::command_env::command(binary);
    cmd.args(ISOLATION)
        .args([
            "--mode",
            "json",
            "--print",
            "--provider",
            &meta.provider,
            "--model",
            &meta.id,
            "--system-prompt",
            team_completion::INSTRUCTIONS,
        ])
        .current_dir(&cwd.0)
        .env("PI_CODING_AGENT_DIR", &cwd.0)
        .env("PI_SKIP_VERSION_CHECK", "1");
    // Use only the selected native account copied to this private auth store.
    // Inherited API keys must not override that account.
    for key in ["OPENAI_API_KEY", "ANTHROPIC_API_KEY", "NEBIUS_API_KEY"] {
        cmd.env_remove(key);
    }
    bounded_process(&mut cmd, &input, cancelled, Duration::from_secs(120))
}
struct PrivateFile(PathBuf);
impl Drop for PrivateFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
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
    let binary = crate::model::provider_probe(ProviderKind::Pi, None)
        .path
        .context("Pi worker executable missing")?;
    let meta = metadata(&binary, model)?;
    ensure!(
        meta.input <= input_cap && meta.output <= output_cap,
        "Registered Pi allowance is too small"
    );
    completion(&binary, model, &meta, system, messages, tools, cancelled)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn team_pi_catalog_pins_endpoint_protocol_and_token_caps() {
        let mut data = json!([{"provider":"openai","id":"pinned","api":"openai-responses","baseUrl":"https://api.openai.com/v1","contextWindow":100000,"maxTokens":8192}]);
        assert_eq!(from_catalog(&data, "openai/pinned").unwrap().output, 8192);
        data[0]["baseUrl"] = json!("https://custom.example/v1");
        assert!(from_catalog(&data, "openai/pinned").is_err());
        data[0]["baseUrl"] = json!("https://api.openai.com/v1");
        data[0]["maxTokens"] = Value::Null;
        assert!(from_catalog(&data, "openai/pinned").is_err());
    }
    #[test]
    #[ignore = "Requires installed current Pi; only fake authentication and loopback provider"]
    fn team_pi_installed_cli_uses_one_request_without_native_tools() {
        let _diagnostics = crate::team_process::fixture_diagnostics();
        let binary = std::env::var_os("ION_TEST_PI_BINARY")
            .map(PathBuf::from)
            .or_else(|| crate::model::provider_probe(ProviderKind::Pi, None).path)
            .unwrap();
        flags(&binary).unwrap();
        // Native cold startup can exceed the small HTTP-only fixture deadline.
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
        let meta = Metadata {
            provider: "openai".into(),
            id: "gpt-5.4".into(),
            api: "openai-responses".into(),
            upstream: "https://api.openai.com/v1",
            input: 272000,
            output: 32768,
        };
        let result = launch(
            &binary,
            &meta,
            &json!({"openai":{"type":"api_key","key":"sk-fixture-only"}}),
            &gate.url,
            "Return the Ion JSON action contract",
            &[],
            &json!([]),
            &AtomicBool::new(false),
        );
        let receipt = gate.receipt();
        let output = result.unwrap_or_else(|e| panic!("Installed Pi failed: {e:#}"));
        let receipt = receipt.unwrap_or_else(|e| {
            panic!(
                "Pi fixture gate failed: {e:#}; output={}",
                String::from_utf8_lossy(&output[..output.len().min(4096)])
            )
        });
        let requests = server.finish();
        assert_eq!(receipt.usage.input_tokens, 100);
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].body["tools"], json!([]));
        assert!(
            requests[0].headers.lines().any(|l| {
                l.split_once(':').is_some_and(|(name, value)| {
                    name.eq_ignore_ascii_case("authorization")
                        && value.trim() == "Bearer sk-fixture-only"
                })
            }),
            "The selected fixture account was not used"
        );
    }
}
