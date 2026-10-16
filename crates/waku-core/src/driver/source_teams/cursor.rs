//! Cursor native ACP observations, rewritten hook input and per-call approval
//! must agree before a private MCP callback reaches the shared Teams worker.
mod binding;
#[cfg(windows)]
mod lease;
#[cfg(not(windows))]
mod lease {
    pub(super) struct HookLease;
    impl HookLease {
        pub(super) fn install(
            _: &std::path::Path,
            _: &str,
            _: &str,
            _: &str,
        ) -> anyhow::Result<Self> {
            anyhow::bail!("Cursor Teams project hooks require the verified Windows runtime")
        }
        pub(super) fn valid(&self) -> anyhow::Result<()> {
            anyhow::bail!("Unsupported native hook platform")
        }
    }
}
use super::{
    WorkspaceContext,
    opencode::{Bridge, read_native_request},
};
use crate::driver::DriverEventSender;
use agent_client_protocol::{
    AcpAgentConfig,
    schema::v1::{HttpHeader, McpServer, McpServerHttp},
};
use anyhow::{Context, Result, ensure};
use binding::{Bindings, FIELD};
use crossbeam_channel::{Sender, bounded};
use lease::HookLease;
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    io::Write,
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        Arc, OnceLock, Weak,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};
const DENIED: &str = "Ion Teams could not verify this native Cursor call. Check project hook access and per-call approval, then retry from the task. No team was automatically retried.";
#[derive(Clone)]
pub(crate) struct CursorBridge(Arc<Inner>);
struct Inner {
    context: WorkspaceContext,
    events: DriverEventSender,
    server: String,
    prefix: String,
    token: String,
    port: u16,
    closed: AtomicBool,
    state: Mutex<State>,
    hook: HookLease,
}
#[derive(Default)]
struct State {
    session: String,
    binding: Option<Bindings>,
    bridge: Option<Bridge>,
    next_turn: u64,
    cancelled_through: u64,
    active_turn: Option<u64>,
}
#[derive(Clone)]
pub(crate) struct CursorApproval {
    bridge: CursorBridge,
    id: String,
}
impl CursorApproval {
    /// Runs on the ACP reader, never from a render path. The shared worker is
    /// authorized before native execution can race its MCP callback.
    pub(crate) fn decide(&self, allowed: bool) -> Result<()> {
        if allowed {
            self.bridge.0.hook.valid()?;
        }
        let mut state = self.bridge.0.state.lock();
        ensure!(
            !self.bridge.0.closed.load(Ordering::Acquire),
            "Cursor source closed"
        );
        let binding = state.binding.as_mut().context("Unbound Cursor source")?;
        binding.approve(&self.id, allowed)?;
        if allowed {
            let invocation = binding.approved(&self.id)?;
            state
                .bridge
                .as_ref()
                .context("Unbound source worker")?
                .witness_native(&invocation.call_id, &invocation.name, invocation.arguments);
        } else if let Some(bridge) = &state.bridge {
            bridge.fail_native(&self.id);
        }
        Ok(())
    }
}
fn private_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}
fn config_env<'a>(config: &'a AcpAgentConfig, name: &str) -> Option<&'a str> {
    config
        .environment()
        .get(name)
        .or_else(|| {
            config
                .environment()
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(name))
                .map(|(_, value)| value)
        })
        .map(String::as_str)
        .filter(|value| !value.is_empty())
}
fn cache_root(cwd: &Path, config: &AcpAgentConfig) -> Result<PathBuf> {
    let data = if let Some(data) = config_env(config, "CURSOR_DATA_DIR") {
        PathBuf::from(data)
    } else {
        PathBuf::from(config_env(config, "USERPROFILE").context("Cursor home is unavailable")?)
            .join(".cursor")
    };
    ensure!(
        data.is_absolute() && cwd.is_absolute(),
        "Cursor cache and workspace must be absolute"
    );
    let cwd = cwd.to_str().context("Cursor workspace is not UTF-8")?;
    // Exact pinned native sanitizer: ASCII alphanumerics, collapsed hyphens.
    let mut key = String::new();
    for ch in cwd.chars() {
        if ch.is_ascii_alphanumeric() {
            key.push(ch);
        } else if !key.is_empty() && !key.ends_with('-') {
            key.push('-');
        }
    }
    if key.ends_with('-') {
        key.pop();
    }
    ensure!(!key.is_empty(), "Cursor project cache key is empty");
    let root = data.join("projects").join(key);
    ensure!(
        root.to_string_lossy().encode_utf16().count() < 260,
        "Cursor hook cache exceeds the Windows launch limit; use a shorter workspace or configured Cursor data directory"
    );
    Ok(root)
}
fn verify(config: &AcpAgentConfig, cwd: &Path) -> Result<()> {
    ensure!(
        cfg!(windows),
        "Cursor Teams needs a verified Windows hook runtime"
    );
    let mut args = config.arguments().to_owned();
    ensure!(
        args.last().map(String::as_str) == Some("acp"),
        "Unsupported Cursor native launch"
    );
    args.pop();
    args.push("--version".into());
    let mut command = crate::command_env::command(config.command());
    command
        .env_clear()
        .envs(config.environment())
        .args(args)
        .current_dir(cwd);
    let bytes = crate::team_process::bounded_process(
        &mut command,
        b"",
        &AtomicBool::new(false),
        Duration::from_secs(15),
    )?;
    ensure!(
        std::str::from_utf8(&bytes)?.trim() == "2026.10.01-e373342",
        "Cursor Teams requires native 2026.10.01-e373342"
    );
    Ok(())
}
impl CursorBridge {
    pub(crate) fn start(
        context: WorkspaceContext,
        events: DriverEventSender,
        cwd: &Path,
        config: &AcpAgentConfig,
    ) -> Result<Self> {
        let root = cache_root(cwd, config)?;
        verify(config, cwd)?;
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let identity = uuid::Uuid::new_v4().simple().to_string();
        let server = format!("ion_teams_{identity}");
        let prefix = format!("ion_{identity}_");
        let matcher = format!(
            "^MCP:{prefix}(list_teams|invoke_team|get_team_runs|await_team|read_team_document)$"
        );
        let hook = HookLease::install(&root, &identity, &matcher, &hook_command(port)?)?;
        let result = Self(Arc::new(Inner {
            context,
            events,
            server,
            prefix,
            token: private_token(),
            port,
            state: Mutex::new(State::default()),
            closed: AtomicBool::new(false),
            hook,
        }));
        let pool = connections()?;
        let weak = Arc::downgrade(&result.0);
        thread::Builder::new()
            .name("ion-cursor-team-listener".into())
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
        Ok(result)
    }
    pub(crate) fn environment(&self) -> Vec<(String, String)> {
        vec![("ION_CURSOR_TEAM_HOOK_TOKEN".into(), self.0.token.clone())]
    }
    pub(crate) fn mcp_servers(&self) -> Vec<McpServer> {
        if self.0.closed.load(Ordering::Acquire) {
            return vec![];
        }
        vec![McpServer::Http(
            McpServerHttp::new(
                &self.0.server,
                format!("http://127.0.0.1:{}/mcp", self.0.port),
            )
            .headers(vec![HttpHeader::new(
                "Authorization",
                format!("Bearer {}", self.0.token),
            )]),
        )]
    }
    pub(crate) fn bind(&self, session: &str) -> Result<()> {
        let mut state = self.0.state.lock();
        ensure!(
            !self.0.closed.load(Ordering::Acquire)
                && (state.session.is_empty() || state.session == session),
            "Cursor session identity changed or closed"
        );
        if state.bridge.is_none() {
            let binding =
                Bindings::new(session.into(), self.0.server.clone(), self.0.prefix.clone())?;
            let worker = Bridge::start(
                self.0.context.clone(),
                format!("cursor:{session}"),
                self.0.events.clone(),
            )?;
            worker.cancel();
            state.bridge = Some(worker);
            state.binding = Some(binding);
            state.session = session.into();
        }
        Ok(())
    }
    pub(crate) fn queue_turn(&self) -> u64 {
        let mut s = self.0.state.lock();
        s.next_turn = s.next_turn.saturating_add(1);
        s.next_turn
    }
    pub(crate) fn activate(&self, turn: u64) -> bool {
        let mut state = self.0.state.lock();
        if self.0.closed.load(Ordering::Acquire)
            || state.binding.is_none()
            || turn <= state.cancelled_through
        {
            return false;
        }
        invalidate(&mut state);
        state.active_turn = Some(turn);
        state.binding.as_mut().unwrap().activate();
        state.bridge.as_ref().unwrap().allow_turn();
        true
    }
    pub(crate) fn active_turn(&self) -> Option<u64> {
        self.0.state.lock().active_turn
    }
    pub(crate) fn finish_turn(&self, turn: Option<u64>) {
        let mut state = self.0.state.lock();
        if turn.is_some() && turn == state.active_turn {
            invalidate(&mut state);
        }
    }
    pub(crate) fn cancel(&self) {
        let mut s = self.0.state.lock();
        s.cancelled_through = s.next_turn;
        invalidate(&mut s);
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
            .is_some_and(|worker| worker.respond(id, option))
    }
    pub(crate) fn observe(&self, event: &Value) {
        let mut state = self.0.state.lock();
        let id = event
            .pointer("/update/toolCallId")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let owned = state
            .binding
            .as_ref()
            .is_some_and(|binding| binding.owns(id))
            || event
                .pointer("/update/rawInput/providerIdentifier")
                .and_then(Value::as_str)
                == Some(&self.0.server);
        if !owned {
            return;
        }
        let result = state.binding.as_mut().map(|binding| binding.observe(event));
        if result.is_some_and(|r| r.is_err()) || event["update"]["status"] == "failed" {
            if let Some(worker) = &state.bridge {
                worker.fail_native(id);
            }
        }
    }
    pub(crate) fn permission(&self, request: &Value) -> Result<Option<CursorApproval>> {
        let id = request
            .pointer("/toolCall/toolCallId")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let mut state = self.0.state.lock();
        let owned = state
            .binding
            .as_ref()
            .is_some_and(|binding| binding.owns(id))
            || request
                .pointer("/toolCall/title")
                .and_then(Value::as_str)
                .is_some_and(|title| {
                    title.starts_with(&format!("{}-{}", self.0.server, self.0.prefix))
                });
        if !owned {
            return Ok(None);
        }
        ensure!(
            !self.0.closed.load(Ordering::Acquire),
            "Cursor source closed"
        );
        let result = state
            .binding
            .as_mut()
            .context("Unbound Cursor source")?
            .witness(request);
        if let Err(error) = result {
            state.binding.as_mut().unwrap().revoke(id);
            if let Some(worker) = &state.bridge {
                worker.fail_native(id);
            }
            return Err(error);
        }
        Ok(Some(CursorApproval {
            bridge: self.clone(),
            id: id.into(),
        }))
    }
    fn hook(&self, event: &Value) -> Value {
        let result = (|| -> Result<Value> {
            self.0.hook.valid()?;
            ensure!(
                !self.0.closed.load(Ordering::Acquire),
                "Cursor source closed"
            );
            self.0
                .state
                .lock()
                .binding
                .as_mut()
                .context("Unbound native source")?
                .hook(event)
        })();
        result.unwrap_or_else(|_| json!({"permission":"deny","user_message":DENIED}))
    }
    fn dispatch(&self, params: &Value) -> Result<Value> {
        self.0.hook.valid()?;
        let (worker, invocation) = {
            let mut state = self.0.state.lock();
            ensure!(
                !self.0.closed.load(Ordering::Acquire),
                "Cursor source closed"
            );
            let binding = state.binding.as_mut().context("Unbound native source")?;
            let id = binding.bound_call(&params["arguments"]);
            let result = binding.consume(
                params["name"].as_str().unwrap_or_default(),
                &params["arguments"],
            );
            let invocation = match result {
                Ok(invocation) => invocation,
                Err(error) => {
                    if let Some(id) = id {
                        binding.revoke(&id);
                        if let Some(worker) = &state.bridge {
                            worker.fail_native(&id);
                        }
                    }
                    return Err(error);
                }
            };
            (
                state.bridge.clone().context("Unbound native worker")?,
                invocation,
            )
        };
        worker.invoke_native(&invocation.call_id, &invocation.name, invocation.arguments)
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
                let f=&tool["function"]; let mut schema=f["parameters"].clone();
                schema["properties"][FIELD]=json!({"type":"string","description":"Provided by the native task; leave unset."});
                json!({"name":format!("{}{}",self.0.prefix,f["name"].as_str().unwrap()),"description":f["description"],"inputSchema":schema})
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
    pub(crate) fn scrub(&self, value: &mut Value) {
        let state = self.0.state.lock();
        if let Some(binding) = &state.binding {
            binding.scrub(value);
        } else {
            scrub(value, &HashMap::new());
        }
        scrub_text(value, &self.0.token, &self.0.prefix, &self.0.server);
    }
}
fn invalidate(state: &mut State) {
    state.active_turn = None;
    if let Some(binding) = &mut state.binding {
        binding.cancel();
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
fn scrub_text(value: &mut Value, token: &str, prefix: &str, server: &str) {
    match value {
        Value::Object(o) => {
            for v in o.values_mut() {
                scrub_text(v, token, prefix, server)
            }
        }
        Value::Array(a) => {
            for v in a {
                scrub_text(v, token, prefix, server)
            }
        }
        Value::String(s) => {
            *s = s
                .replace(token, "")
                .replace(prefix, "")
                .replace(server, "Ion Teams");
        }
        _ => {}
    }
}
fn scrub(value: &mut Value, nonces: &HashMap<String, String>) {
    match value {
        Value::Object(o) => {
            o.remove(FIELD);
            for v in o.values_mut() {
                scrub(v, nonces)
            }
        }
        Value::Array(a) => {
            for v in a {
                scrub(v, nonces)
            }
        }
        Value::String(s) => {
            if let Some(source) = s
                .strip_prefix("```json\n")
                .and_then(|s| s.strip_suffix("\n```"))
            {
                if let Ok(mut parsed) = serde_json::from_str::<Value>(source) {
                    scrub(&mut parsed, nonces);
                    *s = format!(
                        "```json\n{}\n```",
                        serde_json::to_string_pretty(&parsed).unwrap_or_default()
                    );
                    return;
                }
            }
            *s = s.replace(FIELD, "");
            let bytes = s.as_bytes();
            let mut out = String::new();
            let (mut at, mut start) = (0, 0);
            while at + 64 <= bytes.len() {
                if bytes[at..at + 64].iter().all(|b| b.is_ascii_hexdigit())
                    && nonces.contains_key(&s[at..at + 64])
                {
                    out.push_str(&s[start..at]);
                    at += 64;
                    start = at;
                } else {
                    at += 1;
                }
            }
            if start > 0 {
                out.push_str(&s[start..]);
                *s = out;
            }
        }
        _ => {}
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
        thread::Builder::new().name(format!("ion-cursor-team-http-{index}")).spawn(move || {
            while let Ok((mut stream,weak))=received.recv() {
                let reply=(||->Result<(u16,Value)>{
                    let inner=weak.upgrade().context("Cursor bridge closed")?;
                    ensure!(!inner.closed.load(Ordering::Acquire),"Cursor bridge closed");
                    let (method,path,body)=read_native_request(&mut stream,inner.port,&inner.token)?;
                    let bridge=CursorBridge(inner);
                    if method!="POST" {return Ok((405,Value::Null));}
                    match path.as_str() {
                        "/hook"=>Ok((200,bridge.hook(&body))),
                        "/mcp"=>{let value=bridge.mcp(body);Ok((if value.is_null(){202}else{200},value))},
                        _=>anyhow::bail!("Unknown native route"),
                    }
                })().unwrap_or_else(|_|(403,json!({"permission":"deny","user_message":DENIED})));
                let body=if reply.1.is_null(){Vec::new()}else{serde_json::to_vec(&reply.1).unwrap_or_default()};
                if body.len()<=512*1024 {
                    let _=write!(stream,"HTTP/1.1 {} Native\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",reply.0,body.len());
                    let _=stream.write_all(&body);
                }
            }
        })?;
    }
    *pool = Some(sender.clone());
    Ok(sender)
}
pub(super) fn hook_command(port: u16) -> Result<String> {
    ensure!(port != 0, "Invalid Cursor hook transport port");
    Ok(include_str!("cursor/hook.ps1").replace("__ION_HOOK_PORT__", &port.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(windows)]
    fn cursor_hook_cache_refuses_paths_that_windows_cannot_launch_from() {
        let cfg =
            AcpAgentConfig::new("unused").env("CURSOR_DATA_DIR", format!("C:/{}", "x".repeat(260)));
        assert!(cache_root(Path::new("C:/workspace"), &cfg).is_err());
        let cfg = AcpAgentConfig::new("unused").env("CURSOR_DATA_DIR", "C:/private");
        assert_eq!(
            cache_root(Path::new("C:/Workspace/研究/Task"), &cfg).unwrap(),
            PathBuf::from("C:/private/projects/C-Workspace-Task")
        );
    }
    #[test]
    fn cursor_hook_command_keeps_jsonc_comment_markers_out_of_private_transport() {
        let command = hook_command(12345).unwrap();
        assert!(!command.contains("//"));
        assert!(!command.contains("/*"));
        assert!(!command.contains("__ION_HOOK_"));
        assert!(command.contains("'127.0.0.1', 12345"));
        assert!(command.contains("$IonHandler.UseProxy = $false"));
        assert!(command.contains("$IonHandler.AllowAutoRedirect = $false"));
        assert!(command.contains("exit 0"));
    }
    #[test]
    fn cursor_hook_command_keeps_transport_credentials_out_of_configuration() {
        assert!(hook_command(0).is_err());
        let command = hook_command(12345).unwrap();
        assert!(command.contains("GetEnvironmentVariable('ION_CURSOR_TEAM_HOOK_TOKEN')"));
        assert!(command.contains("'Bearer', $IonToken"));
        assert!(!command.contains("__ION_HOOK_TOKEN__"));
    }
}

#[cfg(all(test, windows))]
#[path = "cursor/native_tests.rs"]
mod native_tests;

#[cfg(all(test, windows))]
#[path = "cursor/worker_tests.rs"]
mod worker_tests;
