use super::*;
use crate::driver::{DriverControl, DriverStartOptions, antigravity::AntigravityDriver};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex as StdMutex,
};
use waku_protocol::model::ProviderResumeCursor;
#[test]
#[ignore = "Requires released ION_TEST_ANTIGRAVITY_BINARY=1.3.1; isolated fake native account and localhost only"]
fn antigravity_source_team_native_driver_approved_resume_stale_and_cleanup() {
    native_fixture(false);
}
#[test]
#[ignore = "Requires released ION_TEST_ANTIGRAVITY_BINARY=1.3.1; isolated fake native account and localhost only"]
fn antigravity_source_team_native_driver_stop_approval_cancels_queue_and_recovers() {
    native_fixture(true);
}
fn sse(parts: Value) -> Vec<u8> {
    format!("data: {}\n\n",json!({"modelVersion":"gemini-3.1-pro-preview","candidates":[{"index":0,"content":{"role":"model","parts":parts},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":100,"candidatesTokenCount":12,"totalTokenCount":112}})).into_bytes()
}
fn function(args: Value) -> Vec<u8> {
    sse(
        json!([{"functionCall":{"name":"call_mcp_tool","args":{"ServerName":"ION_NATIVE_SERVER","ToolName":"invoke_team","Arguments":args,"toolSummary":"Saved team","toolAction":"Invoking team"}}}]),
    )
}
fn plugin_paths(root: &Path) -> Vec<PathBuf> {
    fs::read_dir(root.join("work/.agents/plugins"))
        .unwrap()
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("ion-teams-")
        })
        .collect()
}
fn start(
    f: &Fixture,
    binary: &Path,
    url: &str,
    server: Arc<StdMutex<String>>,
    cursor: Option<ProviderResumeCursor>,
) -> (AntigravityDriver, Receiver<DriverEvent>) {
    let root = f.directory.0.clone();
    let model = url.to_owned();
    let before = plugin_paths(&root);
    let (events, received) = crate::driver::test_event_channel();
    let configure = Arc::new(move |command: &mut Command| {
        command.env_clear();
        for (key, value) in std::env::vars().filter(|(key, _)| {
            [
                "PATH",
                "PATHEXT",
                "SYSTEMROOT",
                "WINDIR",
                "COMSPEC",
                "LANG",
                "LC_ALL",
            ]
            .contains(&key.to_ascii_uppercase().as_str())
        }) {
            command.env(
                if cfg!(windows) {
                    key.to_ascii_uppercase()
                } else {
                    key
                },
                value,
            );
        }
        for (key, path) in [
            ("HOME", "home"),
            ("USERPROFILE", "home"),
            ("APPDATA", "home/AppData/Roaming"),
            ("LOCALAPPDATA", "home/AppData/Local"),
            ("TEMP", "temp"),
            ("TMP", "temp"),
            ("TMPDIR", "temp"),
        ] {
            command.env(key, root.join(path));
        }
        command
            .env("GEMINI_API_KEY", "fixture-only")
            .env("GOOGLE_GEMINI_BASE_URL", &model)
            .env("AGY_CLI_DISABLE_AUTO_UPDATE", "true")
            .env("GIT_CONFIG_COUNT", "0")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", root.join("empty-git-config"));
        if command.get_args().any(|arg| arg == "--input-format") {
            let paths: Vec<_> = plugin_paths(&root)
                .into_iter()
                .filter(|p| !before.contains(p))
                .collect();
            assert_eq!(paths.len(), 1, "Missing uniquely owned native plugin");
            *server.lock().unwrap() =
                format!("{}_teams", paths[0].file_name().unwrap().to_str().unwrap());
        }
    });
    let driver = AntigravityDriver::start_configured(
        DriverStartOptions {
            binary: binary.into(),
            cwd: f.directory.0.join("work"),
            mode: RuntimeMode::FullAccess,
            model: Some("gemini-3.1-pro-low".into()),
            reasoning_effort: None,
            service_tier: None,
            context_window: None,
            agent_preset: None,
            computer_use_enabled: false,
            provider_cursor: cursor,
        },
        events,
        Some(f.context.clone()),
        configure,
    )
    .unwrap();
    (driver, received)
}
fn turn(
    driver: &AntigravityDriver,
    events: &Receiver<DriverEvent>,
    stop: bool,
    expect_approval: bool,
) -> ProviderResumeCursor {
    driver.prompt("Ask the saved Engineering team to implement reviewed work using its exact definition revision.".into());
    let until = Instant::now() + Duration::from_secs(80);
    let mut seen = Vec::new();
    let mut approved = false;
    let mut cursor = None;
    loop {
        let event = events
            .recv_timeout(until.saturating_duration_since(Instant::now()))
            .unwrap_or_else(|_| panic!("Native turn timed out: {seen:?}"));
        match &event {
            DriverEvent::Connected {
                provider_cursor: Some(value),
            } => cursor = Some(value.clone()),
            DriverEvent::Permission { request_id, .. }
                if request_id.starts_with("ion-team-approval-") =>
            {
                assert!(expect_approval, "Unexpected native owner approval");
                approved = true;
                if stop {
                    driver.prompt(
                        "Queued before Stop; this prompt must not reach the provider.".into(),
                    );
                    driver.cancel();
                }
                driver.respond(request_id.clone(), "allow".into());
            }
            DriverEvent::Activity { title, detail, .. } if title == "Team tools unavailable" => {
                panic!("Missing production adapter: {detail:?}")
            }
            DriverEvent::Error(error) => panic!("Native turn failed: {error}; events={seen:?}"),
            _ => {}
        }
        let finished = matches!(event, DriverEvent::TurnFinished { .. });
        if let DriverEvent::TurnFinished { success, .. } = &event {
            assert_eq!(*success, !stop, "Native result: {seen:?}");
        }
        seen.push(event);
        if finished {
            break;
        }
    }
    assert_eq!(
        approved, expect_approval,
        "Missing or unexpected native owner approval: {seen:?}"
    );
    cursor.expect("Actual native conversation identity")
}
fn stopped(driver: AntigravityDriver, events: &Receiver<DriverEvent>) {
    drop(driver);
    let until = Instant::now() + Duration::from_secs(15);
    loop {
        if matches!(
            events
                .recv_timeout(until.saturating_duration_since(Instant::now()))
                .unwrap(),
            DriverEvent::ProcessExited
        ) {
            break;
        }
    }
}
fn native_fixture(stop: bool) {
    let binary = PathBuf::from(std::env::var_os("ION_TEST_ANTIGRAVITY_BINARY").unwrap());
    let f = Fixture::new();
    let root = &f.directory.0;
    for dir in [
        "home/.gemini/antigravity-cli",
        "home/.gemini/config",
        "home/AppData/Roaming",
        "home/AppData/Local",
        "temp",
        "work/.agents/plugins",
    ] {
        fs::create_dir_all(root.join(dir)).unwrap();
    }
    fs::write(root.join("empty-git-config"), "").unwrap();
    fs::write(root.join("home/.gemini/antigravity-cli/settings.json"),json!({"modelProvider":"gemini","enableTelemetry":false,"notifications":false,"toolPermission":"strict","showTips":false,"enableTerminalSandbox":false}).to_string()).unwrap();
    let owner = root.join("work/.agents/mcp_config.json");
    let owner_bytes = "// Owner native config\n{\"unknownOwnerField\":true,\"mcpServers\":{\"owner_disabled\":{\"disabled\":true,\"command\":\"fixture-unused\"}},}\n";
    fs::write(&owner, owner_bytes).unwrap();
    let owner_document = root.join("work/owner.txt");
    fs::write(&owner_document, "Owner content — kept exactly.").unwrap();
    let selected_server = Arc::new(StdMutex::new(String::new()));
    let selected = selected_server.clone();
    let mut responses = vec![(200, function(f.args()))];
    if stop {
        responses.push((200, function(f.args())));
        responses.push((200, sse(json!([{"text":"Recovered team queued"}]))));
    } else {
        responses.push((200, sse(json!([{"text":"Team queued"}]))));
        responses.push((200, function(f.args())));
        responses.push((200, sse(json!([{"text":"Resumed team queued"}]))));
        responses.push((200, function(f.args())));
        responses.push((200, sse(json!([{"text":"Changed definition refused"}]))));
    }
    let model = crate::driver::team_test_http::Server::with_auxiliary_handler(
        responses,
        move |request, body| {
            assert!(
                request.body["tools"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .flat_map(|tool| tool["functionDeclarations"].as_array().unwrap())
                    .any(|tool| tool["name"] == "call_mcp_tool"),
                "Normal native MCP wrapper was not advertised"
            );
            String::from_utf8(body)
                .unwrap()
                .replace("ION_NATIVE_SERVER", &selected.lock().unwrap())
                .into_bytes()
        },
        |request| {
            let tools = request.body["tools"].as_array();
            let main = tools.is_some_and(|tools| {
                tools
                    .iter()
                    .flat_map(|tool| {
                        tool["functionDeclarations"]
                            .as_array()
                            .into_iter()
                            .flatten()
                    })
                    .any(|tool| tool["name"] == "view_file" || tool["name"] == "call_mcp_tool")
            });
            assert!(
                request.line.starts_with(if main {
                    "POST /v1beta/models/gemini-3.1-pro-preview:"
                } else {
                    "POST /v1beta/models/gemini-3.1-flash-lite-preview:"
                }) || !main
                    && request
                        .line
                        .starts_with("POST /v1beta/models/gemini-3.1-pro-preview:"),
                "Unexpected native route {}",
                request.line
            );
            (!main).then(|| {
                let data = sse(json!([{"text":"Ion fixture"}]));
                if request.line.contains("gemini-3.1-flash-lite-preview:") {
                    String::from_utf8(data)
                        .unwrap()
                        .replace("gemini-3.1-pro-preview", "gemini-3.1-flash-lite-preview")
                        .into_bytes()
                } else {
                    data
                }
            })
        },
    );
    let (driver, events) = start(&f, &binary, &model.url, selected_server.clone(), None);
    let cursor = turn(&driver, &events, stop, true);
    assert_eq!(fs::read_to_string(&owner).unwrap(), owner_bytes);
    assert_eq!(
        fs::read_to_string(&owner_document).unwrap(),
        "Owner content — kept exactly."
    );
    assert_eq!(f.runs().len(), if stop { 0 } else { 1 });
    let original_plugins = plugin_paths(root);
    assert_eq!(original_plugins.len(), 1);
    if !stop {
        assert_eq!(f.runs()[0].project_id, f.context.project_id);
        let (replacement, replacement_events) = start(
            &f,
            &binary,
            &model.url,
            selected_server.clone(),
            Some(cursor.clone()),
        );
        let resumed = turn(&replacement, &replacement_events, false, true);
        assert_eq!(resumed, cursor);
        assert_eq!(f.runs().len(), 2);
        assert_eq!(
            plugin_paths(root).len(),
            2,
            "Driver handoff did not retain both native plugins"
        );
        stopped(driver, &events);
        let until = Instant::now() + Duration::from_secs(5);
        while original_plugins[0].exists() && Instant::now() < until {
            thread::sleep(Duration::from_millis(20));
        }
        assert!(!original_plugins[0].exists());
        assert_eq!(plugin_paths(root).len(), 1);
        let store = crate::control::ControlStore::new(f.context.control_path.clone());
        let mut team = f.runs()[0].team.clone();
        team.objective = "Changed by owner".into();
        store
            .save(store.load().unwrap().revision, ControlRecord::Team(team))
            .unwrap();
        turn(&replacement, &replacement_events, false, false);
        assert_eq!(
            f.runs().len(),
            2,
            "Stale native definition created another team run"
        );
        stopped(replacement, &replacement_events);
    } else {
        assert!(
            !events
                .try_iter()
                .any(|event| matches!(event, DriverEvent::TurnStarted)),
            "Canceled queued prompt started"
        );
        let recovered = turn(&driver, &events, false, true);
        assert_eq!(
            recovered, cursor,
            "Stop recovery changed the native conversation"
        );
        assert_eq!(f.runs().len(), 1);
        stopped(driver, &events);
    }
    let requests = model.finish();
    let main: Vec<_> = requests
        .iter()
        .filter(|request| {
            request.body["tools"].as_array().is_some_and(|tools| {
                tools
                    .iter()
                    .flat_map(|tool| {
                        tool["functionDeclarations"]
                            .as_array()
                            .into_iter()
                            .flatten()
                    })
                    .any(|tool| tool["name"] == "call_mcp_tool")
            })
        })
        .collect();
    assert_eq!(
        main.len(),
        if stop { 3 } else { 6 },
        "Queued canceled prompt reached provider or main response was lost"
    );
    if !stop {
        let first = main[1].body.to_string();
        assert!(
            first.contains(&f.runs()[0].id.to_string()),
            "Actual native result did not return the persisted invocation"
        );
        let second = main[3].body.to_string();
        assert!(
            second.contains(&f.runs()[1].id.to_string()),
            "Native resumed result did not return the new persisted invocation"
        );
        let stale = main[5].body.to_string();
        assert!(
            stale.contains("Ion team request did not complete"),
            "Changed definition was not refused by the shared worker"
        );
        for request in &main {
            for content in request.body["contents"].as_array().unwrap() {
                for part in content["parts"].as_array().unwrap() {
                    if part.get("functionCall").is_some() {
                        assert_eq!(part["functionCall"]["args"]["Arguments"], f.args());
                    }
                }
            }
        }
    }
    let until = Instant::now() + Duration::from_secs(8);
    while !plugin_paths(root).is_empty() && Instant::now() < until {
        thread::sleep(Duration::from_millis(20));
    }
    assert!(
        plugin_paths(root).is_empty(),
        "Unchanged native plugin remained after driver shutdown"
    );
    assert_eq!(fs::read_to_string(owner).unwrap(), owner_bytes);
    assert_eq!(
        fs::read_to_string(owner_document).unwrap(),
        "Owner content — kept exactly."
    );
}
