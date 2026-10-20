use super::*;
use crossbeam_channel::Receiver;
use uuid::Uuid;
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
}
impl Fixture {
    fn new() -> Self {
        let directory =
            crate::driver::team_codex::EmptyDirectory::create("opencode-source-teams").unwrap();
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
                account_scope: None,
                task_id: Uuid::new_v4(),
                project_id: Uuid::new_v4(),
                control_path: store.path(),
                settings_path: directory.0.join("unused.json"),
                mode: RuntimeMode::Ask,
            },
            directory,
            team,
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
    fn bridge(&self) -> (GrokBridge, Receiver<DriverEvent>) {
        let (events, received) = crate::driver::test_event_channel();
        let bridge = GrokBridge::start(
            self.context.clone(),
            events,
            &self.directory.0,
            &std::env::current_exe().unwrap(),
        )
        .unwrap();
        bridge.bind("native").unwrap();
        assert!(bridge.activate(bridge.queue_turn()));
        (bridge, received)
    }
}

fn event(bridge: &GrokBridge, call: &str, name: &str, args: Value) -> Value {
    json!({"hookEventName":"pre_tool_use","hook_event_name":"PreToolUse","sessionId":"native","toolUseId":call,"toolName":format!("{}__{name}",bridge.0.server_name),"toolInputTruncated":false,"toolInput":{"tool_name":format!("{}__{name}",bridge.0.server_name),"tool_input":args}})
}
fn post(bridge: &GrokBridge, route: &str, body: Value) -> Value {
    reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(12))
        .build()
        .unwrap()
        .post(format!("http://127.0.0.1:{}{route}", bridge.0.port))
        .bearer_auth(&bridge.0.token)
        .json(&body)
        .send()
        .unwrap()
        .json()
        .unwrap()
}
fn prepare(bridge: &GrokBridge, call: &str, name: &str, args: Value) -> Value {
    let mut event = event(bridge, call, name, args);
    bridge.observe(&json!({"sessionId":"native","update":{"sessionUpdate":"tool_call","toolCallId":call,"title":"use_tool","rawInput":event["toolInput"]}}));
    let hook = post(bridge, "/hook", event.clone());
    assert!(
        hook["hookSpecificOutput"]["updatedInput"].is_object(),
        "{hook}"
    );
    event["toolInput"] = hook["hookSpecificOutput"]["updatedInput"].clone();
    event["hookCallbackId"] = json!(bridge.0.callback);
    assert_eq!(bridge.witness(&event)["decision"], "continue");
    event["toolInput"]["tool_input"].clone()
}
fn call(bridge: &GrokBridge, name: &str, args: Value) -> Value {
    post(
        bridge,
        "/mcp",
        json!({"jsonrpc":"2.0","id":"native-mcp-request","method":"tools/call","params":{"name":name,"arguments":args}}),
    )
}
fn permission(events: &Receiver<DriverEvent>) -> String {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match events
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap()
        {
            DriverEvent::Permission { request_id, .. } => return request_id,
            _ => {}
        }
    }
}
#[test]
fn grok_source_team_approval_durable_replay_and_five_tool_catalogue() {
    let fixture = Fixture::new();
    let (bridge, events) = fixture.bridge();
    let catalog = post(
        &bridge,
        "/mcp",
        json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}),
    );
    assert_eq!(catalog["result"]["tools"].as_array().unwrap().len(), 5);
    let arguments = prepare(&bridge, "invoke_1", "invoke_team", fixture.args());
    let pending = {
        let bridge = bridge.clone();
        let arguments = arguments.clone();
        thread::spawn(move || call(&bridge, "invoke_team", arguments))
    };
    assert!(fixture.runs().is_empty());
    let approval = permission(&events);
    assert!(bridge.respond(&approval, "allow"));
    let reply = pending.join().unwrap();
    assert_eq!(reply["result"]["isError"], false, "{reply}");
    assert_eq!(fixture.runs().len(), 1);
    assert_eq!(
        call(&bridge, "invoke_team", arguments)["result"]["isError"],
        true
    );
    bridge.close();
    let (replacement, replacement_events) = fixture.bridge();
    let arguments = prepare(&replacement, "invoke_1", "invoke_team", fixture.args());
    let replay = call(&replacement, "invoke_team", arguments);
    assert_eq!(replay["result"]["isError"], false, "{replay}");
    assert_eq!(fixture.runs().len(), 1);
    assert!(
        !replacement_events
            .try_iter()
            .any(|event| matches!(event, DriverEvent::Permission { .. }))
    );
    replacement.close();
}
#[test]
fn grok_source_team_stop_at_approval_and_turn_tickets_fence_late_calls() {
    let fixture = Fixture::new();
    let (bridge, events) = fixture.bridge();
    let old = bridge.active_turn().unwrap();
    let arguments = prepare(&bridge, "stop_1", "invoke_team", fixture.args());
    let pending = {
        let bridge = bridge.clone();
        let arguments = arguments.clone();
        thread::spawn(move || call(&bridge, "invoke_team", arguments))
    };
    let approval = permission(&events);
    let queued_before_stop = bridge.queue_turn();
    bridge.cancel();
    assert!(!bridge.activate(queued_before_stop));
    let _ = bridge.respond(&approval, "allow");
    assert_eq!(pending.join().unwrap()["result"]["isError"], true);
    assert!(fixture.runs().is_empty());
    let next = bridge.queue_turn();
    assert!(bridge.activate(next));
    bridge.finish_turn(Some(old));
    assert_eq!(bridge.active_turn(), Some(next));
    assert_eq!(
        call(&bridge, "invoke_team", arguments)["result"]["isError"],
        true
    );
    assert!(fixture.runs().is_empty());
    bridge.close();
}
#[test]
fn grok_source_team_unobserved_foreign_truncated_forged_and_changed_calls_are_refused() {
    let fixture = Fixture::new();
    let (bridge, _) = fixture.bridge();
    let arguments = fixture.args();
    let valid = event(&bridge, "native_call", "invoke_team", arguments.clone());
    for (key, value) in [
        ("sessionId", json!("foreign")),
        ("toolInputTruncated", json!(true)),
        ("subagentType", json!("nested")),
        ("hook_event_name", json!("Stop")),
    ] {
        let mut bad = valid.clone();
        bad[key] = value;
        assert_eq!(post(&bridge, "/hook", bad)["decision"], "deny");
    }
    let mut forged = valid.clone();
    forged["toolInput"]["tool_input"][FIELD] = json!("model-chosen");
    assert_eq!(post(&bridge, "/hook", forged)["decision"], "deny");
    // The command credential alone cannot invent a call absent from the ACP stream.
    assert_eq!(post(&bridge, "/hook", valid.clone())["decision"], "deny");
    let bound = prepare(&bridge, "native_call", "invoke_team", arguments);
    let mut changed = bound.clone();
    changed["objective"] = json!("Changed native input");
    assert_eq!(
        call(&bridge, "invoke_team", changed)["result"]["isError"],
        true
    );
    assert_eq!(
        call(&bridge, "invoke_team", bound)["result"]["isError"],
        true
    );
    assert!(fixture.runs().is_empty());
    bridge.close();
}
#[test]
fn grok_source_team_expiry_failure_and_unwitnessed_callbacks_never_enqueue() {
    let fixture = Fixture::new();
    let (bridge, _) = fixture.bridge();
    let mut event = event(&bridge, "unconfirmed", "invoke_team", fixture.args());
    bridge.observe(&json!({"sessionId":"native","update":{"sessionUpdate":"tool_call","toolCallId":"unconfirmed","title":"use_tool","rawInput":event["toolInput"]}}));
    let hook = post(&bridge, "/hook", event.clone());
    let bound = hook["hookSpecificOutput"]["updatedInput"]["tool_input"].clone();
    assert_eq!(
        call(&bridge, "invoke_team", bound.clone())["result"]["isError"],
        true
    );
    event["toolInput"] = hook["hookSpecificOutput"]["updatedInput"].clone();
    event["hookCallbackId"] = json!(bridge.0.callback);
    assert_eq!(bridge.witness(&event)["decision"], "continue");
    bridge
        .0
        .state
        .lock()
        .calls
        .get_mut("unconfirmed")
        .unwrap()
        .expires = Instant::now() - Duration::from_secs(1);
    assert_eq!(
        call(&bridge, "invoke_team", bound)["result"]["isError"],
        true
    );
    let failed = prepare(&bridge, "failed", "invoke_team", fixture.args());
    bridge.observe(&json!({"sessionId":"native","update":{"sessionUpdate":"tool_call_update","toolCallId":"failed","status":"failed"}}));
    assert_eq!(
        call(&bridge, "invoke_team", failed)["result"]["isError"],
        true
    );
    assert!(fixture.runs().is_empty());
    bridge.close();
}
#[test]
fn grok_source_team_hook_leases_preserve_owner_files_and_scrub_native_controls() {
    let fixture = Fixture::new();
    let owner = fixture.directory.0.join(".grok/hooks/owner.json");
    std::fs::create_dir_all(owner.parent().unwrap()).unwrap();
    std::fs::write(&owner, "owner hook").unwrap();
    let (first, _) = fixture.bridge();
    let path = owner.parent().unwrap().join(format!(
        "ion-teams-{}.json",
        fixture.context.task_id.simple()
    ));
    let source = std::fs::read_to_string(&path).unwrap();
    assert!(!source.contains(&first.0.token));
    let bound = prepare(&first, "scrub", "list_teams", json!({}));
    let binding = bound[FIELD].as_str().unwrap();
    let hash = "a".repeat(64);
    let mut value = json!({"rawInput":bound,"content":format!("工程 🚀 {binding} {FIELD} {hash}")});
    first.scrub(&mut value);
    assert!(value["rawInput"].get(FIELD).is_none());
    assert!(!value.to_string().contains(binding));
    assert!(value.to_string().contains("工程 🚀"));
    assert!(value.to_string().contains(&hash));
    let (replacement, _) = fixture.bridge();
    first.close();
    drop(first);
    thread::sleep(Duration::from_millis(100));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    replacement.close();
    drop(replacement);
    let deadline = Instant::now() + Duration::from_secs(3);
    while path.exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(!path.exists());
    assert_eq!(std::fs::read_to_string(owner).unwrap(), "owner hook");
}

