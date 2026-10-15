//! Released Grok through the production Ion ACP driver, with its shipped
//! command helper, isolated fake account and localhost-only model transport.
use super::*;
use crate::driver::{DriverControl, DriverStartOptions, acp::AcpDriver};
use agent_client_protocol::AcpAgentConfig;
use std::{
    fs,
    path::{Path, PathBuf},
};
use waku_protocol::model::ProviderResumeCursor;

#[test]
#[ignore = "Requires ION_TEST_GROK_BINARY=1.0.46 and ION_TEST_ION_DAEMON; private profile/loopback fake model only"]
fn grok_source_team_native_driver_approved_invocation_resume_and_stale_definition() {
    native_fixture(false, true);
}
#[test]
#[ignore = "Requires ION_TEST_GROK_BINARY=1.0.46 and ION_TEST_ION_DAEMON; private profile/loopback fake model only"]
fn grok_source_team_native_driver_stop_at_owner_approval() {
    native_fixture(true, true);
}
#[test]
#[ignore = "Requires ION_TEST_GROK_BINARY=1.0.46 and ION_TEST_ION_DAEMON; private profile/loopback fake model only"]
fn grok_source_team_native_driver_untrusted_project_cannot_invoke() {
    native_fixture(false, false);
}

fn sse(items: Vec<Value>) -> Vec<u8> {
    let response = json!({"id":format!("resp_{}",Uuid::new_v4().simple()),"object":"response","created_at":1,"status":"in_progress","model":"grok-4.6","output":[],"usage":null});
    let mut events = vec![json!({"type":"response.created","response":response})];
    for (index, item) in items.iter().enumerate() {
        let mut initial = item.clone();
        initial["status"] = json!("in_progress");
        if item["type"] == "function_call" {
            initial["arguments"] = json!("");
            events.push(
                json!({"type":"response.output_item.added","output_index":index,"item":initial}),
            );
            events.push(json!({"type":"response.function_call_arguments.delta","item_id":item["id"],"output_index":index,"delta":item["arguments"]}));
            events.push(json!({"type":"response.function_call_arguments.done","item_id":item["id"],"output_index":index,"arguments":item["arguments"]}));
        } else {
            initial["content"] = json!([]);
            events.push(
                json!({"type":"response.output_item.added","output_index":index,"item":initial}),
            );
            events.push(json!({"type":"response.content_part.added","item_id":item["id"],"output_index":index,"content_index":0,"part":{"type":"output_text","text":"","annotations":[]}}));
            events.push(json!({"type":"response.output_text.delta","item_id":item["id"],"output_index":index,"content_index":0,"delta":item["content"][0]["text"]}));
            events.push(json!({"type":"response.output_text.done","item_id":item["id"],"output_index":index,"content_index":0,"text":item["content"][0]["text"]}));
        }
        events.push(json!({"type":"response.output_item.done","output_index":index,"item":item}));
    }
    let mut completed = response;
    completed["status"] = json!("completed");
    completed["output"] = json!(items);
    completed["usage"] = json!({"input_tokens":100,"output_tokens":5,"total_tokens":105,"input_tokens_details":{"cached_tokens":0},"output_tokens_details":{"reasoning_tokens":0}});
    events.push(json!({"type":"response.completed","response":completed}));
    events
        .into_iter()
        .enumerate()
        .map(|(sequence, mut event)| {
            event["sequence_number"] = json!(sequence);
            format!(
                "event: {}\ndata: {event}\n\n",
                event["type"].as_str().unwrap()
            )
        })
        .collect::<String>()
        .into_bytes()
}
fn function(call: &str, name: &str, args: Value) -> Vec<u8> {
    sse(vec![
        json!({"id":format!("fc_{call}"),"type":"function_call","call_id":call,"name":name,"status":"completed","arguments":args.to_string()}),
    ])
}
fn private_config(config: AcpAgentConfig, root: &Path) -> AcpAgentConfig {
    // The SDK inherits its environment. Empty all inherited/merged values before
    // adding the explicit fixture allowlist, private paths and owned hook keys.
    let empty = std::env::vars()
        .map(|(key, _)| {
            (
                if cfg!(windows) {
                    key.to_ascii_uppercase()
                } else {
                    key
                },
                String::new(),
            )
        })
        .chain(config.environment().keys().map(|key| {
            (
                if cfg!(windows) {
                    key.to_ascii_uppercase()
                } else {
                    key.clone()
                },
                String::new(),
            )
        }));
    let mut result = AcpAgentConfig::new(config.command()).envs(empty);
    if cfg!(windows) {
        result = result.args([
            "--cwd".into(),
            root.join("work").to_string_lossy().into_owned(),
        ]);
    }
    result = result.args(config.arguments().to_owned());
    for name in [
        "PATH",
        "PATHEXT",
        "SystemRoot",
        "WINDIR",
        "COMSPEC",
        "LANG",
        "LC_ALL",
    ] {
        if let Ok(value) = std::env::var(name) {
            result = result.env(
                if cfg!(windows) {
                    name.to_ascii_uppercase()
                } else {
                    name.into()
                },
                value,
            );
        }
    }
    for name in [
        "ION_SOURCE_TEAM_HOOK_ENDPOINT",
        "ION_SOURCE_TEAM_HOOK_TOKEN",
    ] {
        result = result.env(
            name,
            config
                .environment()
                .get(name)
                .expect("Production bridge environment"),
        );
    }
    for (name, path) in [
        ("HOME", "home"),
        ("USERPROFILE", "home"),
        ("GROK_HOME", "home"),
        ("APPDATA", "home/AppData/Roaming"),
        ("LOCALAPPDATA", "home/AppData/Local"),
        ("TEMP", "temp"),
        ("TMP", "temp"),
    ] {
        result = result.env(name, root.join(path).to_string_lossy());
    }
    result = result
        .env("ION_GROK_NATIVE_KEY", "fixture-only-not-real")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_COUNT", "0")
        .env(
            "GIT_CONFIG_GLOBAL",
            root.join("empty-git-config").to_string_lossy(),
        )
        .env("GROK_SAMPLER_SHARED_CLIENT", "0")
        .env("GROK_DISABLE_AUTOUPDATER", "1")
        .env("GROK_TELEMETRY_ENABLED", "0")
        .env("GROK_TELEMETRY_TRACE_UPLOAD", "0")
        .env("GROK_WORKSPACE_UPLOAD_QUEUE_ENABLED", "false")
        .env("GROK_MANAGED_CONFIG", "0")
        .env("GROK_FOLDER_TRUST", "1")
        .env("OTEL_TRACES_EXPORTER", "none")
        .env("RUST_LOG", "xai_grok_hooks::discovery=debug");
    if cfg!(windows) {
        result = result.env("GROK_SHELL", "powershell");
    }
    for name in [
        "CRASH_HANDLER",
        "TURN_SUMMARY",
        "TITLE_REFRESH",
        "GOAL_SUMMARY",
        "PROMPT_SUGGESTIONS",
        "MEMORY",
        "SUBAGENTS",
        "WEB_FETCH",
        "WRITE_FILE",
        "TOOL_SEARCH",
        "LSP_TOOLS",
        "MANAGED_MCPS_ENABLED",
        "MANAGED_MCP_GATEWAY_TOOLS_ENABLED",
        "OFFICIAL_MARKETPLACE_AUTO_REGISTER",
        "AGENT_DASHBOARD",
    ] {
        result = result.env(format!("GROK_{name}"), "0");
    }
    for vendor in ["CURSOR", "CLAUDE", "CODEX"] {
        for surface in ["SKILLS", "RULES", "AGENTS", "MCPS", "HOOKS", "SESSIONS"] {
            result = result.env(format!("GROK_{vendor}_{surface}_ENABLED"), "0");
        }
    }
    result
}
fn start(
    f: &Fixture,
    binary: &Path,
    cursor: Option<ProviderResumeCursor>,
) -> (AcpDriver, Receiver<DriverEvent>) {
    let (events, received) = crate::driver::test_event_channel();
    let driver = AcpDriver::start_configured(
        ProviderKind::Grok,
        DriverStartOptions {
            binary: binary.into(),
            cwd: f.directory.0.join("work"),
            mode: RuntimeMode::Ask,
            model: Some("grok-4.6".into()),
            reasoning_effort: None,
            service_tier: None,
            context_window: None,
            agent_preset: None,
            computer_use_enabled: false,
            provider_cursor: cursor,
        },
        events,
        Some(f.context.clone()),
        |config| private_config(config, &f.directory.0),
    )
    .unwrap();
    (driver, received)
}
fn connected(events: &Receiver<DriverEvent>) -> ProviderResumeCursor {
    let deadline = Instant::now() + Duration::from_secs(40);
    loop {
        match events
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("Native connection")
        {
            DriverEvent::Connected {
                provider_cursor: Some(cursor),
            } => return cursor,
            DriverEvent::Error(error) => panic!("Native connection: {error}"),
            DriverEvent::Activity { title, detail, .. } if title == "Team tools unavailable" => {
                panic!("Missing production bridge: {detail:?}")
            }
            _ => {}
        }
    }
}
fn stopped(driver: AcpDriver, events: &Receiver<DriverEvent>) {
    drop(driver);
    let deadline = Instant::now() + Duration::from_secs(12);
    while !matches!(
        events
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("Native shutdown"),
        DriverEvent::ProcessExited
    ) {}
}
fn turn(
    driver: &AcpDriver,
    events: &Receiver<DriverEvent>,
    stop: bool,
    expect_approval: bool,
) -> Vec<DriverEvent> {
    driver.prompt("Ask the saved Engineering team to implement the reviewed work. Use its exact definition revision.".into());
    let deadline = Instant::now() + Duration::from_secs(65);
    let mut seen = Vec::new();
    let mut approved = false;
    loop {
        let event = events
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap_or_else(|_| panic!("Native turn timed out; events={seen:?}"));
        if let DriverEvent::Permission {
            request_id,
            options,
            ..
        } = &event
        {
            let owner_approval = request_id.starts_with("ion-team-approval-");
            if owner_approval {
                assert!(expect_approval, "Unexpected Ion approval");
                approved = true;
            }
            if stop && owner_approval {
                driver.cancel();
            } else {
                let option = options
                    .iter()
                    .find(|option| option.allow)
                    .expect("Native one-time allow option");
                driver.respond(request_id.clone(), option.id.clone());
            }
        }

        if let DriverEvent::Error(error) = &event {
            panic!("Native turn failed: {error}; events={seen:?}");
        }
        let finished = matches!(event, DriverEvent::TurnFinished { .. });
        seen.push(event);
        if finished {
            break;
        }
    }
    assert_eq!(
        approved, expect_approval,
        "Missing or unexpected owner approval; events={seen:?}"
    );
    seen
}
fn native_fixture(stop: bool, trusted: bool) {
    let binary =
        PathBuf::from(std::env::var_os("ION_TEST_GROK_BINARY").expect("Released Grok executable"));
    let helper = PathBuf::from(
        std::env::var_os("ION_TEST_ION_DAEMON").expect("Shipped Ion helper executable"),
    );
    assert!(helper.is_file());
    let f = Fixture::new();
    let root = &f.directory.0;
    for directory in [
        "home",
        "temp",
        "work/.git/objects",
        "work/.git/refs",
        "work/.grok/hooks",
    ] {
        fs::create_dir_all(root.join(directory)).unwrap();
    }
    fs::write(root.join("empty-git-config"), "").unwrap();
    fs::write(root.join("work/.git/HEAD"), "ref: refs/heads/main\n").unwrap();
    fs::write(
        root.join("work/.git/config"),
        "[core]\nrepositoryformatversion=0\nbare=false\n",
    )
    .unwrap();
    let owner = root.join("work/.grok/hooks/owner.json");
    let owner_bytes = r#"{"hooks":{"PreToolUse":[]}}"#;
    fs::write(&owner, owner_bytes).unwrap();
    if trusted {
        let path = fs::canonicalize(root.join("work"))
            .unwrap()
            .to_string_lossy()
            .trim_start_matches(r"\\?\")
            .to_owned();
        fs::write(root.join("home/trusted-hook-projects"), format!("{path}\n")).unwrap();
        fs::write(
            root.join("home/trusted_folders.toml"),
            format!(
                "[folders.{}]\ntrusted=true\ndecided_at=1780000000\n",
                serde_json::to_string(&path).unwrap()
            ),
        )
        .unwrap();
    }
    let prefix = format!("ion_teams_{}", f.context.task_id.simple());
    let args = json!({"tool_name":format!("{prefix}__invoke_team"),"tool_input":f.args()});
    let mut replies = vec![(200, function("ion-native-invoke", "use_tool", args.clone()))];
    if !stop {
        replies.push((
            200,
            crate::driver::team_test_http::responses("grok-4.6", "Team queued"),
        ));
        if trusted {
            replies.push((200, function("ion-native-invoke", "use_tool", args.clone())));
            replies.push((
                200,
                crate::driver::team_test_http::responses("grok-4.6", "Replay acknowledged"),
            ));
            replies.push((200, function("ion-native-stale", "use_tool", args)));
            replies.push((
                200,
                crate::driver::team_test_http::responses("grok-4.6", "Changed definition refused"),
            ));
        }
    }
    let model = crate::driver::team_test_http::Server::with_auxiliary(replies, |request| {
        assert!(
            request.line.starts_with("POST /responses "),
            "Unexpected native route: {}",
            request.line
        );
        assert!(
            request
                .headers
                .to_ascii_lowercase()
                .contains("authorization: bearer fixture-only-not-real")
        );
        let tools = request.body["tools"].as_array().unwrap();
        (tools.len() == 1 && tools[0]["name"] == "session_title").then(|| {
            function(
                "ion-native-title",
                "session_title",
                json!({"session_title":"Ion native fixture"}),
            )
        })
    });
    fs::write(
        root.join("home/config.toml"),
        format!(
            r#"[models]
default="grok-4.6"
max_retries=0
[model."grok-4.6"]
model="grok-4.6"
base_url="{}"
env_key="ION_GROK_NATIVE_KEY"
api_backend="responses"
context_window=500000
max_completion_tokens=32768
max_retries=0
supports_backend_search=false
[features]
remote_fetch=false
managed_config=false
telemetry="off"
codebase_indexing=false
title_refresh=false
[telemetry]
trace_upload=false
[cli]
auto_update=false
[endpoints]
cli_chat_proxy_base_url="http://127.0.0.1:1"
xai_api_base_url="http://127.0.0.1:1"
"#,
            model.url
        ),
    )
    .unwrap();
    let (driver, events) = start(&f, &binary, None);
    let cursor = connected(&events);
    let first = turn(&driver, &events, stop, trusted);
    let generated = owner
        .parent()
        .unwrap()
        .join(format!("ion-teams-{}.json", f.context.task_id.simple()));
    let generated_bytes = fs::read(&generated).unwrap();
    assert!(!String::from_utf8_lossy(&generated_bytes).contains("Bearer"));
    if stop || !trusted {
        assert!(f.runs().is_empty());
    } else {
        assert_eq!(f.runs().len(), 1);
        assert_eq!(f.runs()[0].project_id, f.context.project_id);
    }
    assert!(!format!("{first:?}").contains(FIELD));
    if trusted && !stop {
        let (replacement, replacement_events) = start(&f, &binary, Some(cursor.clone()));
        assert_eq!(connected(&replacement_events), cursor);
        stopped(driver, &events);
        assert_eq!(fs::read(&generated).unwrap(), generated_bytes);
        // An exact durable native-call replay returns the same queued run with
        // no second approval or second persisted invocation.
        turn(&replacement, &replacement_events, false, false);
        assert_eq!(f.runs().len(), 1);
        let store = crate::control::ControlStore::new(f.context.control_path.clone());
        let mut team = f.runs()[0].team.clone();
        team.objective = "Definition changed by the owner".into();
        store
            .save(store.load().unwrap().revision, ControlRecord::Team(team))
            .unwrap();
        assert!(store.load().unwrap().revision > f.revision);
        let stale = turn(&replacement, &replacement_events, false, false);
        assert!(!format!("{stale:?}").contains(FIELD));
        assert_eq!(f.runs().len(), 1, "Changed native definition enqueued work");
        stopped(replacement, &replacement_events);
    } else {
        stopped(driver, &events);
    }
    let requests = model.finish();
    let main: Vec<_> = requests
        .iter()
        .filter(|request| {
            !(request.body["tools"]
                .as_array()
                .is_some_and(|tools| tools.len() == 1 && tools[0]["name"] == "session_title"))
        })
        .collect();
    assert_eq!(
        main.len(),
        if stop {
            1
        } else if trusted {
            6
        } else {
            2
        }
    );
    assert!(
        main[0].body["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["name"] == "use_tool")
    );
    for request in &main {
        for item in request.body["input"].as_array().unwrap() {
            if item["type"] == "function_call" && item["call_id"] == "ion-native-invoke" {
                let authored: Value =
                    serde_json::from_str(item["arguments"].as_str().unwrap()).unwrap();
                assert_eq!(
                    authored,
                    json!({"tool_name":format!("{prefix}__invoke_team"),"tool_input":f.args()})
                );
            }
        }
    }
    if !stop {
        let outputs: Vec<_> = main[1].body["input"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["type"] == "function_call_output")
            .collect();
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0]["call_id"], "ion-native-invoke");
        if trusted {
            assert!(
                outputs[0]["output"]
                    .as_str()
                    .unwrap()
                    .contains(&f.runs()[0].id.to_string()),
                "Missing durable invocation result: {outputs:?}"
            );
        } else {
            assert!(
                outputs[0]["output"]
                    .as_str()
                    .unwrap()
                    .contains("not authorized"),
                "Untrusted native result: {outputs:?}"
            );
        }
    }
    if trusted && !stop {
        let refused = main[5].body["input"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| {
                item["type"] == "function_call_output" && item["call_id"] == "ion-native-stale"
            })
            .expect("Native changed-definition result returned to the provider");
        assert!(
            refused["output"]
                .as_str()
                .unwrap()
                .contains("Ion team request did not complete"),
            "Changed-definition request did not reach the shared worker: {refused}"
        );
    }
    let deadline = Instant::now() + Duration::from_secs(4);
    while generated.exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
    }
    assert!(
        !generated.exists(),
        "Owned native hook leaked after final shutdown"
    );
    assert_eq!(fs::read_to_string(&owner).unwrap(), owner_bytes);
}
