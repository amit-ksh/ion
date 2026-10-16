//! Explicitly selected Kimi source host. Native ACP observations and native
//! executable callbacks must agree before the shared worker receives authority.
use super::{
    WorkspaceContext,
    opencode::{Bridge, read_native_request},
};
use crate::driver::DriverEventSender;
use agent_client_protocol::{AcpAgentConfig, schema::v1::McpServer};
use anyhow::{Context, Result, ensure};
use crossbeam_channel::{Sender, bounded};
use parking_lot::Mutex;
use serde::Deserialize;
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
const HOST: &str =
    "ion-kimi-host/0.1.0 kimi/2.2.0 source/472192c34c1efad89a369870eff0da0b708ce9bb protocol/1";
const COMPILED_HOST: &str = "ion-kimi-host/0.2.0 kimi/2.2.0 source/472192c34c1efad89a369870eff0da0b708ce9bb protocol/1 compiled/1";
const PREFIX: &str = "ion_teams_";
const DENIED: &str = "Ion Teams could not verify this active native Kimi call. Retry from the task; no invocation is automatically retried.";
#[derive(Clone)]
pub(crate) struct KimiBridge(Arc<Inner>);
struct Inner {
    context: WorkspaceContext,
    events: DriverEventSender,
    token: String,
    port: u16,
    closed: AtomicBool,
    state: Mutex<State>,
}
#[derive(Default)]
struct State {
    session: String,
    native: Option<NativeContext>,
    bridge: Option<Bridge>,
    next_turn: u64,
    cancelled_through: u64,
    active_turn: Option<u64>,
    native_turn: Option<u64>,
    last_native_turn: Option<u64>,
    argument_bytes: usize,
    calls: HashMap<String, NativeCall>,
}
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct NativeContext {
    session_id: String,
    agent_id: String,
    generation: u64,
}
struct NativeCall {
    name: String,
    arguments: Value,
    valid: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Callback {
    session_id: String,
    agent_id: String,
    generation: u64,
    turn_id: u64,
    call_id: String,
    name: String,
    arguments: Value,
}
fn valid_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}
impl KimiBridge {
    pub(crate) fn start(
        context: WorkspaceContext,
        events: DriverEventSender,
        cwd: &Path,
        config: &AcpAgentConfig,
    ) -> Result<Self> {
        // Probe the final configured command/environment, not an inferred binary
        // name. Stock Kimi is never silently replaced by a different engine.
        let mut args = config.arguments().to_owned();
        ensure!(
            args.pop().as_deref() == Some("acp"),
            "Unsupported Kimi launch"
        );
        args.push("--ion-host-version".into());
        let mut command = crate::command_env::command(config.command());
        command
            .env_clear()
            .envs(config.environment())
            .args(args)
            .current_dir(cwd);
        let output = crate::team_process::bounded_process(
            &mut command,
            b"",
            &AtomicBool::new(false),
            Duration::from_secs(15),
        )?;
        ensure!(
            [HOST, COMPILED_HOST].contains(&std::str::from_utf8(&output)?.trim()),
            "Select the separately installed Ion Kimi host to enable Teams"
        );
        Self::listen(context, events)
    }
    fn listen(context: WorkspaceContext, events: DriverEventSender) -> Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        let bridge = Self(Arc::new(Inner {
            context,
            events,
            token: format!(
                "{}{}",
                uuid::Uuid::new_v4().simple(),
                uuid::Uuid::new_v4().simple()
            ),
            port: listener.local_addr()?.port(),
            closed: AtomicBool::new(false),
            state: Mutex::new(State::default()),
        }));
        let pool = connections()?;
        let weak = Arc::downgrade(&bridge.0);
        thread::Builder::new()
            .name("ion-kimi-team-listener".into())
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
                "ION_KIMI_TEAM_ENDPOINT".into(),
                format!("http://127.0.0.1:{}", self.0.port),
            ),
            ("ION_KIMI_TEAM_TOKEN".into(), self.0.token.clone()),
        ]
    }
    pub(crate) fn mcp_servers(&self) -> Vec<McpServer> {
        Vec::new()
    }
    pub(crate) fn bind(&self, session: &str) -> Result<()> {
        let mut state = self.0.state.lock();
        ensure!(
            !self.0.closed.load(Ordering::Acquire) && valid_id(session),
            "Invalid Kimi source"
        );
        ensure!(
            state.session.is_empty() || state.session == session,
            "Kimi session changed"
        );
        ensure!(
            state
                .native
                .as_ref()
                .is_none_or(|n| n.session_id == session),
            "Native context belongs to another session"
        );
        if state.bridge.is_none() {
            let worker = Bridge::start(
                self.0.context.clone(),
                format!("kimi:{session}"),
                self.0.events.clone(),
            )?;
            worker.cancel();
            state.bridge = Some(worker);
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
        if let Some(worker) = &self.0.state.lock().bridge {
            worker.close();
        }
    }
    pub(crate) fn respond(&self, id: &str, option: &str) -> bool {
        self.0
            .state
            .lock()
            .bridge
            .as_ref()
            .is_some_and(|b| b.respond(id, option))
    }
    pub(crate) fn scrub(&self, value: &mut Value) {
        match value {
            Value::String(text) => {
                *text = text.replace(&self.0.token, "[private Ion bridge]");
            }
            Value::Array(a) => {
                for v in a {
                    self.scrub(v);
                }
            }
            Value::Object(o) => {
                for v in o.values_mut() {
                    self.scrub(v);
                }
            }
            _ => {}
        }
    }
    pub(crate) fn observe(&self, event: &Value) {
        let mut state = self.0.state.lock();
        if event["sessionId"].as_str() != Some(&state.session) || state.session.is_empty() {
            return;
        }
        let update = &event["update"];
        if !matches!(
            update["sessionUpdate"].as_str(),
            Some("tool_call" | "tool_call_update")
        ) {
            return;
        }
        let Some(id) = update["toolCallId"].as_str().filter(|id| valid_id(id)) else {
            return;
        };
        if update["status"] == "failed" {
            if state.calls.contains_key(id) {
                revoke(&mut state, id);
            }
            return;
        }
        // Lazy pending/delta/progress/history notifications are not executable
        // authority. Only canonical native Started includes all three fields.
        if update["status"] != "in_progress" || update.get("rawInput").is_none() {
            return;
        }
        let name = update["title"]
            .as_str()
            .and_then(|n| n.strip_prefix(PREFIX))
            .filter(|n| super::accepts(n));
        let Some(name) = name else {
            if state.calls.contains_key(id) {
                revoke(&mut state, id);
            }
            return;
        };
        let args = &update["rawInput"];
        if let Some(existing) = state.calls.get(id) {
            if existing.name != name || &existing.arguments != args {
                revoke(&mut state, id);
            }
            return;
        }
        let Some((turn, call)) = id.split_once(':') else {
            return;
        };
        let Ok(turn_id) = turn.parse::<u64>() else {
            return;
        };
        if turn_id.to_string() != turn || !valid_id(call) || state.calls.len() >= 2048 {
            return;
        }
        let size = serde_json::to_vec(args).map_or(usize::MAX, |v| v.len());
        let valid = !self.0.closed.load(Ordering::Acquire)
            && state.active_turn.is_some()
            && args.is_object()
            && size <= 40 * 1024
            && state.argument_bytes.saturating_add(size) <= 1024 * 1024
            && state.native_turn.map_or_else(
                || state.last_native_turn.is_none_or(|last| turn_id > last),
                |current| current <= turn_id,
            );
        if valid {
            state.native_turn = Some(turn_id);
            state.argument_bytes += size;
            if let Some(worker) = &state.bridge {
                worker.witness_native(id, name, args.clone());
            }
        }
        state.calls.insert(
            id.into(),
            NativeCall {
                name: name.into(),
                arguments: if valid { args.clone() } else { Value::Null },
                valid,
            },
        );
    }
    fn dispatch(&self, route: &str, body: Value) -> Result<Value> {
        ensure!(!self.0.closed.load(Ordering::Acquire), "Kimi source closed");
        if route == "/tools" {
            ensure!(body == json!({}), "No arguments accepted");
            return Ok(super::tools());
        }
        if route == "/context" {
            let native: NativeContext = serde_json::from_value(body)?;
            ensure!(
                native.agent_id == "main" && valid_id(&native.session_id),
                "Invalid native context"
            );
            let mut state = self.0.state.lock();
            ensure!(
                state.native.is_none()
                    && (state.session.is_empty() || state.session == native.session_id),
                "Native context already bound; reconnect the host"
            );
            state.native = Some(native);
            return Ok(json!({"ok":true}));
        }
        if route == "/revoke" {
            let native: NativeContext = serde_json::from_value(body)?;
            let state = self.0.state.lock();
            ensure!(
                same_context(
                    &state,
                    &native.session_id,
                    &native.agent_id,
                    native.generation
                ),
                "Unrelated native context"
            );
            drop(state);
            self.close();
            return Ok(json!({"ok":true}));
        }
        ensure!(
            route == "/call" || route == "/cancel",
            "Unknown native route"
        );
        let call: Callback = serde_json::from_value(body)?;
        ensure!(
            valid_id(&call.call_id) && super::accepts(&call.name) && call.arguments.is_object(),
            "Invalid native callback"
        );
        let id = format!("{}:{}", call.turn_id, call.call_id);
        ensure!(valid_id(&id), "Native identity too long");
        let deadline = Instant::now() + Duration::from_secs(5);
        let worker = loop {
            let mut state = self.0.state.lock();
            ensure!(
                !self.0.closed.load(Ordering::Acquire)
                    && same_context(&state, &call.session_id, &call.agent_id, call.generation),
                "Unrelated native context"
            );
            if route == "/cancel" {
                revoke(&mut state, &id);
                return Ok(json!({"ok":true}));
            }
            ensure!(
                state.active_turn.is_some() && state.session == call.session_id,
                "Source stopped or unbound"
            );
            if let Some(native) = state.calls.get(&id) {
                if !native.valid || native.name != call.name || native.arguments != call.arguments {
                    revoke(&mut state, &id);
                    anyhow::bail!("Changed or stale native callback");
                }
                break state.bridge.clone().context("Source worker is not bound")?;
            }
            drop(state);
            ensure!(Instant::now() < deadline, "Unobserved native callback");
            thread::sleep(Duration::from_millis(25));
        };
        worker.invoke_native(&id, &call.name, call.arguments)
    }
}
fn same_context(state: &State, session: &str, agent: &str, generation: u64) -> bool {
    state.native.as_ref().is_some_and(|n| {
        n.session_id == session && n.agent_id == agent && n.generation == generation
    })
}
fn revoke(state: &mut State, id: &str) {
    if let Some(call) = state.calls.get_mut(id) {
        call.valid = false;
        call.arguments = Value::Null;
    }
    // Keep a tombstone even when cancellation beats ACP delivery.
    else if state.calls.len() < 2048 {
        state.calls.insert(
            id.into(),
            NativeCall {
                name: String::new(),
                arguments: Value::Null,
                valid: false,
            },
        );
    }
    if let Some(worker) = &state.bridge {
        worker.fail_native(id);
    }
}
fn invalidate(state: &mut State) {
    if let Some(turn) = state.native_turn.take() {
        state.last_native_turn = Some(turn);
    }
    state.active_turn = None;
    state.argument_bytes = 0;
    for call in state.calls.values_mut() {
        call.valid = false;
        call.arguments = Value::Null;
    }
    if let Some(worker) = &state.bridge {
        worker.cancel();
    }
}
impl Drop for Inner {
    fn drop(&mut self) {
        self.closed.store(true, Ordering::Release);
        if let Some(worker) = &self.state.get_mut().bridge {
            worker.close();
        }
    }
}
type Connection = (TcpStream, Weak<Inner>);
fn connections() -> Result<Sender<Connection>> {
    static POOL: OnceLock<Mutex<Option<Sender<Connection>>>> = OnceLock::new();
    let mut pool = POOL.get_or_init(|| Mutex::new(None)).lock();
    if let Some(sender) = &*pool {
        return Ok(sender.clone());
    }
    let (sender, received) = bounded::<Connection>(16);
    type Invocation = (TcpStream, Weak<Inner>, Value);
    let (calls, invocations) = bounded::<Invocation>(4);
    for index in 0..4 {
        let invocations = invocations.clone();
        thread::Builder::new()
            .name(format!("ion-kimi-team-call-{index}"))
            .spawn(move || {
                while let Ok((mut stream, weak, body)) = invocations.recv() {
                    let reply = weak
                        .upgrade()
                        .and_then(|inner| KimiBridge(inner).dispatch("/call", body).ok())
                        .unwrap_or_else(|| json!({"ok":false,"text":DENIED}));
                    let _ = write_reply(&mut stream, &reply);
                }
            })?;
    }
    // Read/control handlers never wait for model work or owner approval.
    // Abort/revoke messages retain capacity while call workers are occupied.
    for index in 0..2 {
        let received = received.clone();
        let calls = calls.clone();
        thread::Builder::new()
            .name(format!("ion-kimi-team-http-{index}"))
            .spawn(move || {
                while let Ok((mut stream, weak)) = received.recv() {
                    let parsed = (|| -> Result<_> {
                        let inner = weak.upgrade().context("Kimi source closed")?;
                        let (method, path, body) =
                            read_native_request(&mut stream, inner.port, &inner.token)?;
                        ensure!(method == "POST", "Unsupported method");
                        Ok((inner, path, body))
                    })();
                    if let Ok((inner, path, body)) = parsed {
                        if path == "/call" {
                            if let Err(error) =
                                calls.try_send((stream, Arc::downgrade(&inner), body))
                            {
                                let (mut refused, _, _) = error.into_inner();
                                let _ =
                                    write_reply(&mut refused, &json!({"ok":false,"text":DENIED}));
                            }
                            continue;
                        }
                        let reply = KimiBridge(inner)
                            .dispatch(&path, body)
                            .unwrap_or_else(|_| json!({"ok":false,"text":DENIED}));
                        let _ = write_reply(&mut stream, &reply);
                    } else {
                        let _ = write_reply(&mut stream, &json!({"ok":false,"text":DENIED}));
                    }
                }
            })?;
    }
    *pool = Some(sender.clone());
    Ok(sender)
}
fn write_reply(stream: &mut TcpStream, value: &Value) -> Result<()> {
    let bytes = serde_json::to_vec(value)?;
    ensure!(bytes.len() <= 512 * 1024, "Native result too large");
    write!(
        stream,
        "HTTP/1.1 200 Native\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        bytes.len()
    )?;
    stream.write_all(&bytes)?;
    Ok(())
}
#[cfg(test)]
#[path = "kimi_tests.rs"]
mod tests;
