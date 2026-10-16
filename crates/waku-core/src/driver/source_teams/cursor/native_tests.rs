//! Real pinned Cursor and production ACP driver; private account and loopback only.
use super::*;
use crate::driver::{DriverControl, DriverStartOptions, acp::AcpDriver};
use crossbeam_channel::Receiver;
use std::{
    fs,
    process::{Child, Command, Stdio},
    time::Instant,
};
use uuid::Uuid;
use waku_protocol::{
    control::{AgentProfile, ControlRecord, Team, TeamMember},
    model::{DriverEvent, ProviderKind, ProviderResumeCursor, RuntimeMode},
    teams::{AgentAccess, AgentExecution, TeamRunLimits},
};
struct Host(Child);
impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
struct Fixture {
    directory: crate::driver::team_codex::EmptyDirectory,
    host: Host,
    native: Value,
    ready: PathBuf,
    work: PathBuf,
    context: WorkspaceContext,
}
impl Fixture {
    fn new(scenario: &str) -> Self {
        let runtime = PathBuf::from(
            std::env::var_os("ION_TEST_CURSOR_RUNTIME").expect("Pinned Cursor runtime"),
        );
        let dir =
            crate::driver::team_codex::EmptyDirectory::create("cursor-native-driver").unwrap();
        let store = crate::control::ControlStore::new(dir.0.join("control.db"));
        let agent = Uuid::new_v4();
        let team = Uuid::new_v4();
        store
            .save(
                0,
                ControlRecord::Agent(AgentProfile {
                    id: agent,
                    name: "Lead".into(),
                    expertise: "Coding".into(),
                    instructions: "Delegate".into(),
                    expected_output: "Report".into(),
                    provider: ProviderKind::Nebius,
                    model: Some("pinned".into()),
                    execution: AgentExecution::default(),
                    access: AgentAccess {
                        workspace_write: true,
                        ..Default::default()
                    },
                    max_parallel_tasks: 1,
                }),
            )
            .unwrap();
        store
            .save(
                store.load().unwrap().revision,
                ControlRecord::Team(Team {
                    id: team,
                    name: "Engineering".into(),
                    objective: "Reviewed work".into(),
                    limits: Some(TeamRunLimits {
                        max_tokens: Some(500000),
                        ..Default::default()
                    }),
                    members: vec![TeamMember {
                        agent_id: agent,
                        parent_id: None,
                    }],
                }),
            )
            .unwrap();
        let context = WorkspaceContext {
            task_id: Uuid::new_v4(),
            project_id: Uuid::new_v4(),
            control_path: store.path(),
            settings_path: dir.0.join("unused.json"),
            mode: RuntimeMode::Ask,
        };
        let args = json!({"team_id":team,"definition_revision":store.load().unwrap().revision,"objective":"Implement reviewed work"});
        let args_path = dir.0.join("arguments.json");
        fs::write(&args_path, args.to_string()).unwrap();
        let ready = dir.0.join("ready.json");
        let host_script = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../scripts/fixtures/cursor-native-controller.mjs");
        let stderr = fs::File::create(dir.0.join("host.stderr")).unwrap();
        let host = Host(
            Command::new(runtime.join("node.exe"))
                .arg(host_script)
                .arg(scenario)
                .env_clear()
                .envs(
                    ["SystemRoot", "WINDIR", "PATH"]
                        .into_iter()
                        .filter_map(|k| std::env::var(k).ok().map(|v| (k, v))),
                )
                .env("ION_TEST_CURSOR_RUNTIME", &runtime)
                .env("ION_CURSOR_CONTRACT_TEMP", &dir.0)
                .env("ION_CURSOR_DRIVER_ARGS", &args_path)
                .env("ION_CURSOR_NATIVE_READY", &ready)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(stderr)
                .spawn()
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(15);
        while !ready.is_file() {
            assert!(
                Instant::now() < deadline,
                "Native fixture host not ready: {}",
                fs::read_to_string(dir.0.join("host.stderr")).unwrap()
            );
            thread::sleep(Duration::from_millis(25));
        }
        let native: Value = serde_json::from_slice(&fs::read(&ready).unwrap()).unwrap();
        let work = PathBuf::from(native["root"].as_str().unwrap()).join("work");
        Self {
            directory: dir,
            host,
            native,
            ready,
            work,
            context,
        }
    }
    fn start(&self, cursor: Option<ProviderResumeCursor>) -> (AcpDriver, Receiver<DriverEvent>) {
        let (sender, events) = crate::driver::test_event_channel();
        let driver = AcpDriver::start_configured(
            ProviderKind::Cursor,
            DriverStartOptions {
                binary: PathBuf::from(self.native["exe"].as_str().unwrap()),
                cwd: self.work.clone(),
                mode: RuntimeMode::Ask,
                model: None,
                reasoning_effort: None,
                service_tier: None,
                context_window: None,
                agent_preset: None,
                computer_use_enabled: false,
                provider_cursor: cursor,
            },
            sender,
            Some(self.context.clone()),
            |config| self.config(config),
        )
        .unwrap();
        (driver, events)
    }
    fn config(&self, config: AcpAgentConfig) -> AcpAgentConfig {
        let empty = std::env::vars()
            .map(|(k, _)| (k.to_ascii_uppercase(), String::new()))
            .chain(
                config
                    .environment()
                    .keys()
                    .map(|k| (k.to_ascii_uppercase(), String::new())),
            );
        AcpAgentConfig::new(self.native["exe"].as_str().unwrap())
            .args(
                self.native["args"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_owned()),
            )
            .envs(empty)
            .envs(
                self.native["env"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned())),
            )
    }
    fn runs(&self) -> Vec<waku_protocol::teams::TeamRunSnapshot> {
        crate::teams::TeamCoordinator::new(self.context.control_path.clone())
            .runs_for_source(self.context.task_id)
            .unwrap()
    }
    fn root(&self) -> PathBuf {
        cache_root(&self.work, &self.config(AcpAgentConfig::new("unused"))).unwrap()
    }
    fn finish(mut self) -> Value {
        self.host
            .0
            .stdin
            .as_mut()
            .unwrap()
            .write_all(b"done\n")
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.host.0.try_wait().unwrap().is_none() {
            assert!(Instant::now() < deadline, "Fixture service shutdown");
            thread::sleep(Duration::from_millis(25));
        }
        let result =
            serde_json::from_slice(&fs::read(self.ready.with_extension("json.result")).unwrap())
                .unwrap();
        assert!(self.directory.0.is_dir());
        result
    }
}
fn connected(events: &Receiver<DriverEvent>) -> ProviderResumeCursor {
    let deadline = Instant::now() + Duration::from_secs(45);
    loop {
        match events
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("Native connection")
        {
            DriverEvent::Connected {
                provider_cursor: Some(cursor),
            } => return cursor,
            DriverEvent::Error(e) => panic!("Native connection: {e}"),
            DriverEvent::Activity { title, detail, .. } if title == "Team tools unavailable" => {
                panic!("Missing production bridge: {detail:?}")
            }
            _ => {}
        }
    }
}
fn stopped(driver: AcpDriver, events: &Receiver<DriverEvent>) {
    drop(driver);
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if matches!(
            events
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .expect("Native shutdown"),
            DriverEvent::ProcessExited
        ) {
            break;
        }
    }
}
fn turn(
    driver: &AcpDriver,
    events: &Receiver<DriverEvent>,
    stop: bool,
    invalid: bool,
    expect_native: bool,
    expect_team: bool,
) {
    driver.prompt("Invoke the saved Engineering team using its current revision.".into());
    let deadline = Instant::now() + Duration::from_secs(50);
    let (mut native, mut team) = (false, false);
    let mut seen = Vec::new();
    loop {
        let event = events
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap_or_else(|_| panic!("Native turn timed out: {seen:?}"));
        if let DriverEvent::Permission {
            request_id,
            options,
            detail,
            ..
        } = &event
        {
            assert!(!detail.contains(FIELD));
            assert!(!detail.contains("ion_teams_"));
            if request_id.starts_with("ion-team-approval-") {
                team = true;
                if stop {
                    driver.cancel();
                } else {
                    driver.respond(
                        request_id.clone(),
                        options.iter().find(|o| o.allow).unwrap().id.clone(),
                    );
                }
            } else {
                native = true;
                assert!(options.iter().all(|o| o.id != "allow-always"));
                driver.respond(
                    request_id.clone(),
                    if invalid {
                        "allow-always".into()
                    } else {
                        options.iter().find(|o| o.allow).unwrap().id.clone()
                    },
                );
            }
        }
        if let DriverEvent::Error(e) = &event {
            panic!("Native turn error: {e}; {seen:?}");
        }
        let done = matches!(event, DriverEvent::TurnFinished { .. });
        seen.push(event);
        if done {
            break;
        }
    }
    assert_eq!(native, expect_native, "Native approval mismatch: {seen:?}");
    assert_eq!(team, expect_team, "Team approval mismatch: {seen:?}");
    assert!(!format!("{seen:?}").contains(FIELD));
}
fn wait_for(condition: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(Instant::now() < deadline, "Native hook cleanup");
        thread::sleep(Duration::from_millis(25));
    }
}
#[test]
#[ignore = "Requires pinned ION_TEST_CURSOR_RUNTIME; private fake account and loopback only"]
fn cursor_source_team_native_driver_invocation_resume_replay_and_stale_definition() {
    let f = Fixture::new("driver-replay");
    let owner = f.root().join(".cursor/hooks.json");
    fs::create_dir_all(owner.parent().unwrap()).unwrap();
    let owner_bytes = br#"{"version":1,"ownerField":true,"hooks":{}}"#;
    fs::write(&owner, owner_bytes).unwrap();
    let own = f.root().join(".claude/settings.local.json");
    let (first, events) = f.start(None);
    let cursor = connected(&events);
    turn(&first, &events, false, false, true, true);
    assert_eq!(f.runs().len(), 1);
    let run = f.runs()[0].id;
    assert_eq!(f.runs()[0].project_id, f.context.project_id);
    let (replacement, replacement_events) = f.start(Some(cursor.clone()));
    assert_eq!(connected(&replacement_events), cursor);
    let hooks: Value = serde_json::from_slice(&fs::read(&own).unwrap()).unwrap();
    assert_eq!(hooks["hooks"]["PreToolUse"].as_array().unwrap().len(), 2);
    stopped(first, &events);
    wait_for(|| {
        serde_json::from_slice::<Value>(&fs::read(&own).unwrap()).unwrap()["hooks"]["PreToolUse"]
            .as_array()
            .unwrap()
            .len()
            == 1
    });
    turn(&replacement, &replacement_events, false, false, true, false);
    assert_eq!(f.runs().len(), 1);
    assert_eq!(f.runs()[0].id, run);
    let store = crate::control::ControlStore::new(f.context.control_path.clone());
    let mut team = f.runs()[0].team.clone();
    team.objective = "Owner changed the team".into();
    store
        .save(store.load().unwrap().revision, ControlRecord::Team(team))
        .unwrap();
    turn(&replacement, &replacement_events, false, false, true, false);
    assert_eq!(f.runs().len(), 1);
    stopped(replacement, &replacement_events);
    wait_for(|| !own.exists());
    assert_eq!(fs::read(owner).unwrap(), owner_bytes);
    let result = f.finish();
    assert_eq!(result["modelSelections"].as_array().unwrap().len(), 3);
    assert!(
        result["nativeResults"][0]
            .as_str()
            .unwrap()
            .contains(&run.to_string())
    );
    assert!(
        result["nativeResults"][1]
            .as_str()
            .unwrap()
            .contains(&run.to_string())
    );
    assert!(
        result["nativeResults"][2]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains("definition")
    );
}
#[test]
#[ignore = "Requires pinned ION_TEST_CURSOR_RUNTIME; private fake account and loopback only"]
fn cursor_source_team_native_driver_stop_and_new_turn() {
    let f = Fixture::new("driver-fresh");
    let own = f.root().join(".claude/settings.local.json");
    let (driver, events) = f.start(None);
    connected(&events);
    turn(&driver, &events, true, false, true, true);
    assert!(f.runs().is_empty());
    turn(&driver, &events, false, false, true, true);
    assert_eq!(f.runs().len(), 1);
    stopped(driver, &events);
    wait_for(|| !own.exists());
    let result = f.finish();
    assert_eq!(result["modelSelections"].as_array().unwrap().len(), 2);
}
#[test]
#[ignore = "Requires pinned ION_TEST_CURSOR_RUNTIME; private fake account and loopback only"]
fn cursor_source_team_native_driver_rejects_unadvertised_remembered_approval() {
    let f = Fixture::new("driver-fresh");
    let (driver, events) = f.start(None);
    connected(&events);
    turn(&driver, &events, false, true, true, false);
    assert!(f.runs().is_empty());
    stopped(driver, &events);
    f.finish();
}
#[test]
#[ignore = "Requires pinned ION_TEST_CURSOR_RUNTIME; private fake account and loopback only"]
fn cursor_source_team_native_driver_refuses_preapproved_native_route() {
    let f = Fixture::new("driver-preapproved");
    let (driver, events) = f.start(None);
    connected(&events);
    turn(&driver, &events, false, false, false, false);
    assert!(f.runs().is_empty());
    stopped(driver, &events);
    let result = f.finish();
    assert!(
        result["nativeResults"][0]
            .as_str()
            .unwrap()
            .contains(DENIED)
    );
}
