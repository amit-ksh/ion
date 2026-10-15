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
    /// Pi and Oh My Pi load only this session-scoped extension. The native
    /// account, model, settings and ordinary tools remain owned by their RPC.
    pub(crate) fn configure_pi(
        &self,
        command: &mut std::process::Command,
        flavor: &str,
    ) -> Result<()> {
        ensure!(matches!(flavor, "pi" | "ohmypi"), "Invalid Pi dialect");
        let source = self.pi_extension(flavor);
        let path = self.0.directory.0.join("ion-teams.ts");
        std::fs::write(&path, source)?;
        command.arg("--extension").arg(path);
        Ok(())
    }
    fn pi_extension(&self, flavor: &str) -> String {
        let definitions = super::tools().as_array().unwrap().iter().map(|tool| {
            let f = &tool["function"];
            json!({"name":f["name"],"description":f["description"],"parameters":f["parameters"]})
        }).collect::<Vec<_>>();
        format!(
            r#"const endpoint = {endpoint};
const credential = {credential};
const flavor = {flavor};
const definitions = {definitions};
export default function (pi) {{
  for (const def of definitions) pi.registerTool({{
    name: 'ion_teams_' + def.name, label: def.name.replaceAll('_', ' '),
    description: def.description, parameters: def.parameters,
    async execute(call, args, signal, _update, ctx) {{
      const session = ctx?.sessionManager?.getSessionId();
      if (!session || typeof call !== 'string' || !call || signal?.aborted)
        throw new Error('Ion Teams requires a live native source call.');
      const response = await fetch(endpoint, {{method:'POST', headers:{{'Content-Type':'application/json', Authorization:'Bearer ' + credential}},
        body:JSON.stringify({{session_id:flavor + ':' + session, message_id:call, call_id:call, name:def.name, arguments:args}}), signal}});
      const reply = await response.json();
      if (!response.ok || !reply.ok) throw new Error(reply.text || 'Ion team request did not complete.');
      return {{content:[{{type:'text',text:reply.text}}], details:{{}}}};
    }}
  }});
}}
"#,
            endpoint = json!(format!("http://127.0.0.1:{}/call", self.0.port)),
            credential = json!(self.0.token),
            flavor = json!(flavor),
            definitions = json!(definitions),
        )
    }
    /// Add one temporary Cordis overlay; retain the native profile and login.
    pub(crate) fn configure_deepseek(&self, command: &mut std::process::Command) -> Result<()> {
        let plugin = self.0.directory.0.join("ion-deepseek-teams.mjs");
        let patch = self.0.directory.0.join("ion-deepseek-teams.patch.json");
        std::fs::write(&plugin, self.deepseek_plugin())?;
        let uri = url::Url::from_file_path(&plugin)
            .map_err(|_| anyhow::anyhow!("Invalid DeepSeek plugin path"))?;
        std::fs::write(
            &patch,
            serde_json::to_vec(
                &json!([{"insert":[{"id":"ion-source-teams","name":uri.as_str(),"inject":["tools"]}]}]),
            )?,
        )?;
        command.arg("--patch").arg(patch);
        command.env(
            "ION_DEEPSEEK_TEAM_ENDPOINT",
            format!("http://127.0.0.1:{}/call", self.0.port),
        );
        command.env("ION_DEEPSEEK_TEAM_TOKEN", &self.0.token);
        Ok(())
    }
    fn deepseek_plugin(&self) -> String {
        let definitions = super::tools()
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["function"].clone())
            .collect::<Vec<_>>();
        format!(
            r#"const definitions = {definitions};
export const name = 'ion-source-team-tools';
export const inject = ['tools'];
export function apply(ctx) {{
  const endpoint = process.env.ION_DEEPSEEK_TEAM_ENDPOINT;
  const credential = process.env.ION_DEEPSEEK_TEAM_TOKEN;
  if (!endpoint || !credential) return;
  for (const def of definitions) ctx.tools.register({{
    name: 'ion_teams_' + def.name, description: def.description, parameters: def.parameters,
    output: {{schema: {{type:'string'}}, render(_args, text) {{ return [{{type:'text',text}}]; }} }},
    timeoutMs: 360000,
    async execute(args, exec) {{
      const session = exec.agent?.session?.id;
      const call = exec.callId;
      if (typeof session !== 'string' || !session || typeof call !== 'string' || !call ||
          exec.parent !== undefined || exec.rootCallId !== call || exec.signal.aborted)
        throw new Error('Ion Teams requires a live main-session model tool call.');
      const response = await fetch(endpoint, {{method:'POST', headers:{{'Content-Type':'application/json',Authorization:'Bearer ' + credential}},
        body:JSON.stringify({{session_id:'deepseek:' + session,message_id:call,call_id:call,name:def.name,arguments:args}}),
        signal:AbortSignal.any([exec.signal,AbortSignal.timeout(340000)])}});
      const reply = await response.json();
      if (!response.ok || !reply.ok) throw new Error(reply.text || 'Ion team request did not complete.');
      return reply.text;
    }}
  }});
}}
"#,
            definitions = json!(definitions)
        )
    }
    pub(crate) fn native_binding(&self, session: &str, generation: &str) -> Value {
        json!({"endpoint":format!("http://127.0.0.1:{}/call",self.0.port),"credential":self.0.token,"session":session,"generation":generation})
    }
    pub(crate) fn configure_amp(&self, command: &mut std::process::Command, prefix: &str) {
        command
            .env(
                "ION_AMP_TEAM_ENDPOINT",
                format!("http://127.0.0.1:{}/call", self.0.port),
            )
            .env("ION_AMP_TEAM_TOKEN", &self.0.token)
            .env("ION_AMP_TEAM_PREFIX", prefix);
    }
    pub(crate) fn amp_plugin(&self, prefix: &str) -> String {
        let definitions=super::tools().as_array().unwrap().iter().map(|tool|{let f=&tool["function"];json!({"name":f["name"],"description":f["description"],"parameters":f["parameters"]})}).collect::<Vec<_>>();
        format!(
            r#"const endpoint=process.env.ION_AMP_TEAM_ENDPOINT;
const credential=process.env.ION_AMP_TEAM_TOKEN;
const prefix={prefix};
const definitions={definitions};
export default function(amp) {{
  if(!endpoint || !credential || process.env.ION_AMP_TEAM_PREFIX!==prefix) return;
  for(const def of definitions) amp.registerTool({{name:def.name,description:def.description,inputSchema:def.parameters,
    async execute() {{ throw new Error('Ion Teams requires its native call hook.'); }} }});
  amp.on('tool.call',async(event)=>{{
    const def=definitions.find(def=>event.tool===prefix+def.name);
    if(!def) return {{action:'allow'}};
    if(!event.thread?.id || !event.toolUseID) return {{action:'reject-and-continue',message:'Native team call identity is missing'}};
    try {{
      const response=await fetch(endpoint,{{method:'POST',headers:{{'Content-Type':'application/json',Authorization:'Bearer '+credential}},
        body:JSON.stringify({{session_id:'amp:'+event.thread.id,message_id:event.toolUseID,call_id:event.toolUseID,name:def.name,arguments:event.input}}),signal:AbortSignal.timeout(340000)}});
      const reply=await response.json();
      if(!response.ok || !reply.ok) return {{action:'reject-and-continue',message:reply.text||'Ion team call did not complete'}};
      return {{action:'synthesize',result:{{output:reply.text}}}};
    }} catch {{return {{action:'reject-and-continue',message:'Ion team call stopped or unavailable. No invocation is automatically retried.'}};}}
  }});
}}
"#,
            prefix = json!(prefix),
            definitions = json!(definitions)
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
        let bridge = Bridge::start(context, session.clone(), events)?;
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
    pub(crate) fn start(
        context: WorkspaceContext,
        session: String,
        events: DriverEventSender,
    ) -> Result<Self> {
        ensure!(valid_id(&session), "Invalid native session");
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
        Ok(bridge)
    }
    pub(crate) fn witness_native(&self, call: &str, name: &str, arguments: Value) {
        self.observe(&json!({"type":"message.updated","properties":{"info":{"sessionID":self.0.session,"id":call,"role":"assistant"}}}));
        self.observe(&json!({"type":"message.part.updated","properties":{"part":{"sessionID":self.0.session,"messageID":call,"callID":call,"type":"tool","tool":format!("{PREFIX}{name}"),"state":{"status":"running","input":arguments}}}}));
    }
    pub(crate) fn invoke_native(&self, call: &str, name: &str, arguments: Value) -> Result<Value> {
        self.0.handle(Request {
            session_id: self.0.session.clone(),
            message_id: call.into(),
            call_id: call.into(),
            name: name.into(),
            arguments,
        })
    }
    pub(crate) fn fail_native(&self, call: &str) {
        let id = identity(&self.0.session, call, call);
        if let Some(witness) = self.0.state.lock().witnesses.get_mut(&id) {
            witness.valid = false;
            witness.cancelled.store(true, Ordering::Release);
        }
    }
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
    /// A native RPC tool execution start is the authority for Pi's extension
    /// callback. The loopback token alone never registers a call. Pi has no
    /// assistant message ID, so its stable native toolCallId fills both slots.
    pub(crate) fn observe_pi(&self, event: &Value, settled_event: &str) {
        match event["type"].as_str() {
            Some("tool_execution_start") => {
                let Some(call) = event["toolCallId"].as_str().filter(|id| valid_id(id)) else {
                    return;
                };
                let Some(name) = event["toolName"]
                    .as_str()
                    .filter(|name| name.starts_with(PREFIX))
                else {
                    return;
                };
                self.observe(&json!({"type":"message.updated","properties":{"info":{"sessionID":self.0.session,"role":"assistant","id":call}}}));
                self.observe(&json!({"type":"message.part.updated","properties":{"part":{"sessionID":self.0.session,"messageID":call,"callID":call,"type":"tool","tool":name,"state":{"status":"running","input":event["args"]}}}}));
            }
            Some("tool_execution_end") if event["isError"] == true => {
                let Some(call) = event["toolCallId"].as_str() else {
                    return;
                };
                let id = identity(&self.0.session, call, call);
                let mut state = self.0.state.lock();
                if let Some(witness) = state.witnesses.get_mut(&id) {
                    witness.valid = false;
                    witness.cancelled.store(true, Ordering::Release);
                }
            }
            Some(kind) if kind == settled_event && event["isTerminal"] != false => self.cancel(),
            _ => {}
        }
    }
    /// Only the driver's live main-session downlink reaches this observer.
    /// History reads are never used as authorization for a tool callback.
    pub(crate) fn observe_deepseek(&self, envelope: &Value) {
        let payload = envelope.get("payload").unwrap_or(envelope);
        if payload["sessionId"]
            .as_str()
            .map(|id| format!("deepseek:{id}"))
            != Some(self.0.session.clone())
        {
            return;
        }
        if payload["type"] == "session/event" {
            let event = &payload["event"];
            let data = &event["data"];
            match event["type"].as_str() {
                Some("tool/call") => {
                    let arguments = data["arguments"]
                        .as_str()
                        .and_then(|text| serde_json::from_str::<Value>(text).ok())
                        .unwrap_or_else(|| data["arguments"].clone());
                    self.observe_pi(&json!({"type":"tool_execution_start","toolCallId":data["callId"],"toolName":data["name"],"args":arguments}),"deepseek-end");
                }
                Some("tool/result")
                    if data.get("error").is_some_and(|error| !error.is_null())
                        || data.pointer("/message/isError") == Some(&Value::Bool(true))
                        || data.pointer("/message/content/0/isError")
                            == Some(&Value::Bool(true)) =>
                {
                    let call = data
                        .pointer("/message/toolCallId")
                        .or_else(|| data.pointer("/message/source/callId"))
                        .or_else(|| data.pointer("/message/content/0/toolCallId"))
                        .cloned()
                        .unwrap_or(Value::Null);
                    self.observe_pi(
                        &json!({"type":"tool_execution_end","toolCallId":call,"isError":true}),
                        "deepseek-end",
                    );
                }
                Some("turn/end") => self.cancel(),
                _ => {}
            }
        }
    }
    pub(crate) fn observe_amp(&self, event: &Value, prefix: &str) {
        if event["session_id"].as_str().map(|id| format!("amp:{id}"))
            != Some(self.0.session.clone())
            || !event["parent_tool_use_id"].is_null()
        {
            return;
        }
        if event["type"] == "assistant" {
            for block in event["message"]["content"].as_array().into_iter().flatten() {
                let Some(name) = block["name"]
                    .as_str()
                    .and_then(|n| n.strip_prefix(prefix))
                    .filter(|n| super::accepts(n))
                else {
                    continue;
                };
                if block["type"] == "tool_use" {
                    self.observe_pi(&json!({"type":"tool_execution_start","toolCallId":block["id"],"toolName":format!("{PREFIX}{name}"),"args":block["input"]}),"amp-end");
                }
            }
            if event["message"]["stop_reason"] == "end_turn" {
                self.cancel();
            }
        } else if event["type"] == "user" {
            for block in event["message"]["content"].as_array().into_iter().flatten() {
                if block["type"] == "tool_result" && block["is_error"] == true {
                    self.observe_pi(&json!({"type":"tool_execution_end","toolCallId":block["tool_use_id"],"isError":true}),"amp-end");
                }
            }
        } else if event["type"] == "result" && event["is_error"] == true {
            self.cancel();
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
    let (method, path, body) = read_native_request(stream, hub.port, &hub.token)?;
    ensure!(method == "POST" && path == "/call", "Invalid route");
    Ok(serde_json::from_value(body)?)
}
pub(super) fn read_native_request(
    stream: &mut TcpStream,
    port: u16,
    token: &str,
) -> Result<(String, String, Value)> {
    // Accepted sockets can inherit nonblocking mode on Windows. Reads belong
    // to bounded background handlers and must honor the timeout below.
    stream.set_nonblocking(false)?;
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
    let first = lines.next().context("Missing request line")?;
    let parts = first.split(' ').collect::<Vec<_>>();
    ensure!(
        parts.len() == 3
            && parts[2] == "HTTP/1.1"
            && (parts[0] == "POST" || parts[0] == "GET" && parts[1] == "/mcp"),
        "Invalid route"
    );
    let method = parts[0].to_owned();
    let path = parts[1].to_owned();
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
        headers.get("host") == Some(&format!("127.0.0.1:{port}").as_str())
            && headers.get("authorization") == Some(&format!("Bearer {token}").as_str())
            && (method == "GET" || headers.get("content-type") == Some(&"application/json")),
        "Invalid loopback authorization"
    );
    if method == "GET" {
        ensure!(
            headers
                .get("content-length")
                .is_none_or(|length| *length == "0"),
            "GET body is unsupported"
        );
        return Ok((method, path, Value::Null));
    }
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
    Ok((method, path, serde_json::from_slice(&bytes)?))
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

    include!("opencode2_native_tests.rs");

    fn oc2_event(kind: &str, session: &str, data: Value) -> Value {
        let mut data = data;
        data["sessionID"] = json!(session);
        json!({"type":kind,"data":data})
    }
    fn oc2_witness(observer: &super::super::opencode2::Observer, call: &str, args: Value) {
        observer.observe(&oc2_event(
            "session.step.started",
            "native",
            json!({"assistantMessageID":"message"}),
        ));
        observer.observe(&oc2_event(
            "session.tool.input.started",
            "native",
            json!({"assistantMessageID":"message","id":call,"name":"ion_fixture_invoke_team"}),
        ));
        observer.observe(&oc2_event(
            "session.tool.input.ended",
            "native",
            json!({"assistantMessageID":"message","id":call,"text":args.to_string()}),
        ));
    }
    #[test]
    fn opencode2_source_team_live_identity_approval_replay_and_stop() {
        let f = Fixture::new();
        let hub = Hub::start().unwrap();
        let (bridge, events) = f.bridge(&hub, "opencode2:native");
        let observer =
            super::super::opencode2::Observer::new(bridge.clone(), "native", "ion_fixture_");
        observer.allow_turn();
        oc2_witness(&observer, "call", f.args());
        let calling = bridge.clone();
        let args = f.args();
        let queued = thread::spawn(move || {
            calling.0.handle(request(
                "opencode2:native",
                "message",
                "call",
                "invoke_team",
                args,
            ))
        });
        let approval = permission(&events);
        assert!(observer.respond(&approval, "allow"));
        assert_eq!(queued.join().unwrap().unwrap()["ok"], true);
        assert_eq!(f.runs().len(), 1);
        observer.close();
        let (resumed, events) = f.bridge(&hub, "opencode2:native");
        let observer =
            super::super::opencode2::Observer::new(resumed.clone(), "native", "ion_fixture_");
        observer.allow_turn();
        oc2_witness(&observer, "call", f.args());
        assert_eq!(
            resumed
                .0
                .handle(request(
                    "opencode2:native",
                    "message",
                    "call",
                    "invoke_team",
                    f.args()
                ))
                .unwrap()["ok"],
            true
        );
        assert!(
            events
                .try_iter()
                .all(|event| !matches!(event, DriverEvent::Permission { .. }))
        );
        assert_eq!(f.runs().len(), 1);
        observer.cancel();
        observer.observe(&oc2_event("session.execution.started", "native", json!({})));
        observer.allow_turn();
        oc2_witness(&observer, "call", f.args());
        assert!(
            resumed
                .0
                .handle(request(
                    "opencode2:native",
                    "message",
                    "call",
                    "invoke_team",
                    f.args()
                ))
                .is_err()
        );
    }
    #[test]
    fn opencode2_source_team_foreign_partial_changed_failed_and_terminal_calls_refuse() {
        for kind in [
            "session.tool.failed",
            "session.execution.succeeded",
            "session.execution.failed",
            "session.execution.interrupted",
            "session.deleted",
            "session.idle",
        ] {
            let f = Fixture::new();
            let hub = Hub::start().unwrap();
            let (bridge, _) = f.bridge(&hub, "opencode2:native");
            let observer =
                super::super::opencode2::Observer::new(bridge.clone(), "native", "ion_fixture_");
            observer.allow_turn();
            observer.observe(&oc2_event(
                "session.step.started",
                "foreign",
                json!({"assistantMessageID":"message"}),
            ));
            observer.observe(&oc2_event("session.tool.input.started","foreign",json!({"assistantMessageID":"message","id":"call","name":"ion_fixture_invoke_team"})));
            assert!(bridge.0.state.lock().messages.is_empty());
            observer.observe(&oc2_event("session.tool.called","native",json!({"assistantMessageID":"message","id":"call","input":f.args(),"executed":true})));
            assert!(bridge.0.state.lock().witnesses.is_empty());
            oc2_witness(&observer, "call", f.args());
            observer.observe(&oc2_event(
                kind,
                "native",
                json!({"assistantMessageID":"message","id":"call"}),
            ));
            assert!(
                bridge
                    .0
                    .handle(request(
                        "opencode2:native",
                        "message",
                        "call",
                        "invoke_team",
                        f.args()
                    ))
                    .is_err()
            );
            assert!(f.runs().is_empty());
        }
        let f = Fixture::new();
        let hub = Hub::start().unwrap();
        let (bridge, _) = f.bridge(&hub, "opencode2:native");
        let observer =
            super::super::opencode2::Observer::new(bridge.clone(), "native", "ion_fixture_");
        observer.allow_turn();
        oc2_witness(&observer, "changed", f.args());
        let mut args = f.args();
        args["objective"] = json!("changed scope");
        observer.observe(&oc2_event(
            "session.tool.called",
            "native",
            json!({"assistantMessageID":"message","id":"changed","input":args,"executed":true}),
        ));
        assert!(
            bridge
                .0
                .handle(request(
                    "opencode2:native",
                    "message",
                    "changed",
                    "invoke_team",
                    f.args()
                ))
                .is_err()
        );
        oc2_witness(&observer, "declined", f.args());
        observer.observe(&oc2_event("session.tool.called","native",json!({"assistantMessageID":"message","id":"declined","input":f.args(),"executed":true})));
        assert!(
            bridge
                .0
                .handle(request(
                    "opencode2:native",
                    "message",
                    "declined",
                    "invoke_team",
                    f.args()
                ))
                .is_err()
        );
        assert!(f.runs().is_empty());
    }
    fn deepseek_frame(session: &str, kind: &str, data: Value) -> Value {
        json!({"payload":{"type":"session/event","sessionId":session,"event":{"seq":1,"type":kind,"data":data}}})
    }
    #[test]
    fn deepseek_source_team_live_calls_preserve_owner_approval_replay_and_stop() {
        let f = Fixture::new();
        let hub = Hub::start().unwrap();
        let (bridge, events) = f.bridge(&hub, "deepseek:native-session");
        let call = deepseek_frame(
            "native-session",
            "tool/call",
            json!({"callId":"call-dsh","name":"ion_teams_invoke_team","arguments":f.args().to_string()}),
        );
        bridge.observe_deepseek(&call);
        let invoking = bridge.clone();
        let args = f.args();
        let worker = thread::spawn(move || {
            invoking.0.handle(request(
                "deepseek:native-session",
                "call-dsh",
                "call-dsh",
                "invoke_team",
                args,
            ))
        });
        let approval = permission(&events);
        bridge.respond(&approval, "allow");
        assert_eq!(worker.join().unwrap().unwrap()["ok"], true);
        assert_eq!(f.runs().len(), 1);
        bridge.close();
        let (resumed, events) = f.bridge(&hub, "deepseek:native-session");
        resumed.observe_deepseek(&call);
        assert_eq!(
            resumed
                .0
                .handle(request(
                    "deepseek:native-session",
                    "call-dsh",
                    "call-dsh",
                    "invoke_team",
                    f.args()
                ))
                .unwrap()["ok"],
            true
        );
        assert!(
            events
                .try_iter()
                .all(|event| !matches!(event, DriverEvent::Permission { .. }))
        );
        assert_eq!(f.runs().len(), 1);
        resumed.cancel();
        resumed.allow_turn();
        assert!(
            resumed
                .0
                .handle(request(
                    "deepseek:native-session",
                    "call-dsh",
                    "call-dsh",
                    "invoke_team",
                    f.args()
                ))
                .is_err()
        );
    }
    #[test]
    fn deepseek_source_team_foreign_arguments_failed_results_and_end_revoke_callbacks() {
        let f = Fixture::new();
        let hub = Hub::start().unwrap();
        let (bridge, _) = f.bridge(&hub, "deepseek:native-session");
        let frame = deepseek_frame(
            "foreign-session",
            "tool/call",
            json!({"callId":"foreign","name":"ion_teams_invoke_team","arguments":f.args()}),
        );
        bridge.observe_deepseek(&frame);
        assert!(bridge.0.state.lock().witnesses.is_empty());
        bridge.observe_deepseek(&deepseek_frame(
            "native-session",
            "tool/ptc-dispatch-start",
            json!({"subCallId":"nested","name":"ion_teams_invoke_team","arguments":f.args()}),
        ));
        assert!(bridge.0.state.lock().witnesses.is_empty());
        let frame = deepseek_frame(
            "native-session",
            "tool/call",
            json!({"callId":"changed","name":"ion_teams_invoke_team","arguments":f.args()}),
        );
        bridge.observe_deepseek(&frame);
        let mut changed = frame.clone();
        changed["payload"]["event"]["data"]["arguments"]["objective"] = json!("Changed scope");
        bridge.observe_deepseek(&changed);
        assert!(
            bridge
                .0
                .handle(request(
                    "deepseek:native-session",
                    "changed",
                    "changed",
                    "invoke_team",
                    f.args()
                ))
                .is_err()
        );
        bridge.observe_deepseek(&deepseek_frame(
            "native-session",
            "tool/call",
            json!({"callId":"failed","name":"ion_teams_get_team_runs","arguments":{}}),
        ));
        bridge.observe_deepseek(&deepseek_frame(
            "native-session",
            "tool/result",
            json!({"message":{"toolCallId":"failed","isError":true,"content":[]}}),
        ));
        assert!(
            bridge
                .0
                .handle(request(
                    "deepseek:native-session",
                    "failed",
                    "failed",
                    "get_team_runs",
                    json!({})
                ))
                .is_err()
        );
        bridge.observe_deepseek(&deepseek_frame("native-session", "turn/end", json!({})));
        assert!(
            bridge
                .0
                .handle(request(
                    "deepseek:native-session",
                    "changed",
                    "changed",
                    "invoke_team",
                    f.args()
                ))
                .is_err()
        );
        assert!(f.runs().is_empty());
    }
    #[test]
    fn deepseek_source_team_overlay_is_additive_and_has_no_live_credentials() {
        let hub = Hub::start().unwrap();
        let mut command = std::process::Command::new("native-dsh");
        command.args(["--profile", "existing-native-profile"]);
        hub.configure_deepseek(&mut command).unwrap();
        let args = command
            .get_args()
            .map(|v| v.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            &args[..3],
            ["--profile", "existing-native-profile", "--patch"]
        );
        let patch: Value = serde_json::from_slice(&std::fs::read(&args[3]).unwrap()).unwrap();
        let uri = url::Url::parse(patch[0]["insert"][0]["name"].as_str().unwrap()).unwrap();
        let source = std::fs::read_to_string(uri.to_file_path().unwrap()).unwrap();
        assert!(!source.contains(&hub.0.token));
        assert!(!source.contains(&format!("127.0.0.1:{}", hub.0.port)));
        assert_eq!(patch[0]["insert"].as_array().unwrap().len(), 1);
        assert!(
            command
                .get_envs()
                .any(|(key, value)| key == "ION_DEEPSEEK_TEAM_TOKEN"
                    && value == Some(std::ffi::OsStr::new(&hub.0.token)))
        );
    }
    #[test]
    #[ignore = "Requires verified native DeepSeek runtime; private fake profile and loopback only"]
    fn deepseek_source_team_generated_plugin_calls_owner_through_actual_native_runtime() {
        use std::io::{BufRead, BufReader};
        use std::process::{Child, Stdio};
        let binary = std::env::var_os("ION_TEST_DEEPSEEK_BINARY")
            .map(std::path::PathBuf::from)
            .expect("Set ION_TEST_DEEPSEEK_BINARY");
        let f = Fixture::new();
        let hub = Hub::start().unwrap();
        let mut stale_args = f.args();
        stale_args["definition_revision"] = json!(0);
        let wire=[
            json!({"id":"native-tool","object":"chat.completion.chunk","model":"deepseek-v4-flash","choices":[{"index":0,"delta":{"role":"assistant","tool_calls":[{"index":0,"id":"call_dsh_native","type":"function","function":{"name":"ion_teams_list_teams","arguments":"{}"}},{"index":1,"id":"call_dsh_invoke","type":"function","function":{"name":"ion_teams_invoke_team","arguments":f.args().to_string()}},{"index":2,"id":"call_dsh_stale","type":"function","function":{"name":"ion_teams_invoke_team","arguments":stale_args.to_string()}}]},"finish_reason":null}]}),
            json!({"id":"native-tool","object":"chat.completion.chunk","model":"deepseek-v4-flash","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}),
            json!({"id":"native-tool","object":"chat.completion.chunk","model":"deepseek-v4-flash","choices":[],"usage":{"prompt_tokens":100,"completion_tokens":5,"total_tokens":105}}),
        ].iter().map(|row|format!("data: {row}\n\n")).collect::<String>()+"data: [DONE]\n\n";
        let (ready, readiness) = bounded::<()>(1);
        let first_request = Arc::new(AtomicBool::new(true));
        let server = crate::driver::team_test_http::Server::with_hook(
            vec![
                (200, wire.into_bytes()),
                (
                    200,
                    crate::driver::team_test_http::chat("deepseek-v4-flash", "Done"),
                ),
            ],
            move |_| {
                if first_request.swap(false, Ordering::AcqRel) {
                    readiness
                        .recv_timeout(Duration::from_secs(10))
                        .expect("owner observed the native main session before tool response");
                }
            },
        );
        let (_profile, mut command) =
            crate::driver::team_deepseek::source_tools_fixture_command(&binary, &server.url)
                .unwrap();
        hub.configure_deepseek(&mut command).unwrap();
        command
            .arg("Use the native Ion team catalog tool.")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        struct Running(
            Child,
            #[cfg(windows)] Option<crate::command_env::ProcessTree>,
        );
        impl Drop for Running {
            fn drop(&mut self) {
                #[cfg(windows)]
                drop(self.1.take());
                #[cfg(unix)]
                unsafe {
                    libc::kill(-(self.0.id() as i32), libc::SIGKILL);
                }
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let child = crate::command_env::spawn(&mut command).unwrap();
        #[cfg(windows)]
        let tree = crate::command_env::ProcessTree::attach(&child).unwrap();
        let mut child = Running(
            child,
            #[cfg(windows)]
            Some(tree),
        );
        let output = child.0.stdout.take().unwrap();
        let stderr = child.0.stderr.take().unwrap();
        let diagnostics = thread::spawn(move || {
            use std::io::Read;
            let mut bytes = Vec::new();
            let _ = stderr.take(16 * 1024).read_to_end(&mut bytes);
            bytes
        });
        let (sender, frames) = crossbeam_channel::unbounded();
        let reader = thread::spawn(move || {
            use std::io::Read;
            for line in BufReader::new(output.take(1024 * 1024)).lines().take(200) {
                let _ = sender.send(line.unwrap());
            }
        });
        let deadline = Instant::now() + Duration::from_secs(60);
        let mut bridge: Option<Bridge> = None;
        let mut session = String::new();
        let mut tool_returned = false;
        let mut finished = false;
        let mut approved = None;
        let mut failures = HashSet::<String>::new();
        let mut failed_result = false;
        while let Ok(line) = frames.recv_timeout(deadline.saturating_duration_since(Instant::now()))
        {
            if line.trim() == "Done" {
                finished = true;
                continue;
            }
            let Some(raw) = line.strip_prefix("__ION_DSH_EVENT__") else {
                continue;
            };
            let row: Value = serde_json::from_str(raw).unwrap();
            let native_session = row["sessionId"].as_str().unwrap();
            if bridge.is_none() {
                session = native_session.into();
                let (native_bridge, events) = f.bridge(&hub, &format!("deepseek:{session}"));
                let authorizing = native_bridge.clone();
                approved = Some(thread::spawn(move || {
                    let approval = permission(&events);
                    authorizing.respond(&approval, "allow");
                }));
                bridge = Some(native_bridge);
                ready.try_send(()).unwrap();
            }
            assert_eq!(native_session, session);
            bridge.as_ref().unwrap().observe_deepseek(&json!({"payload":{"type":"session/event","sessionId":session,"event":row["event"]}}));
            if row["event"]["type"] == "tool/call" {
                let data = &row["event"]["data"];
                let args = data["arguments"]
                    .as_str()
                    .and_then(|text| serde_json::from_str::<Value>(text).ok())
                    .unwrap_or_else(|| data["arguments"].clone());
                if args["definition_revision"].as_u64() == Some(0) {
                    failures.insert(data["callId"].as_str().unwrap().into());
                }
            }
            if row["event"]["type"] == "tool/result" {
                let data = &row["event"]["data"];
                let call = data
                    .pointer("/message/toolCallId")
                    .or_else(|| data.pointer("/message/source/callId"))
                    .or_else(|| data.pointer("/message/content/0/toolCallId"))
                    .and_then(Value::as_str)
                    .unwrap();
                if failures.contains(call) {
                    let state = bridge.as_ref().unwrap().0.state.lock();
                    assert!(
                        state.witnesses[&identity(&format!("deepseek:{session}"), call, call)]
                            .cancelled
                            .load(Ordering::Acquire)
                    );
                    failed_result = true;
                } else if data.to_string().contains("Engineering") {
                    tool_returned = true;
                }
            }
        }
        while child.0.try_wait().unwrap().is_none() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        let status = child.0.try_wait().unwrap();
        if status.is_none() {
            panic!("Native DeepSeek fixture timed out");
        }
        reader.join().unwrap();
        let diagnostics = diagnostics.join().unwrap();
        assert!(
            status.unwrap().success(),
            "Native fixture failed: {}",
            String::from_utf8_lossy(&diagnostics)
        );
        assert!(tool_returned && finished && failed_result);
        approved.unwrap().join().unwrap();
        assert_eq!(f.runs().len(), 1);
        let requests = server.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        let tools = requests[0].body["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 5);
        assert!(tools.iter().all(|tool| {
            tool["function"]["name"]
                .as_str()
                .unwrap()
                .starts_with("ion_teams_")
        }));
        assert!(
            requests[1].body["messages"]
                .as_array()
                .unwrap()
                .iter()
                .any(|message| message["role"] == "tool"
                    && message["content"]
                        .as_str()
                        .is_some_and(|text| text.contains("Engineering")))
        );
    }
    #[test]
    fn pi_source_team_native_calls_replay_and_fence_stop_for_both_dialects() {
        for (flavor, settled) in [("pi", "agent_settled"), ("ohmypi", "agent_end")] {
            let f = Fixture::new();
            let hub = Hub::start().unwrap();
            let session = format!("{flavor}:native");
            let (bridge, events) = f.bridge(&hub, &session);
            let witness = json!({"type":"tool_execution_start","toolCallId":"call_native","toolName":"ion_teams_invoke_team","args":f.args()});
            bridge.observe_pi(&witness, settled);
            let calling = bridge.clone();
            let args = f.args();
            let peer_session = session.clone();
            let queued = thread::spawn(move || {
                calling.0.handle(request(
                    &peer_session,
                    "call_native",
                    "call_native",
                    "invoke_team",
                    args,
                ))
            });
            let approval = permission(&events);
            bridge.respond(&approval, "allow");
            assert_eq!(queued.join().unwrap().unwrap()["ok"], true);
            assert_eq!(f.runs().len(), 1);
            bridge.close();
            let (resumed, events) = f.bridge(&hub, &session);
            resumed.observe_pi(&witness, settled);
            let reply = resumed
                .0
                .handle(request(
                    &session,
                    "call_native",
                    "call_native",
                    "invoke_team",
                    f.args(),
                ))
                .unwrap();
            assert_eq!(reply["ok"], true);
            assert_eq!(f.runs().len(), 1);
            assert!(
                !events
                    .try_iter()
                    .any(|e| matches!(e, DriverEvent::Permission { .. }))
            );
            resumed.observe_pi(&json!({"type":settled,"isTerminal":false}), settled);
            assert!(!resumed.0.state.lock().blocked);
            resumed.observe_pi(&json!({"type":settled}), settled);
            resumed.allow_turn();
            resumed.observe_pi(&witness, settled);
            assert!(
                resumed
                    .0
                    .handle(request(
                        &session,
                        "call_native",
                        "call_native",
                        "invoke_team",
                        f.args()
                    ))
                    .is_err()
            );
            resumed.close();
        }
    }
    #[test]
    fn pi_source_team_changed_native_arguments_and_tool_failure_revoke_calls() {
        let f = Fixture::new();
        let hub = Hub::start().unwrap();
        let (bridge, _) = f.bridge(&hub, "pi:native");
        let mut witness = json!({"type":"tool_execution_start","toolCallId":"call_native","toolName":"ion_teams_get_team_runs","args":{}});
        bridge.observe_pi(&witness, "agent_settled");
        witness["args"] = json!({"injected":true});
        bridge.observe_pi(&witness, "agent_settled");
        assert!(
            bridge
                .0
                .handle(request(
                    "pi:native",
                    "call_native",
                    "call_native",
                    "get_team_runs",
                    json!({})
                ))
                .is_err()
        );
        bridge.observe_pi(&json!({"type":"tool_execution_start","toolCallId":"call_failure","toolName":"ion_teams_get_team_runs","args":{}}), "agent_settled");
        bridge.observe_pi(
            &json!({"type":"tool_execution_end","toolCallId":"call_failure","isError":true}),
            "agent_settled",
        );
        assert!(
            bridge
                .0
                .handle(request(
                    "pi:native",
                    "call_failure",
                    "call_failure",
                    "get_team_runs",
                    json!({})
                ))
                .is_err()
        );
        assert!(f.runs().is_empty());
        bridge.close();
    }
    #[test]
    #[ignore = "Requires native Pi and Oh My Pi binaries; private fake account and loopback only"]
    fn pi_source_team_installed_rpc_calls_native_extension_in_both_flavors() {
        use crate::driver::{
            DriverControl, DriverStartOptions,
            pi::{PiDriver, PiFlavor},
        };
        for flavor in [PiFlavor::Pi, PiFlavor::OhMyPi] {
            let binary = if flavor == PiFlavor::Pi {
                std::env::var_os("ION_TEST_PI_BINARY")
                    .map(std::path::PathBuf::from)
                    .or_else(|| crate::model::provider_probe(ProviderKind::Pi, None).path)
                    .unwrap()
            } else {
                std::env::var_os("ION_TEST_OMP_BINARY")
                    .map(std::path::PathBuf::from)
                    .or_else(|| crate::model::provider_probe(ProviderKind::OhMyPi, None).path)
                    .unwrap()
            };
            let f = Fixture::new();
            let mut context = f.context.clone();
            context.mode = RuntimeMode::FullAccess;
            let item = json!({"type":"function_call","id":"fc_native_1","call_id":"call_native_1","name":"ion_teams_list_teams","arguments":"{}","status":"completed"});
            let wire = [
                json!({"type":"response.created","response":{"id":"resp_tool","status":"in_progress"}}),
                json!({"type":"response.output_item.added","output_index":0,"item":{"type":"function_call","id":"fc_native_1","call_id":"call_native_1","name":"ion_teams_list_teams","arguments":""}}),
                json!({"type":"response.function_call_arguments.delta","item_id":"fc_native_1","output_index":0,"delta":"{}"}),
                json!({"type":"response.output_item.done","output_index":0,"item":item.clone()}),
                json!({"type":"response.completed","response":{"id":"resp_tool","status":"completed","model":"fixture-native","output":[item],"usage":{"input_tokens":100,"output_tokens":5,"total_tokens":105,"input_tokens_details":{"cached_tokens":0},"output_tokens_details":{"reasoning_tokens":0}}}}),
            ].iter().map(|v| format!("event: {}\ndata: {v}\n\n",v["type"].as_str().unwrap())).collect::<String>().into_bytes();
            let server = crate::driver::team_test_http::Server::with_timeout_and_optional_preflight(
                vec![
                    (200, wire),
                    (
                        200,
                        crate::driver::team_test_http::responses("fixture-native", "Done"),
                    ),
                ],
                Duration::from_secs(120),
                "GET /models HTTP/1.1",
            );
            let root = &f.directory.0;
            for name in ["home", "agent", "work"] {
                std::fs::create_dir(root.join(name)).unwrap();
            }
            let agent = root.join("agent");
            let provider = json!({"api":"openai-responses","baseUrl":server.url,"apiKey":"sk-fixture-only","models":[{"id":"fixture-native","name":"Fixture native","api":"openai-responses","baseUrl":server.url,"contextWindow":100000,"maxTokens":8192,"input":["text"],"reasoning":false}]});
            if flavor == PiFlavor::Pi {
                std::fs::write(
                    agent.join("models.json"),
                    json!({"providers":{"openai":provider}}).to_string(),
                )
                .unwrap();
                std::fs::write(
                    agent.join("auth.json"),
                    json!({"openai":{"type":"api_key","key":"sk-fixture-only"}}).to_string(),
                )
                .unwrap();
                std::fs::write(agent.join("settings.json"),json!({"retry":{"enabled":false},"compaction":{"enabled":false},"enableVersionCheck":false}).to_string()).unwrap();
            } else {
                std::fs::write(
                    agent.join("models.yml"),
                    json!({"providers":{"openai":provider}}).to_string(),
                )
                .unwrap();
                std::fs::write(agent.join("config.yml"),json!({"startup":{"checkUpdate":false,"setupWizard":false,"quiet":true},"retry":{"enabled":false,"modelFallback":false,"usageAwareFallback":false},"compaction":{"enabled":false,"midTurnEnabled":false,"asyncEnabled":false},"mcp":{"enableProjectConfig":false},"enabledProviders":["openai"],"enabledModels":["openai/fixture-native"],"modelRoles":{"default":"openai/fixture-native","smol":"openai/fixture-native","slow":"openai/fixture-native","plan":"openai/fixture-native"},"telemetry":{"otlp":{"export":{"enabled":false}}}}).to_string()).unwrap();
            }
            let (events, received) = crate::driver::test_event_channel();
            let driver = PiDriver::start_configured(
                flavor,
                DriverStartOptions {
                    binary,
                    cwd: root.join("work"),
                    mode: RuntimeMode::FullAccess,
                    model: Some("openai/fixture-native".into()),
                    reasoning_effort: None,
                    service_tier: None,
                    context_window: None,
                    agent_preset: None,
                    computer_use_enabled: false,
                    provider_cursor: None,
                },
                events,
                Some(context),
                |command| {
                    let runtime = command
                        .get_envs()
                        .filter_map(|(k, v)| v.map(|v| (k.to_os_string(), v.to_os_string())))
                        .collect::<Vec<_>>();
                    let allowed = |k: &std::ffi::OsStr| {
                        matches!(
                            k.to_string_lossy().to_ascii_uppercase().as_str(),
                            "PATH"
                                | "SYSTEMROOT"
                                | "WINDIR"
                                | "COMSPEC"
                                | "PATHEXT"
                                | "TEMP"
                                | "TMP"
                                | "TMPDIR"
                                | "LANG"
                                | "LC_ALL"
                        )
                    };
                    command
                        .env_clear()
                        .envs(std::env::vars_os().filter(|(k, _)| allowed(k)))
                        .envs(runtime.into_iter().filter(|(k, _)| allowed(k)));
                    command
                        .env("HOME", root.join("home"))
                        .env("USERPROFILE", root.join("home"))
                        .env("PI_CODING_AGENT_DIR", &agent)
                        .env("PI_CONFIG_DIR", ".omp")
                        .env("PI_SKIP_VERSION_CHECK", "1");
                    if flavor == PiFlavor::Pi {
                        command.args([
                            "--offline",
                            "--no-builtin-tools",
                            "--tools",
                            "ion_teams_list_teams,ion_teams_invoke_team,ion_teams_get_team_runs,ion_teams_await_team,ion_teams_read_team_document",
                            "--no-skills",
                            "--no-prompt-templates",
                            "--no-context-files",
                            "--no-themes",
                        ]);
                    } else {
                        command
                            .args([
                                "--no-tools",
                                "--no-lsp",
                                "--no-pty",
                                "--no-skills",
                                "--no-rules",
                                "--no-title",
                                "--no-prewalk",
                                "--config",
                            ])
                            .arg(agent.join("config.yml"));
                    }
                },
            )
            .unwrap();
            let deadline = Instant::now() + Duration::from_secs(120);
            loop {
                assert!(
                    Instant::now() < deadline,
                    "Native fixture failed to connect"
                );
                match received.recv_timeout(Duration::from_secs(5)) {
                    Ok(DriverEvent::Connected { .. }) => break,
                    Ok(DriverEvent::Error(error)) => panic!("{flavor:?} startup: {error}"),
                    _ => {}
                }
            }
            driver.prompt("Use ion_teams_list_teams then report the returned team name".into());
            let mut evidence = false;
            loop {
                assert!(Instant::now() < deadline, "Native fixture failed to settle");
                match received.recv_timeout(Duration::from_secs(5)) {
                    Ok(DriverEvent::Activity {
                        detail: Some(text),
                        complete: true,
                        ..
                    }) if text.contains("Engineering") => evidence = true,
                    Ok(DriverEvent::TurnFinished { success, .. }) => {
                        assert!(success);
                        break;
                    }
                    Ok(DriverEvent::Error(error)) => panic!("{flavor:?} source tool: {error}"),
                    _ => {}
                }
            }
            assert!(
                evidence,
                "{flavor:?}: actual native tool callback did not return saved Teams"
            );
            drop(driver);
            let requests = server.finish();
            assert!(requests.iter().all(|request| matches!(
                request.line.as_str(),
                "GET /models HTTP/1.1" | "POST /responses HTTP/1.1"
            )));
            let requests = requests
                .into_iter()
                .filter(|request| request.line == "POST /responses HTTP/1.1")
                .collect::<Vec<_>>();
            assert_eq!(requests.len(), 2);
            assert_eq!(requests[0].body["model"], "fixture-native");
            assert_eq!(requests[0].body["tools"].as_array().unwrap().len(), 5);
            assert!(
                requests[0]
                    .headers
                    .to_ascii_lowercase()
                    .contains("authorization: bearer sk-fixture-only")
            );
            let input = requests[1].body["input"].as_array().unwrap();
            assert!(input.iter().any(|i| {
                i["type"] == "function_call_output"
                    && i["output"]
                        .as_str()
                        .is_some_and(|s| s.contains("Engineering"))
            }));
            assert!(f.runs().is_empty(), "The list call should not start a team");
        }
    }
    fn amp_frame(session: &str, call: &str, name: &str, args: Value) -> Value {
        json!({"type":"assistant","session_id":session,"parent_tool_use_id":null,"message":{"content":[{"type":"tool_use","id":call,"name":format!("plugin__ion_fixture__{name}"),"input":args}],"stop_reason":"tool_use"}})
    }
    #[test]
    fn amp_source_team_native_identity_approval_replay_and_stop() {
        let f = Fixture::new();
        let hub = Hub::start().unwrap();
        let session = "amp:T-native";
        let (bridge, events) = f.bridge(&hub, session);
        bridge.observe_amp(
            &amp_frame("T-native", "toolu_1", "invoke_team", f.args()),
            "plugin__ion_fixture__",
        );
        let pending = bridge.clone();
        let args = f.args();
        let call = thread::spawn(move || {
            pending
                .0
                .handle(request(
                    "amp:T-native",
                    "toolu_1",
                    "toolu_1",
                    "invoke_team",
                    args,
                ))
                .unwrap()
        });
        let id = permission(&events);
        assert!(f.runs().is_empty());
        assert!(bridge.respond(&id, "allow"));
        assert_eq!(call.join().unwrap()["ok"], true);
        assert_eq!(f.runs().len(), 1);
        assert_eq!(
            bridge
                .0
                .handle(request(
                    session,
                    "toolu_1",
                    "toolu_1",
                    "invoke_team",
                    f.args()
                ))
                .unwrap()["ok"],
            true
        );
        let mut changed = f.args();
        changed["objective"] = json!("Changed scope");
        bridge.observe_amp(
            &amp_frame("T-native", "toolu_1", "invoke_team", changed.clone()),
            "plugin__ion_fixture__",
        );
        assert!(
            bridge
                .0
                .handle(request(
                    session,
                    "toolu_1",
                    "toolu_1",
                    "invoke_team",
                    changed
                ))
                .is_err()
        );
        bridge.observe_amp(&json!({"type":"assistant","session_id":"T-native","parent_tool_use_id":null,"message":{"content":[],"stop_reason":"end_turn"}}),"plugin__ion_fixture__");
        bridge.allow_turn();
        bridge.observe_amp(
            &amp_frame("T-native", "toolu_1", "invoke_team", f.args()),
            "plugin__ion_fixture__",
        );
        assert!(
            bridge
                .0
                .handle(request(
                    session,
                    "toolu_1",
                    "toolu_1",
                    "invoke_team",
                    f.args()
                ))
                .is_err()
        );
        bridge.close();
        let (resumed, events) = f.bridge(&hub, session);
        resumed.observe_amp(
            &amp_frame("T-native", "toolu_1", "invoke_team", f.args()),
            "plugin__ion_fixture__",
        );
        assert_eq!(
            resumed
                .0
                .handle(request(
                    session,
                    "toolu_1",
                    "toolu_1",
                    "invoke_team",
                    f.args()
                ))
                .unwrap()["ok"],
            true
        );
        assert_eq!(f.runs().len(), 1);
        assert!(
            !events
                .try_iter()
                .any(|e| matches!(e, DriverEvent::Permission { .. }))
        );
    }
    #[test]
    fn amp_source_team_foreign_thread_nested_call_wrong_plugin_and_failed_tool_refuse() {
        let f = Fixture::new();
        let hub = Hub::start().unwrap();
        let (bridge, _) = f.bridge(&hub, "amp:T-native");
        let mut frame = amp_frame("T-foreign", "toolu_1", "list_teams", json!({}));
        bridge.observe_amp(&frame, "plugin__ion_fixture__");
        assert!(bridge.0.state.lock().witnesses.is_empty());
        frame["session_id"] = json!("T-native");
        frame["parent_tool_use_id"] = json!("child");
        bridge.observe_amp(&frame, "plugin__ion_fixture__");
        assert!(bridge.0.state.lock().witnesses.is_empty());
        frame["parent_tool_use_id"] = Value::Null;
        bridge.observe_amp(&frame, "plugin__different__");
        assert!(bridge.0.state.lock().witnesses.is_empty());
        bridge.observe_amp(&frame, "plugin__ion_fixture__");
        bridge.observe_amp(&json!({"type":"user","session_id":"T-native","parent_tool_use_id":null,"message":{"content":[{"type":"tool_result","tool_use_id":"toolu_1","is_error":true}]}}),"plugin__ion_fixture__");
        assert!(
            bridge
                .0
                .handle(request(
                    "amp:T-native",
                    "toolu_1",
                    "toolu_1",
                    "list_teams",
                    json!({})
                ))
                .is_err()
        );
        assert!(f.runs().is_empty());
    }
    #[test]
    #[ignore = "Requires installed Amp binary and its unmodified extracted plugin runtime; fake profile/localhost only"]
    fn amp_source_team_generated_hook_calls_owner_through_actual_native_runtime() {
        use std::io::BufRead;
        use std::process::Stdio;
        let binary = std::path::PathBuf::from(std::env::var_os("ION_TEST_AMP_BINARY").unwrap());
        let runtime = std::path::PathBuf::from(std::env::var_os("ION_TEST_AMP_RUNTIME").unwrap());
        let f = Fixture::new();
        let hub = Hub::start().unwrap();
        let (bridge, _) = f.bridge(&hub, "amp:T-native");
        let plugin = f.directory.0.join("hook.ts");
        let source = hub.amp_plugin("plugin__ion_fixture__");
        assert!(!source.contains(&hub.0.token));
        assert!(!source.contains(&format!("127.0.0.1:{}", hub.0.port)));
        std::fs::write(&plugin, source).unwrap();
        let home = f.directory.0.join("home");
        std::fs::create_dir(&home).unwrap();
        let mut command = crate::command_env::command(&binary);
        command
            .args([runtime.as_os_str(), plugin.as_os_str()])
            .current_dir(&f.directory.0)
            .env_clear()
            .envs(std::env::vars_os().filter(|(k, _)| {
                matches!(
                    k.to_string_lossy().to_ascii_uppercase().as_str(),
                    "PATH" | "SYSTEMROOT" | "WINDIR" | "COMSPEC" | "PATHEXT" | "TEMP" | "TMP"
                )
            }))
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .env("APPDATA", &home)
            .env("LOCALAPPDATA", &home)
            .env("BUN_BE_BUN", "1")
            .env(
                "AMP_PLUGIN_URI",
                url::Url::from_file_path(&plugin).unwrap().as_str(),
            )
            .env("AMP_PLUGIN_NAME", "ion_fixture")
            .env("AMP_PLUGIN_RUNTIME_LOG_FILE", home.join("runtime.log"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        hub.configure_amp(&mut command, "plugin__ion_fixture__");
        let mut child = crate::command_env::spawn(&mut command).unwrap();
        let mut stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (sender, received) = bounded(64);
        let reader = thread::spawn(move || {
            for line in std::io::BufReader::new(stdout)
                .lines()
                .map_while(Result::ok)
            {
                if let Ok(value) = serde_json::from_str::<Value>(&line) {
                    if sender.send(value).is_err() {
                        break;
                    }
                }
            }
        });
        let write = |stdin: &mut std::process::ChildStdin, value: Value| {
            writeln!(stdin, "{value}").unwrap();
            stdin.flush().unwrap();
        };
        let response = |stdin: &mut std::process::ChildStdin,
                        id: &str,
                        method: &str,
                        params: Value|
         -> Value {
            write(
                stdin,
                json!({"type":"request","id":id,"method":method,"params":params}),
            );
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                assert!(Instant::now() < deadline, "Native Amp response missing");
                let value = received.recv_timeout(Duration::from_secs(1)).unwrap();
                if value["type"] == "request" {
                    let result = if value["method"] == "client.info" {
                        json!({"ampURL":"https://amp-fixture.invalid","workspaceRoot":url::Url::from_directory_path(&f.directory.0).unwrap().as_str(),"authenticatedUser":null,"executorKind":"local"})
                    } else {
                        Value::Null
                    };
                    write(
                        stdin,
                        json!({"type":"response","id":value["id"],"result":result}),
                    );
                }
                if value["type"] == "response" && value["id"] == id {
                    return value;
                }
            }
        };
        let mut initialized = false;
        for index in 0..40 {
            let reply = response(
                &mut stdin,
                &format!("init-{index}"),
                "events.list",
                json!({}),
            );
            if reply["result"]
                .as_array()
                .is_some_and(|events| events.iter().any(|e| e == "tool.call"))
            {
                initialized = true;
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
        assert!(initialized);
        let tools = response(&mut stdin, "tools", "tool.list", json!({}));
        assert_eq!(tools["result"].as_array().unwrap().len(), 5);
        bridge.observe_amp(
            &amp_frame("T-native", "toolu_native", "list_teams", json!({})),
            "plugin__ion_fixture__",
        );
        let args = json!({"toolUseID":"toolu_native","tool":"plugin__ion_fixture__list_teams","input":{},"thread":{"id":"T-native"}});
        let reply = response(&mut stdin, "native-call", "tool.call", args.clone());
        assert_eq!(reply["result"]["action"], "synthesize");
        assert!(
            reply["result"]["result"]["output"]
                .as_str()
                .unwrap()
                .contains("Engineering")
        );
        let replay = response(&mut stdin, "replay", "tool.call", args.clone());
        assert_eq!(replay["result"], reply["result"]);
        bridge.cancel();
        let stopped = response(&mut stdin, "stopped", "tool.call", args);
        assert_eq!(stopped["result"]["action"], "reject-and-continue");
        assert!(f.runs().is_empty());
        response(&mut stdin, "dispose", "dispose", json!({}));
        drop(stdin);
        let deadline = Instant::now() + Duration::from_secs(3);
        while child.try_wait().unwrap().is_none() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        let _ = child.kill();
        let _ = child.wait();
        drop(received);
        let _ = reader.join();
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
    fn opencode_source_team_loopback_accepts_delayed_headers_on_nonblocking_socket() {
        let hub = Hub::start().unwrap();
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (mut accepted, _) = listener.accept().unwrap();
        // Windows may inherit the listener mode. Explicitly reproduce that
        // mode on every host, with headers arriving after the first read begins.
        accepted.set_nonblocking(true).unwrap();
        let inner = hub.0.clone();
        let (started, received) = bounded(1);
        let reader = thread::spawn(move || {
            started.send(()).unwrap();
            read_request(&mut accepted, &inner)
        });
        received.recv().unwrap();
        thread::sleep(Duration::from_millis(100));
        let body = json!({"session_id":"pi:delayed","message_id":"call_1","call_id":"call_1","name":"list_teams","arguments":{}}).to_string();
        write!(client,"POST /call HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",hub.0.port,hub.0.token,body.len()).unwrap();
        let request = reader.join().unwrap().unwrap();
        assert_eq!(request.session_id, "pi:delayed");
        assert_eq!(request.name, "list_teams");
        assert_eq!(request.arguments, json!({}));
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
