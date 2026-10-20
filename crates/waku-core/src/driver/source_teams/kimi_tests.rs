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
    fn bridge(&self) -> (KimiBridge, Receiver<DriverEvent>) {
        let (events, received) = crate::driver::test_event_channel();
        let bridge = KimiBridge::listen(self.context.clone(), events).unwrap();
        bridge
            .dispatch(
                "/context",
                json!({"session_id":"native","agent_id":"main","generation":1}),
            )
            .unwrap();
        bridge.bind("native").unwrap();
        assert!(bridge.activate(bridge.queue_turn()));
        (bridge, received)
    }
}

fn event(call: &str, name: &str, args: Value) -> Value {
    json!({"sessionId":"native","update":{"sessionUpdate":"tool_call_update","toolCallId":call,"title":format!("ion_teams_{name}"),"status":"in_progress","rawInput":args}})
}
fn callback(call: &str, name: &str, args: Value) -> Value {
    let (turn, id) = call.split_once(':').unwrap();
    json!({"session_id":"native","agent_id":"main","generation":1,"turn_id":turn.parse::<u64>().unwrap(),"call_id":id,"name":name,"arguments":args})
}
fn approval(received: &Receiver<DriverEvent>) -> String {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let DriverEvent::Permission { request_id, .. } = received
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap()
        {
            return request_id;
        }
    }
}
fn invoke(bridge: &KimiBridge, body: Value) -> (thread::JoinHandle<()>, Receiver<Result<Value>>) {
    let bridge = bridge.clone();
    let (sent, received) = bounded(1);
    let child = thread::spawn(move || {
        let _ = sent.send(bridge.dispatch("/call", body));
    });
    (child, received)
}
#[test]
fn kimi_source_catalog_uses_production_schemas_and_native_callback() {
    let f = Fixture::new();
    let (b, _) = f.bridge();
    assert_eq!(
        b.dispatch("/tools", json!({})).unwrap(),
        super::super::tools()
    );
    b.observe(&event("1:a", "list_teams", json!({"query":"Engineering"})));
    let reply = b
        .dispatch(
            "/call",
            callback("1:a", "list_teams", json!({"query":"Engineering"})),
        )
        .unwrap();
    assert_eq!(reply["ok"], true);
    assert!(reply["text"].as_str().unwrap().contains("Engineering"));
    assert!(f.runs().is_empty());
    b.close();
}
#[test]
fn kimi_source_real_worker_approval_limits_and_durable_replay() {
    let f = Fixture::new();
    let (b, received) = f.bridge();
    let args = f.args();
    b.observe(&event("1:a", "invoke_team", args.clone()));
    let body = callback("1:a", "invoke_team", args);
    let (child, result) = invoke(&b, body.clone());
    let permission = approval(&received);
    assert!(f.runs().is_empty());
    assert!(b.respond(&permission, "allow"));
    let reply = result
        .recv_timeout(Duration::from_secs(10))
        .unwrap()
        .unwrap();
    child.join().unwrap();
    assert_eq!(reply["ok"], true, "{reply}");
    assert_eq!(f.runs().len(), 1);
    assert_eq!(b.dispatch("/call", body).unwrap(), reply);
    assert_eq!(f.runs().len(), 1);
    assert_eq!(f.runs()[0].limits.max_tokens, Some(500000));
    b.close();
}
#[test]
fn kimi_source_denied_worker_starts_no_run() {
    let f = Fixture::new();
    let (b, received) = f.bridge();
    b.observe(&event("1:a", "invoke_team", f.args()));
    let (child, result) = invoke(&b, callback("1:a", "invoke_team", f.args()));
    assert!(b.respond(&approval(&received), "reject"));
    assert_eq!(
        result
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap()["ok"],
        false
    );
    child.join().unwrap();
    assert!(f.runs().is_empty());
    b.close();
}
#[test]
fn kimi_source_stop_releases_approval_and_fences_queued_turns() {
    let f = Fixture::new();
    let (b, received) = f.bridge();
    b.observe(&event("1:a", "invoke_team", f.args()));
    let (child, result) = invoke(&b, callback("1:a", "invoke_team", f.args()));
    let permission = approval(&received);
    let queued = b.queue_turn();
    b.cancel();
    let _ = b.respond(&permission, "allow");
    assert!(
        result
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .is_err()
    );
    child.join().unwrap();
    assert!(!b.activate(queued));
    assert!(f.runs().is_empty());
    assert!(b.activate(b.queue_turn()));
    assert!(
        b.dispatch("/call", callback("1:a", "invoke_team", f.args()))
            .is_err()
    );
    b.close();
}
#[test]
fn kimi_source_changed_native_event_revokes_pending_worker() {
    let f = Fixture::new();
    let (b, received) = f.bridge();
    b.observe(&event("1:a", "invoke_team", f.args()));
    let (child, result) = invoke(&b, callback("1:a", "invoke_team", f.args()));
    let permission = approval(&received);
    let mut args = f.args();
    args["objective"] = json!("Changed");
    b.observe(&event("1:a", "invoke_team", args));
    let _ = b.respond(&permission, "allow");
    assert!(
        result
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .is_err()
    );
    child.join().unwrap();
    assert!(f.runs().is_empty());
    b.close();
}
#[test]
fn kimi_source_changed_callback_revokes_pending_worker() {
    let f = Fixture::new();
    let (b, received) = f.bridge();
    b.observe(&event("1:a", "invoke_team", f.args()));
    let (child, result) = invoke(&b, callback("1:a", "invoke_team", f.args()));
    let permission = approval(&received);
    let mut changed = callback("1:a", "invoke_team", f.args());
    changed["arguments"]["objective"] = json!("Changed");
    assert!(b.dispatch("/call", changed).is_err());
    let _ = b.respond(&permission, "allow");
    assert!(
        result
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .is_err()
    );
    child.join().unwrap();
    assert!(f.runs().is_empty());
    b.close();
}
#[test]
fn kimi_source_rejects_foreign_context_and_unknown_fields() {
    let f = Fixture::new();
    let (b, _) = f.bridge();
    b.observe(&event("1:a", "list_teams", json!({})));
    for (key, value) in [
        ("session_id", json!("foreign")),
        ("agent_id", json!("child")),
        ("generation", json!(2)),
        ("task_id", json!("untrusted")),
    ] {
        let mut body = callback("1:a", "list_teams", json!({}));
        body[key] = value;
        assert!(b.dispatch("/call", body).is_err(), "{key}");
    }
    assert_eq!(
        b.dispatch("/call", callback("1:a", "list_teams", json!({})))
            .unwrap()["ok"],
        true
    );
    b.close();
}
#[test]
fn kimi_source_context_can_precede_acp_bind_but_never_change() {
    let f = Fixture::new();
    let (events, _) = crate::driver::test_event_channel();
    let b = KimiBridge::listen(f.context.clone(), events).unwrap();
    let context = json!({"session_id":"native","agent_id":"main","generation":1});
    b.dispatch("/context", context.clone()).unwrap();
    assert!(b.bind("foreign").is_err());
    b.bind("native").unwrap();
    assert!(b.dispatch("/context", context.clone()).is_err());
    b.dispatch("/revoke", context).unwrap();
    assert!(b.activate(b.queue_turn()) == false);
    assert!(b.bind("native").is_err());
}
#[test]
fn kimi_source_pending_and_progress_are_not_executable_witnesses() {
    let f = Fixture::new();
    let (b, _) = f.bridge();
    for status in ["pending", "completed"] {
        let mut e = event("1:a", "list_teams", json!({}));
        e["update"]["status"] = json!(status);
        b.observe(&e);
    }
    let mut delta = event("1:a", "list_teams", json!({}));
    delta["update"].as_object_mut().unwrap().remove("rawInput");
    b.observe(&delta);
    assert!(b.0.state.lock().calls.is_empty());
    let mut foreign = event("1:a", "list_teams", json!({}));
    foreign["sessionId"] = json!("foreign");
    b.observe(&foreign);
    assert!(b.0.state.lock().calls.is_empty());
    b.observe(&event("1:a", "list_teams", json!({})));
    assert_eq!(
        b.dispatch("/call", callback("1:a", "list_teams", json!({})))
            .unwrap()["ok"],
        true
    );
    b.close();
}
#[test]
fn kimi_source_native_turn_identity_is_not_reused_after_settlement() {
    let f = Fixture::new();
    let (b, _) = f.bridge();
    b.observe(&event("1:a", "list_teams", json!({})));
    b.finish_turn(b.active_turn());
    assert!(b.activate(b.queue_turn()));
    b.observe(&event("1:late", "list_teams", json!({})));
    assert!(
        b.dispatch("/call", callback("1:late", "list_teams", json!({})))
            .is_err()
    );
    b.observe(&event("2:a", "list_teams", json!({})));
    assert_eq!(
        b.dispatch("/call", callback("2:a", "list_teams", json!({})))
            .unwrap()["ok"],
        true
    );
    b.close();
}
#[test]
fn kimi_source_abort_before_observation_keeps_tombstone() {
    let f = Fixture::new();
    let (b, _) = f.bridge();
    let body = callback("1:a", "list_teams", json!({}));
    b.dispatch("/cancel", body.clone()).unwrap();
    b.observe(&event("1:a", "list_teams", json!({})));
    assert!(b.dispatch("/call", body).is_err());
    b.close();
}
#[test]
fn kimi_source_failed_native_call_cancels_worker() {
    let f = Fixture::new();
    let (b, received) = f.bridge();
    b.observe(&event("1:a", "invoke_team", f.args()));
    let (child, result) = invoke(&b, callback("1:a", "invoke_team", f.args()));
    let permission = approval(&received);
    b.observe(&json!({"sessionId":"native","update":{"sessionUpdate":"tool_call_update","toolCallId":"1:a","status":"failed"}}));
    let _ = b.respond(&permission, "allow");
    assert!(
        result
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .is_err()
    );
    child.join().unwrap();
    assert!(f.runs().is_empty());
    b.close();
}
#[test]
fn kimi_source_callback_can_arrive_before_native_acp_delivery() {
    let f = Fixture::new();
    let (b, _) = f.bridge();
    let (child, result) = invoke(&b, callback("1:a", "list_teams", json!({})));
    thread::sleep(Duration::from_millis(75));
    b.observe(&event("1:a", "list_teams", json!({})));
    assert_eq!(
        result
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap()["ok"],
        true
    );
    child.join().unwrap();
    b.close();
}
#[test]
fn kimi_source_loopback_authorization_and_schema_route() {
    let f = Fixture::new();
    let (b, _) = f.bridge();
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();
    let url = format!("http://127.0.0.1:{}/tools", b.0.port);
    let post = |token: &str, origin: bool| {
        let mut r = client.post(&url).bearer_auth(token).json(&json!({}));
        if origin {
            r = r.header("Origin", "http://localhost");
        }
        r.send().and_then(|reply| reply.json::<Value>())
    };
    assert!(post("wrong", false).map_or(true, |v| v["ok"] == false));
    assert!(post(&b.0.token, true).map_or(true, |v| v["ok"] == false));
    assert_eq!(post(&b.0.token, false).unwrap(), super::super::tools());
    b.close();
}
#[test]
fn kimi_source_private_credential_is_scrubbed() {
    let f = Fixture::new();
    let (b, _) = f.bridge();
    let mut v = json!({"nested":[format!("failure {}",b.0.token)]});
    b.scrub(&mut v);
    assert!(!v.to_string().contains(&b.0.token));
    b.close();
}

#[test]
fn kimi_source_steer_can_advance_native_turn_without_reopening_older_turns() {
    let f = Fixture::new();
    let (b, _) = f.bridge();
    b.observe(&event("1:first", "list_teams", json!({})));
    assert_eq!(
        b.dispatch("/call", callback("1:first", "list_teams", json!({})))
            .unwrap()["ok"],
        true
    );
    // ACP steer can create another native turn while the driver retains its
    // active request envelope. That main-context callback is still legitimate.
    b.observe(&event("2:steered", "list_teams", json!({})));
    assert_eq!(
        b.dispatch("/call", callback("2:steered", "list_teams", json!({})))
            .unwrap()["ok"],
        true
    );
    b.observe(&event("1:late", "list_teams", json!({})));
    assert!(
        b.dispatch("/call", callback("1:late", "list_teams", json!({})))
            .is_err()
    );
    b.finish_turn(b.active_turn());
    assert!(b.activate(b.queue_turn()));
    b.observe(&event("2:late", "list_teams", json!({})));
    assert!(
        b.dispatch("/call", callback("2:late", "list_teams", json!({})))
            .is_err()
    );
    b.close();
}

#[path = "kimi_native_tests.rs"]
mod native;
