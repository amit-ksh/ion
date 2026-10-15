//! Stdin/stdout native hook transport. This process never loads daemon state or
//! provider accounts; the live owner validates identity and owns all effects.
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    time::Duration,
};

const LIMIT: usize = 44 * 1024;
const ENDPOINT: &str = "ION_SOURCE_TEAM_HOOK_ENDPOINT";
const TOKEN: &str = "ION_SOURCE_TEAM_HOOK_TOKEN";
const REJECTED: &str =
    "Ion Teams hook unavailable. Retry from the source task after checking its connection.";

/// Handle hook mode before daemon token/config/database initialization. Invalid
/// hook input must never fall through into ordinary daemon startup.
pub fn run_cli(arguments: &[String]) -> Option<i32> {
    if arguments.first().map(String::as_str) != Some("--source-team-hook") {
        return None;
    }
    let (value, code) = if arguments.len() == 2 && arguments[1] == "--help" {
        (
            json!({"description":"Internal native Teams hook: reads one bounded JSON event from stdin and returns native hook JSON.","environment":[ENDPOINT,TOKEN],"flags":["--help"]}),
            0,
        )
    } else if arguments.len() != 1 {
        (
            json!({"decision":"deny","reason":"Unknown native hook argument. Valid flags: --help."}),
            2,
        )
    } else {
        match request() {
            Ok(value) => (value, 0),
            Err(_) => (json!({"decision":"deny","reason":REJECTED}), 2),
        }
    };
    let mut stdout = std::io::stdout().lock();
    let written =
        serde_json::to_writer(&mut stdout, &value).is_ok() && stdout.write_all(b"\n").is_ok();
    Some(if written { code } else { 2 })
}
fn request() -> anyhow::Result<Value> {
    let endpoint = std::env::var(ENDPOINT)?;
    let parsed = url::Url::parse(&endpoint)?;
    // Require the literal owning loopback route, without normalization aliases,
    // credentials, redirects or proxy discovery.
    let port = parsed
        .port()
        .filter(|port| *port != 0)
        .ok_or_else(|| anyhow::anyhow!("Invalid hook endpoint"))?;
    anyhow::ensure!(
        endpoint == format!("http://127.0.0.1:{port}/hook"),
        "Invalid hook endpoint"
    );
    let token = std::env::var(TOKEN)?;
    anyhow::ensure!(
        (32..=256).contains(&token.len())
            && token
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')),
        "Invalid hook token"
    );
    let mut bytes = Vec::new();
    std::io::stdin()
        .take((LIMIT + 1) as u64)
        .read_to_end(&mut bytes)?;
    anyhow::ensure!(bytes.len() <= LIMIT, "Hook input exceeds limit");
    let event: Value = serde_json::from_slice(&bytes)?;
    anyhow::ensure!(event.is_object(), "Hook event must be an object");
    let mut response = reqwest::blocking::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(1))
        .timeout(Duration::from_secs(5))
        .build()?
        .post(parsed)
        .bearer_auth(token)
        .json(&event)
        .send()?
        .error_for_status()?;
    anyhow::ensure!(
        response.status().is_success(),
        "Hook response was not successful"
    );
    let mut output = Vec::new();
    response
        .by_ref()
        .take((LIMIT + 1) as u64)
        .read_to_end(&mut output)?;
    anyhow::ensure!(output.len() <= LIMIT, "Hook response exceeds limit");
    let value: Value = serde_json::from_slice(&output)?;
    anyhow::ensure!(value.is_object(), "Hook response must be an object");
    Ok(value)
}
