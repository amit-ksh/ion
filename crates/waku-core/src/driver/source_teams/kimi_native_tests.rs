//! Real pinned source engine, actual production ACP/worker and private HTTP model.
//! Fake login records forwarding only; authenticated owner acceptance is separate.
use super::*;
use crate::driver::{DriverControl, DriverStartOptions, acp::AcpDriver};
use std::{fs, path::PathBuf, process::Command};
use waku_protocol::model::ProviderResumeCursor;
fn sse(name: &str, args: Value) -> Vec<u8> {
    let chunks = [
        json!({"id":"chat-ion","object":"chat.completion.chunk","created":1,"model":"ion-native-model","choices":[{"index":0,"delta":{"role":"assistant","tool_calls":[{"index":0,"id":"same-native-id","type":"function","function":{"name":name,"arguments":args.to_string()}}]},"finish_reason":null}]}),
        json!({"id":"chat-ion","object":"chat.completion.chunk","created":1,"model":"ion-native-model","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}),
    ];
    let mut out = chunks
        .iter()
        .map(|v| format!("data: {v}\n\n"))
        .collect::<String>();
    out.push_str("data: [DONE]\n\n");
    out.into_bytes()
}
fn private_config(config: AcpAgentConfig, root: &Path, guard: &Path) -> AcpAgentConfig {
    let empty = std::env::vars()
        .map(|(key, _)| (key.to_ascii_uppercase(), String::new()))
        .chain(
            config
                .environment()
                .keys()
                .map(|key| (key.to_ascii_uppercase(), String::new())),
        );
    let mut result = AcpAgentConfig::new(config.command())
        .args(config.arguments().to_owned())
        .envs(empty);
    for name in ["SystemRoot", "WINDIR", "PATH", "PATHEXT", "COMSPEC"] {
        if let Ok(value) = std::env::var(name) {
            result = result.env(name.to_ascii_uppercase(), value);
        }
    }
    for (name, path) in [
        ("HOME", "home"),
        ("USERPROFILE", "home"),
        ("KIMI_CODE_HOME", "home"),
        ("APPDATA", "appdata"),
        ("LOCALAPPDATA", "local"),
        ("TEMP", "temp"),
        ("TMP", "temp"),
    ] {
        result = result.env(name, root.join(path).to_string_lossy());
    }
    result
        .env(
            "NODE_OPTIONS",
            format!("--import {}", url::Url::from_file_path(guard).unwrap()),
        )
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_COUNT", "0")
        .env(
            "GIT_CONFIG_GLOBAL",
            root.join("empty-git-config").to_string_lossy(),
        )
}
fn connect(
    f: &Fixture,
    binary: &Path,
    guard: &Path,
    cursor: Option<ProviderResumeCursor>,
) -> (AcpDriver, Receiver<DriverEvent>, ProviderResumeCursor) {
    let started = Instant::now();
    let (events, received) = crate::driver::test_event_channel();
    let driver = AcpDriver::start_configured(
        ProviderKind::Kimi,
        DriverStartOptions {
            binary: PathBuf::from(std::env::var_os("ION_TEST_KIMI_NODE").unwrap()),
            cwd: f.directory.0.join("work"),
            mode: RuntimeMode::Ask,
            model: None,
            reasoning_effort: None,
            service_tier: None,
            context_window: None,
            agent_preset: None,
            computer_use_enabled: false,
            provider_cursor: cursor,
        },
        events,
        Some(f.context.clone()),
        |config| {
            private_config(config, &f.directory.0, guard)
                .env("ION_KIMI_SOURCE_HOST", binary.to_string_lossy())
        },
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(150);
    let cursor = loop {
        match received
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("Native connection")
        {
            DriverEvent::Connected {
                provider_cursor: Some(cursor),
            } => break cursor,
            DriverEvent::Error(error) => panic!("Native Kimi connection: {error}"),
            DriverEvent::Activity { title, detail, .. } if title == "Team tools unavailable" => {
                panic!("Production bridge missing: {detail:?}")
            }
            _ => {}
        }
    };
    eprintln!(
        "Native Kimi source-host connection: {} ms",
        started.elapsed().as_millis()
    );
    (driver, received, cursor)
}
fn turn(
    driver: &AcpDriver,
    received: &Receiver<DriverEvent>,
    mode: &str,
) -> (Vec<DriverEvent>, usize) {
    driver.prompt("Use the appropriate native tool to complete the requested work.".into());
    let deadline = Instant::now() + Duration::from_secs(70);
    let mut seen = Vec::new();
    let mut owner = 0;
    loop {
        let event = received
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap_or_else(|_| panic!("Native Kimi turn timed out: {seen:?}"));
        if let DriverEvent::Permission {
            request_id,
            options,
            ..
        } = &event
        {
            let ion = request_id.starts_with("ion-team-approval-");
            if ion {
                owner += 1;
            }
            if ion && mode == "stop" {
                driver.cancel();
            } else {
                let allow = !(ion && mode == "deny");
                let option = options
                    .iter()
                    .find(|o| o.allow == allow)
                    .expect("Native permission option");
                driver.respond(request_id.clone(), option.id.clone());
            }
        }
        if let DriverEvent::Error(error) = &event {
            panic!("Native Kimi turn: {error}; {seen:?}");
        }
        let end = matches!(event, DriverEvent::TurnFinished { .. });
        seen.push(event);
        if end {
            break;
        }
    }
    (seen, owner)
}
fn shutdown(driver: AcpDriver, received: &Receiver<DriverEvent>) {
    drop(driver);
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if matches!(
            received
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .expect("Native host shutdown"),
            DriverEvent::ProcessExited
        ) {
            break;
        }
    }
}
fn fixture(mode: &str) {
    let source =
        PathBuf::from(std::env::var_os("ION_TEST_KIMI_SOURCE").expect("Explicit pinned source"));
    let node =
        PathBuf::from(std::env::var_os("ION_TEST_KIMI_NODE").expect("Explicit verified Node"));
    let f = Fixture::new();
    let root = &f.directory.0;
    for dir in ["home", "work", "temp", "appdata", "local"] {
        fs::create_dir_all(root.join(dir)).unwrap();
    }
    fs::write(root.join("empty-git-config"), "").unwrap();
    fs::write(
        root.join("work/AGENTS.md"),
        "# Fixture\nPreserve ion-production-workspace-marker.\n",
    )
    .unwrap();
    fs::write(root.join("work/evidence.txt"), "ion-native-read-evidence").unwrap();
    let project_tsconfig = "{\"compilerOptions\":{\"experimentalDecorators\":false}}";
    fs::write(root.join("work/tsconfig.json"), project_tsconfig).unwrap();
    let guard = root.join("network-guard.mjs");
    fs::write(&guard,r#"import net from 'node:net';import tls from 'node:tls';
function check(raw){let args=Array.isArray(raw[0])?raw[0]:raw;let host;if(typeof args[0]==='object'){if(args[0].path)throw Error('Fixture pipes disabled');host=args[0].host??args[0].hostname??'localhost';}else if(typeof args[0]==='number')host=typeof args[1]==='string'?args[1]:'localhost';else throw Error('Fixture network path disabled');if(!['127.0.0.1','localhost','::1'].includes(host))throw Error('Fixture nonlocal network disabled');}
const connect=net.Socket.prototype.connect;net.Socket.prototype.connect=function(...args){check(args);return connect.apply(this,args);};const secure=tls.connect;tls.connect=function(...args){check(args);return secure.apply(this,args);};"#).unwrap();
    let login = root.join("native-login.mjs");
    fs::write(&login,"import {writeFileSync} from 'node:fs';if(process.argv[2]==='--version')console.log('2.2.0');else writeFileSync(process.env.KIMI_CODE_HOME+'/login-forward.json',JSON.stringify({args:process.argv.slice(2),home:process.env.KIMI_CODE_HOME}));").unwrap();
    let prepare = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scripts/kimi-host/prepare-source-host.mjs");
    let output = Command::new(&node)
        .arg(&prepare)
        .arg("--source")
        .arg(&source)
        .arg("--node")
        .arg(&node)
        .arg("--native-command")
        .arg(&node)
        .arg("--native-entry")
        .arg(&login)
        .arg("--output")
        .arg(root.join("host"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "Host preparation: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let binary = root.join(if cfg!(windows) {
        "host/ion-kimi-host.cmd"
    } else {
        "host/ion-kimi-host"
    });
    let config_before = fs::read(root.join("host/host.json")).unwrap();
    // Test real launcher login delegation, never claiming the fake logs in.
    let mut command = crate::command_env::command(&binary);
    command
        .args(["acp", "--login", "--region", "global"])
        .env("KIMI_CODE_HOME", root.join("home"));
    let bytes = crate::team_process::bounded_process(
        &mut command,
        b"",
        &AtomicBool::new(false),
        Duration::from_secs(15),
    )
    .unwrap();
    assert!(bytes.is_empty());
    let forwarded: Value =
        serde_json::from_slice(&fs::read(root.join("home/login-forward.json")).unwrap()).unwrap();
    assert_eq!(
        forwarded["args"],
        json!(["acp", "--login", "--region", "global"])
    );
    let mut args = f.args();
    let name = if mode == "read" {
        args = json!({"path":root.join("work/evidence.txt")});
        "Read"
    } else {
        "ion_teams_invoke_team"
    };
    if mode == "stale" {
        let store = crate::control::ControlStore::new(f.context.control_path.clone());
        let mut team = store
            .load()
            .unwrap()
            .records
            .iter()
            .find_map(|record| {
                if let ControlRecord::Team(team) = record {
                    Some(team.clone())
                } else {
                    None
                }
            })
            .unwrap();
        team.objective = "Updated definition".into();
        store
            .save(store.load().unwrap().revision, ControlRecord::Team(team))
            .unwrap();
    }
    let mut replies = vec![(200, sse(name, args))];
    if mode != "stop" {
        replies.push((
            200,
            crate::driver::team_test_http::chat("ion-native-model", "Native work complete"),
        ));
    }
    if mode == "resume" {
        replies.push((
            200,
            sse("ion_teams_list_teams", json!({"query":"Engineering"})),
        ));
        replies.push((
            200,
            crate::driver::team_test_http::chat("ion-native-model", "Resumed native work complete"),
        ));
    }
    let model =
        crate::driver::team_test_http::Server::with_timeout(replies, Duration::from_secs(180));
    let config = format!(
        "defaultModel=\"ion-fixture\"\n[models.ion-fixture]\nname=\"ion-native-model\"\nprotocol=\"openai\"\nbaseUrl=\"{}/v1\"\napiKey=\"ion-disposable-native-key\"\nmaxContextSize=32768\n",
        model.url
    );
    fs::write(root.join("home/config.toml"), &config).unwrap();
    let (driver, received, cursor) = connect(&f, &binary, &guard, None);
    let (_, approvals) = turn(&driver, &received, mode);
    assert_eq!(approvals, usize::from(!matches!(mode, "read" | "stale")));
    assert_eq!(
        f.runs().len(),
        usize::from(matches!(mode, "allow" | "resume"))
    );
    if let Some(run) = f.runs().first() {
        assert_eq!(run.limits.max_tokens, Some(500000));
    }
    shutdown(driver, &received);
    if mode == "resume" {
        let (driver, received, loaded) = connect(&f, &binary, &guard, Some(cursor.clone()));
        assert_eq!(loaded.native_id(), cursor.native_id());
        let (_, approvals) = turn(&driver, &received, "allow");
        assert_eq!(approvals, 0);
        shutdown(driver, &received);
    }
    let requests = model.finish();
    assert_eq!(
        requests.len(),
        if mode == "stop" {
            1
        } else if mode == "resume" {
            4
        } else {
            2
        }
    );
    let first = &requests[0];
    assert!(first.line.starts_with("POST /v1/chat/completions "));
    assert_eq!(first.body["model"], "ion-native-model");
    assert!(
        first
            .headers
            .to_ascii_lowercase()
            .contains("authorization: bearer ion-disposable-native-key")
    );
    assert!(
        first.body["messages"]
            .to_string()
            .contains("ion-production-workspace-marker")
    );
    let tools = first.body["tools"].as_array().unwrap();
    for name in [
        "Read",
        "Write",
        "Bash",
        "ion_teams_list_teams",
        "ion_teams_invoke_team",
        "ion_teams_get_team_runs",
        "ion_teams_await_team",
        "ion_teams_read_team_document",
    ] {
        assert!(
            tools.iter().any(|t| t["function"]["name"] == name),
            "Missing native tool {name}"
        );
    }
    for tool in super::super::super::tools().as_array().unwrap() {
        let function = &tool["function"];
        let actual = tools
            .iter()
            .find(|t| {
                t["function"]["name"] == format!("ion_teams_{}", function["name"].as_str().unwrap())
            })
            .unwrap();
        assert_eq!(actual["function"]["parameters"], function["parameters"]);
    }
    if mode != "stop" {
        let messages = requests[1].body["messages"].as_array().unwrap();
        let result = messages
            .iter()
            .find(|m| m["role"] == "tool")
            .expect("Actual native result in next model request");
        assert_eq!(result["tool_call_id"], "same-native-id");
        if mode == "read" {
            assert!(result.to_string().contains("ion-native-read-evidence"));
        } else if mode == "allow" || mode == "resume" {
            assert!(result.to_string().contains(&f.runs()[0].id.to_string()));
        }
    }
    assert_eq!(
        fs::read(root.join("work/tsconfig.json")).unwrap(),
        project_tsconfig.as_bytes()
    );
    assert_eq!(
        fs::read(root.join("home/config.toml")).unwrap(),
        config.as_bytes()
    );
    assert_eq!(
        fs::read(root.join("host/host.json")).unwrap(),
        config_before
    );
}
#[test]
#[ignore = "requires explicit pinned Kimi source and Node runtime"]
fn kimi_source_native_driver_approval_and_result() {
    fixture("allow");
}
#[test]
#[ignore = "requires explicit pinned Kimi source and Node runtime"]
fn kimi_source_native_driver_denial() {
    fixture("deny");
}
#[test]
#[ignore = "requires explicit pinned Kimi source and Node runtime"]
fn kimi_source_native_driver_stop() {
    fixture("stop");
}
#[test]
#[ignore = "requires explicit pinned Kimi source and Node runtime"]
fn kimi_source_native_driver_stale_definition() {
    fixture("stale");
}
#[test]
#[ignore = "requires explicit pinned Kimi source and Node runtime"]
fn kimi_source_native_driver_builtin_read() {
    fixture("read");
}
#[test]
#[ignore = "requires explicit pinned Kimi source and Node runtime"]
fn kimi_source_native_driver_resume() {
    fixture("resume");
}
