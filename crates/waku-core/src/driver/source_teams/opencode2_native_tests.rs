#[test]
fn native_inbox_time_supports_released_and_legacy_formats_without_inventing_timestamp() {
    let base = json!({"id":"msg_test","sessionID":"ses_test","type":"user","payload":{"text":"hello"},"delivery":"queue"});
    let mut legacy = base.clone();
    legacy["timeCreated"] = json!(123.0);
    let mut release = base.clone();
    release["time"] = json!({"created":123.0});
    let left: crate::opencode2_api::InboxUser = serde_json::from_value(legacy.clone()).unwrap();
    assert_eq!(left, serde_json::from_value(release).unwrap());
    assert!(serde_json::from_value::<crate::opencode2_api::InboxUser>(base).is_err());
    legacy["time"] = json!({"created":124.0});
    assert!(serde_json::from_value::<crate::opencode2_api::InboxUser>(legacy).is_err());
}
// Private native acceptance and release-identity compatibility checks.
#[test]
fn released_server_info_fallback_preserves_identity_and_authentication_failure() {
    use crate::driver::team_test_http::Server;
    let info = json!({"version":"2.0.29","pid":123,"urls":["http://127.0.0.1:1234"],"paths":{"tmp":"fixture"}});
    let server = Server::new(vec![
        (404, vec![]),
        (200, serde_json::to_vec(&info).unwrap()),
    ]);
    let port = url::Url::parse(&server.url).unwrap().port().unwrap();
    let result = crate::opencode2_api::health(&crate::http_wire::Endpoint::local(port)).unwrap();
    assert!(result.healthy);
    assert_eq!(result.pid, 123);
    assert_eq!(result.version, "2.0.29");
    let requests = server.finish();
    assert!(requests[0].line.starts_with("GET /api/health "));
    assert!(requests[1].line.starts_with("GET /api/info "));
    let server = Server::new(vec![(401, vec![])]);
    let port = url::Url::parse(&server.url).unwrap().port().unwrap();
    assert_eq!(
        crate::opencode2_api::health(&crate::http_wire::Endpoint::local(port))
            .unwrap_err()
            .status(),
        Some(401)
    );
    assert_eq!(server.finish().len(), 1);
}
#[test]
#[ignore = "Requires ION_NATIVE_OPENCODE2_BINARY=2.0.29; private native profile and loopback fake model only"]
fn opencode2_source_team_native_plugin_through_actual_driver() {
    native_opencode2_source_fixture(false);
}
#[test]
#[ignore = "Requires ION_NATIVE_OPENCODE2_BINARY=2.0.29; private native profile and loopback fake model only"]
fn opencode2_source_team_native_stop_at_owner_approval() {
    native_opencode2_source_fixture(true);
}
fn native_opencode2_source_fixture(stop_at_approval: bool) {
    use crate::{
        driver::{DriverControl, DriverStartOptions},
        http_wire::{Endpoint, request_json},
        opencode2_service::Opencode2Service,
    };
    use std::{
        fs,
        net::TcpListener,
        path::PathBuf,
        process::{Child, Command, Stdio},
    };
    struct NativeHost {
        child: Child,
        log: PathBuf,
    }
    impl Drop for NativeHost {
        fn drop(&mut self) {
            if self.child.try_wait().ok().flatten().is_none() {
                #[cfg(windows)]
                {
                    let _ = Command::new("taskkill")
                        .args(["/PID", &self.child.id().to_string(), "/T", "/F"])
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .status();
                }
                #[cfg(not(windows))]
                {
                    let _ = self.child.kill();
                }
                let _ = self.child.wait();
            }
            if std::thread::panicking() {
                eprintln!(
                    "Native fixture stderr: {}",
                    fs::read_to_string(&self.log).unwrap_or_default()
                );
                eprintln!(
                    "Native fixture stdout: {}",
                    fs::read_to_string(self.log.with_file_name("stdout.txt")).unwrap_or_default()
                );
                for log in walk_fixture_logs(self.log.parent().unwrap()) {
                    eprintln!("Native fixture log: {log}");
                }
            }
        }
    }
    fn walk_fixture_logs(root: &std::path::Path) -> Vec<String> {
        let mut out = Vec::new();
        let mut pending = vec![root.to_path_buf()];
        while let Some(path) = pending.pop() {
            let Ok(entries) = fs::read_dir(path) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    pending.push(path);
                } else if path.extension().is_some_and(|e| e == "log") {
                    let text = fs::read_to_string(path).unwrap_or_default();
                    out.extend(
                        text.lines()
                            .filter(|l| l.contains("level=ERROR") || l.contains("error:"))
                            .take(8)
                            .map(str::to_owned),
                    );
                }
            }
        }
        out
    }
    let f = Fixture::new();
    let native = PathBuf::from(
        std::env::var_os("ION_NATIVE_OPENCODE2_BINARY")
            .expect("Set the verified native 2.0.29 executable"),
    );
    let prefix = format!("ion_teams_{}_", f.context.task_id.simple());
    let args = f.args();
    let tool = format!("{prefix}invoke_team");
    let chunks = [
        json!({"id":"chat_ion_fixture","object":"chat.completion.chunk","model":"fixture","choices":[{"index":0,"delta":{"role":"assistant","tool_calls":[{"index":0,"id":"call_native_ion","type":"function","function":{"name":tool,"arguments":args.to_string()}}]},"finish_reason":null}]}),
        json!({"id":"chat_ion_fixture","object":"chat.completion.chunk","model":"fixture","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}),
        json!({"id":"chat_ion_fixture","object":"chat.completion.chunk","model":"fixture","choices":[],"usage":{"prompt_tokens":100,"completion_tokens":20,"total_tokens":120}}),
    ];
    let mut reply = chunks
        .iter()
        .map(|v| format!("data: {v}\n\n"))
        .collect::<String>();
    reply.push_str("data: [DONE]\n\n");
    let mut responses = vec![(200, reply.into_bytes())];
    if !stop_at_approval {
        responses.push((
            200,
            crate::driver::team_test_http::chat("fixture", "Team queued"),
        ));
    }
    let model =
        crate::driver::team_test_http::Server::with_timeout(responses, Duration::from_secs(65));
    let root = &f.directory.0;
    for name in ["home", "work", "temp", "config", "data", "state", "cache"] {
        fs::create_dir_all(root.join(name)).unwrap();
    }
    // A pre-existing owner plugin must remain byte-for-byte untouched.
    fs::create_dir_all(root.join("work/.opencode/plugins")).unwrap();
    let owner = root.join("work/.opencode/plugins/owner.ts");
    let owner_source = "export default {id:'ion-owner-fixture',async setup(){}};";
    fs::write(&owner, owner_source).unwrap();
    let config = json!({"plugins":["*","-opencode.provider.*"],"snapshots":false,"formatter":false,"compaction":{"auto":false},"agents":{"title":{"disabled":true},"summary":{"disabled":true},"build":{"steps":4}},"permissions":[{"action":"*","resource":"*","effect":"deny"},{"action":format!("{prefix}*"),"resource":"*","effect":"allow"}],"providers":{"ion-fixture":{"name":"Ion isolated fixture","package":"@opencode/ai/providers/openai-compatible","settings":{"baseURL":format!("{}/v1",model.url),"apiKey":"ion-fake-key"},"models":{"fixture":{"name":"Fixture","tool_call":true,"limit":{"context":128000,"output":2048}}}}}});
    let port = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let password = format!("ion-fixture-{}", Uuid::new_v4());
    let log = root.join("stderr.txt");
    let mut command = Command::new(&native);
    command
        .args([
            "serve",
            "--hostname",
            "127.0.0.1",
            "--port",
            &port.to_string(),
        ])
        .current_dir(root.join("work"))
        .env_clear()
        .stdin(Stdio::null())
        .stdout(fs::File::create(root.join("stdout.txt")).unwrap())
        .stderr(fs::File::create(&log).unwrap());
    for name in ["PATH", "SystemRoot", "WINDIR", "COMSPEC", "PATHEXT"] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    for (name, path) in [
        ("HOME", "home"),
        ("USERPROFILE", "home"),
        ("OPENCODE_TEST_HOME", "home"),
        ("APPDATA", "home"),
        ("LOCALAPPDATA", "home"),
        ("TEMP", "temp"),
        ("TMP", "temp"),
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_DATA_HOME", "data"),
        ("XDG_STATE_HOME", "state"),
        ("XDG_CACHE_HOME", "cache"),
    ] {
        command.env(name, root.join(path));
    }
    command
        .env("OPENCODE_CONFIG_CONTENT", config.to_string())
        .env("OPENCODE_SERVER_PASSWORD", &password)
        .env("OPENCODE_DISABLE_MODELS_FETCH", "1")
        .env("OPENCODE_DISABLE_AUTOUPDATE", "1")
        .env("OPENCODE_DISABLE_TELEMETRY", "1")
        .env("NPM_CONFIG_REGISTRY", "http://127.0.0.1:1");
    let mut host = NativeHost {
        child: command.spawn().unwrap(),
        log,
    };
    let endpoint =
        Endpoint::basic(&format!("http://127.0.0.1:{port}"), "opencode", &password).unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let health = crate::opencode2_api::health(&endpoint);
        if health.is_ok() {
            break;
        }

        assert!(
            host.child.try_wait().unwrap().is_none(),
            "Private native host exited"
        );
        assert!(
            Instant::now() < deadline,
            "Private native host did not become healthy: {health:?}"
        );
        thread::sleep(Duration::from_millis(100));
    }
    let service =
        Opencode2Service::adopt_fixture(&native, endpoint.clone(), "2.0.29", host.child.id());
    let (events, received) = crate::driver::test_event_channel();
    let driver = crate::driver::opencode2::OpenCode2Driver::start_on_service(
        DriverStartOptions {
            binary: native,
            cwd: root.join("work"),
            mode: RuntimeMode::Ask,
            model: Some("ion-fixture/fixture".into()),
            reasoning_effort: None,
            service_tier: None,
            context_window: None,
            agent_preset: None,
            computer_use_enabled: false,
            provider_cursor: None,
        },
        events,
        Some(f.context.clone()),
        service.clone(),
    )
    .unwrap();
    let mut captured = None;
    let mut native_session = String::new();
    for event in received.try_iter() {
        if let DriverEvent::Connected {
            provider_cursor: Some(ProviderResumeCursor::OpenCode2 { session_id, .. }),
        } = event
        {
            captured = Some(service.subscribe(&session_id));
            native_session = session_id;
        } else if let DriverEvent::Activity { title, detail, .. } = event {
            assert_ne!(title, "Team tools unavailable", "{detail:?}");
        }
    }
    let captured = captured.expect("Native source cursor missing");
    let directory = fs::canonicalize(root.join("work"))
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("location[directory]", &directory)
        .finish();
    let id = format!("ion-teams-{}", f.context.task_id.simple());
    let rpc = |method: &str, input: Value| {
        request_json(
            &endpoint,
            "POST",
            &format!("/api/rpc/{id}/{method}?{query}"),
            Some(&json!({"input":input})),
            Duration::from_secs(3),
        )
        .unwrap()["output"]
            .clone()
    };
    let binding = |generation: &str| json!({"generation":generation,"session":native_session,"endpoint":"http://127.0.0.1:1/call","credential":"0".repeat(64)});
    assert_eq!(
        rpc("bind", binding("fixture-old"))["generation"],
        "fixture-old"
    );
    assert_eq!(
        rpc("bind", binding("fixture-new"))["generation"],
        "fixture-new"
    );
    assert_eq!(
        rpc("revoke", json!({"generation":"fixture-old"}))["revoked"],
        false
    );
    assert_eq!(rpc("status", json!({}))["generation"], "fixture-new");
    assert_eq!(
        rpc("revoke", json!({"generation":"fixture-new"}))["revoked"],
        true
    );
    assert_eq!(rpc("status", json!({}))["ready"], false);
    let generated = root.join("work/.opencode/plugins").join(format!("{id}.ts"));
    assert!(!fs::read_to_string(&generated).unwrap().contains(&password));
    driver.prompt("Invoke the saved engineering team, then report its result.".into());
    let mut approved = false;
    let mut finished = false;
    let mut text = String::new();
    let deadline = Instant::now() + Duration::from_secs(45);
    while Instant::now() < deadline {
        match received.recv_timeout(Duration::from_millis(100)) {
            Ok(DriverEvent::Permission { request_id, .. }) => {
                assert!(
                    request_id.starts_with("ion-team-approval-"),
                    "Unexpected native permission: {request_id}"
                );
                assert!(!approved);
                assert!(f.runs().is_empty());
                approved = true;
                if stop_at_approval {
                    driver.cancel();
                } else {
                    driver.respond(request_id, "allow".into());
                }
            }
            Ok(DriverEvent::TextDelta(delta)) => text.push_str(&delta),
            Ok(DriverEvent::TurnFinished { success, .. }) => {
                assert_eq!(success, !stop_at_approval);
                finished = true;
                break;
            }
            Ok(DriverEvent::Error(error)) => panic!("Native source fixture: {error}"),
            Ok(DriverEvent::Activity {
                ref title,
                ref detail,
                ..
            }) if title == "Team tools unavailable" => {
                panic!("Native plugin unavailable: {detail:?}")
            }
            _ => {}
        }
    }
    assert!(finished, "Native turn did not finish: {text}");
    let requests = model.finish();
    let native_results = requests
        .iter()
        .flat_map(|r| r.body["messages"].as_array().into_iter().flatten())
        .filter(|m| m["role"] == "tool")
        .cloned()
        .collect::<Vec<_>>();
    let frames = captured
        .rx
        .try_iter()
        .filter_map(|frame| match frame {
            crate::opencode2_service::HubFrame::Event(v)
                if v["type"].as_str().is_some_and(|s| {
                    s.starts_with("session.tool")
                        || s.starts_with("session.execution")
                        || s == "session.step.started"
                }) =>
            {
                Some(v)
            }
            _ => None,
        })
        .take(40)
        .collect::<Vec<_>>();
    assert!(
        approved,
        "Native tool results: {native_results:?}; native frames: {frames:?}"
    );
    assert!(
        frames
            .iter()
            .any(|frame| frame["type"] == "session.tool.called"
                && frame["data"]["executed"] == false),
        "Native local tool execution flag changed: {frames:?}"
    );
    assert_eq!(f.runs().len(), if stop_at_approval { 0 } else { 1 });
    assert_eq!(requests.len(), if stop_at_approval { 1 } else { 2 });
    if !stop_at_approval {
        assert!(text.contains("Team queued"));
    }
    assert!(
        requests[0].body["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["function"]["name"] == tool)
    );
    if !stop_at_approval {
        assert!(
            requests[1].body["messages"]
                .as_array()
                .unwrap()
                .iter()
                .any(|m| m["role"] == "tool"
                    && m["content"]
                        .as_str()
                        .is_some_and(|s| s.contains(&f.runs()[0].id.to_string())))
        );
    }
    drop(driver);
    assert!(
        crate::opencode2_api::health(&endpoint).is_ok(),
        "Driver teardown must leave adopted host running"
    );
    assert_eq!(fs::read_to_string(&owner).unwrap(), owner_source);
    let deadline = Instant::now() + Duration::from_secs(4);
    while generated.exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
    }
    assert!(
        !generated.exists(),
        "Owned generated plugin must be cleaned after the final driver lease"
    );
}
