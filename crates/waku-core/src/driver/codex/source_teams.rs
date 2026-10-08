//! Codex's native dynamic tools translated into owner-scoped Ion team calls.
//! One bounded worker keeps provider reads/interrupts responsive during approval
//! and waits. Only the live main turn can submit work; no native ID is a grant.
use super::CommandMessage;
use crate::driver::{DriverEventSender, WorkspaceContext, source_teams};
use anyhow::{Context, Result, ensure};
use crossbeam_channel::{Sender, bounded};
use parking_lot::Mutex;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;
use waku_protocol::{
    control::AgentProfile,
    model::{ActivityKind, DriverEvent, PermissionOption},
    teams::{AgentAccess, TeamRunLimits},
};

#[derive(Clone)]
pub(super) struct Bridge {
    state: Arc<State>,
    jobs: Sender<Call>,
    commands: Sender<CommandMessage>,
}
struct State {
    scope: Mutex<Option<Scope>>,
    approvals: Mutex<HashMap<String, Sender<bool>>>,
    closed: AtomicBool,
}
struct Scope {
    thread: String,
    turn: String,
    cancelled: Arc<AtomicBool>,
    witnesses: HashMap<String, [u8; 32]>,
}
struct Call {
    rpc_id: Value,
    request_id: String,
    name: String,
    arguments: Value,
    cancelled: Arc<AtomicBool>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NativeCall {
    thread_id: String,
    turn_id: String,
    call_id: String,
    tool: String,
    #[serde(default)]
    namespace: Option<String>,
    arguments: Value,
}

pub(super) fn tools() -> Value {
    Value::Array(source_teams::tools().as_array().unwrap().iter().map(|tool| {
        let f=&tool["function"];
        json!({"type":"function","name":format!("ion_{}",f["name"].as_str().unwrap()),"description":f["description"],"inputSchema":f["parameters"],"deferLoading":false})
    }).collect())
}
impl Bridge {
    pub(super) fn start(
        context: WorkspaceContext,
        commands: Sender<CommandMessage>,
        events: DriverEventSender,
    ) -> Result<Self> {
        let state = Arc::new(State {
            scope: Mutex::new(None),
            approvals: Mutex::new(HashMap::new()),
            closed: AtomicBool::new(false),
        });
        let (jobs, received) = bounded::<Call>(4);
        let worker_state = state.clone();
        let writer = commands.clone();
        thread::Builder::new().name("ion-codex-team-tools".into()).spawn(move || {
            while !worker_state.closed.load(Ordering::Acquire) {
                let call=match received.recv_timeout(Duration::from_millis(100)) {
                    Ok(call)=>call,
                    Err(crossbeam_channel::RecvTimeoutError::Timeout)=>continue,
                    Err(_)=>break,
                };
                let activity=|complete, detail| DriverEvent::Activity { id:Some(call.request_id.clone()), kind:ActivityKind::Tool, title:call.name.replace('_'," "), detail, complete };
                let _=events.send(activity(false,None));
                let result=source_teams::execute(&context,&call.request_id,&call.name,&call.arguments,&call.cancelled, |profile, limits| {
                    approve(&worker_state, &call.cancelled, &events, profile, limits)
                });
                let (success,text)=match result {
                    Ok(text)=>(true,text),
                    // Native/database error payloads and raw arguments stay private.
                    Err(_)=>(false,"Ion team request did not complete. Check team setup, current definitions, source-task approval and run status. No invocation is automatically retried.".into()),
                };
                let _=events.send(activity(true,Some(text.clone())));
                let _=writer.send(CommandMessage::TeamResult(reply(call.rpc_id,success,text)));
            }
        })?;
        Ok(Self {
            state,
            jobs,
            commands,
        })
    }
    /// Observe only main-thread notifications. Subagents cannot select a scope.
    pub(super) fn observe(&self, value: &Value, main_thread: Option<&str>) {
        if value.pointer("/params/threadId").and_then(Value::as_str) != main_thread
            || main_thread.is_none()
        {
            return;
        }
        let Some(turn) = value.pointer("/params/turn/id").and_then(Value::as_str) else {
            return;
        };
        match value["method"].as_str() {
            Some("turn/started") => {
                let mut scope = self.state.scope.lock();
                if scope
                    .as_ref()
                    .is_some_and(|s| Some(s.thread.as_str()) == main_thread && s.turn == turn)
                {
                    return;
                }
                if let Some(old) = scope.take() {
                    old.cancelled.store(true, Ordering::Release);
                }
                *scope = Some(Scope {
                    thread: main_thread.unwrap().into(),
                    turn: turn.into(),
                    cancelled: Arc::new(AtomicBool::new(false)),
                    witnesses: HashMap::new(),
                });
            }
            Some("turn/completed") => {
                let mut scope = self.state.scope.lock();
                if scope
                    .as_ref()
                    .is_some_and(|s| Some(s.thread.as_str()) == main_thread && s.turn == turn)
                {
                    if let Some(old) = scope.take() {
                        old.cancelled.store(true, Ordering::Release);
                    }
                }
            }
            _ => {}
        }
    }
    pub(super) fn dispatch(&self, value: &Value) -> bool {
        if value["method"] != "item/tool/call" {
            return false;
        }
        let Some(id) = value.get("id").filter(|id| valid_rpc_id(id)).cloned() else {
            return true;
        };
        let prepared = (|| -> Result<Call> {
            ensure!(!self.state.closed.load(Ordering::Acquire), "Bridge closed");
            let native: NativeCall = serde_json::from_value(value["params"].clone())?;
            ensure!(
                native.namespace.is_none() && native.arguments.is_object(),
                "Invalid native tool namespace/arguments"
            );
            ensure!(
                [&native.thread_id, &native.turn_id, &native.call_id]
                    .iter()
                    .all(|s| !s.is_empty() && s.len() <= 256),
                "Native call identity is invalid"
            );
            let name = native
                .tool
                .strip_prefix("ion_")
                .filter(|name| source_teams::accepts(name))
                .context("Unknown Ion tool")?
                .to_owned();
            ensure!(
                serde_json::to_vec(&native.arguments)?.len() <= 40 * 1024,
                "Native arguments exceed limit"
            );
            let key =
                serde_json::to_vec(&json!([native.thread_id, native.turn_id, native.call_id]))?;
            let request_id = format!("codex-team-{:x}", Sha256::digest(key));
            let witness: [u8; 32] =
                Sha256::digest(serde_json::to_vec(&json!([name, native.arguments]))?).into();
            let mut guard = self.state.scope.lock();
            let scope = guard.as_mut().context("No active main turn")?;
            ensure!(
                scope.thread == native.thread_id
                    && scope.turn == native.turn_id
                    && !scope.cancelled.load(Ordering::Acquire),
                "Unrelated or stopped native call"
            );
            if let Some(saved) = scope.witnesses.get(&request_id) {
                ensure!(saved == &witness, "Native replay changed tool arguments");
            } else {
                ensure!(scope.witnesses.len() < 128, "Turn team call limit reached");
                scope.witnesses.insert(request_id.clone(), witness);
            }
            Ok(Call {
                rpc_id: id.clone(),
                request_id,
                name,
                arguments: native.arguments,
                cancelled: scope.cancelled.clone(),
            })
        })();
        if let Ok(call) = prepared {
            match self.jobs.try_send(call) {
                Ok(()) => return true,
                Err(error) => {
                    let call = error.into_inner();
                    self.reject(call.rpc_id);
                }
            }
        } else {
            self.reject(id);
        }
        true
    }
    fn reject(&self, id: Value) {
        let _=self.commands.send(CommandMessage::TeamResult(reply(id,false,"Ion team call rejected: no matching active source turn, invalid arguments, or bridge busy. No team was started by this rejected call.".into())));
    }
    pub(super) fn respond(&self, id: &str, option: &str) -> bool {
        if !id.starts_with("ion-team-approval-") {
            return false;
        }
        if let Some(sender) = self.state.approvals.lock().remove(id) {
            let _ = sender.send(option == "allow");
        }
        true // Late approval IDs must never become native JSON-RPC replies.
    }
    pub(super) fn cancel_turn(&self) {
        if let Some(scope) = self.state.scope.lock().as_ref() {
            scope.cancelled.store(true, Ordering::Release);
        }
    }
    pub(super) fn close(&self) {
        self.state.closed.store(true, Ordering::Release);
        self.cancel_turn();
        self.state.approvals.lock().clear();
    }
}
fn valid_rpc_id(id: &Value) -> bool {
    id.as_i64().is_some()
        || id.as_u64().is_some()
        || id.as_str().is_some_and(|s| !s.is_empty() && s.len() <= 256)
}
fn reply(id: Value, success: bool, text: String) -> Value {
    json!({"id":id,"result":{"contentItems":[{"type":"inputText","text":text}],"success":success}})
}
fn approve(
    state: &State,
    cancelled: &AtomicBool,
    events: &DriverEventSender,
    profile: &AgentProfile,
    limits: &TeamRunLimits,
) -> Result<AgentAccess> {
    ensure!(
        !state.closed.load(Ordering::Acquire) && !cancelled.load(Ordering::Acquire),
        "Stopped before approval"
    );
    let id = format!("ion-team-approval-{}", Uuid::new_v4());
    let (sender, received) = bounded(1);
    state.approvals.lock().insert(id.clone(), sender);
    let result = (|| {
        events.send(DriverEvent::Permission {
            request_id:id.clone(), title:format!("Use team agent {}?",profile.name),
            detail:format!("Harness: {}\nModel: {}\nExecution environment: {}\nConfigured access: {}\nRun limits: {}\nEdits use an isolated branch. App actions require their own exact approval.", profile.provider.display_name(),profile.model.as_deref().unwrap_or("default"),profile.execution.environment.label(),serde_json::to_string(&profile.access)?,serde_json::to_string(limits)?),
            options:vec![PermissionOption{id:"allow".into(),label:"Start team".into(),allow:true},PermissionOption{id:"reject".into(),label:"Decline".into(),allow:false}],
        }).map_err(|_|anyhow::anyhow!("Source task closed"))?;
        let deadline = Instant::now() + Duration::from_secs(300);
        loop {
            ensure!(
                !state.closed.load(Ordering::Acquire)
                    && !cancelled.load(Ordering::Acquire)
                    && Instant::now() < deadline,
                "Team approval stopped or expired"
            );
            match received.recv_timeout(Duration::from_millis(100)) {
                Ok(true) => {
                    ensure!(!cancelled.load(Ordering::Acquire), "Approval canceled");
                    return Ok(profile.access.clone());
                }
                Ok(false) => anyhow::bail!("Team invocation declined"),
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
                Err(_) => anyhow::bail!("Approval channel closed"),
            }
        }
    })();
    state.approvals.lock().remove(&id);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::{Receiver, unbounded};
    use waku_protocol::{
        control::{ControlRecord, Team, TeamMember},
        model::{ProviderKind, RuntimeMode},
        teams::AgentExecution,
    };
    struct Fixture {
        _directory: crate::driver::team_codex::EmptyDirectory,
        context: WorkspaceContext,
        team: Uuid,
        revision: u64,
    }
    impl Fixture {
        fn new(mode: RuntimeMode) -> Self {
            let directory =
                crate::driver::team_codex::EmptyDirectory::create("codex-source-teams").unwrap();
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
            let revision = store.load().unwrap().revision;
            let context = WorkspaceContext {
                task_id: Uuid::new_v4(),
                project_id: Uuid::new_v4(),
                control_path: store.path(),
                settings_path: directory.0.join("unused.json"),
                mode,
            };
            Self {
                _directory: directory,
                context,
                team,
                revision,
            }
        }
        fn bridge(&self) -> (Bridge, Receiver<CommandMessage>, Receiver<DriverEvent>) {
            let (commands, replies) = unbounded();
            let (events, received) = crate::driver::test_event_channel();
            let bridge = Bridge::start(self.context.clone(), commands, events).unwrap();
            bridge.observe(
                &json!({"method":"turn/started","params":{"threadId":"main","turn":{"id":"turn"}}}),
                Some("main"),
            );
            (bridge, replies, received)
        }
        fn invoke(&self, id: Value, call: &str) -> Value {
            self.call(id,call,"invoke_team",json!({"team_id":self.team,"definition_revision":self.revision,"objective":"Implement reviewed work"}))
        }
        fn call(&self, id: Value, call: &str, name: &str, args: Value) -> Value {
            json!({"method":"item/tool/call","id":id,"params":{"threadId":"main","turnId":"turn","callId":call,"tool":format!("ion_{name}"),"arguments":args}})
        }
        fn runs(&self) -> Vec<waku_protocol::teams::TeamRunSnapshot> {
            crate::teams::TeamCoordinator::new(self.context.control_path.clone())
                .runs_for_source(self.context.task_id)
                .unwrap()
        }
    }
    fn response(replies: &Receiver<CommandMessage>) -> Value {
        match replies.recv_timeout(Duration::from_secs(3)).unwrap() {
            CommandMessage::TeamResult(value) => value,
            _ => panic!("Team result expected"),
        }
    }
    fn permission(events: &Receiver<DriverEvent>) -> String {
        loop {
            if let DriverEvent::Permission {
                request_id, detail, ..
            } = events.recv_timeout(Duration::from_secs(3)).unwrap()
            {
                assert!(detail.contains("workspaceWrite") && detail.contains("500000"));
                return request_id;
            }
        }
    }
    #[test]
    fn codex_source_team_approval_replay_and_restart_keep_exact_scope() {
        let f = Fixture::new(RuntimeMode::Ask);
        let (bridge, replies, events) = f.bridge();
        let call = f.invoke(json!(10), "native-call");
        assert!(bridge.dispatch(&call));
        let approval = permission(&events);
        assert!(f.runs().is_empty());
        assert!(bridge.respond(&approval, "allow"));
        let result = response(&replies);
        assert_eq!(result["id"], 10);
        assert_eq!(result["result"]["success"], true);
        let runs = f.runs();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].project_id, f.context.project_id);
        assert!(runs[0].work[0].access.workspace_write);
        assert_eq!(runs[0].limits.max_tokens, Some(500000));
        assert!(bridge.dispatch(&call));
        assert_eq!(response(&replies)["result"]["success"], true);
        let mut changed = call.clone();
        changed["params"]["arguments"]["objective"] = json!("Different work");
        assert!(bridge.dispatch(&changed));
        assert_eq!(response(&replies)["result"]["success"], false);
        assert_eq!(f.runs().len(), 1);
        bridge.close();
        let (reopened, replies, events) = f.bridge();
        let mut original = call;
        original["id"] = json!("new-peer-rpc-id");
        assert!(reopened.dispatch(&original));
        assert_eq!(response(&replies)["result"]["success"], true);
        assert!(
            !events
                .try_iter()
                .any(|e| matches!(e, DriverEvent::Permission { .. }))
        );
        assert_eq!(f.runs().len(), 1);
        // Changed replay must also fail after the in-memory witness was lost.
        assert!(reopened.dispatch(&changed));
        assert_eq!(response(&replies)["result"]["success"], false);
        reopened.close();
    }
    #[test]
    fn codex_source_team_tools_deny_foreign_turns_arguments_and_names() {
        let f = Fixture::new(RuntimeMode::FullAccess);
        let (bridge, replies, _events) = f.bridge();
        let good = f.invoke(json!(1), "denied");
        for (key, value) in [
            ("threadId", json!("subagent")),
            ("turnId", json!("stale")),
            ("callId", json!("")),
            ("namespace", json!("foreign")),
            ("tool", json!("ion_tokenfactory_exec")),
            ("arguments", json!([])),
        ] {
            let mut call = good.clone();
            call["params"][key] = value;
            assert!(bridge.dispatch(&call));
            assert_eq!(response(&replies)["result"]["success"], false, "{key}");
        }
        let mut forged = good.clone();
        forged["params"]["arguments"]["project_id"] = json!(Uuid::new_v4());
        assert!(bridge.dispatch(&forged));
        assert_eq!(response(&replies)["result"]["success"], false);
        assert!(f.runs().is_empty());
        bridge.observe(&json!({"method":"turn/started","params":{"threadId":"subagent","turn":{"id":"stale"}}}),Some("main"));
        // A main-turn, unaffected by the unrelated subagent notification, still works.
        let allowed = f.invoke(json!(3), "allowed");
        bridge.dispatch(&allowed);
        assert_eq!(response(&replies)["result"]["success"], true);
        bridge.cancel_turn();
        bridge.dispatch(&f.invoke(json!(4), "after-stop"));
        assert_eq!(response(&replies)["result"]["success"], false);
        assert_eq!(f.runs().len(), 1);
        bridge.close();
        assert!(bridge.respond("ion-team-approval-stale", "allow"));
        assert!(!bridge.respond("native-request", "accept"));
    }
    #[test]
    fn codex_source_team_decline_cancel_and_backpressure_never_start_work() {
        let f = Fixture::new(RuntimeMode::Ask);
        let (bridge, replies, events) = f.bridge();
        bridge.dispatch(&f.invoke(json!(1), "declined"));
        let id = permission(&events);
        bridge.respond(&id, "reject");
        assert_eq!(response(&replies)["result"]["success"], false);
        bridge.dispatch(&f.invoke(json!(2), "canceled"));
        let id = permission(&events);
        for n in 3..7 {
            bridge.dispatch(&f.invoke(json!(n), &format!("queued-{n}")));
        }
        bridge.dispatch(&f.invoke(json!(7), "overflow"));
        let overflow = response(&replies);
        assert_eq!(overflow["id"], 7);
        assert_eq!(overflow["result"]["success"], false);
        bridge.cancel_turn();
        bridge.respond(&id, "allow");
        for _ in 0..5 {
            assert_eq!(response(&replies)["result"]["success"], false);
        }
        assert!(f.runs().is_empty());
        bridge.close();
    }
    #[test]
    fn codex_source_team_catalog_run_reads_and_source_invocation_cap() {
        let f = Fixture::new(RuntimeMode::FullAccess);
        let (bridge, replies, _events) = f.bridge();
        bridge.dispatch(&f.call(json!(1), "catalog", "list_teams", json!({})));
        let catalog = response(&replies);
        assert_eq!(catalog["result"]["success"], true);
        let body: Value = serde_json::from_str(
            catalog["result"]["contentItems"][0]["text"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(body["definition_revision"], f.revision);
        assert_eq!(body["teams"][0]["id"], f.team.to_string());
        for n in 0..5 {
            bridge.dispatch(&f.invoke(json!(n + 2), &format!("invoke-{n}")));
            assert_eq!(response(&replies)["result"]["success"], n < 4);
        }
        assert_eq!(f.runs().len(), 4);
        bridge.dispatch(&f.call(json!(9), "run-list", "get_team_runs", json!({})));
        let result = response(&replies);
        assert_eq!(result["result"]["success"], true);
        let body: Value = serde_json::from_str(
            result["result"]["contentItems"][0]["text"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(body.as_array().unwrap().len(), 4);
        bridge.dispatch(&f.call(
            json!(10),
            "foreign-read",
            "read_team_document",
            json!({"run_id":Uuid::new_v4(),"artifact_id":Uuid::new_v4()}),
        ));
        assert_eq!(response(&replies)["result"]["success"], false);
        bridge.close();
        let specs = tools();
        assert_eq!(specs.as_array().unwrap().len(), 5);
        for spec in specs.as_array().unwrap() {
            assert_eq!(spec["type"], "function");
            assert_eq!(spec["inputSchema"]["additionalProperties"], false);
            assert!(
                !spec["inputSchema"]["properties"]
                    .as_object()
                    .unwrap()
                    .contains_key("project_id")
            );
        }
    }
    #[test]
    fn codex_source_team_cancel_during_commit_or_error_revokes_invocations() {
        for fail_authorization in [false, true] {
            let f = Fixture::new(RuntimeMode::Ask);
            let arguments = f.invoke(json!(1), "original")["params"]["arguments"].clone();
            let cancelled = AtomicBool::new(false);
            if fail_authorization {
                source_teams::execute(
                    &f.context,
                    "already-queued",
                    "invoke_team",
                    &arguments,
                    &cancelled,
                    |profile, _| Ok(profile.access.clone()),
                )
                .unwrap();
            }
            let result = source_teams::execute(
                &f.context,
                "racing-invocation",
                "invoke_team",
                &arguments,
                &cancelled,
                |profile, _| {
                    cancelled.store(true, Ordering::Release);
                    if fail_authorization {
                        anyhow::bail!("Source closed during approval");
                    }
                    Ok(profile.access.clone())
                },
            );
            assert!(result.is_err());
            let runs = f.runs();
            assert_eq!(runs.len(), 1);
            assert_eq!(
                runs[0].status,
                waku_protocol::teams::TeamRunStatus::Canceled
            );
        }
    }
    fn native_function(call: &str, name: &str, args: Value) -> Vec<u8> {
        let item = json!({"id":format!("fc_{call}"),"type":"function_call","status":"completed","call_id":call,"name":name,"arguments":args.to_string()});
        let response = json!({"id":format!("resp_{call}"),"object":"response","created_at":1,"status":"completed","model":"gpt-5.4","output":[item.clone()],"usage":{"input_tokens":100,"output_tokens":10,"total_tokens":110,"input_tokens_details":{"cached_tokens":0},"output_tokens_details":{"reasoning_tokens":0}}});
        let mut begun = item.clone();
        begun["arguments"] = json!("");
        begun["status"] = json!("in_progress");
        [json!({"type":"response.created","response":{"id":format!("resp_{call}"),"status":"in_progress"}}),
            json!({"type":"response.output_item.added","output_index":0,"item":begun}),
            json!({"type":"response.function_call_arguments.delta","item_id":format!("fc_{call}"),"output_index":0,"delta":args.to_string()}),
            json!({"type":"response.function_call_arguments.done","item_id":format!("fc_{call}"),"output_index":0,"arguments":args.to_string()}),
            json!({"type":"response.output_item.done","output_index":0,"item":item}),
            json!({"type":"response.completed","response":response})]
            .iter().map(|v|format!("event: {}\ndata: {v}\n\n",v["type"].as_str().unwrap())).collect::<String>().into_bytes()
    }
    fn native_driver(
        native: &std::path::Path,
        home: &std::path::Path,
        work: &std::path::Path,
        temporary: &std::path::Path,
        context: WorkspaceContext,
        base: String,
        events: DriverEventSender,
        cursor: Option<waku_protocol::model::ProviderResumeCursor>,
    ) -> super::super::CodexDriver {
        use crate::driver::DriverStartOptions;
        let home = home.to_owned();
        let temporary = temporary.to_owned();
        let system_root = std::env::var_os("SystemRoot");
        super::super::CodexDriver::start_configured(
            DriverStartOptions{binary:native.to_owned(),cwd:work.to_owned(),mode:RuntimeMode::Ask,model:Some("gpt-5.4".into()),reasoning_effort:None,service_tier:None,context_window:None,agent_preset:None,computer_use_enabled:false,provider_cursor:cursor},
            events,Some(context),move |command| {
                command.env_clear();
                if let Some(root)=system_root {command.env("SystemRoot",root);}
                command.env("CODEX_HOME",&home).env("HOME",&home).env("USERPROFILE",&home).env("APPDATA",&home).env("LOCALAPPDATA",&home).env("TMP",&temporary).env("TEMP",&temporary).env("TMPDIR",&temporary)
                    .env("ION_CODEX_FIXTURE_KEY","sk-ion-native-fixture-only").env("OPENAI_API_KEY","sk-ion-native-fixture-only");
                for config in [
                    "model_provider=\"ion_fixture\"".to_owned(),
                    format!("model_providers.ion_fixture={{name=\"Private Ion fixture\",base_url={},wire_api=\"responses\",env_key=\"ION_CODEX_FIXTURE_KEY\",requires_openai_auth=false,request_max_retries=0,stream_max_retries=0,supports_websockets=false}}",serde_json::to_string(&base).unwrap()),
                    "cli_auth_credentials_store=\"file\"".into(),
                    "features.shell_tool=false".into(),"features.unified_exec=false".into(),"features.multi_agent=false".into(),"features.apps=false".into(),"features.plugins=false".into(),"web_search=\"disabled\"".into(),"project_doc_max_bytes=0".into(),
                ] {command.args(["-c",&config]);}
            },false
        ).unwrap()
    }
    fn wait_native_exit(received: &Receiver<DriverEvent>) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if matches!(
                received.recv_timeout(Duration::from_millis(100)),
                Ok(DriverEvent::ProcessExited)
            ) {
                return;
            }
        }
        panic!("Private native fixture process did not exit");
    }
    #[test]
    #[ignore = "Requires ION_NATIVE_CODEX_BINARY; private fake account and localhost Responses fixture only"]
    fn codex_source_team_native_app_server_invokes_through_actual_ion_driver() {
        use crate::driver::DriverControl;
        let native = std::env::var_os("ION_NATIVE_CODEX_BINARY")
            .map(std::path::PathBuf::from)
            .expect("Set the exact native Codex executable for this isolated fixture");
        let f = Fixture::new(RuntimeMode::Ask);
        let home = f._directory.0.join("native-home");
        std::fs::create_dir(&home).unwrap();
        let work = f._directory.0.join("native-work");
        std::fs::create_dir(&work).unwrap();
        let temporary = f._directory.0.join("native-temporary");
        std::fs::create_dir(&temporary).unwrap();
        let responses = vec![
            (200, native_function("list", "ion_list_teams", json!({}))),
            (
                200,
                native_function(
                    "invoke",
                    "ion_invoke_team",
                    json!({"team_id":f.team,"definition_revision":f.revision,"objective":"Implement reviewed work"}),
                ),
            ),
            (200, native_function("runs", "ion_get_team_runs", json!({}))),
            (
                200,
                crate::driver::team_test_http::responses(
                    "gpt-5.4",
                    "Team queued; inspect its run for completion.",
                ),
            ),
        ];
        let server = crate::driver::team_test_http::Server::with_response_handler(
            responses,
            |request, body| {
                assert!(request.line.starts_with("POST /responses "));
                assert_eq!(request.body["model"], "gpt-5.4");
                assert!(request.headers.contains("sk-ion-native-fixture-only"));
                let tools = request.body["tools"].as_array().unwrap();
                assert!(tools.iter().any(|t| t["name"] == "ion_invoke_team"
                    || t["function"]["name"] == "ion_invoke_team"));
                assert!(!tools.iter().any(|t| matches!(
                    t["name"].as_str(),
                    Some("shell" | "exec_command" | "spawn_agent")
                )));
                body
            },
        );
        let base = server.url.clone();
        let (events, received) = crate::driver::test_event_channel();
        let driver = native_driver(
            &native,
            &home,
            &work,
            &temporary,
            f.context.clone(),
            base,
            events,
            None,
        );
        let deadline = Instant::now() + Duration::from_secs(25);
        let mut connected = false;
        let mut cursor = None;
        let mut approved = false;
        let mut completed = false;
        let mut text = String::new();
        while Instant::now() < deadline {
            let Ok(event) = received.recv_timeout(Duration::from_millis(200)) else {
                continue;
            };
            match event {
                DriverEvent::Connected { provider_cursor } if !connected => {
                    cursor = provider_cursor;
                    connected = true;
                    driver.prompt(
                        "Use the saved engineering team and report its actual queue state.".into(),
                    );
                }
                DriverEvent::Permission {
                    request_id, detail, ..
                } => {
                    assert!(request_id.starts_with("ion-team-approval-"));
                    assert!(detail.contains("workspaceWrite") && detail.contains("500000"));
                    assert!(f.runs().is_empty());
                    approved = true;
                    driver.respond(request_id, "allow".into());
                }
                DriverEvent::TextDelta(delta) => text.push_str(&delta),
                DriverEvent::TurnFinished { success, .. } => {
                    assert!(success, "Native source turn failed");
                    completed = true;
                    break;
                }
                DriverEvent::Error(error) => panic!("Native fixture failed: {error}"),
                _ => {}
            }
        }
        driver.cancel();
        drop(driver);
        wait_native_exit(&received);
        assert!(
            connected && approved && completed,
            "Native bridge did not complete its bounded fixture"
        );
        assert!(text.contains("Team queued"));
        let runs = f.runs();
        assert_eq!(runs.len(), 1);
        assert!(runs[0].work[0].access.workspace_write);
        assert_eq!(runs[0].project_id, f.context.project_id);
        assert_eq!(runs[0].limits.max_tokens, Some(500000));
        assert!(
            runs[0]
                .origin
                .as_ref()
                .unwrap()
                .request_id
                .starts_with("codex-team-")
        );
        let requests = server.finish();
        assert_eq!(requests.len(), 4);
        let outputs: Vec<Value> = requests
            .iter()
            .flat_map(|r| r.body["input"].as_array().unwrap().iter())
            .filter(|item| item["type"] == "function_call_output")
            .cloned()
            .collect();
        assert!(outputs.iter().any(|item| {
            item["call_id"] == "invoke"
                && item["output"]
                    .as_str()
                    .is_some_and(|text| text.contains(&runs[0].id.to_string()))
        }));
        let resumed_server = crate::driver::team_test_http::Server::with_response_handler(
            vec![
                (
                    200,
                    native_function("resumed-runs", "ion_get_team_runs", json!({})),
                ),
                (
                    200,
                    crate::driver::team_test_http::responses(
                        "gpt-5.4",
                        "Existing team run inspected after reopening.",
                    ),
                ),
            ],
            |request, body| {
                assert!(
                    request.body["tools"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|t| t["name"] == "ion_get_team_runs")
                );
                assert!(request.headers.contains("sk-ion-native-fixture-only"));
                body
            },
        );
        let (events, received) = crate::driver::test_event_channel();
        let driver = native_driver(
            &native,
            &home,
            &work,
            &temporary,
            f.context.clone(),
            resumed_server.url.clone(),
            events,
            Some(cursor.unwrap()),
        );
        let deadline = Instant::now() + Duration::from_secs(25);
        let mut resumed = false;
        while Instant::now() < deadline {
            let Ok(event) = received.recv_timeout(Duration::from_millis(200)) else {
                continue;
            };
            match event {
                DriverEvent::Connected { .. } => driver.prompt(
                    "Inspect my previously invoked team without starting another run.".into(),
                ),
                DriverEvent::TurnFinished { success, .. } => {
                    assert!(success);
                    resumed = true;
                    break;
                }
                DriverEvent::Error(error) => panic!("Native resume fixture failed: {error}"),
                DriverEvent::Permission { .. } => {
                    panic!("Read-only resume must not request another grant")
                }
                _ => {}
            }
        }
        drop(driver);
        wait_native_exit(&received);
        assert!(resumed, "Native resume did not finish its bounded fixture");
        assert_eq!(f.runs().len(), 1);
        let requests = resumed_server.finish();
        assert_eq!(requests.len(), 2);
        assert!(
            requests[1].body["input"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["type"] == "function_call_output"
                    && item["call_id"] == "resumed-runs"
                    && item["output"]
                        .as_str()
                        .is_some_and(|text| text.contains(&runs[0].id.to_string())))
        );
    }
}
