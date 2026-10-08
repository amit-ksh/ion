//! OpenCode's native plugin calls, scoped by the owner's observed SSE tool parts.
//! A loopback credential alone never authorizes a call. The source session,
//! assistant message, native call ID, tool and complete arguments must match.
use super::{
    WorkspaceContext,
    worker::{Call, Worker},
};
use crate::driver::DriverEventSender;
use anyhow::{Context, Result, ensure};
use crossbeam_channel::{Sender, bounded};
use parking_lot::Mutex;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc, OnceLock, Weak,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;

const PREFIX: &str = "ion_teams_";
const LIMIT: usize = 44 * 1024;
const MAX_IDENTITIES: usize = 2048;
const REJECTED: &str = "Ion team request rejected: unrelated or changed native call, stopped source, or bridge busy. No invocation is automatically retried.";
#[derive(Clone)]
pub(crate) struct Hub(Arc<HubInner>);
struct HubInner {
    sessions: Mutex<HashMap<String, Weak<BridgeInner>>>,
    closed: Arc<AtomicBool>,
    token: String,
    port: u16,
    directory: crate::driver::team_codex::EmptyDirectory,
}
impl Drop for HubInner {
    fn drop(&mut self) {
        self.closed.store(true, Ordering::Release);
        for bridge in self.sessions.lock().values().filter_map(Weak::upgrade) {
            bridge.close();
        }
    }
}
#[derive(Clone)]
pub(crate) struct Bridge(Arc<BridgeInner>);
struct BridgeInner {
    session: String,
    state: Arc<Mutex<State>>,
    worker: Worker,
}
impl Drop for BridgeInner {
    fn drop(&mut self) {
        self.close();
    }
}
#[derive(Default)]
struct State {
    blocked: bool,
    messages: HashSet<String>,
    witnesses: HashMap<String, Witness>,
    pending: HashMap<String, Sender<Value>>,
}
struct Witness {
    message: String,
    call: String,
    name: String,
    arguments: Value,
    valid: bool,
    cancelled: Arc<AtomicBool>,
    result: Option<Value>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    session_id: String,
    message_id: String,
    call_id: String,
    name: String,
    arguments: Value,
}
fn identity(session: &str, message: &str, call: &str) -> String {
    format!(
        "opencode-team-{:x}",
        Sha256::digest(serde_json::to_vec(&json!([session, message, call])).unwrap())
    )
}
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 256 && !id.chars().any(char::is_control)
}
impl Hub {
    pub(crate) fn start() -> Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        let hub = Self(Arc::new(HubInner {
            sessions: Mutex::new(HashMap::new()),
            closed: Arc::new(AtomicBool::new(false)),
            token: format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple()),
            port: listener.local_addr()?.port(),
            directory: crate::driver::team_codex::EmptyDirectory::create("opencode-teams-plugin")?,
        }));
        let connections = connections()?;
        let weak = Arc::downgrade(&hub.0);
        let closed = hub.0.closed.clone();
        thread::Builder::new()
            .name("ion-opencode-team-listener".into())
            .spawn(move || {
                while !closed.load(Ordering::Acquire) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let _ = connections.try_send((stream, weak.clone()));
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(25))
                        }
                        Err(_) => break,
                    }
                }
            })?;
        Ok(hub)
    }
    /// Adds just our local plugin, preserving inherited content and all native
    /// global/project/OAuth plugins. The owner never writes project config.
    pub(crate) fn close(&self) {
        self.0.closed.store(true, Ordering::Release);
        for bridge in self.0.sessions.lock().values().filter_map(Weak::upgrade) {
            bridge.close();
        }
    }
    pub(crate) fn configure(
        &self,
        command: &mut std::process::Command,
        environment: &[(String, String)],
    ) -> Result<()> {
        let inherited = match environment
            .iter()
            .rev()
            .find(|(name, _)| name == "OPENCODE_CONFIG_CONTENT")
        {
            Some((_, value)) => Some(value.clone()),
            None => match std::env::var("OPENCODE_CONFIG_CONTENT") {
                Ok(value) => Some(value),
                Err(std::env::VarError::NotPresent) => None,
                Err(_) => anyhow::bail!("OPENCODE_CONFIG_CONTENT is not valid UTF-8"),
            },
        };
        let mut config: Value = match inherited {
            Some(raw) => parse_content(&raw)
                .context("OPENCODE_CONFIG_CONTENT must be a JSON object to add Ion Teams")?,
            None => json!({}),
        };
        let object = config
            .as_object_mut()
            .context("OPENCODE_CONFIG_CONTENT must contain an object")?;
        let plugins = object
            .entry("plugin")
            .or_insert(json!([]))
            .as_array_mut()
            .context("OpenCode plugin config must be an array")?;
        let path = self.0.directory.0.join("ion-teams.mjs");
        let url = url::Url::from_file_path(&path)
            .map_err(|_| anyhow::anyhow!("Invalid private plugin path"))?;
        plugins.push(json!(url.as_str()));
        std::fs::write(&path, self.plugin())?;
        command.env("OPENCODE_CONFIG_CONTENT", serde_json::to_string(&config)?);
        Ok(())
    }
    fn plugin(&self) -> String {
        let tools = super::tools()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                let f = &v["function"];
                // OpenCode 1.18 supports legacy JSON schemas without requiring an
                // npm dependency. Its legacy registry requires every property;
                // default these two optional owner arguments inside the plugin.
                let mut args = f["parameters"]["properties"].clone();
                if f["name"] == "list_teams" {
                    args = json!({});
                }
                if f["name"] == "await_team" {
                    args.as_object_mut().unwrap().remove("max_wait_seconds");
                }
                json!({"name":f["name"],"description":f["description"],"args":args})
            })
            .collect::<Vec<_>>();
        format!(
            r#"const endpoint = {endpoint};
const credential = {credential};
const definitions = {definitions};
export default async () => ({{tool: Object.fromEntries(definitions.map(def => ['ion_teams_' + def.name, {{
  description: def.description,
  args: def.args,
  async execute(args, context) {{
    if (!context.sessionID || !context.messageID || !context.callID || context.abort?.aborted)
      throw new Error('Ion Teams requires a live native source call.');
    const response = await fetch(endpoint, {{method:'POST', headers:{{'Content-Type':'application/json', Authorization:'Bearer ' + credential}},
      body:JSON.stringify({{session_id:context.sessionID, message_id:context.messageID, call_id:context.callID, name:def.name, arguments:args}}),
      signal: context.abort}});
    const reply = await response.json();
    if (!response.ok || !reply.ok) throw new Error(reply.text || 'Ion team request did not complete.');
    return reply.text;
  }}
}}]))}});
"#,
            endpoint = json!(format!("http://127.0.0.1:{}/call", self.0.port)),
            credential = json!(self.0.token),
            definitions = json!(tools)
        )
    }
    pub(crate) fn attach(
        &self,
        context: WorkspaceContext,
        session: String,
        events: DriverEventSender,
    ) -> Result<Bridge> {
        ensure!(
            valid_id(&session) && !self.0.closed.load(Ordering::Acquire),
            "Invalid or closed native server"
        );
        let state = Arc::new(Mutex::new(State::default()));
        let delivered = state.clone();
        let worker = Worker::start(context, events, move |peer, success, text| {
            let Some(id) = peer.as_str() else {
                return;
            };
            let mut state = delivered.lock();
            if let Some(witness) = state.witnesses.get_mut(id) {
                if !witness.cancelled.load(Ordering::Acquire) {
                    witness.result = Some(json!({"ok":success,"text":text}));
                }
            }
            if let Some(sender) = state.pending.remove(id) {
                let result = state
                    .witnesses
                    .get(id)
                    .and_then(|w| w.result.clone())
                    .unwrap_or_else(|| json!({"ok":false,"text":REJECTED}));
                let _ = sender.try_send(result);
            }
        })?;
        let bridge = Bridge(Arc::new(BridgeInner {
            session: session.clone(),
            state,
            worker,
        }));
        let mut sessions = self.0.sessions.lock();
        sessions.retain(|_, b| b.strong_count() > 0);
        ensure!(
            sessions
                .get(&session)
                .and_then(Weak::upgrade)
                .is_none_or(|bridge| bridge.worker.is_closed()),
            "This native OpenCode session already belongs to a source task"
        );
        sessions.insert(session, Arc::downgrade(&bridge.0));
        Ok(bridge)
    }
}
impl Bridge {
    pub(crate) fn observe(&self, event: &Value) {
        let p = &event["properties"];
        match event["type"].as_str() {
            Some("message.updated")
                if p["info"]["sessionID"].as_str() == Some(&self.0.session)
                    && p["info"]["role"] == "assistant" =>
            {
                let mut state = self.0.state.lock();
                if !state.blocked && state.messages.len() < MAX_IDENTITIES {
                    if let Some(id) = p["info"]["id"].as_str().filter(|id| valid_id(id)) {
                        state.messages.insert(id.into());
                    }
                }
            }
            Some("message.part.updated") => {
                let part = &p["part"];
                if part["sessionID"].as_str() != Some(&self.0.session) || part["type"] != "tool" {
                    return;
                }
                let Some(message) = part["messageID"].as_str().filter(|id| valid_id(id)) else {
                    return;
                };
                let Some(call) = part["callID"].as_str().filter(|id| valid_id(id)) else {
                    return;
                };
                let Some(name) = part["tool"]
                    .as_str()
                    .and_then(|s| s.strip_prefix(PREFIX))
                    .filter(|s| super::accepts(s))
                else {
                    return;
                };
                let arguments = &part["state"]["input"];
                let id = identity(&self.0.session, message, call);
                let mut state = self.0.state.lock();
                if let Some(w) = state.witnesses.get_mut(&id) {
                    if w.name != name || w.arguments != *arguments {
                        w.valid = false;
                        w.cancelled.store(true, Ordering::Release);
                    }
                    if part["state"]["status"] == "error" {
                        w.cancelled.store(true, Ordering::Release);
                    }
                } else if !state.blocked
                    && state.messages.contains(message)
                    && part["state"]["status"] == "running"
                    && arguments.is_object()
                    && serde_json::to_vec(arguments).is_ok_and(|v| v.len() <= 40 * 1024)
                    && state.witnesses.len() < MAX_IDENTITIES
                {
                    state.witnesses.insert(
                        id,
                        Witness {
                            message: message.into(),
                            call: call.into(),
                            name: name.into(),
                            arguments: arguments.clone(),
                            valid: true,
                            cancelled: Arc::new(AtomicBool::new(false)),
                            result: None,
                        },
                    );
                }
            }
            Some("session.idle" | "session.error")
                if p["sessionID"].as_str() == Some(&self.0.session) =>
            {
                self.cancel()
            }
            _ => {}
        }
    }
    pub(crate) fn allow_turn(&self) {
        if !self.0.worker.is_closed() {
            self.0.state.lock().blocked = false;
        }
    }
    pub(crate) fn cancel(&self) {
        self.0.cancel();
    }
    pub(crate) fn close(&self) {
        self.0.close();
    }
    pub(crate) fn respond(&self, id: &str, option: &str) -> bool {
        self.0.worker.respond(id, option)
    }
}
impl BridgeInner {
    fn cancel(&self) {
        let mut state = self.state.lock();
        state.blocked = true;
        for witness in state.witnesses.values_mut() {
            witness.cancelled.store(true, Ordering::Release);
            witness.result = None;
            witness.arguments = Value::Null;
            witness.valid = false;
        }
        // Tombstones survive a new turn so late or replayed old calls cannot
        // become a new authorization. Keep memory bounded, failing closed.
        for (_, sender) in state.pending.drain() {
            let _ = sender.try_send(json!({"ok":false,"text":REJECTED}));
        }
    }
    fn close(&self) {
        self.worker.close();
        self.cancel();
    }
    fn handle(&self, request: Request) -> Result<Value> {
        ensure!(
            request.session_id == self.session
                && valid_id(&request.message_id)
                && valid_id(&request.call_id)
                && super::accepts(&request.name)
                && request.arguments.is_object()
                && serde_json::to_vec(&request.arguments)?.len() <= 40 * 1024,
            "Invalid native identity"
        );
        let id = identity(&self.session, &request.message_id, &request.call_id);
        // The native running part is published before plugin execute, but its
        // SSE delivery may lag the loopback callback. Wait only for that exact
        // identity; never read historical parts as fresh authorization.
        let deadline = Instant::now() + Duration::from_secs(5);
        let (received, cancelled) = loop {
            let mut state = self.state.lock();
            ensure!(!state.blocked && !self.worker.is_closed(), "Source stopped");
            if let Some(w) = state.witnesses.get(&id) {
                ensure!(
                    w.valid
                        && !w.cancelled.load(Ordering::Acquire)
                        && w.message == request.message_id
                        && w.call == request.call_id
                        && w.name == request.name
                        && w.arguments == request.arguments,
                    "Unrelated native call"
                );
                if let Some(result) = &w.result {
                    return Ok(result.clone());
                }
                let cancelled = w.cancelled.clone();
                ensure!(
                    !state.pending.contains_key(&id) && state.pending.len() < 5,
                    "Source bridge busy"
                );
                let (sender, received) = bounded(1);
                state.pending.insert(id.clone(), sender);
                let call = Call {
                    peer: json!(id),
                    request_id: id.clone(),
                    name: request.name.clone(),
                    arguments: request.arguments.clone(),
                    cancelled: cancelled.clone(),
                };
                if self.worker.submit(call).is_err() {
                    state.pending.remove(&id);
                    anyhow::bail!("Source bridge queue full");
                }
                break (received, cancelled);
            }
            drop(state);
            ensure!(Instant::now() < deadline, "Unobserved native call");
            thread::sleep(Duration::from_millis(25));
        };
        let deadline = Instant::now() + Duration::from_secs(335);
        loop {
            if cancelled.load(Ordering::Acquire)
                || self.worker.is_closed()
                || Instant::now() >= deadline
            {
                cancelled.store(true, Ordering::Release);
                self.state.lock().pending.remove(&id);
                anyhow::bail!("Source call stopped or expired");
            }
            match received.recv_timeout(Duration::from_millis(100)) {
                Ok(value) => {
                    ensure!(
                        !cancelled.load(Ordering::Acquire) && !self.worker.is_closed(),
                        "Source call stopped before delivery"
                    );
                    return Ok(value);
                }
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                Err(_) => anyhow::bail!("Source closed"),
            }
        }
    }
}
// Four HTTP handlers for the entire owner, regardless of open workspace count.
// Holding an active connection must not keep its server generation alive.
type Connection = (TcpStream, Weak<HubInner>);
fn connections() -> Result<Sender<Connection>> {
    static POOL: OnceLock<Mutex<Option<Sender<Connection>>>> = OnceLock::new();
    let mut pool = POOL.get_or_init(|| Mutex::new(None)).lock();
    if let Some(sender) = &*pool {
        return Ok(sender.clone());
    }
    let (sender, received) = bounded::<Connection>(16);
    for index in 0..4 {
        let received = received.clone();
        thread::Builder::new()
            .name(format!("ion-opencode-team-http-{index}"))
            .spawn(move || {
                while let Ok((mut stream, weak)) = received.recv() {
                    let response = weak
                        .upgrade()
                        .and_then(|hub| {
                            if hub.closed.load(Ordering::Acquire) {
                                None
                            } else {
                                read_request(&mut stream, &hub).ok().map(|r| (hub, r))
                            }
                        })
                        .and_then(|(hub, request)| {
                            let bridge = hub
                                .sessions
                                .lock()
                                .get(&request.session_id)
                                .and_then(Weak::upgrade)?;
                            drop(hub);
                            bridge.handle(request).ok()
                        })
                        .unwrap_or_else(|| json!({"ok":false,"text":REJECTED}));
                    let _ = write_response(&mut stream, &response);
                }
            })?;
    }
    *pool = Some(sender.clone());
    Ok(sender)
}
fn read_request(stream: &mut TcpStream, hub: &HubInner) -> Result<Request> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let mut head = Vec::new();
    let mut byte = [0];
    let deadline = Instant::now() + Duration::from_secs(5);
    while !head.ends_with(b"\r\n\r\n") {
        ensure!(
            head.len() < 8192 && Instant::now() < deadline,
            "Invalid HTTP head"
        );
        stream.read_exact(&mut byte)?;
        head.push(byte[0]);
    }
    let head = std::str::from_utf8(&head)?;
    let mut lines = head.split("\r\n");
    ensure!(lines.next() == Some("POST /call HTTP/1.1"), "Invalid route");
    let mut headers = HashMap::new();
    for line in lines.filter(|s| !s.is_empty()) {
        let (name, value) = line.split_once(':').context("Invalid HTTP header")?;
        ensure!(
            headers
                .insert(name.to_ascii_lowercase(), value.trim())
                .is_none(),
            "Duplicate header"
        );
    }
    ensure!(
        !headers.contains_key("origin") && !headers.contains_key("transfer-encoding"),
        "Browser or chunked request rejected"
    );
    ensure!(
        headers.get("host") == Some(&format!("127.0.0.1:{}", hub.port).as_str())
            && headers.get("authorization") == Some(&format!("Bearer {}", hub.token).as_str())
            && headers.get("content-type") == Some(&"application/json"),
        "Invalid loopback authorization"
    );
    let len: usize = headers
        .get("content-length")
        .context("Missing content length")?
        .parse()?;
    ensure!(len > 0 && len <= LIMIT, "Request too large");
    let mut bytes = vec![0; len];
    let mut offset = 0;
    while offset < len {
        ensure!(Instant::now() < deadline, "HTTP body deadline exceeded");
        stream.set_read_timeout(Some(
            deadline
                .saturating_duration_since(Instant::now())
                .max(Duration::from_millis(1)),
        ))?;
        let read = stream.read(&mut bytes[offset..])?;
        ensure!(read > 0, "Incomplete HTTP body");
        offset += read;
    }
    Ok(serde_json::from_slice(&bytes)?)
}
// Native config content accepts JSONC. Preserve strings (including comment
// markers in URLs) while removing comments and trailing commas for serde.
fn parse_content(raw: &str) -> Result<Value> {
    let mut bytes = raw.as_bytes().to_vec();
    let mut index = 0;
    let mut quoted = false;
    while index < bytes.len() {
        if quoted {
            if bytes[index] == b'\\' {
                index += 2;
                continue;
            }
            if bytes[index] == b'"' {
                quoted = false;
            }
        } else if bytes[index] == b'"' {
            quoted = true;
        } else if bytes[index..].starts_with(b"//") {
            while index < bytes.len() && bytes[index] != b'\n' {
                bytes[index] = b' ';
                index += 1;
            }
            continue;
        } else if bytes[index..].starts_with(b"/*") {
            let start = index;
            index += 2;
            while index + 1 < bytes.len() && !bytes[index..].starts_with(b"*/") {
                index += 1;
            }
            ensure!(index + 1 < bytes.len(), "Unterminated JSONC comment");
            index += 2;
            bytes[start..index].fill(b' ');
            continue;
        }
        index += 1;
    }
    index = 0;
    quoted = false;
    while index < bytes.len() {
        if quoted {
            if bytes[index] == b'\\' {
                index += 2;
                continue;
            }
            if bytes[index] == b'"' {
                quoted = false;
            }
        } else if bytes[index] == b'"' {
            quoted = true;
        } else if bytes[index] == b',' {
            let next = bytes[index + 1..].iter().find(|b| !b.is_ascii_whitespace());
            if matches!(next, Some(b']' | b'}')) {
                bytes[index] = b' ';
            }
        }
        index += 1;
    }
    Ok(serde_json::from_slice(&bytes)?)
}
fn write_response(stream: &mut TcpStream, value: &Value) -> Result<()> {
    let body = serde_json::to_vec(value)?;
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(&body)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::Receiver;
    use waku_protocol::{
        control::{AgentProfile, ControlRecord, Team, TeamMember},
        model::{DriverEvent, ProviderKind, ProviderResumeCursor, RuntimeMode},
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
        fn bridge(&self, hub: &Hub, session: &str) -> (Bridge, Receiver<DriverEvent>) {
            let (events, received) = crate::driver::test_event_channel();
            (
                hub.attach(self.context.clone(), session.into(), events)
                    .unwrap(),
                received,
            )
        }
    }
    fn observe(bridge: &Bridge, message: &str, call: &str, name: &str, args: Value) {
        bridge.observe(&json!({"type":"message.updated","properties":{"info":{"id":message,"sessionID":bridge.0.session,"role":"assistant"}}}));
        bridge.observe(&part(&bridge.0.session, message, call, name, args));
    }
    fn part(session: &str, message: &str, call: &str, name: &str, args: Value) -> Value {
        json!({"type":"message.part.updated","properties":{"part":{"sessionID":session,"messageID":message,"callID":call,"type":"tool","tool":format!("{PREFIX}{name}"),"state":{"status":"running","input":args}}}})
    }
    fn request(session: &str, message: &str, call: &str, name: &str, args: Value) -> Request {
        Request {
            session_id: session.into(),
            message_id: message.into(),
            call_id: call.into(),
            name: name.into(),
            arguments: args,
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
    fn opencode_source_team_owner_grant_replay_and_resume() {
        let f = Fixture::new();
        let hub = Hub::start().unwrap();
        let (bridge, events) = f.bridge(&hub, "ses_native");
        observe(&bridge, "msg_1", "call_1", "invoke_team", f.args());
        let pending = bridge.clone();
        let args = f.args();
        let call = thread::spawn(move || {
            pending
                .0
                .handle(request(
                    "ses_native",
                    "msg_1",
                    "call_1",
                    "invoke_team",
                    args,
                ))
                .unwrap()
        });
        let approval = permission(&events);
        assert!(f.runs().is_empty());
        assert!(bridge.respond(&approval, "allow"));
        assert_eq!(call.join().unwrap()["ok"], true);
        assert_eq!(f.runs().len(), 1);
        assert!(f.runs()[0].work[0].access.workspace_write);
        assert_eq!(f.runs()[0].project_id, f.context.project_id);
        assert_eq!(f.runs()[0].limits.max_tokens, Some(500000));
        assert_eq!(
            bridge
                .0
                .handle(request(
                    "ses_native",
                    "msg_1",
                    "call_1",
                    "invoke_team",
                    f.args()
                ))
                .unwrap()["ok"],
            true
        );
        bridge.close();
        drop(bridge);
        let (resumed, events) = f.bridge(&hub, "ses_native");
        observe(&resumed, "msg_1", "call_1", "invoke_team", f.args());
        assert_eq!(
            resumed
                .0
                .handle(request(
                    "ses_native",
                    "msg_1",
                    "call_1",
                    "invoke_team",
                    f.args()
                ))
                .unwrap()["ok"],
            true
        );
        assert!(
            !events
                .try_iter()
                .any(|e| matches!(e, DriverEvent::Permission { .. }))
        );
        assert_eq!(f.runs().len(), 1);
        resumed.close();
    }
    #[test]
    fn opencode_source_team_changed_or_foreign_calls_never_enqueue() {
        let f = Fixture::new();
        let hub = Hub::start().unwrap();
        let (bridge, _) = f.bridge(&hub, "ses_native");
        observe(&bridge, "msg_1", "call_1", "invoke_team", f.args());
        let mut changed = f.args();
        changed["objective"] = json!("Changed work");
        assert!(
            bridge
                .0
                .handle(request(
                    "ses_native",
                    "msg_1",
                    "call_1",
                    "invoke_team",
                    changed.clone()
                ))
                .is_err()
        );
        assert!(
            bridge
                .0
                .handle(request(
                    "ses_foreign",
                    "msg_1",
                    "call_1",
                    "invoke_team",
                    f.args()
                ))
                .is_err()
        );
        bridge.observe(&part(
            "ses_native",
            "msg_1",
            "call_1",
            "invoke_team",
            changed,
        ));
        assert!(
            bridge
                .0
                .handle(request(
                    "ses_native",
                    "msg_1",
                    "call_1",
                    "invoke_team",
                    f.args()
                ))
                .is_err()
        );
        assert!(f.runs().is_empty());
        bridge.close();
    }
    #[test]
    fn opencode_source_team_stop_revokes_pending_approval_and_old_identities() {
        let f = Fixture::new();
        let hub = Hub::start().unwrap();
        let (bridge, events) = f.bridge(&hub, "ses_native");
        observe(&bridge, "msg_1", "call_1", "invoke_team", f.args());
        let pending = bridge.clone();
        let args = f.args();
        let call = thread::spawn(move || {
            pending.0.handle(request(
                "ses_native",
                "msg_1",
                "call_1",
                "invoke_team",
                args,
            ))
        });
        let approval = permission(&events);
        bridge.cancel();
        bridge.respond(&approval, "allow");
        assert!(call.join().unwrap().is_err());
        assert!(f.runs().is_empty());
        bridge.allow_turn();
        observe(&bridge, "msg_1", "call_1", "invoke_team", f.args());
        assert!(
            bridge
                .0
                .handle(request(
                    "ses_native",
                    "msg_1",
                    "call_1",
                    "invoke_team",
                    f.args()
                ))
                .is_err()
        );
        observe(&bridge, "msg_2", "call_2", "get_team_runs", json!({}));
        let reply = bridge
            .0
            .handle(request(
                "ses_native",
                "msg_2",
                "call_2",
                "get_team_runs",
                json!({}),
            ))
            .unwrap();
        assert_eq!(reply["ok"], true);
        assert_eq!(reply["text"], "[]");
        bridge.close();
    }
    #[test]
    fn opencode_source_team_shared_server_keeps_task_scope_and_teardown() {
        let f = Fixture::new();
        let hub = Hub::start().unwrap();
        let (bridge, _) = f.bridge(&hub, "ses_one");
        let mut context = f.context.clone();
        context.task_id = Uuid::new_v4();
        context.project_id = Uuid::new_v4();
        let (events, _) = crate::driver::test_event_channel();
        let other = hub
            .attach(context, "ses_two".into(), events.clone())
            .unwrap();
        assert!(
            hub.attach(f.context.clone(), "ses_one".into(), events)
                .is_err()
        );
        bridge.observe(&json!({"type":"message.updated","properties":{"info":{"id":"msg_foreign","sessionID":"ses_two","role":"assistant"}}}));
        bridge.observe(&part(
            "ses_two",
            "msg_foreign",
            "call_foreign",
            "get_team_runs",
            json!({}),
        ));
        assert!(bridge.0.state.lock().messages.is_empty());
        assert!(bridge.0.state.lock().witnesses.is_empty());
        other.observe(&part(
            "ses_two",
            "msg_user",
            "call_user",
            "get_team_runs",
            json!({}),
        ));
        assert!(
            other.0.state.lock().witnesses.is_empty(),
            "Unobserved assistant message is not trusted"
        );
        hub.close();
        assert!(bridge.0.worker.is_closed() && other.0.worker.is_closed());
    }
    #[test]
    fn opencode_source_team_config_preserves_native_plugins_and_other_fields() {
        let hub = Hub::start().unwrap();
        let mut command = std::process::Command::new("not-executed");
        hub.configure(&mut command,&[("OPENCODE_CONFIG_CONTENT".into(),json!({"plugin":["original-auth-plugin"],"mcp":{"original":{"type":"remote","url":"https://example.invalid"}},"provider":{"original":{"options":{"timeout":77}}}}).to_string())]).unwrap();
        let config: Value = serde_json::from_str(
            command
                .get_envs()
                .find(|(k, _)| *k == "OPENCODE_CONFIG_CONTENT")
                .unwrap()
                .1
                .unwrap()
                .to_str()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(config["plugin"][0], "original-auth-plugin");
        assert_eq!(config["plugin"].as_array().unwrap().len(), 2);
        assert_eq!(config["provider"]["original"]["options"]["timeout"], 77);
        assert!(config["mcp"]["original"].is_object());
        assert!(
            std::fs::read_to_string(hub.0.directory.0.join("ion-teams.mjs"))
                .unwrap()
                .contains("context.callID")
        );
        assert!(
            hub.configure(
                &mut command,
                &[("OPENCODE_CONFIG_CONTENT".into(), "[]".into())]
            )
            .is_err()
        );
        assert!(
            hub.configure(
                &mut command,
                &[(
                    "OPENCODE_CONFIG_CONTENT".into(),
                    r#"{"plugin":false}"#.into()
                )]
            )
            .is_err()
        );
        hub.close();
    }
    #[test]
    fn opencode_source_team_jsonc_content_retains_strings_and_native_fields() {
        let raw = r#"{ // Native comment
            "plugin": ["original",], /* Native comment */
            "provider":{"example":{"options":{"baseURL":"https://example.invalid/a//b", "apiKey":"escaped\"/*literal*/",},},},
        }"#;
        let parsed = parse_content(raw).unwrap();
        assert_eq!(parsed["plugin"], json!(["original"]));
        assert_eq!(
            parsed["provider"]["example"]["options"]["baseURL"],
            "https://example.invalid/a//b"
        );
        assert_eq!(
            parsed["provider"]["example"]["options"]["apiKey"],
            "escaped\"/*literal*/"
        );
        assert!(parse_content("{/* unterminated").is_err());
        assert!(parse_content("{\"plugin\": [1 2]}").is_err());
    }
    fn http(hub: &Hub, body: Value, token: &str, extra: &str) -> Value {
        let endpoint = crate::http_wire::Endpoint {
            auth: Some(format!("Bearer {token}")),
            ..crate::http_wire::Endpoint::local(hub.0.port)
        };
        if extra.is_empty() {
            return crate::http_wire::request_json(
                &endpoint,
                "POST",
                "/call",
                Some(&body),
                Duration::from_secs(10),
            )
            .unwrap_or_else(|_| json!({"ok":false,"text":"Connection rejected"}));
        }
        let mut stream = TcpStream::connect(("127.0.0.1", hub.0.port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let bytes = serde_json::to_vec(&body).unwrap();
        write!(stream,"POST /call HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n",hub.0.port,bytes.len()).unwrap();
        stream.write_all(&bytes).unwrap();
        let mut result = String::new();
        if stream.read_to_string(&mut result).is_err() {
            return json!({"ok":false,"text":"Connection rejected"});
        }
        serde_json::from_str(result.split_once("\r\n\r\n").unwrap().1).unwrap()
    }
    #[test]
    fn opencode_source_team_loopback_rejects_browser_bad_auth_and_unknown_scope() {
        let f = Fixture::new();
        let hub = Hub::start().unwrap();
        let (bridge, _) = f.bridge(&hub, "ses_native");
        observe(&bridge, "msg_1", "call_1", "get_team_runs", json!({}));
        let body = json!({"session_id":"ses_native","message_id":"msg_1","call_id":"call_1","name":"get_team_runs","arguments":{}});
        assert_eq!(http(&hub, body.clone(), "wrong", "")["ok"], false);
        assert_eq!(
            http(
                &hub,
                body.clone(),
                &hub.0.token,
                "Origin: https://example.invalid\r\n"
            )["ok"],
            false
        );
        assert_eq!(
            http(&hub, body.clone(), &hub.0.token, "Content-Length: 0\r\n")["ok"],
            false
        );
        let mut altered = body.clone();
        altered["project_id"] = json!(Uuid::new_v4());
        assert_eq!(http(&hub, altered, &hub.0.token, "")["ok"], false);
        let mut foreign = body.clone();
        foreign["session_id"] = json!("ses_other");
        assert_eq!(http(&hub, foreign, &hub.0.token, "")["ok"], false);
        let reply = http(&hub, body, &hub.0.token, "");
        assert_eq!(reply["ok"], true);
        assert_eq!(reply["text"], "[]");
        hub.close();
    }
    #[test]
    fn opencode_source_team_callback_waits_for_exact_native_event_without_blocking_reader() {
        let f = Fixture::new();
        let hub = Hub::start().unwrap();
        let (bridge, _) = f.bridge(&hub, "ses_native");
        let pending = bridge.clone();
        let started = Instant::now();
        let call = thread::spawn(move || {
            pending
                .0
                .handle(request(
                    "ses_native",
                    "msg_1",
                    "call_1",
                    "list_teams",
                    json!({}),
                ))
                .unwrap()
        });
        thread::sleep(Duration::from_millis(50));
        observe(&bridge, "msg_1", "call_1", "list_teams", json!({}));
        let result = call.join().unwrap();
        assert_eq!(result["ok"], true);
        assert!(result["text"].as_str().unwrap().contains("Engineering"));
        assert!(started.elapsed() < Duration::from_secs(5));
        bridge.close();
    }
    #[test]
    fn opencode_source_team_loopback_credential_without_native_witness_cannot_enqueue() {
        let f = Fixture::new();
        let hub = Hub::start().unwrap();
        let (bridge, events) = f.bridge(&hub, "ses_native");
        let body = json!({"session_id":"ses_native","message_id":"msg_unobserved","call_id":"call_unobserved","name":"invoke_team","arguments":f.args()});
        assert_eq!(http(&hub, body, &hub.0.token, "")["ok"], false);
        assert!(f.runs().is_empty());
        assert!(
            !events
                .try_iter()
                .any(|event| matches!(event, DriverEvent::Permission { .. }))
        );
        bridge.close();
        hub.close();
    }
    fn native_reply(call: &str, name: &str, args: Value) -> Vec<u8> {
        let item = json!({"id":format!("fc_{call}"),"type":"function_call","call_id":format!("call_{call}"),"name":format!("{PREFIX}{name}"),"arguments":args.to_string(),"status":"completed"});
        let response = json!({"id":format!("resp_{call}"),"object":"response","created_at":1,"status":"completed","model":"gpt-5.4","output":[item.clone()],"usage":{"input_tokens":100,"output_tokens":20,"input_tokens_details":{"cached_tokens":0},"output_tokens_details":{"reasoning_tokens":0}}});
        [json!({"type":"response.created","response":{"id":format!("resp_{call}"),"status":"in_progress"}}),json!({"type":"response.output_item.added","output_index":0,"item":{"id":format!("fc_{call}"),"type":"function_call","call_id":format!("call_{call}"),"name":format!("{PREFIX}{name}"),"arguments":""}}),json!({"type":"response.function_call_arguments.delta","item_id":format!("fc_{call}"),"output_index":0,"delta":args.to_string()}),json!({"type":"response.function_call_arguments.done","item_id":format!("fc_{call}"),"output_index":0,"arguments":args.to_string()}),json!({"type":"response.output_item.done","output_index":0,"item":item}),json!({"type":"response.completed","response":response})]
            .iter().map(|v|format!("event: {}\ndata: {v}\n\n",v["type"].as_str().unwrap())).collect::<String>().into_bytes()
    }
    fn private_native_server(f: &Fixture, base: &str) -> crate::opencode_pool::PooledServer {
        let native = std::env::var_os("ION_NATIVE_OPENCODE_BINARY")
            .map(std::path::PathBuf::from)
            .expect("Set exact native OpenCode executable");
        let root = &f.directory.0;
        for name in [
            "home",
            "work",
            "temp",
            "data/opencode",
            "config/node_modules",
            "config/opencode/node_modules",
            "cache",
            "state",
        ] {
            std::fs::create_dir_all(root.join(name)).unwrap();
        }
        // The import-free legacy plugin needs no npm code. Predeclare the
        // config dependency in a private lock so native setup does not fetch.
        for config_path in ["config", "config/opencode"] {
            std::fs::write(
                root.join(config_path).join("package.json"),
                r#"{"dependencies":{"@opencode-ai/plugin":"1.18.15"}}"#,
            )
            .unwrap();
            std::fs::write(
                root.join(config_path).join("package-lock.json"),
                r#"{"packages":{"":{"dependencies":{"@opencode-ai/plugin":"1.18.15"}}}}"#,
            )
            .unwrap();
        }
        std::fs::write(
            root.join("data/opencode/auth.json"),
            r#"{"openai":{"type":"api","key":"sk-ion-native-fixture-only"}}"#,
        )
        .unwrap();
        let config = json!({"share":"disabled","autoupdate":false,"plugin":[],"mcp":{},"lsp":false,"formatter":false,"compaction":{"auto":false},"agent":{"title":{"disable":true},"summary":{"disable":true},"compaction":{"disable":true},"build":{"steps":8}},"provider":{"openai":{"options":{"baseURL":base,"apiKey":"sk-ion-native-fixture-only"},"models":{"gpt-5.4":{"id":"gpt-5.4","limit":{"context":272000,"output":32768}}}}}});
        let mut environment = vec![("OPENCODE_CONFIG_CONTENT".into(), config.to_string())];
        for (name, path) in [
            ("HOME", "home"),
            ("USERPROFILE", "home"),
            ("APPDATA", "home"),
            ("LOCALAPPDATA", "home"),
            ("TMP", "temp"),
            ("TEMP", "temp"),
            ("TMPDIR", "temp"),
            ("XDG_CONFIG_HOME", "config"),
            ("OPENCODE_CONFIG_DIR", "config"),
            ("XDG_DATA_HOME", "data"),
            ("XDG_CACHE_HOME", "cache"),
            ("XDG_STATE_HOME", "state"),
        ] {
            environment.push((name.into(), root.join(path).to_string_lossy().into_owned()));
        }
        for name in [
            "OPENCODE_DISABLE_AUTOUPDATE",
            "OPENCODE_DISABLE_DEFAULT_PLUGINS",
            "OPENCODE_DISABLE_AUTOCOMPACT",
            "OPENCODE_DISABLE_LSP_DOWNLOAD",
            "OPENCODE_DISABLE_CLAUDE_CODE",
            "OPENCODE_DISABLE_MODELS_FETCH",
            "OPENCODE_DISABLE_PROJECT_CONFIG",
        ] {
            environment.push((name.into(), "true".into()));
        }
        let system_root = std::env::var_os("SystemRoot");
        let server = crate::opencode_session::OpenCodeServer::start_configured(
            &native,
            &root.join("work"),
            &environment,
            move |c| {
                c.env_clear();
                if let Some(system_root) = system_root {
                    c.env("SystemRoot", system_root);
                }
                #[cfg(unix)]
                c.env("PATH", "/usr/bin:/bin");
                c.env("OPENAI_API_KEY", "sk-ion-native-fixture-only")
                    .env("NPM_CONFIG_REGISTRY", "http://127.0.0.1:1");
            },
        )
        .unwrap();
        assert_eq!(
            server.request("GET", "/global/health", None).unwrap()["version"],
            "1.18.15"
        );
        let tools = server
            .request("GET", "/experimental/tool/ids", None)
            .unwrap();
        assert!(
            tools
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v == "ion_teams_invoke_team")
        );
        assert!(
            tools.as_array().unwrap().iter().any(|v| v == "read"),
            "Normal tools must remain available"
        );
        crate::opencode_pool::PooledServer::dedicated(server)
    }
    fn driver(
        f: &Fixture,
        server: crate::opencode_pool::PooledServer,
        cursor: Option<ProviderResumeCursor>,
    ) -> (
        crate::driver::opencode::OpenCodeDriver,
        Receiver<DriverEvent>,
    ) {
        let (events, received) = crate::driver::test_event_channel();
        let driver = crate::driver::opencode::OpenCodeDriver::start_with_server(
            crate::driver::DriverStartOptions {
                binary: std::env::var_os("ION_NATIVE_OPENCODE_BINARY")
                    .unwrap()
                    .into(),
                cwd: f.directory.0.join("work"),
                mode: RuntimeMode::Ask,
                model: Some("openai/gpt-5.4".into()),
                reasoning_effort: None,
                service_tier: None,
                context_window: None,
                agent_preset: None,
                computer_use_enabled: false,
                provider_cursor: cursor,
            },
            events,
            Some(f.context.clone()),
            Some(server),
        )
        .unwrap();
        (driver, received)
    }
    fn run_turn(
        f: &Fixture,
        driver: &crate::driver::opencode::OpenCodeDriver,
        received: &Receiver<DriverEvent>,
        approve: bool,
    ) -> Option<ProviderResumeCursor> {
        use crate::driver::DriverControl;
        let mut cursor = None;
        let mut approved = false;
        let deadline = Instant::now() + Duration::from_secs(35);
        let mut text = String::new();
        driver.prompt("Use the saved team and inspect its real run status.".into());
        while Instant::now() < deadline {
            match received.recv_timeout(Duration::from_millis(100)) {
                Ok(DriverEvent::Connected { provider_cursor }) => cursor = provider_cursor,
                Ok(DriverEvent::Permission { request_id, .. }) => {
                    assert!(approve);
                    assert!(request_id.starts_with("ion-team-approval-"));
                    assert!(f.runs().is_empty());
                    approved = true;
                    driver.respond(request_id, "allow".into());
                }
                Ok(DriverEvent::TextDelta(delta)) => text.push_str(&delta),
                Ok(DriverEvent::TurnFinished { success, .. }) => {
                    assert!(success);
                    assert_eq!(approved, approve);
                    assert!(text.contains("Team queued"));
                    return cursor;
                }
                Ok(DriverEvent::Error(error)) => {
                    panic!("Private native OpenCode fixture failed: {error}")
                }
                _ => {}
            }
        }
        panic!("Native OpenCode source turn did not finish; text={text}");
    }
    #[test]
    #[ignore = "Requires ION_NATIVE_OPENCODE_BINARY=1.18.15; private profile, fake account and localhost Responses only"]
    fn opencode_source_team_native_plugin_start_resume_and_stop_through_actual_driver() {
        use crate::driver::DriverControl;
        let mut f = Fixture::new();
        let responses = vec![
            (200, native_reply("list", "list_teams", json!({}))),
            (200, native_reply("invoke", "invoke_team", f.args())),
            (200, native_reply("runs", "get_team_runs", json!({}))),
            (
                200,
                crate::driver::team_test_http::responses(
                    "gpt-5.4",
                    "Team queued; inspect its run for completion.",
                ),
            ),
            (
                200,
                native_reply("resumed-runs", "get_team_runs", json!({})),
            ),
            (
                200,
                crate::driver::team_test_http::responses(
                    "gpt-5.4",
                    "Team queued; still inspect completion.",
                ),
            ),
            (200, native_reply("stopped-invoke", "invoke_team", f.args())),
        ];
        let provider = crate::driver::team_test_http::Server::with_response_handler(
            responses,
            |request, body| {
                assert!(request.line.starts_with("POST /responses "));
                assert_eq!(request.body["model"], "gpt-5.4");
                assert!(request.headers.contains("sk-ion-native-fixture-only"));
                assert!(
                    request.body["tools"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|v| v["name"] == "ion_teams_invoke_team")
                );
                body
            },
        );
        let server = private_native_server(&f, &provider.url);
        let (source, events) = driver(&f, server.clone(), None);
        let cursor = run_turn(&f, &source, &events, true).unwrap();
        assert_eq!(f.runs().len(), 1);
        assert!(f.runs()[0].work[0].access.workspace_write);
        assert!(
            f.runs()[0]
                .origin
                .as_ref()
                .unwrap()
                .request_id
                .starts_with("opencode-team-")
        );
        drop(source);
        let (resumed, events) = driver(&f, server.clone(), Some(cursor));
        run_turn(&f, &resumed, &events, false);
        assert_eq!(f.runs().len(), 1);
        // A new source asks for a new grant. Stop on that pending approval.
        f.context.task_id = Uuid::new_v4();
        let (source, stopped_events) = driver(&f, server.clone(), None);
        source.prompt("Start another team.".into());
        let approval = permission(&stopped_events);
        source.cancel();
        source.respond(approval, "allow".into());
        thread::sleep(Duration::from_millis(250));
        assert!(f.runs().is_empty());
        drop(source);
        assert!(
            !events.try_iter().any(|event| matches!(
                event,
                DriverEvent::Permission { .. }
                    | DriverEvent::Activity { .. }
                    | DriverEvent::TextDelta(_)
            )),
            "A second task's native events must not reach the first task on their shared server"
        );
        drop(resumed);
        server.shutdown(Duration::from_secs(3));
        assert_eq!(
            provider.finish().len(),
            7,
            "No retry, alternate model, or post-Stop provider call is accepted"
        );
    }
}
