//! Grok's native file-hook rewrite is confirmed independently over the owned
//! ACP connection before its MCP callback can reach the shared Teams worker.
use super::{
    WorkspaceContext,
    amp::ProjectPlugin,
    opencode::{Bridge, read_native_request},
};
use crate::driver::DriverEventSender;
use agent_client_protocol::schema::v1::{HttpHeader, McpServer, McpServerHttp};
use anyhow::{Context, Result, ensure};
use crossbeam_channel::{Sender, bounded};
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    io::Write,
    net::{TcpListener, TcpStream},
    path::Path,
    sync::{
        Arc, OnceLock, Weak,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

const FIELD: &str = "__ion_native_binding";
const MAX_CALLS: usize = 2048;
const MAX_ARGUMENT_BYTES: usize = 1024 * 1024;
const DENIED: &str = "Ion Teams request was not authorized by the active source task. Check Grok project hook trust and retry from the task; no invocation is automatically retried.";
#[derive(Clone)]
pub(crate) struct GrokBridge(Arc<Inner>);
struct Inner {
    context: WorkspaceContext,
    events: DriverEventSender,
    server_name: String,
    callback: String,
    token: String,
    port: u16,
    state: Mutex<State>,
    closed: AtomicBool,
    _hook: ProjectPlugin,
}
#[derive(Default)]
struct State {
    session: String,
    bridge: Option<Bridge>,
    running: bool,
    next_turn: u64,
    cancelled_through: u64,
    active_turn: Option<u64>,
    argument_bytes: usize,
    calls: HashMap<String, NativeCall>,
    nonces: HashMap<String, String>,
}
struct NativeCall {
    name: String,
    arguments: Value,
    nonce: String,
    expires: Instant,
    valid: bool,
    confirmed: bool,
    consumed: bool,
}
fn trace_refusal(_stage: &str, _error: &anyhow::Error) {
    // Diagnostics for the isolated fixture contain only fixed validation labels.
    // No native arguments, IDs, account data or private credentials are printed.
    #[cfg(test)]
    if std::env::var_os("ION_TEST_GROK_BINARY").is_some() {
        eprintln!("Native Grok {_stage} refusal: {_error}");
    }
}
fn valid_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}
fn nonce() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}
impl GrokBridge {
    pub(crate) fn start(
        context: WorkspaceContext,
        events: DriverEventSender,
        cwd: &Path,
        helper: &Path,
    ) -> Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        let server_name = format!("ion_teams_{}", context.task_id.simple());
        let matcher = format!(
            "^{server_name}__(list_teams|invoke_team|get_team_runs|await_team|read_team_document)$"
        );
        let source=json!({"hooks":{"PreToolUse":[{"matcher":matcher,"hooks":[{"type":"command","command":hook_command(helper)?,"timeout":6}]}]}}).to_string();
        let hook = ProjectPlugin::install_at(
            cwd,
            ".grok",
            &format!("ion-teams-{}.json", context.task_id.simple()),
            &source,
        )?;
        let bridge = Self(Arc::new(Inner {
            callback: format!("ion-teams-{}", context.task_id.simple()),
            context,
            events,
            server_name,
            token: nonce(),
            port: listener.local_addr()?.port(),
            state: Mutex::new(State::default()),
            closed: AtomicBool::new(false),
            _hook: hook,
        }));
        let pool = connections()?;
        let weak = Arc::downgrade(&bridge.0);
        thread::Builder::new()
            .name("ion-grok-team-listener".into())
            .spawn(move || {
                loop {
                    let Some(inner) = weak.upgrade() else { break };
                    if inner.closed.load(Ordering::Acquire) {
                        break;
                    }
                    drop(inner);
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let _ = pool.try_send((stream, weak.clone()));
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(25))
                        }
                        Err(_) => break,
                    }
                }
            })?;
        Ok(bridge)
    }
    pub(crate) fn environment(&self) -> Vec<(String, String)> {
        vec![
            (
                "ION_SOURCE_TEAM_HOOK_ENDPOINT".into(),
                format!("http://127.0.0.1:{}/hook", self.0.port),
            ),
            ("ION_SOURCE_TEAM_HOOK_TOKEN".into(), self.0.token.clone()),
        ]
    }
    pub(crate) fn mcp_servers(&self) -> Vec<McpServer> {
        if self.0.closed.load(Ordering::Acquire) {
            return Vec::new();
        }
        vec![McpServer::Http(
            McpServerHttp::new(
                &self.0.server_name,
                format!("http://127.0.0.1:{}/mcp", self.0.port),
            )
            .headers(vec![HttpHeader::new(
                "Authorization",
                format!("Bearer {}", self.0.token),
            )]),
        )]
    }
    pub(crate) fn metadata(&self) -> serde_json::Map<String, Value> {
        if self.0.closed.load(Ordering::Acquire) {
            return serde_json::Map::new();
        }
        json!({"x.ai/hooks":{"PreToolUse":[{"matcher":format!("^{}__",self.0.server_name),"hookCallbackIds":[self.0.callback],"timeout":5}]},"mcpConfig":{self.0.server_name.clone():{"startupTimeoutMs":10000,"toolTimeoutMs":335000}}}).as_object().unwrap().clone()
    }
    pub(crate) fn bind(&self, session: &str) -> Result<()> {
        ensure!(
            valid_id(session) && !self.0.closed.load(Ordering::Acquire),
            "Invalid or closed Grok session"
        );
        let mut state = self.0.state.lock();
        ensure!(
            state.session.is_empty() || state.session == session,
            "Grok session identity changed"
        );
        if state.bridge.is_none() {
            let bridge = Bridge::start(
                self.0.context.clone(),
                format!("grok:{session}"),
                self.0.events.clone(),
            )?;
            bridge.cancel();
            state.bridge = Some(bridge);
            state.session = session.into();
        }
        Ok(())
    }
    pub(crate) fn queue_turn(&self) -> u64 {
        let mut state = self.0.state.lock();
        state.next_turn = state.next_turn.saturating_add(1);
        state.next_turn
    }
    pub(crate) fn activate(&self, turn: u64) -> bool {
        let mut state = self.0.state.lock();
        if self.0.closed.load(Ordering::Acquire)
            || state.bridge.is_none()
            || turn <= state.cancelled_through
        {
            return false;
        }
        invalidate(&mut state);
        state.active_turn = Some(turn);
        state.running = true;
        state.bridge.as_ref().unwrap().allow_turn();
        true
    }
    pub(crate) fn active_turn(&self) -> Option<u64> {
        self.0.state.lock().active_turn
    }
    pub(crate) fn finish_turn(&self, turn: Option<u64>) {
        let mut state = self.0.state.lock();
        if turn.is_some() && state.active_turn == turn {
            invalidate(&mut state);
        }
    }
    pub(crate) fn cancel(&self) {
        let mut state = self.0.state.lock();
        state.cancelled_through = state.next_turn;
        invalidate(&mut state);
    }
    pub(crate) fn close(&self) {
        self.0.closed.store(true, Ordering::Release);
        self.cancel();
        if let Some(bridge) = &self.0.state.lock().bridge {
            bridge.close();
        }
    }
    pub(crate) fn respond(&self, id: &str, option: &str) -> bool {
        self.0
            .state
            .lock()
            .bridge
            .as_ref()
            .is_some_and(|bridge| bridge.respond(id, option))
    }
    /// This runs only for an actual reverse request from the owned ACP reader.
    pub(crate) fn witness(&self, event: &Value) -> Value {
        self.witness_inner(event).unwrap_or_else(|error| {
            trace_refusal("ACP witness", &error);
            json!({"decision":"deny","systemMessage":DENIED})
        })
    }
    fn witness_inner(&self, event: &Value) -> Result<Value> {
        ensure!(
            event["hookCallbackId"] == self.0.callback,
            "Unrelated native hook callback"
        );
        let (call_id, name, arguments, _) = self.event(event, true)?;
        let nonce = arguments[FIELD].as_str().context("Unbound native call")?;
        let mut canonical = arguments.clone();
        canonical.as_object_mut().unwrap().remove(FIELD);
        let mut state = self.0.state.lock();
        ensure!(
            state.running && !self.0.closed.load(Ordering::Acquire),
            "Source task stopped"
        );
        let call = state
            .calls
            .get_mut(&call_id)
            .context("Unobserved native command hook")?;
        if !call.valid
            || call.consumed
            || call.expires < Instant::now()
            || call.name != name
            || call.nonce != nonce
            || call.arguments != canonical
        {
            call.valid = false;
            if let Some(bridge) = &state.bridge {
                bridge.fail_native(&call_id);
            }
            anyhow::bail!("Changed or expired native call");
        }
        call.confirmed = true;
        state
            .bridge
            .as_ref()
            .context("Native session is not bound")?
            .witness_native(&call_id, &name, canonical);
        Ok(json!({"decision":"continue"}))
    }
    /// Validate native identity independently of model arguments. Context and
    /// project ownership are fixed by the daemon, never supplied in this event.
    fn event(&self, event: &Value, bound: bool) -> Result<(String, String, Value, Option<Value>)> {
        ensure!(
            event["hookEventName"] == "pre_tool_use"
                && event["toolInputTruncated"] == false
                && event.get("subagentType").is_none_or(Value::is_null),
            "Unsupported native hook event"
        );
        if let Some(alias) = event.get("hook_event_name") {
            ensure!(alias == "PreToolUse", "Conflicting native hook event");
        }
        let state = self.0.state.lock();
        ensure!(
            state.running
                && !state.session.is_empty()
                && event["sessionId"].as_str() == Some(&state.session),
            "Unrelated or stopped source session"
        );
        drop(state);
        let call = event["toolUseId"]
            .as_str()
            .filter(|id| valid_id(id))
            .context("Invalid native call ID")?;
        let tool = event["toolName"]
            .as_str()
            .context("Missing native tool name")?;
        let name = tool
            .strip_prefix(&format!("{}__", self.0.server_name))
            .filter(|name| super::accepts(name))
            .context("Unrelated native tool")?;
        let input = &event["toolInput"];
        let (args, wrapper) =
            if input.get("tool_name").is_some() || input.get("tool_input").is_some() {
                ensure!(
                    input.as_object().is_some_and(|object| object.len() == 2)
                        && input["tool_name"] == tool,
                    "Invalid native MCP wrapper"
                );
                (input["tool_input"].clone(), Some(input.clone()))
            } else {
                (input.clone(), None)
            };
        ensure!(
            args.is_object()
                && serde_json::to_vec(&args)?.len() <= 40 * 1024
                && (bound || args.get(FIELD).is_none()),
            "Invalid or model-supplied native binding"
        );
        Ok((call.into(), name.into(), args, wrapper))
    }
    fn hook(&self, event: &Value) -> Value {
        self.hook_inner(event).unwrap_or_else(|error| {
            trace_refusal("command hook", &error);
            json!({"decision":"deny","reason":DENIED})
        })
    }
    fn hook_inner(&self, event: &Value) -> Result<Value> {
        let (call_id, name, arguments, wrapper) = self.event(event, false)?;
        let deadline = Instant::now() + Duration::from_secs(4);
        let binding = loop {
            let mut state = self.0.state.lock();
            ensure!(
                state.running && !self.0.closed.load(Ordering::Acquire),
                "Source task stopped"
            );
            if let Some(previous) = state.calls.get_mut(&call_id) {
                if previous.arguments != arguments || previous.name != name {
                    previous.valid = false;
                }
                let authorized =
                    previous.valid && !previous.consumed && previous.expires >= Instant::now();
                let binding = previous.nonce.clone();
                if !authorized {
                    previous.valid = false;
                    if let Some(bridge) = &state.bridge {
                        bridge.fail_native(&call_id);
                    }
                    anyhow::bail!("Native call reused or changed");
                }
                break binding;
            }
            drop(state);
            ensure!(
                Instant::now() < deadline,
                "Native call has not been observed by the owner"
            );
            thread::sleep(Duration::from_millis(10));
        };
        let mut rewritten = arguments;
        rewritten[FIELD] = json!(binding);
        let rewritten = if let Some(mut wrapper) = wrapper {
            wrapper["tool_input"] = rewritten;
            wrapper
        } else {
            rewritten
        };
        Ok(json!({"hookSpecificOutput":{"hookEventName":"PreToolUse","updatedInput":rewritten}}))
    }
    fn mcp(&self, request: Value) -> Value {
        let id = request["id"].clone();
        let method = request["method"].as_str().unwrap_or_default();
        if request["jsonrpc"] != "2.0" || !request.is_object() {
            return json!({"jsonrpc":"2.0","id":id,"error":{"code":-32600,"message":"Invalid request"}});
        }
        if method.starts_with("notifications/") {
            return Value::Null;
        }
        let result = match method {
            "initialize" => {
                json!({"protocolVersion":request.pointer("/params/protocolVersion").and_then(Value::as_str).unwrap_or("2025-03-26"),"capabilities":{"tools":{}},"serverInfo":{"name":"ion-teams","version":"1"}})
            }
            "ping" => json!({}),
            "tools/list" => json!({"tools":super::tools().as_array().unwrap().iter().map(|tool| {
                let function=&tool["function"];
                let mut schema=function["parameters"].clone();
                schema["properties"][FIELD]=json!({"type":"string","description":"Native owner binding; leave unset."});
                json!({"name":function["name"],"description":function["description"],"inputSchema":schema})
            }).collect::<Vec<_>>()}),
            "tools/call" => match self.dispatch(&request["params"]) {
                Ok(value) => {
                    json!({"content":[{"type":"text","text":value["text"].as_str().unwrap_or(DENIED)}],"isError":value["ok"]!=true})
                }
                Err(_) => json!({"content":[{"type":"text","text":DENIED}],"isError":true}),
            },
            _ => {
                return json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"Unknown method"}});
            }
        };
        json!({"jsonrpc":"2.0","id":id,"result":result})
    }
    fn dispatch(&self, params: &Value) -> Result<Value> {
        let name = params["name"]
            .as_str()
            .filter(|name| super::accepts(name))
            .context("Unrelated MCP tool")?;
        let mut arguments = params["arguments"].clone();
        let nonce = arguments
            .as_object_mut()
            .context("Invalid MCP arguments")?
            .remove(FIELD)
            .and_then(|v| v.as_str().map(str::to_owned))
            .context("Missing native binding")?;
        let (bridge, call_id) = {
            let mut state = self.0.state.lock();
            ensure!(
                state.running && !self.0.closed.load(Ordering::Acquire),
                "Stopped source task"
            );
            let call_id = state
                .nonces
                .get(&nonce)
                .context("Unknown native binding")?
                .clone();
            let call = state
                .calls
                .get_mut(&call_id)
                .context("Unknown native call")?;
            if call.name != name || call.arguments != arguments {
                call.valid = false;
                if let Some(bridge) = &state.bridge {
                    bridge.fail_native(&call_id);
                }
                anyhow::bail!("Native callback changed arguments");
            }
            ensure!(
                call.valid && call.confirmed && !call.consumed && call.expires >= Instant::now(),
                "Unwitnessed or replayed native callback"
            );
            call.consumed = true;
            (
                state.bridge.clone().context("Unbound native session")?,
                call_id,
            )
        };
        bridge.invoke_native(&call_id, name, arguments)
    }
    pub(crate) fn observe(&self, notification: &Value) {
        let mut state = self.0.state.lock();
        if notification["sessionId"].as_str() != Some(&state.session) {
            return;
        }
        let update = &notification["update"];
        let Some(call_id) = update["toolCallId"].as_str().filter(|id| valid_id(id)) else {
            return;
        };
        if update["sessionUpdate"] == "tool_call"
            && !state.calls.contains_key(call_id)
            && state.calls.len() < MAX_CALLS
        {
            let raw = &update["rawInput"];
            let target = raw["tool_name"]
                .as_str()
                .or_else(|| update["title"].as_str());
            let Some(name) = target
                .and_then(|target| target.strip_prefix(&format!("{}__", self.0.server_name)))
                .filter(|name| super::accepts(name))
            else {
                return;
            };
            let mut arguments = raw.get("tool_input").unwrap_or(raw).clone();
            if !arguments.is_object() || arguments.get(FIELD).is_some() {
                return;
            }
            let Ok(bytes) = serde_json::to_vec(&arguments) else {
                return;
            };
            if bytes.len() > 40 * 1024 {
                return;
            }
            let binding = nonce();
            let valid = state.running
                && state.active_turn.is_some()
                && !self.0.closed.load(Ordering::Acquire)
                && state.argument_bytes.saturating_add(bytes.len()) <= MAX_ARGUMENT_BYTES;
            if valid {
                state.argument_bytes += bytes.len();
            } else {
                // Keep a bounded identity tombstone so a rejected call cannot
                // acquire fresh authority after Stop or an argument-budget reset.
                arguments = Value::Null;
            }
            state.nonces.insert(binding.clone(), call_id.into());
            state.calls.insert(
                call_id.into(),
                NativeCall {
                    name: name.into(),
                    arguments,
                    nonce: binding,
                    expires: Instant::now() + Duration::from_secs(30),
                    valid,
                    confirmed: false,
                    consumed: false,
                },
            );
        }
        if update["status"] == "failed" {
            if let Some(call) = state.calls.get_mut(call_id) {
                call.valid = false;
            }
            if let Some(bridge) = &state.bridge {
                bridge.fail_native(call_id);
            }
        }
    }
    pub(crate) fn scrub(&self, value: &mut Value) {
        let state = self.0.state.lock();
        scrub(value, &state.nonces);
    }
}
fn invalidate(state: &mut State) {
    state.running = false;
    state.active_turn = None;
    state.argument_bytes = 0;
    for call in state.calls.values_mut() {
        call.valid = false;
        call.arguments = Value::Null;
    }
    if let Some(bridge) = &state.bridge {
        bridge.cancel();
    }
}
impl Drop for Inner {
    fn drop(&mut self) {
        self.closed.store(true, Ordering::Release);
        if let Some(bridge) = &self.state.get_mut().bridge {
            bridge.close();
        }
    }
}
fn scrub(value: &mut Value, nonces: &HashMap<String, String>) {
    match value {
        Value::Object(object) => {
            object.remove(FIELD);
            for value in object.values_mut() {
                scrub(value, nonces);
            }
        }
        Value::Array(values) => {
            for value in values {
                scrub(value, nonces);
            }
        }
        Value::String(text) => {
            if text.contains(FIELD) {
                *text = text.replace(FIELD, "");
            }
            // Scan fixed-length hex bindings once, without searching once per
            // remembered native call. Native notifications run off the UI thread.
            if text.len() >= 64 && !nonces.is_empty() {
                let bytes = text.as_bytes();
                let mut result = String::new();
                let (mut at, mut start) = (0, 0);
                while at + 64 <= bytes.len() {
                    if bytes[at..at + 64]
                        .iter()
                        .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
                        && nonces.contains_key(&text[at..at + 64])
                    {
                        result.push_str(&text[start..at]);
                        at += 64;
                        start = at;
                    } else {
                        at += 1;
                    }
                }
                if start > 0 {
                    result.push_str(&text[start..]);
                    *text = result;
                }
            }
        }
        _ => {}
    }
}
fn hook_command(helper: &Path) -> Result<String> {
    let path = helper.to_str().context("Native helper path is not UTF-8")?;
    ensure!(
        !path.chars().any(char::is_control) && !path.contains(['$', '`']),
        "Native helper path needs a shell contract check"
    );
    #[cfg(windows)]
    {
        let shell = crate::command_env::shell_environment()
            .into_iter()
            .find(|(key, _)| key.to_string_lossy().eq_ignore_ascii_case("GROK_SHELL"))
            .map(|(_, v)| v.to_string_lossy().to_ascii_lowercase());
        match shell.as_deref() {
            Some("cmd" | "cmd.exe") => {
                ensure!(
                    !path.contains(['%', '!', '^', '&', '|', '<', '>', '(', ')', '"']),
                    "Native helper path needs a cmd quoting check"
                );
                Ok(format!("\"{path}\" --source-team-hook"))
            }
            Some("bash" | "gitbash" | "git-bash") => Ok(format!(
                "'{}' --source-team-hook",
                path.replace('\\', "/").replace('\'', "'\\''")
            )),
            _ => Ok(format!(
                "& '{}' --source-team-hook",
                path.replace('\'', "''")
            )),
        }
    }
    #[cfg(not(windows))]
    {
        Ok(format!(
            "'{}' --source-team-hook",
            path.replace('\'', "'\\''")
        ))
    }
}
type Connection = (TcpStream, Weak<Inner>);
fn connections() -> Result<Sender<Connection>> {
    static POOL: OnceLock<Mutex<Option<Sender<Connection>>>> = OnceLock::new();
    let mut pool = POOL.get_or_init(Default::default).lock();
    if let Some(sender) = &*pool {
        return Ok(sender.clone());
    }
    let (sender, received) = bounded::<Connection>(16);
    for index in 0..4 {
        let received = received.clone();
        thread::Builder::new()
            .name(format!("ion-grok-team-http-{index}"))
            .spawn(move || {
                while let Ok((mut stream, weak)) = received.recv() {
                    let reply = (|| -> Result<(u16, Value)> {
                        let inner = weak.upgrade().context("Source bridge closed")?;
                        ensure!(
                            !inner.closed.load(Ordering::Acquire),
                            "Source bridge closed"
                        );
                        let (method, path, body) =
                            read_native_request(&mut stream, inner.port, &inner.token)?;
                        let bridge = GrokBridge(inner);
                        if method == "GET" {
                            return Ok((405, Value::Null));
                        }
                        match path.as_str() {
                            "/hook" => Ok((200, bridge.hook(&body))),
                            "/mcp" => {
                                let result = bridge.mcp(body);
                                Ok((if result.is_null() { 202 } else { 200 }, result))
                            }
                            _ => anyhow::bail!("Unknown native route"),
                        }
                    })()
                    .unwrap_or_else(|_| (403, json!({"decision":"deny","reason":DENIED})));
                    let _ = write_reply(&mut stream, reply.0, &reply.1);
                }
            })?;
    }
    *pool = Some(sender.clone());
    Ok(sender)
}
fn write_reply(stream: &mut TcpStream, status: u16, value: &Value) -> Result<()> {
    let body = if value.is_null() {
        Vec::new()
    } else {
        serde_json::to_vec(value)?
    };
    ensure!(
        body.len() <= 512 * 1024,
        "Native result exceeds its byte limit"
    );
    write!(
        stream,
        "HTTP/1.1 {status} Native\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(&body)?;
    Ok(())
}

pub(crate) fn verify(binary: &Path) -> Result<()> {
    let mut command = crate::command_env::command(binary);
    command.arg("--version");
    let output = crate::team_process::bounded_process(
        &mut command,
        b"",
        &AtomicBool::new(false),
        Duration::from_secs(15),
    )?;
    ensure!(
        std::str::from_utf8(&output)?
            .trim()
            .split_whitespace()
            .take(2)
            .collect::<Vec<_>>()
            == ["grok", "1.0.46"],
        "Grok source Teams requires the verified native 1.0.46 runtime"
    );
    Ok(())
}

#[cfg(test)]
#[path = "grok_tests.rs"]
mod tests;
