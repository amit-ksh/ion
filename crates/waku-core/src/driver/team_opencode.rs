//! OpenCode's pure print mode, direct provider auth and one-use model transport.
//! Custom/OAuth plugin routes are not guessed or replaced with API credentials.
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
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::Duration,
};
use waku_protocol::model::{DriverEvent, ProviderKind};

struct Metadata {
    provider: String,
    id: String,
    upstream: &'static str,
    input: u64,
    output: u64,
}
fn metadata(binary: &Path, model: &str) -> Result<Metadata> {
    let (provider, id) = model
        .split_once('/')
        .context("Choose an exact provider/model ID for OpenCode")?;
    let upstream = match provider {
        "openai" => "https://api.openai.com/v1",
        "anthropic" => "https://api.anthropic.com/v1",
        "nebius" => "https://api.tokenfactory.nebius.com/v1",
        _ => anyhow::bail!(
            "This OpenCode provider needs a verified bounded transport; direct OpenAI, Anthropic and Nebius API routes are supported"
        ),
    };
    let cwd = EmptyDirectory::create("opencode-catalog")?;
    let mut command = crate::command_env::command(binary);
    command
        .args(["models", provider, "--verbose"])
        .current_dir(&cwd.0)
        .env("XDG_CONFIG_HOME", cwd.0.join("config"))
        .env("XDG_DATA_HOME", cwd.0.join("data"))
        .env("OPENCODE_CONFIG_DIR", cwd.0.join("config"))
        .env(
            "OPENCODE_CONFIG_CONTENT",
            r#"{"plugin":[],"mcp":{},"autoupdate":false}"#,
        )
        .env_remove("OPENCODE_CONFIG")
        .env("OPENCODE_DISABLE_AUTOUPDATE", "true")
        .env("OPENCODE_DISABLE_DEFAULT_PLUGINS", "true")
        .env("OPENCODE_DISABLE_MODELS_FETCH", "true")
        .env_remove("OPENCODE_AUTH_CONTENT");
    let bytes = bounded_process(
        &mut command,
        b"",
        &AtomicBool::new(false),
        Duration::from_secs(45),
    )?;
    let entries = catalog(std::str::from_utf8(&bytes)?)?;
    let mut matching = entries
        .iter()
        .filter(|v| v["providerID"].as_str() == Some(provider) && v["id"].as_str() == Some(id));
    let entry = matching
        .next()
        .context("Pinned OpenCode model metadata is unavailable")?;
    ensure!(
        matching.next().is_none(),
        "OpenCode catalog identity is ambiguous"
    );
    let input = entry["limit"]["context"]
        .as_u64()
        .filter(|n| (1..=32_000_000).contains(n))
        .context("OpenCode context limit is unavailable")?;
    let output = entry["limit"]["output"]
        .as_u64()
        .filter(|n| (1..=32_000_000).contains(n))
        .context("OpenCode output limit is unavailable")?;
    ensure!(
        entry["api"]["url"].as_str() == Some(upstream),
        "Selected OpenCode model has a custom or unknown endpoint"
    );
    ensure!(
        entry["api"]["id"].as_str().is_none_or(|api| api == id),
        "Pinned OpenCode model is an alias; select its exact API model ID"
    );
    Ok(Metadata {
        provider: provider.into(),
        id: id.into(),
        upstream,
        input,
        output,
    })
}
fn catalog(text: &str) -> Result<Vec<Value>> {
    let mut values = Vec::new();
    let mut rest = text;
    // Native verbose listing uses model-id lines followed by standalone JSON.
    while let Some(start) = rest.find('{') {
        let mut stream = serde_json::Deserializer::from_str(&rest[start..]).into_iter::<Value>();
        let value = stream.next().context("OpenCode model metadata missing")??;
        let length = stream.byte_offset();
        ensure!(length > 0, "Invalid OpenCode catalog block");
        values.push(value);
        rest = &rest[start + length..];
    }
    Ok(values)
}
fn auth(provider: &str) -> Result<Value> {
    let home = dirs::home_dir().context("OpenCode home is unavailable")?;
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share"));
    let path = data.join("opencode/auth.json");
    ensure!(
        std::fs::metadata(&path)
            .context("Sign in with opencode auth login using a direct provider API key")?
            .len()
            <= 256 * 1024,
        "OpenCode auth store exceeds its bound"
    );
    let value: Value = serde_json::from_slice(&std::fs::read(path)?)?;
    let entry = &value[provider];
    ensure!(
        entry["type"] == "api" && entry["key"].as_str().is_some_and(|k| !k.is_empty()),
        "This Teams transport requires the selected OpenCode provider's direct API login; OAuth/custom plugin routes are not replaced"
    );
    Ok(json!({provider:entry}))
}
fn flags(binary: &Path) -> Result<()> {
    let mut command = crate::command_env::command(binary);
    command.args(["run", "--help"]);
    let help = bounded_process(
        &mut command,
        b"",
        &AtomicBool::new(false),
        Duration::from_secs(30),
    )?;
    ensure!(
        std::str::from_utf8(&help)?.contains("--pure"),
        "Update OpenCode: pure mode is required for Teams"
    );
    Ok(())
}
pub(crate) fn ready(binary: &Path, model: &str) -> Result<()> {
    let metadata = metadata(binary, model)?;
    auth(&metadata.provider)?;
    flags(binary)
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
    .context("Pinned OpenCode model missing")?;
    let meta = metadata(binary, model)?;
    super::nebius::team::run_with_harness(
        coordinator,
        start,
        workspace,
        events,
        cancelled,
        ProviderKind::OpenCode,
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
        model,
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
    team_completion::actions(&receipt.text, tools, receipt.usage)
}
fn launch(
    binary: &Path,
    model: &str,
    meta: &Metadata,
    selected_auth: &Value,
    url: &str,
    system: &str,
    messages: &[Value],
    tools: &Value,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>> {
    let cwd = EmptyDirectory::create("opencode")?;
    let auth_path = cwd.0.join("data/opencode");
    std::fs::create_dir_all(&auth_path)?;
    let auth_file = PrivateCredential(auth_path.join("auth.json"));
    // Copy only the selected account into a private process-local data root.
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    use std::io::Write;
    options
        .open(&auth_file.0)?
        .write_all(&serde_json::to_vec(&selected_auth.clone())?)?;
    let config = json!({"share":"disabled","autoupdate":false,"plugin":[],"mcp":{},"permission":{"*":"deny"},"compaction":{"auto":false},"agent":{"title":{"disable":true},"summary":{"disable":true},"compaction":{"disable":true},"ion-team":{"mode":"primary","description":"Bounded Ion Teams completion","prompt":team_completion::INSTRUCTIONS,"model":model,"steps":1,"permission":{"*":"deny"}}},"provider":{meta.provider.clone():{"options":{"baseURL":url},"models":{meta.id.clone():{"id":meta.id,"limit":{"context":meta.input,"output":meta.output}}}}}});
    let mut command = crate::command_env::command(binary);
    command
        .args([
            "run",
            "--pure",
            "--format",
            "json",
            "--agent",
            "ion-team",
            "--model",
            model,
            "--title",
            "Ion Teams completion",
        ])
        .current_dir(&cwd.0)
        .env("XDG_CONFIG_HOME", cwd.0.join("config"))
        .env("XDG_DATA_HOME", cwd.0.join("data"))
        .env("OPENCODE_CONFIG_DIR", cwd.0.join("config"))
        .env("OPENCODE_CONFIG_CONTENT", serde_json::to_string(&config)?)
        .env_remove("OPENCODE_CONFIG")
        .env_remove("OPENCODE_MODELS_URL")
        .env("OPENCODE_DISABLE_AUTOUPDATE", "true")
        .env("OPENCODE_DISABLE_DEFAULT_PLUGINS", "true")
        .env("OPENCODE_DISABLE_MODELS_FETCH", "true")
        .env_remove("OPENCODE_AUTH_CONTENT")
        .env("OPENCODE_DISABLE_AUTOCOMPACT", "true")
        .env("OPENCODE_DISABLE_LSP_DOWNLOAD", "true")
        .env("OPENCODE_DISABLE_CLAUDE_CODE", "true");
    // Use only the selected native account copied to this private auth store.
    // Inherited API keys must not override that account.
    for key in ["OPENAI_API_KEY", "ANTHROPIC_API_KEY", "NEBIUS_API_KEY"] {
        command.env_remove(key);
    }
    let input =
        serde_json::to_vec(&json!({"instructions":system,"messages":messages,"tools":tools}))?;
    ensure!(
        input.len() <= 128 * 1024,
        "OpenCode Teams context exceeds its limit"
    );
    let result = bounded_process(&mut command, &input, cancelled, Duration::from_secs(120));
    // Always remove the one exact credential file, including errors/cancellation.
    drop(auth_file);
    let _ = std::fs::remove_dir(&auth_path);
    result
}
// Keep cleanup tied to scope, including serialization and pre-spawn errors.
struct PrivateCredential(PathBuf);
impl Drop for PrivateCredential {
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
    let binary = crate::model::provider_probe(ProviderKind::OpenCode, None)
        .path
        .context("OpenCode worker executable missing")?;
    // Probe the native catalog once per worker request, not twice via ready.
    flags(&binary)?;
    let meta = metadata(&binary, model)?;
    ensure!(
        meta.input <= input_cap && meta.output <= output_cap,
        "Registered OpenCode allowance is too small"
    );
    completion(&binary, model, &meta, system, messages, tools, cancelled)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn team_opencode_catalog_uses_native_exact_model_metadata() {
        let entries=catalog("anthropic/exact\n{\"id\":\"exact\",\"providerID\":\"anthropic\",\"limit\":{\"context\":200000,\"output\":8192}}\nopenai/exact\n{\"id\":\"exact\",\"providerID\":\"openai\"}\n").unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["limit"]["context"], 200000);
        assert!(catalog("bad\n{invalid}").is_err());
    }
}

#[cfg(test)]
mod installed_tests {
    use super::*;
    #[test]
    #[ignore = "Requires installed OpenCode pure mode; only fake authentication and loopback provider"]
    fn team_opencode_installed_cli_uses_one_request_without_native_tools() {
        let _diagnostics = crate::team_process::fixture_diagnostics();
        let binary = std::env::var_os("ION_TEST_OPENCODE_BINARY")
            .map(PathBuf::from)
            .or_else(|| crate::model::provider_probe(ProviderKind::OpenCode, None).path)
            .unwrap();
        let server = crate::driver::team_test_http::Server::new(vec![(
            200,
            crate::driver::team_test_http::responses("gpt-5.4", r#"{"text":"ready","calls":[]}"#),
        )]);
        let gate = team_gate::Gate::start("gpt-5.4", &format!("{}/responses", server.url)).unwrap();
        let meta = Metadata {
            provider: "openai".into(),
            id: "gpt-5.4".into(),
            upstream: "https://api.openai.com/v1",
            input: 272000,
            output: 32768,
        };
        let result = launch(
            &binary,
            "openai/gpt-5.4",
            &meta,
            &json!({"openai":{"type":"api","key":"sk-fixture-only"}}),
            &gate.url,
            "Return the Ion action contract",
            &[],
            &json!([]),
            &AtomicBool::new(false),
        );
        let receipt = gate.receipt();
        if let Err(e) = result {
            panic!("Installed OpenCode failed: {e:#}");
        }
        let requests = server.finish();
        assert_eq!(receipt.unwrap().usage.input_tokens, 100);
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
