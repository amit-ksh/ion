use super::*;
use crossbeam_channel::Receiver;
use waku_protocol::{
    control::{AgentProfile, ControlRecord, Team, TeamMember},
    model::{DriverEvent, ProviderKind, RuntimeMode},
    teams::{AgentAccess, AgentExecution, TeamRunLimits},
};
struct Fixture {
    directory: crate::driver::team_codex::EmptyDirectory,
    context: WorkspaceContext,
    team: Uuid,
    revision: u64,
    session: String,
}
impl Fixture {
    fn new() -> Self {
        let directory =
            crate::driver::team_codex::EmptyDirectory::create("antigravity-source-teams").unwrap();
        let store = crate::control::ControlStore::new(directory.0.join("control.db"));
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
        Self {
            revision: store.load().unwrap().revision,
            context: WorkspaceContext {
                task_id: Uuid::new_v4(),
                project_id: Uuid::new_v4(),
                control_path: store.path(),
                settings_path: directory.0.join("unused.json"),
                mode: RuntimeMode::Ask,
            },
            directory,
            team,
            session: Uuid::new_v4().to_string(),
        }
    }
    fn args(&self) -> Value {
        json!({"team_id":self.team,"definition_revision":self.revision,"objective":"Implement reviewed work"})
    }
    fn runs(&self) -> Vec<waku_protocol::teams::TeamRunSnapshot> {
        crate::teams::TeamCoordinator::new(self.context.control_path.clone())
            .runs_for_source(self.context.task_id)
            .unwrap()
    }
    fn bridge(&self) -> (AntigravityBridge, Receiver<DriverEvent>) {
        let (events, received) = crate::driver::test_event_channel();
        let bridge =
            AntigravityBridge::start(self.context.clone(), events, &self.directory.0).unwrap();
        assert!(bridge.begin(Arc::new(AtomicBool::new(false))));
        bridge
            .observe(&json!({"event":"init","conversation_id":self.session}))
            .unwrap();
        (bridge, received)
    }
}
fn event(b: &AntigravityBridge, step: u64, name: &str, args: Value) -> Value {
    json!({"event":"step_update","step_update":{"conversation_id":b.0.state.lock().session,"step_index":step,"step_type":"tool","state":"ACTIVE","tool_name":"call_mcp_tool","tool_info":{"name":"call_mcp_tool","parameters":{"ServerName":b.0.server,"ToolName":name,"Arguments":args}}}})
}
fn params(b: &AntigravityBridge, step: u64, name: &str, args: Value) -> Value {
    json!({"name":name,"arguments":args,"_meta":{"antigravity.google/conversation_id":b.0.state.lock().session,"progressToken":format!("11111111-1111-1111-1111-111111111111:{step}")}})
}
fn post(b: &AntigravityBridge, body: Value) -> Value {
    reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(12))
        .build()
        .unwrap()
        .post(format!("http://127.0.0.1:{}/mcp", b.0.port))
        .bearer_auth(&b.0.token)
        .json(&body)
        .send()
        .unwrap()
        .json()
        .unwrap()
}
fn call(b: &AntigravityBridge, params: Value) -> Value {
    post(
        b,
        json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":params}),
    )
}
fn permission(events: &Receiver<DriverEvent>) -> String {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        if let DriverEvent::Permission { request_id, .. } = events
            .recv_timeout(until.saturating_duration_since(Instant::now()))
            .unwrap()
        {
            return request_id;
        }
    }
}
#[test]
fn antigravity_source_team_approval_durable_replay_and_catalogue() {
    let f = Fixture::new();
    let (b, events) = f.bridge();
    assert_eq!(
        post(&b, json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}))["result"]["tools"]
            .as_array()
            .unwrap()
            .len(),
        5
    );
    b.observe(&event(&b, 2, "invoke_team", f.args())).unwrap();
    let input = params(&b, 2, "invoke_team", f.args());
    let pending = {
        let b = b.clone();
        let input = input.clone();
        thread::spawn(move || call(&b, input))
    };
    let id = permission(&events);
    assert!(f.runs().is_empty());
    b.respond(&id, "allow");
    assert_eq!(pending.join().unwrap()["result"]["isError"], false);
    assert_eq!(f.runs().len(), 1);
    assert_eq!(call(&b, input)["result"]["isError"], true);
    b.close();
    drop(b);
    let (replacement, received) = f.bridge();
    replacement
        .observe(&event(&replacement, 2, "invoke_team", f.args()))
        .unwrap();
    assert_eq!(
        call(
            &replacement,
            params(&replacement, 2, "invoke_team", f.args())
        )["result"]["isError"],
        false
    );
    assert_eq!(f.runs().len(), 1);
    assert!(
        !received
            .try_iter()
            .any(|e| matches!(e, DriverEvent::Permission { .. }))
    );
    replacement.close();
}
#[test]
fn antigravity_source_team_stop_revokes_approval_and_cannot_resurrect_old_steps() {
    let f = Fixture::new();
    let (b, events) = f.bridge();
    let e = event(&b, 2, "invoke_team", f.args());
    b.observe(&e).unwrap();
    let input = params(&b, 2, "invoke_team", f.args());
    let pending = {
        let b = b.clone();
        let input = input.clone();
        thread::spawn(move || call(&b, input))
    };
    let id = permission(&events);
    b.cancel();
    b.respond(&id, "allow");
    assert_eq!(pending.join().unwrap()["result"]["isError"], true);
    assert!(f.runs().is_empty());
    assert!(b.begin(Arc::new(AtomicBool::new(false))));
    b.observe(&e).unwrap();
    assert_eq!(call(&b, input)["result"]["isError"], true);
    let flag = Arc::new(AtomicBool::new(true));
    assert!(!b.begin(flag));
    b.close();
}
#[test]
fn antigravity_source_team_changed_callback_and_native_error_revoke_pending_approval() {
    for changed in ["callback", "error", "server", "tool"] {
        let f = Fixture::new();
        let (b, events) = f.bridge();
        let mut e = event(&b, 2, "invoke_team", f.args());
        b.observe(&e).unwrap();
        let mut input = params(&b, 2, "invoke_team", f.args());
        let pending = {
            let b = b.clone();
            let input = input.clone();
            thread::spawn(move || call(&b, input))
        };
        let id = permission(&events);
        match changed {
            "error" => e["step_update"]["state"] = json!("ERROR"),
            "server" => {
                e["step_update"]["tool_info"]["parameters"]["ServerName"] = json!("foreign-server")
            }
            "tool" => {
                e["step_update"]["tool_info"]["parameters"]["ToolName"] = json!("unrelated-tool")
            }
            _ => {
                input["arguments"]["objective"] = json!("Changed after approval");
                assert_eq!(call(&b, input)["result"]["isError"], true);
            }
        }
        if changed != "callback" {
            b.observe(&e).unwrap();
        }
        b.respond(&id, "allow");
        assert_eq!(pending.join().unwrap()["result"]["isError"], true);
        assert!(f.runs().is_empty());
        b.close();
    }
}
#[test]
fn antigravity_source_team_foreign_metadata_wrapper_and_changed_stream_are_refused() {
    let f = Fixture::new();
    let (b, _) = f.bridge();
    let mut foreign = event(&b, 2, "invoke_team", f.args());
    foreign["step_update"]["conversation_id"] = json!(Uuid::new_v4().to_string());
    b.observe(&foreign).unwrap();
    assert!(!b.0.state.lock().calls.contains_key(&2));
    foreign = event(&b, 2, "invoke_team", f.args());
    foreign["step_update"]["tool_info"]["parameters"]["ServerName"] = json!("another_plugin");
    b.observe(&foreign).unwrap();
    assert!(!b.0.state.lock().calls.contains_key(&2));
    b.observe(&event(&b, 2, "invoke_team", f.args())).unwrap();
    let mut input = params(&b, 2, "invoke_team", f.args());
    input["_meta"]["antigravity.google/conversation_id"] = json!(Uuid::new_v4().to_string());
    assert_eq!(call(&b, input)["result"]["isError"], true);
    let mut changed = event(&b, 2, "invoke_team", f.args());
    changed["step_update"]["tool_info"]["parameters"]["Arguments"]["objective"] =
        json!("Changed stream");
    b.observe(&changed).unwrap();
    assert_eq!(
        call(&b, params(&b, 2, "invoke_team", f.args()))["result"]["isError"],
        true
    );
    assert!(f.runs().is_empty());
    b.close();
}
#[test]
fn antigravity_source_team_argument_bounds_and_terminal_fence() {
    let f = Fixture::new();
    let (b, _) = f.bridge();
    assert!(
        b.observe(&event(
            &b,
            1,
            "invoke_team",
            json!({"objective":"x".repeat(40*1024)})
        ))
        .is_err()
    );
    let args = json!({"query":"x".repeat(32000)});
    for index in 1..=32 {
        b.observe(&event(&b, index, "list_teams", args.clone()))
            .unwrap();
    }
    assert!(b.observe(&event(&b, 33, "list_teams", args)).is_err());
    b.observe(&json!({"event":"result","result":{"status":"SUCCESS"}}))
        .unwrap();
    assert_eq!(
        call(&b, params(&b, 1, "list_teams", json!({})))["result"]["isError"],
        true
    );
    assert_eq!(b.0.state.lock().bytes, 0);
    assert!(
        b.0.state
            .lock()
            .calls
            .values()
            .all(|call| call.arguments.is_null())
    );
    b.close();
}
#[test]
fn antigravity_source_team_metadata_cannot_be_supplied_only_in_arguments() {
    let f = Fixture::new();
    let (b, _) = f.bridge();
    b.observe(&event(&b, 2, "invoke_team", f.args())).unwrap();
    let mut input = params(&b, 2, "invoke_team", f.args());
    input["arguments"]["_meta"] = input["_meta"].clone();
    input.as_object_mut().unwrap().remove("_meta");
    assert_eq!(call(&b, input)["result"]["isError"], true);
    assert!(f.runs().is_empty());
    b.close();
}
#[path = "antigravity_native_tests.rs"]
mod native;
