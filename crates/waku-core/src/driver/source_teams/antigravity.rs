//! Antigravity's native MCP envelope is bound to the owned main-conversation stream.
use super::{
    WorkspaceContext,
    opencode::{Bridge, read_native_request},
    project_bundle::ProjectBundle,
};
use crate::driver::DriverEventSender;
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
use uuid::Uuid;

const DENIED: &str = "Ion team request was not authorized by the active Antigravity task. Check team setup, current definitions and approval; no invocation is automatically retried.";
#[derive(Clone)]
pub(crate) struct AntigravityBridge(Arc<Inner>);
struct Inner {
    context: WorkspaceContext,
    events: DriverEventSender,
    server: String,
    port: u16,
    token: String,
    closed: Arc<AtomicBool>,
    state: Mutex<State>,
    _plugin: ProjectBundle,
}
#[derive(Default)]
struct State {
    session: String,
    bridge: Option<Bridge>,
    turn: Option<Arc<AtomicBool>>,
    native_run: Option<Uuid>,
    calls: HashMap<u64, Call>,
    bytes: usize,
}
struct Call {
    name: String,
    arguments: Value,
    valid: bool,
    consumed: bool,
}
fn active(state: &State) -> bool {
    state
        .turn
        .as_ref()
        .is_some_and(|flag| !flag.load(Ordering::Acquire))
}
fn valid_session(id: &str) -> bool {
    Uuid::parse_str(id).is_ok_and(|uuid| uuid.hyphenated().to_string() == id)
}
impl AntigravityBridge {
    pub(crate) fn start(
        context: WorkspaceContext,
        events: DriverEventSender,
        cwd: &Path,
    ) -> Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let id = Uuid::new_v4().simple().to_string();
        let plugin_name = format!("ion-teams-{id}");
        let server = format!("{plugin_name}_teams");
        let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let plugin=ProjectBundle::install(cwd,&format!("ion-teams-{id}"),
            &json!({"name":plugin_name,"description":"Ion source-task Teams tools"}).to_string(),
            &json!({"mcpServers":{"teams":{"serverUrl":format!("http://127.0.0.1:{port}/mcp"),"headers":{"Authorization":format!("Bearer {token}")},"timeoutSeconds":335}}}).to_string())?;
        let this = Self(Arc::new(Inner {
            context,
            events,
            server,
            port,
            token,
            closed: Arc::new(AtomicBool::new(false)),
            state: Mutex::new(State::default()),
            _plugin: plugin,
        }));
        let queue = connections()?;
        let weak = Arc::downgrade(&this.0);
        let closed = this.0.closed.clone();
        thread::Builder::new()
            .name("ion-antigravity-team-listener".into())
            .spawn(move || {
                while !closed.load(Ordering::Acquire) {
                    match listener.accept() {
                        Ok((socket, _)) => {
                            let _ = queue.try_send((socket, weak.clone()));
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(25))
                        }
                        Err(_) => break,
                    }
                }
            })?;
        Ok(this)
    }
    pub(crate) fn begin(&self, flag: Arc<AtomicBool>) -> bool {
        let mut state = self.0.state.lock();
        if flag.load(Ordering::Acquire) || self.0.closed.load(Ordering::Acquire) {
            return false;
        }
        invalidate(&mut state);
        state.turn = Some(flag);
        if let Some(bridge) = &state.bridge {
            bridge.allow_turn();
        }
        true
    }
    pub(crate) fn finish(&self) {
        invalidate(&mut self.0.state.lock());
    }
    pub(crate) fn cancel(&self) {
        self.finish();
    }
    pub(crate) fn close(&self) {
        self.0.closed.store(true, Ordering::Release);
        let mut state = self.0.state.lock();
        invalidate(&mut state);
        if let Some(bridge) = &state.bridge {
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
    pub(crate) fn observe(&self, event: &Value) -> Result<()> {
        let mut state = self.0.state.lock();
        if !active(&state) || self.0.closed.load(Ordering::Acquire) {
            return Ok(());
        }
        if event["event"] == "init" {
            let session = event["conversation_id"]
                .as_str()
                .filter(|id| valid_session(id))
                .context("Invalid native Antigravity session")?;
            ensure!(
                state.session.is_empty() || state.session == session,
                "Native Antigravity session changed"
            );
            if state.bridge.is_none() {
                let bridge = Bridge::start(
                    self.0.context.clone(),
                    format!("antigravity:{session}"),
                    self.0.events.clone(),
                )?;
                bridge.allow_turn();
                state.bridge = Some(bridge);
                state.session = session.into();
            }
            return Ok(());
        }
        if event["event"] == "result" {
            invalidate(&mut state);
            return Ok(());
        }
        if event["event"] != "step_update" {
            return Ok(());
        }
        let step = &event["step_update"];
        if step["conversation_id"].as_str() != Some(&state.session) || step["step_type"] != "tool" {
            return Ok(());
        }
        let Some(index) = step["step_index"].as_u64() else {
            return Ok(());
        };
        if step["state"] != "ACTIVE" {
            if let Some(call) = state.calls.get_mut(&index) {
                call.valid = false;
            }
            if let Some(bridge) = &state.bridge {
                bridge.fail_native(&index.to_string());
            }
            return Ok(());
        }
        let params = &step["tool_info"]["parameters"];
        let target = step["tool_name"] == "call_mcp_tool"
            && step["tool_info"]["name"] == "call_mcp_tool"
            && params["ServerName"] == self.0.server;
        let arguments = &params["Arguments"];
        if let Some(call) = state.calls.get_mut(&index) {
            if !target
                || !call.valid
                || params["ToolName"].as_str() != Some(&call.name)
                || call.arguments != *arguments
            {
                call.valid = false;
                if let Some(bridge) = &state.bridge {
                    bridge.fail_native(&index.to_string());
                }
            }
            return Ok(());
        }
        if !target {
            return Ok(());
        }
        let Some(name) = params["ToolName"]
            .as_str()
            .filter(|name| super::accepts(name))
        else {
            return Ok(());
        };
        let bytes = serde_json::to_vec(arguments)?.len();
        ensure!(
            arguments.is_object()
                && bytes <= 40 * 1024
                && state.calls.len() < 2048
                && state.bytes.saturating_add(bytes) <= 1024 * 1024,
            "Native Antigravity call exceeded its bounds"
        );
        state.bytes += bytes;
        state.calls.insert(
            index,
            Call {
                name: name.into(),
                arguments: arguments.clone(),
                valid: true,
                consumed: false,
            },
        );
        state
            .bridge
            .as_ref()
            .context("Unbound native Antigravity session")?
            .witness_native(&index.to_string(), name, arguments.clone());
        Ok(())
    }
    fn dispatch(&self, params: &Value) -> Result<Value> {
        let meta = &params["_meta"];
        let session = meta["antigravity.google/conversation_id"]
            .as_str()
            .filter(|id| valid_session(id))
            .context("Missing native session")?;
        let (run, index) = meta["progressToken"]
            .as_str()
            .and_then(|v| v.rsplit_once(':'))
            .context("Missing native step")?;
        let run = Uuid::parse_str(run)?;
        let step: u64 = index.parse()?;
        ensure!(index == step.to_string(), "Invalid native step");
        let name = params["name"]
            .as_str()
            .filter(|name| super::accepts(name))
            .context("Unknown team tool")?;
        let args = &params["arguments"];
        ensure!(
            args.is_object() && serde_json::to_vec(args)?.len() <= 40 * 1024,
            "Invalid native arguments"
        );
        let until = Instant::now() + Duration::from_secs(4);
        let bridge = loop {
            let mut state = self.0.state.lock();
            ensure!(
                active(&state) && !self.0.closed.load(Ordering::Acquire),
                "Stopped source"
            );
            ensure!(
                state.session.is_empty() || state.session == session,
                "Unrelated native conversation"
            );
            if let Some(call) = state.calls.get_mut(&step) {
                if call.name != name || call.arguments != *args {
                    call.valid = false;
                    if let Some(bridge) = &state.bridge {
                        bridge.fail_native(index);
                    }
                    anyhow::bail!("Changed native arguments");
                }
                ensure!(
                    call.valid && !call.consumed,
                    "Native call expired or consumed"
                );
                ensure!(
                    state.native_run.is_none_or(|id| id == run),
                    "Unrelated native process"
                );
                state.native_run = Some(run);
                state.calls.get_mut(&step).unwrap().consumed = true;
                break state.bridge.clone().context("Unbound source")?;
            }
            drop(state);
            ensure!(Instant::now() < until, "Native call was not witnessed");
            thread::sleep(Duration::from_millis(10));
        };
        bridge.invoke_native(index, name, args.clone())
    }
    fn mcp(&self, body: Value) -> Value {
        let id = body.get("id").cloned();
        if id.is_none() {
            return Value::Null;
        }
        let id = id.unwrap();
        let result = match body["method"].as_str() {
            Some("initialize") => {
                json!({"protocolVersion":body["params"]["protocolVersion"],"capabilities":{"tools":{}},"serverInfo":{"name":"ion-teams","version":"1"}})
            }
            Some("ping") => json!({}),
            Some("tools/list") => {
                json!({"tools":super::tools().as_array().unwrap().iter().map(|tool|{let f=&tool["function"];json!({"name":f["name"],"description":f["description"],"inputSchema":f["parameters"]})}).collect::<Vec<_>>()})
            }
            Some("tools/call") => {
                let result = self
                    .dispatch(&body["params"])
                    .unwrap_or_else(|_| json!({"ok":false,"text":DENIED}));
                json!({"isError":result["ok"]!=true,"content":[{"type":"text","text":result["text"].as_str().unwrap_or(DENIED)}]})
            }
            _ => {
                return json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"Method unavailable"}});
            }
        };
        json!({"jsonrpc":"2.0","id":id,"result":result})
    }
}
fn invalidate(state: &mut State) {
    state.turn = None;
    state.native_run = None;
    state.bytes = 0;
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
type Connection = (TcpStream, Weak<Inner>);
fn connections() -> Result<Sender<Connection>> {
    static POOL: OnceLock<Mutex<Option<Sender<Connection>>>> = OnceLock::new();
    let mut pool = POOL.get_or_init(Default::default).lock();
    if let Some(sender) = &*pool {
        return Ok(sender.clone());
    }
    let (sent, received) = bounded::<Connection>(16);
    for index in 0..4 {
        let received = received.clone();
        thread::Builder::new().name(format!("ion-antigravity-team-http-{index}")).spawn(move||{
            while let Ok((mut socket,weak))=received.recv(){
                let result=(||->Result<(u16,Value)>{
                    let inner=weak.upgrade().context("Closed native bridge")?;
                    ensure!(!inner.closed.load(Ordering::Acquire),"Closed native bridge");
                    let (method,path,body)=read_native_request(&mut socket,inner.port,&inner.token)?;
                    ensure!(path=="/mcp","Unknown native route");
                    if method=="GET"{return Ok((405,Value::Null));}
                    let reply=AntigravityBridge(inner).mcp(body);
                    Ok((if reply.is_null(){202}else{200},reply))
                })().unwrap_or_else(|_|(403,json!({"error":DENIED})));
                let bytes=serde_json::to_vec(&result.1).unwrap_or_default();
                let _=write!(socket,"HTTP/1.1 {} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",result.0,bytes.len());
                let _=socket.write_all(&bytes);
            }
        })?;
    }
    *pool = Some(sent.clone());
    Ok(sent)
}
#[cfg(test)]
#[path = "antigravity_tests.rs"]
mod tests;
