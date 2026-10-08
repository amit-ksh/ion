//! Native SDK MCP requests bound to observed main-session tool-use identities.
use super::CommandMessage;
use crate::driver::{
    DriverEventSender, WorkspaceContext,
    source_teams::{
        self,
        worker::{Call, Worker},
    },
};
use anyhow::{Context, Result, ensure};
use crossbeam_channel::Sender;
use parking_lot::Mutex;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
const SERVER: &str = "ion_teams";
#[derive(Clone)]
pub(super) struct Bridge {
    session: String,
    state: Arc<Mutex<State>>,
    worker: Worker,
    commands: Sender<CommandMessage>,
}
#[derive(Default)]
struct State {
    blocked: bool,
    witnesses: HashMap<String, Witness>,
    pending: HashMap<String, Arc<AtomicBool>>,
}
struct Witness {
    name: String,
    arguments: Value,
    cancelled: Arc<AtomicBool>,
    valid: bool,
}
pub(super) fn configure(command: &mut std::process::Command) {
    command.args([
        "--mcp-config",
        &json!({"mcpServers":{SERVER:{"type":"sdk","name":SERVER}}}).to_string(),
    ]);
    // This permits reaching Ion's handler; owner authorization remains separate.
    let allowed = [
        "list_teams",
        "invoke_team",
        "get_team_runs",
        "await_team",
        "read_team_document",
    ]
    .iter()
    .map(|name| format!("mcp__{SERVER}__{name}"))
    .collect::<Vec<_>>()
    .join(",");
    command.args(["--allowedTools", &allowed]);
}
pub(super) fn initialize() -> Value {
    json!({"type":"control_request","request_id":"ion-team-initialize","request":{"subtype":"initialize","hooks":null}})
}
impl Bridge {
    pub(super) fn start(
        context: WorkspaceContext,
        session: String,
        commands: Sender<CommandMessage>,
        events: DriverEventSender,
    ) -> Result<Self> {
        let state = Arc::new(Mutex::new(State::default()));
        let delivered = state.clone();
        let writer = commands.clone();
        let worker = Worker::start(context, events, move |peer, success, text| {
            let id = peer["control"].as_str().unwrap();
            if delivered.lock().pending.remove(id).is_some() {
                let _=writer.send(CommandMessage::TeamResult(response(id,json!({"jsonrpc":"2.0","id":peer["mcp"],"result":{"content":[{"type":"text","text":text}],"isError":!success}}))));
            }
        })?;
        Ok(Self {
            session,
            state,
            worker,
            commands,
        })
    }
    pub(super) fn allow_turn(&self) {
        let mut state = self.state.lock();
        if state.blocked {
            for witness in state.witnesses.values() {
                witness.cancelled.store(true, Ordering::Release);
            }
            state.witnesses.clear();
            state.blocked = false;
        }
    }
    pub(super) fn observe(&self, value: &Value) {
        if value["session_id"].as_str() != Some(self.session.as_str())
            || !value["parent_tool_use_id"].is_null()
        {
            return;
        }
        if value["type"] == "result" {
            let mut state = self.state.lock();
            for witness in state.witnesses.values() {
                witness.cancelled.store(true, Ordering::Release);
            }
            state.witnesses.clear();
            return;
        }
        if value["type"] != "assistant" || self.worker.is_closed() {
            return;
        }
        let mut state = self.state.lock();
        if state.blocked {
            return;
        }
        for tool in value["message"]["content"].as_array().into_iter().flatten() {
            if tool["type"] != "tool_use" {
                continue;
            }
            let Some(id) = tool["id"].as_str().filter(|id| valid_string(id)) else {
                continue;
            };
            let Some(name) = tool["name"]
                .as_str()
                .and_then(|s| s.strip_prefix("mcp__ion_teams__"))
                .filter(|s| source_teams::accepts(s))
            else {
                continue;
            };
            let arguments = &tool["input"];
            let Ok(encoded) = serde_json::to_vec(arguments) else {
                continue;
            };
            if !arguments.is_object() || encoded.len() > 40 * 1024 {
                continue;
            }
            if let Some(witness) = state.witnesses.get_mut(id) {
                if witness.name != name || witness.arguments != *arguments {
                    witness.valid = false;
                    witness.cancelled.store(true, Ordering::Release);
                }
            } else if state.witnesses.len() < 128 {
                state.witnesses.insert(
                    id.into(),
                    Witness {
                        name: name.into(),
                        arguments: arguments.clone(),
                        cancelled: Arc::new(AtomicBool::new(false)),
                        valid: true,
                    },
                );
            }
        }
    }
    pub(super) fn dispatch(&self, value: &Value) -> bool {
        if value["type"] == "control_cancel_request" {
            if let Some(id) = value["request_id"].as_str() {
                if let Some(cancelled) = self.state.lock().pending.remove(id) {
                    cancelled.store(true, Ordering::Release);
                    return true;
                }
            }
            return false;
        }
        if value["type"] != "control_request"
            || value["request"]["subtype"] != "mcp_message"
            || value["request"]["server_name"] != SERVER
        {
            return false;
        }
        let Some(control) = value["request_id"].as_str().filter(|id| valid_string(id)) else {
            return true;
        };
        let message = &value["request"]["message"];
        if message["jsonrpc"] != "2.0" || self.worker.is_closed() {
            self.reject(control, message);
            return true;
        }
        let method = message["method"].as_str();
        let immediate = match method {
            Some("initialize")
                if matches!(
                    message["params"]["protocolVersion"].as_str(),
                    Some("2024-11-05" | "2025-03-26" | "2025-06-18" | "2025-11-25")
                ) =>
            {
                Some(
                    json!({"protocolVersion":message["params"]["protocolVersion"],"capabilities":{"tools":{}},"serverInfo":{"name":SERVER,"version":"1.0.0"}}),
                )
            }
            Some("notifications/initialized") if message.get("id").is_none() => Some(json!({})),
            Some("ping") => Some(json!({})),
            Some("tools/list")
                if message["params"].is_null()
                    || message["params"].as_object().is_some_and(|p| p.is_empty()) =>
            {
                Some(
                    json!({"tools":source_teams::tools().as_array().unwrap().iter().map(|t| {let f=&t["function"];json!({"name":f["name"],"description":f["description"],"inputSchema":f["parameters"],"annotations":{"readOnlyHint":f["name"]!="invoke_team"}})}).collect::<Vec<_>>()}),
                )
            }
            _ => None,
        };
        if let Some(result) = immediate {
            let mut reply = json!({"jsonrpc":"2.0","result":result});
            if let Some(id) = message.get("id").filter(|id| valid_peer(id)) {
                reply["id"] = id.clone();
            } else if method != Some("notifications/initialized") {
                self.reject(control, message);
                return true;
            }
            let _ = self
                .commands
                .send(CommandMessage::TeamResult(response(control, reply)));
            return true;
        }
        let prepared = (|| -> Result<Option<Call>> {
            ensure!(
                method == Some("tools/call") && valid_peer(&message["id"]),
                "Invalid MCP request"
            );
            let params = message["params"]
                .as_object()
                .context("Missing tool parameters")?;
            ensure!(
                params
                    .keys()
                    .all(|k| matches!(k.as_str(), "name" | "arguments" | "_meta")),
                "Unknown MCP parameters"
            );
            let meta = params
                .get("_meta")
                .and_then(Value::as_object)
                .context("Native tool identity missing")?;
            ensure!(
                meta.keys()
                    .all(|k| matches!(k.as_str(), "claudecode/toolUseId" | "progressToken")),
                "Unknown native metadata"
            );
            ensure!(
                meta.get("progressToken").is_none_or(valid_peer),
                "Invalid progress identity"
            );
            let tool_id = meta
                .get("claudecode/toolUseId")
                .and_then(Value::as_str)
                .filter(|id| valid_string(id))
                .context("Invalid native tool identity")?;
            let mut state = self.state.lock();
            ensure!(!state.blocked, "Source turn stopped");
            let witness = state
                .witnesses
                .get(tool_id)
                .context("Unobserved main-session tool use")?;
            ensure!(
                witness.valid
                    && !witness.cancelled.load(Ordering::Acquire)
                    && params.get("name").and_then(Value::as_str) == Some(witness.name.as_str())
                    && params.get("arguments") == Some(&witness.arguments),
                "Changed or canceled native call"
            );
            let call = Call {
                peer: json!({"control":control,"mcp":message["id"]}),
                request_id: format!(
                    "claude-team-{:x}",
                    Sha256::digest(serde_json::to_vec(&json!([self.session, tool_id]))?)
                ),
                name: witness.name.clone(),
                arguments: witness.arguments.clone(),
                cancelled: witness.cancelled.clone(),
            };
            if state.pending.contains_key(control) {
                return Ok(None);
            }
            state.pending.insert(control.into(), call.cancelled.clone());
            Ok(Some(call))
        })();
        match prepared {
            Ok(Some(call)) => {
                if self.worker.submit(call).is_err() {
                    self.state.lock().pending.remove(control);
                    self.reject(control, message);
                }
            }
            Ok(None) => {}
            Err(_) => self.reject(control, message),
        }
        true
    }
    fn reject(&self, control: &str, message: &Value) {
        let _=self.commands.send(CommandMessage::TeamResult(response(control,json!({"jsonrpc":"2.0","id":message.get("id").filter(|id|valid_peer(id)).cloned().unwrap_or(Value::Null),"error":{"code":-32602,"message":"Ion team request rejected: invalid or unrelated native call, canceled source or bridge busy. No invocation is automatically retried."}}))));
    }
    pub(super) fn respond(&self, id: &str, option: &str) -> bool {
        self.worker.respond(id, option)
    }
    pub(super) fn cancel(&self) {
        let mut state = self.state.lock();
        state.blocked = true;
        for cancelled in state.pending.values() {
            cancelled.store(true, Ordering::Release);
        }
        for witness in state.witnesses.values() {
            witness.cancelled.store(true, Ordering::Release);
        }
    }
    pub(super) fn close(&self) {
        self.worker.close();
        self.cancel();
    }
}
fn valid_string(s: &str) -> bool {
    !s.is_empty() && s.len() <= 256
}
fn valid_peer(id: &Value) -> bool {
    id.as_i64().is_some() || id.as_u64().is_some() || id.as_str().is_some_and(valid_string)
}
fn response(id: &str, mcp: Value) -> Value {
    json!({"type":"control_response","response":{"subtype":"success","request_id":id,"response":{"mcp_response":mcp}}})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::{Receiver, unbounded};
    use std::time::{Duration, Instant};
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
                crate::driver::team_codex::EmptyDirectory::create("claude-source-teams").unwrap();
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
                mode: RuntimeMode::Ask,
            };
            Self {
                directory,
                context,
                team,
                revision,
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
        fn bridge(&self) -> (Bridge, Receiver<CommandMessage>, Receiver<DriverEvent>) {
            let (commands, replies) = unbounded();
            let (events, received) = crate::driver::test_event_channel();
            (
                Bridge::start(
                    self.context.clone(),
                    "native-session".into(),
                    commands,
                    events,
                )
                .unwrap(),
                replies,
                received,
            )
        }
    }
    fn observed(tool: &str, name: &str, args: Value) -> Value {
        json!({"type":"assistant","session_id":"native-session","parent_tool_use_id":null,"message":{"id":"native-message","content":[{"type":"tool_use","id":tool,"name":format!("mcp__ion_teams__{name}"),"input":args}]}})
    }
    fn call(peer: &str, rpc: Value, tool: &str, name: &str, args: Value) -> Value {
        json!({"type":"control_request","request_id":peer,"request":{"subtype":"mcp_message","server_name":SERVER,"message":{"jsonrpc":"2.0","method":"tools/call","id":rpc,"params":{"name":name,"arguments":args,"_meta":{"claudecode/toolUseId":tool,"progressToken":2}}}}})
    }
    fn result(replies: &Receiver<CommandMessage>) -> Value {
        match replies.recv_timeout(Duration::from_secs(10)).unwrap() {
            CommandMessage::TeamResult(v) => v["response"]["response"]["mcp_response"].clone(),
            _ => panic!("Native MCP result expected"),
        }
    }
    fn permission(events: &Receiver<DriverEvent>) -> String {
        loop {
            if let DriverEvent::Permission {
                request_id, detail, ..
            } = events.recv_timeout(Duration::from_secs(10)).unwrap()
            {
                assert!(detail.contains("workspaceWrite") && detail.contains("500000"));
                return request_id;
            }
        }
    }
    #[test]
    fn claude_source_team_grants_replay_and_restart_use_native_tool_identity() {
        let f = Fixture::new();
        let (bridge, replies, events) = f.bridge();
        let witness = observed("native-tool", "invoke_team", f.args());
        bridge.observe(&witness);
        assert!(bridge.dispatch(&call(
            "peer-1",
            json!(2),
            "native-tool",
            "invoke_team",
            f.args()
        )));
        let approval = permission(&events);
        assert!(f.runs().is_empty());
        bridge.respond(&approval, "allow");
        assert_eq!(result(&replies)["result"]["isError"], false);
        let run = f.runs().remove(0);
        assert!(run.work[0].access.workspace_write);
        assert_eq!(run.project_id, f.context.project_id);
        bridge.dispatch(&call(
            "peer-2",
            json!(99),
            "native-tool",
            "invoke_team",
            f.args(),
        ));
        assert_eq!(result(&replies)["result"]["isError"], false);
        assert_eq!(f.runs().len(), 1);
        bridge.close();
        let (bridge, replies, events) = f.bridge();
        bridge.observe(&witness);
        bridge.dispatch(&call(
            "new-peer",
            json!("new-rpc"),
            "native-tool",
            "invoke_team",
            f.args(),
        ));
        assert_eq!(result(&replies)["result"]["isError"], false);
        assert_eq!(f.runs()[0].id, run.id);
        assert!(
            !events
                .try_iter()
                .any(|e| matches!(e, DriverEvent::Permission { .. }))
        );
        let mut changed = f.args();
        changed["objective"] = json!("Changed work");
        bridge.observe(&observed("native-tool", "invoke_team", changed.clone()));
        bridge.dispatch(&call(
            "changed",
            json!(100),
            "native-tool",
            "invoke_team",
            changed,
        ));
        assert!(result(&replies)["error"].is_object());
        assert_eq!(f.runs().len(), 1);
        bridge.close();
    }
    #[test]
    fn claude_source_team_rejects_unobserved_foreign_subagent_and_changed_calls() {
        let f = Fixture::new();
        let (bridge, replies, _events) = f.bridge();
        let request = call("peer", json!(2), "tool", "invoke_team", f.args());
        bridge.dispatch(&request);
        assert!(result(&replies)["error"].is_object());
        for (key, value) in [
            ("session_id", json!("foreign")),
            ("parent_tool_use_id", json!("subagent")),
        ] {
            let mut witness = observed("tool", "invoke_team", f.args());
            witness[key] = value;
            bridge.observe(&witness);
            bridge.dispatch(&request);
            assert!(result(&replies)["error"].is_object());
        }
        bridge.observe(&observed("tool", "invoke_team", f.args()));
        for pointer in [
            "/request/message/params/_meta/claudecode~1toolUseId",
            "/request/message/params/name",
            "/request/message/params/arguments/objective",
        ] {
            let mut bad = request.clone();
            *bad.pointer_mut(pointer).unwrap() = json!("changed");
            bridge.dispatch(&bad);
            assert!(result(&replies)["error"].is_object());
        }
        let mut missing = request.clone();
        missing["request"]["message"]["params"]
            .as_object_mut()
            .unwrap()
            .remove("_meta");
        bridge.dispatch(&missing);
        assert!(result(&replies)["error"].is_object());
        let mut forged = f.args();
        forged["project_id"] = json!(Uuid::new_v4());
        bridge.observe(&observed("forged", "invoke_team", forged.clone()));
        bridge.dispatch(&call(
            "forged-peer",
            json!(5),
            "forged",
            "invoke_team",
            forged,
        ));
        assert_eq!(result(&replies)["result"]["isError"], true);
        assert!(f.runs().is_empty());
        bridge.close();
    }
    #[test]
    fn claude_source_team_changed_witness_cancels_pending_approval() {
        let f = Fixture::new();
        let (bridge, replies, events) = f.bridge();
        bridge.observe(&observed("tool", "invoke_team", f.args()));
        bridge.dispatch(&call("peer", json!(2), "tool", "invoke_team", f.args()));
        let approval = permission(&events);
        let mut changed = f.args();
        changed["objective"] = json!("Changed work");
        bridge.observe(&observed("tool", "invoke_team", changed));
        bridge.respond(&approval, "allow");
        assert_eq!(result(&replies)["result"]["isError"], true);
        assert!(f.runs().is_empty());
        bridge.close();
    }
    #[test]
    fn claude_source_team_native_cancel_and_stop_never_accept_late_approval() {
        let f = Fixture::new();
        let (bridge, replies, events) = f.bridge();
        bridge.observe(&observed("tool", "invoke_team", f.args()));
        bridge.dispatch(&call("peer", json!(2), "tool", "invoke_team", f.args()));
        let approval = permission(&events);
        assert!(bridge.dispatch(&json!({"type":"control_cancel_request","request_id":"peer"})));
        bridge.respond(&approval, "allow");
        assert!(replies.recv_timeout(Duration::from_millis(400)).is_err());
        assert!(f.runs().is_empty());
        bridge.observe(&observed("other-tool", "invoke_team", f.args()));
        bridge.dispatch(&call(
            "other-peer",
            json!(3),
            "other-tool",
            "invoke_team",
            f.args(),
        ));
        let approval = permission(&events);
        bridge.cancel();
        bridge.respond(&approval, "allow");
        assert_eq!(result(&replies)["result"]["isError"], true);
        assert!(f.runs().is_empty());
        bridge.allow_turn();
        bridge.observe(&observed("catalog", "list_teams", json!({})));
        bridge.dispatch(&call(
            "catalog-peer",
            json!(4),
            "catalog",
            "list_teams",
            json!({}),
        ));
        assert_eq!(result(&replies)["result"]["isError"], false);
        assert!(bridge.respond("ion-team-approval-expired", "allow"));
        assert!(!bridge.respond("native-permission", "allow"));
        bridge.close();
    }
    #[test]
    fn claude_source_team_mcp_catalog_and_configuration_have_only_five_functions() {
        let f = Fixture::new();
        let (bridge, replies, _events) = f.bridge();
        for (method, params) in [
            ("initialize", json!({"protocolVersion":"2025-11-25"})),
            ("notifications/initialized", Value::Null),
            ("tools/list", Value::Null),
        ] {
            let mut message = json!({"jsonrpc":"2.0","method":method});
            if method != "notifications/initialized" {
                message["id"] = json!(1);
            }
            if !params.is_null() {
                message["params"] = params;
            }
            bridge.dispatch(&json!({"type":"control_request","request_id":"handshake","request":{"subtype":"mcp_message","server_name":SERVER,"message":message}}));
            let result = result(&replies);
            assert!(result["error"].is_null());
            if method == "tools/list" {
                assert_eq!(result["result"]["tools"].as_array().unwrap().len(), 5);
            }
        }
        let mut command = std::process::Command::new("unused");
        configure(&mut command);
        let args: Vec<_> = command
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        let config: Value = serde_json::from_str(&args[1]).unwrap();
        assert_eq!(config["mcpServers"][SERVER]["type"], "sdk");
        assert_eq!(
            args.iter()
                .filter(|s| s.as_str() == "--allowedTools")
                .count(),
            1
        );
        assert_eq!(args.last().unwrap().split(',').count(), 5);
        assert!(f.runs().is_empty());
        bridge.close();
    }
    fn native_reply(call: &str, tool: Option<(&str, Value)>, text: &str) -> Vec<u8> {
        let is_tool = tool.is_some();
        let (block, reason) = if let Some((name, args)) = tool {
            (
                json!({"type":"tool_use","id":format!("toolu_{call}"),"name":format!("mcp__ion_teams__{name}"),"input":args}),
                "tool_use",
            )
        } else {
            (json!({"type":"text","text":text}), "end_turn")
        };
        let mut initial = block.clone();
        let delta = if is_tool {
            initial["input"] = json!({});
            json!({"type":"input_json_delta","partial_json":block["input"].to_string()})
        } else {
            initial["text"] = json!("");
            json!({"type":"text_delta","text":text})
        };
        [json!({"type":"message_start","message":{"id":format!("msg_{call}"),"type":"message","role":"assistant","model":"claude-sonnet-4-6","content":[],"stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":100,"output_tokens":0}}}),
        json!({"type":"content_block_start","index":0,"content_block":initial}),json!({"type":"content_block_delta","index":0,"delta":delta}),json!({"type":"content_block_stop","index":0}),json!({"type":"message_delta","delta":{"stop_reason":reason,"stop_sequence":null},"usage":{"output_tokens":20}}),json!({"type":"message_stop"})]
            .iter().map(|e|format!("event: {}\ndata: {e}\n\n",e["type"].as_str().unwrap())).collect::<String>().into_bytes()
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
    ) -> super::super::ClaudeDriver {
        let home = home.to_owned();
        let temporary = temporary.to_owned();
        let system_root = std::env::var_os("SystemRoot");
        let git_bash = std::env::var_os("ION_NATIVE_CLAUDE_GIT_BASH");
        super::super::ClaudeDriver::start_configured(
            crate::driver::DriverStartOptions {
                binary: native.to_owned(),
                cwd: work.to_owned(),
                mode: RuntimeMode::Ask,
                model: Some("claude-sonnet-4-6".into()),
                reasoning_effort: None,
                service_tier: None,
                context_window: None,
                agent_preset: None,
                computer_use_enabled: false,
                provider_cursor: cursor,
            },
            events,
            Some(context),
            move |command| {
                command.env_clear();
                if let Some(root) = system_root {
                    command.env("SystemRoot", root);
                }
                for name in [
                    "HOME",
                    "USERPROFILE",
                    "APPDATA",
                    "LOCALAPPDATA",
                    "CLAUDE_CONFIG_DIR",
                ] {
                    command.env(name, &home);
                }
                for name in ["TMP", "TEMP", "TMPDIR"] {
                    command.env(name, &temporary);
                }
                if let Some(bash) = git_bash {
                    command.env("CLAUDE_CODE_GIT_BASH_PATH", bash);
                }
                #[cfg(unix)]
                command.env("PATH", "/usr/bin:/bin");
                command
                    .env("ANTHROPIC_API_KEY", "sk-ant-ion-native-fixture-only")
                    .env("ANTHROPIC_BASE_URL", base)
                    .env("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1")
                    .env("CLAUDE_CODE_DISABLE_AUTO_MEMORY", "1")
                    .env("ENABLE_CLAUDEAI_MCP_SERVERS", "false")
                    .args([
                        "--tools",
                        "",
                        "--disable-slash-commands",
                        "--strict-mcp-config",
                        "--setting-sources",
                        "",
                        "--settings",
                        r#"{"disableAllHooks":true}"#,
                        "--no-chrome",
                        "--max-turns",
                        "5",
                    ]);
            },
            false,
        )
        .unwrap()
    }
    fn model_requests(
        requests: Vec<crate::driver::team_test_http::Request>,
    ) -> Vec<crate::driver::team_test_http::Request> {
        assert!(
            requests
                .iter()
                .filter(|r| r.line == "HEAD /api/hello HTTP/1.1")
                .count()
                <= 1
        );
        requests
            .into_iter()
            .filter(|r| r.line != "HEAD /api/hello HTTP/1.1")
            .collect()
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
        panic!("Private native Claude fixture did not exit");
    }
    #[test]
    #[ignore = "Requires ION_NATIVE_CLAUDE_BINARY and Git Bash on Windows; private fake account and localhost Messages only"]
    fn claude_source_team_native_sdk_start_and_resume_through_actual_ion_driver() {
        use crate::driver::DriverControl;
        let native = std::env::var_os("ION_NATIVE_CLAUDE_BINARY")
            .map(std::path::PathBuf::from)
            .expect("Set exact native Claude executable");
        let f = Fixture::new();
        let home = f.directory.0.join("home");
        let work = f.directory.0.join("work");
        let temporary = f.directory.0.join("temporary");
        for directory in [&home, &work, &temporary] {
            std::fs::create_dir(directory).unwrap();
        }
        let server = crate::driver::team_test_http::Server::with_optional_preflight(
            vec![
                (
                    200,
                    native_reply("list", Some(("list_teams", json!({}))), ""),
                ),
                (
                    200,
                    native_reply("invoke", Some(("invoke_team", f.args())), ""),
                ),
                (
                    200,
                    native_reply("runs", Some(("get_team_runs", json!({}))), ""),
                ),
                (
                    200,
                    native_reply(
                        "done",
                        None,
                        "Team queued; inspect its actual run for completion.",
                    ),
                ),
            ],
            |request, body| {
                assert!(
                    request.line.starts_with("POST /v1/messages"),
                    "Unexpected native request: {}",
                    request.line
                );
                assert_eq!(request.body["model"], "claude-sonnet-4-6");
                assert!(request.headers.contains("sk-ant-ion-native-fixture-only"));
                let tools = request.body["tools"].as_array().unwrap();
                assert!(
                    tools
                        .iter()
                        .any(|t| t["name"] == "mcp__ion_teams__invoke_team")
                );
                assert!(!tools.iter().any(|t| matches!(
                    t["name"].as_str(),
                    Some("Bash" | "Read" | "Write" | "Task" | "Agent")
                )));
                body
            },
            "HEAD /api/hello HTTP/1.1",
        );
        let (events, received) = crate::driver::test_event_channel();
        let driver = native_driver(
            &native,
            &home,
            &work,
            &temporary,
            f.context.clone(),
            server.url.clone(),
            events,
            None,
        );
        let deadline = Instant::now() + Duration::from_secs(25);
        let mut prompted = false;
        let mut approved = false;
        let mut finished = false;
        let mut cursor = None;
        let mut text = String::new();
        while Instant::now() < deadline {
            let Ok(event) = received.recv_timeout(Duration::from_millis(200)) else {
                continue;
            };
            match event {
                DriverEvent::Connected { provider_cursor } if !prompted => {
                    cursor = provider_cursor;
                    prompted = true;
                    driver.prompt(
                        "Use my saved engineering team and report its actual queue state.".into(),
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
                    assert!(success);
                    finished = true;
                    break;
                }
                DriverEvent::Error(error) => panic!("Native Claude fixture failed: {error}"),
                _ => {}
            }
        }
        drop(driver);
        wait_native_exit(&received);
        assert!(
            prompted && approved && finished,
            "Native Claude team fixture did not finish"
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
                .starts_with("claude-team-")
        );
        let requests = model_requests(server.finish());
        assert_eq!(requests.len(), 4);
        assert!(
            requests[3].body["messages"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|m| m["content"].as_array().into_iter().flatten())
                .any(|item| item["type"] == "tool_result"
                    && item["tool_use_id"] == "toolu_invoke"
                    && item["content"]
                        .to_string()
                        .contains(&runs[0].id.to_string()))
        );
        let resumed_server = crate::driver::team_test_http::Server::with_optional_preflight(
            vec![
                (
                    200,
                    native_reply("resumed-runs", Some(("get_team_runs", json!({}))), ""),
                ),
                (
                    200,
                    native_reply(
                        "resumed-done",
                        None,
                        "Existing team inspected after reopening.",
                    ),
                ),
            ],
            |request, body| {
                assert!(request.headers.contains("sk-ant-ion-native-fixture-only"));
                assert!(
                    request.body["tools"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|t| t["name"] == "mcp__ion_teams__get_team_runs")
                );
                body
            },
            "HEAD /api/hello HTTP/1.1",
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
            cursor,
        );
        let deadline = Instant::now() + Duration::from_secs(25);
        let mut prompted = false;
        let mut finished = false;
        while Instant::now() < deadline {
            let Ok(event) = received.recv_timeout(Duration::from_millis(200)) else {
                continue;
            };
            match event {
                DriverEvent::Connected { .. } if !prompted => {
                    prompted = true;
                    driver.prompt(
                        "Inspect my previously invoked team without creating a new run.".into(),
                    );
                }
                DriverEvent::TurnFinished { success, .. } => {
                    assert!(success);
                    finished = true;
                    break;
                }
                DriverEvent::Permission { .. } => {
                    panic!("Run readback must not require another grant")
                }
                DriverEvent::Error(error) => panic!("Native Claude resume fixture failed: {error}"),
                _ => {}
            }
        }
        drop(driver);
        wait_native_exit(&received);
        assert!(finished, "Native Claude resume did not finish");
        assert_eq!(f.runs().len(), 1);
        let requests = model_requests(resumed_server.finish());
        assert_eq!(requests.len(), 2);
        assert!(
            requests[1].body["messages"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|m| m["content"].as_array().into_iter().flatten())
                .any(|item| item["type"] == "tool_result"
                    && item["tool_use_id"] == "toolu_resumed-runs"
                    && item["content"]
                        .to_string()
                        .contains(&runs[0].id.to_string()))
        );

        // Closing a source while Ion's owner approval is outstanding must reap
        // the actual CLI and cannot turn the outstanding grant into a new run.
        let stopped_server = crate::driver::team_test_http::Server::with_optional_preflight(
            vec![(
                200,
                native_reply("stopped", Some(("invoke_team", f.args())), ""),
            )],
            |request, body| {
                assert!(request.line.starts_with("POST /v1/messages"));
                assert!(request.headers.contains("sk-ant-ion-native-fixture-only"));
                body
            },
            "HEAD /api/hello HTTP/1.1",
        );
        let (events, received) = crate::driver::test_event_channel();
        let driver = native_driver(
            &native,
            &home,
            &work,
            &temporary,
            f.context.clone(),
            stopped_server.url.clone(),
            events,
            None,
        );
        let deadline = Instant::now() + Duration::from_secs(25);
        let mut approval = None;
        while Instant::now() < deadline {
            match received.recv_timeout(Duration::from_millis(100)) {
                Ok(DriverEvent::Connected { .. }) => {
                    driver.prompt("Use my saved team; ask before its lead can edit.".into())
                }
                Ok(DriverEvent::Permission { request_id, .. }) => {
                    approval = Some(request_id);
                    break;
                }
                Ok(DriverEvent::Error(error)) => {
                    panic!("Native Claude pending approval failed: {error}")
                }
                _ => {}
            }
        }
        assert!(
            approval
                .as_deref()
                .is_some_and(|id| id.starts_with("ion-team-approval-"))
        );
        assert_eq!(f.runs().len(), 1);
        drop(driver);
        wait_native_exit(&received);
        assert_eq!(f.runs().len(), 1);
        assert_eq!(model_requests(stopped_server.finish()).len(), 1);
    }
}
