//! A revoked native binding must also cancel the real shared worker's approval.
use super::*;
use uuid::Uuid;
use waku_protocol::{
    control::{AgentProfile, ControlRecord, Team, TeamMember},
    model::{DriverEvent, ProviderKind, RuntimeMode},
    teams::{AgentAccess, AgentExecution, TeamRunLimits},
};
#[test]
fn cursor_source_team_changed_callback_and_permission_revoke_pending_worker() {
    for permission in [false, true] {
        let dir = crate::driver::team_codex::EmptyDirectory::create("cursor-revocation").unwrap();
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
        let (events, received) = crate::driver::test_event_channel();
        let bridge = CursorBridge(Arc::new(Inner {
            context: context.clone(),
            events,
            server: "ion-server".into(),
            prefix: "ion_private_".into(),
            token: private_token(),
            port: 12345,
            closed: AtomicBool::new(false),
            state: Mutex::new(State::default()),
            hook: HookLease::install(&dir.0, "one", "^MCP:ion_private_invoke_team$", "exit 0")
                .unwrap(),
        }));
        bridge.bind("native-session").unwrap();
        assert!(bridge.activate(bridge.queue_turn()));
        let args = json!({"team_id":team,"definition_revision":store.load().unwrap().revision,"objective":"Reviewed work"});
        bridge.observe(&json!({"sessionId":"native-session","update":{"sessionUpdate":"tool_call","toolCallId":"native-call","status":"pending","rawInput":{"providerIdentifier":"ion-server","toolName":"ion_private_invoke_team","args":args}}}));
        let output=bridge.hook(&json!({"session_id":"native-session","conversation_id":"native-session","hook_event_name":"preToolUse","tool_use_id":"hook-call","tool_name":"MCP:ion_private_invoke_team","tool_input":args}));
        assert_eq!(output["permission"], "allow");
        let mut input = args;
        input[FIELD] = output["updated_input"][FIELD].clone();
        let mut native = json!({"sessionId":"native-session","toolCall":{"toolCallId":"native-call","title":"ion-server-ion_private_invoke_team: ion_private_invoke_team","content":[{"type":"content","content":{"type":"text","text":format!("```json\n{}\n```",serde_json::to_string_pretty(&input).unwrap())}}]}});
        bridge
            .permission(&native)
            .unwrap()
            .unwrap()
            .decide(true)
            .unwrap();
        let active = bridge.clone();
        let request = json!({"name":"ion_private_invoke_team","arguments":input});
        let (sent, result) = crossbeam_channel::bounded(1);
        let call = request.clone();
        let worker = thread::spawn(move || {
            let _ = sent.send(active.dispatch(&call));
        });
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let approval = loop {
            if let DriverEvent::Permission { request_id, .. } = received
                .recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
                .unwrap()
            {
                break request_id;
            }
        };
        if permission {
            native["toolCall"]["content"][0]["content"]["text"] = json!("malformed permission");
            assert!(bridge.permission(&native).is_err());
        } else {
            let mut changed = request;
            changed["arguments"]["objective"] = json!("Changed after approval");
            assert!(bridge.dispatch(&changed).is_err());
        }
        let _ = bridge.respond(&approval, "allow");
        assert!(
            result
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .is_err()
        );
        worker.join().unwrap();
        assert!(
            crate::teams::TeamCoordinator::new(context.control_path)
                .runs_for_source(context.task_id)
                .unwrap()
                .is_empty()
        );
        bridge.close();
    }
}