#[test]
fn grok_source_team_changed_hook_revokes_inflight_owner_approval() {
    let fixture = Fixture::new();
    let (bridge, events) = fixture.bridge();
    let arguments = prepare(&bridge, "pending", "invoke_team", fixture.args());
    let pending = {
        let bridge = bridge.clone();
        thread::spawn(move || call(&bridge, "invoke_team", arguments))
    };
    let request = permission(&events);
    let mut changed = fixture.args();
    changed["objective"] = json!("Changed while approval was pending");
    assert_eq!(
        post(
            &bridge,
            "/hook",
            event(&bridge, "pending", "invoke_team", changed)
        )["decision"],
        "deny"
    );
    bridge.respond(&request, "allow");
    assert_eq!(pending.join().unwrap()["result"]["isError"], true);
    assert!(fixture.runs().is_empty());
    bridge.close();
}
#[test]
fn grok_source_team_aggregate_arguments_are_bounded_and_reset_between_turns() {
    let fixture = Fixture::new();
    let (bridge, _) = fixture.bridge();
    let args = json!({"query":"x".repeat(39 * 1024)});
    for index in 0..40 {
        let event = event(
            &bridge,
            &format!("large-{index}"),
            "list_teams",
            args.clone(),
        );
        bridge.observe(&json!({"sessionId":"native","update":{"sessionUpdate":"tool_call","toolCallId":event["toolUseId"],"title":"use_tool","rawInput":event["toolInput"]}}));
    }
    {
        let state = bridge.0.state.lock();
        assert!(state.argument_bytes <= MAX_ARGUMENT_BYTES);
        assert!(state.argument_bytes >= MAX_ARGUMENT_BYTES - 40 * 1024);
        assert_eq!(state.calls.len(), 40);
        assert!(state.calls["large-39"].arguments.is_null());
        assert!(!state.calls["large-39"].valid);
    }
    assert_eq!(
        post(
            &bridge,
            "/hook",
            event(&bridge, "large-39", "list_teams", args.clone())
        )["decision"],
        "deny"
    );
    bridge.cancel();
    assert_eq!(bridge.0.state.lock().argument_bytes, 0);
    assert!(bridge.activate(bridge.queue_turn()));
    let fresh = prepare(&bridge, "fresh", "list_teams", args.clone());
    assert!(fresh[FIELD].is_string());
    assert_eq!(
        post(
            &bridge,
            "/hook",
            event(&bridge, "large-39", "list_teams", args)
        )["decision"],
        "deny"
    );
    bridge.close();
}

#[path = "grok_native_tests.rs"]
mod native;
