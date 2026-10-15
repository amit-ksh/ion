//! OpenCode 2 native plugin execution, witnessed by the adopted main-session stream.
use super::{
    WorkspaceContext,
    amp::ProjectPlugin,
    opencode::{Bridge, Hub},
};
use crate::{driver::DriverEventSender, http_wire::Endpoint, opencode2_service::Opencode2Service};
use anyhow::{Context, Result, ensure};
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Clone)]
pub(crate) struct OpenCode2Bridge(Arc<Inner>);
struct Inner {
    hub: Hub,
    observer: Observer,
    service: Arc<Opencode2Service>,
    directory: String,
    id: String,
    generation: String,
    _plugin: ProjectPlugin,
}
fn rpc(
    endpoint: &Endpoint,
    directory: &str,
    id: &str,
    method: &str,
    input: Value,
) -> Result<Value> {
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("location[directory]", directory)
        .finish();
    let path = format!("/api/rpc/{id}/{method}?{query}");
    let result = crate::http_wire::request_json(
        endpoint,
        "POST",
        &path,
        Some(&json!({"input":input})),
        Duration::from_secs(2),
    )?;
    result
        .get("output")
        .cloned()
        .context("Native plugin RPC result is missing")
}
impl OpenCode2Bridge {
    pub(crate) fn start(
        context: WorkspaceContext,
        events: DriverEventSender,
        cwd: &Path,
        service: &Arc<Opencode2Service>,
        session: &str,
    ) -> Result<Self> {
        ensure!(
            service.version().as_deref() == Some("2.0.29"),
            "OpenCode 2 source-team tools require the verified 2.0.29 plugin runtime; native solo execution remains available"
        );
        let hub = Hub::start()?;
        let binding = hub.attach(context.clone(), format!("opencode2:{session}"), events)?;
        binding.cancel();
        let id = format!("ion-teams-{}", context.task_id.simple());
        let prefix = format!("ion_teams_{}_", context.task_id.simple());
        let plugin = ProjectPlugin::install_at(
            cwd,
            ".opencode",
            &format!("{id}.ts"),
            &plugin(&id, &prefix),
        )?;
        let bridge = Self(Arc::new(Inner {
            hub,
            observer: Observer::new(binding, session, &prefix),
            service: service.clone(),
            directory: std::fs::canonicalize(cwd)?.to_string_lossy().into_owned(),
            id,
            generation: uuid::Uuid::new_v4().to_string(),
            _plugin: plugin,
        }));
        // Discovery reloads asynchronously without restarting the adopted server.
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            match bridge.ensure_connected() {
                Ok(()) => return Ok(bridge),
                Err(error) if Instant::now() >= deadline => {
                    return Err(error.context("Native Ion team plugin did not become ready"));
                }
                Err(_) => std::thread::sleep(Duration::from_millis(150)),
            }
        }
    }
    pub(crate) fn ensure_connected(&self) -> Result<()> {
        ensure!(
            self.0.service.version().as_deref() == Some("2.0.29"),
            "Native source-team plugin version is no longer verified"
        );
        ensure!(
            !self.0.observer.closed.load(Ordering::Acquire),
            "Source bridge closed"
        );
        let reply = rpc(
            &self.0.service.endpoint(),
            &self.0.directory,
            &self.0.id,
            "bind",
            self.0
                .hub
                .native_binding(&self.0.observer.session, &self.0.generation),
        )?;
        ensure!(
            reply["ready"] == true
                && reply["generation"] == self.0.generation
                && reply["version"] == "2.0.29",
            "Native team plugin generation changed"
        );
        Ok(())
    }
    pub(crate) fn allow_turn(&self) {
        self.0.observer.allow_turn();
    }
    pub(crate) fn cancel(&self) {
        self.0.observer.cancel();
    }
    pub(crate) fn respond(&self, id: &str, option: &str) -> bool {
        self.0.observer.respond(id, option)
    }
    pub(crate) fn close(&self) {
        self.0.observer.close();
        self.0.hub.close();
    }
    pub(crate) fn observe(&self, event: &Value) {
        self.0.observer.observe(event);
    }
}
/// Live native identities can be verified independently of native discovery/HTTP.
pub(crate) struct Observer {
    binding: Bridge,
    session: String,
    prefix: String,
    names: Mutex<HashMap<(String, String), String>>,
    stopped: AtomicBool,
    closed: AtomicBool,
}
impl Observer {
    pub(crate) fn new(binding: Bridge, session: &str, prefix: &str) -> Self {
        binding.cancel();
        Self {
            binding,
            session: session.into(),
            prefix: prefix.into(),
            names: Mutex::new(HashMap::new()),
            stopped: AtomicBool::new(true),
            closed: AtomicBool::new(false),
        }
    }
    pub(crate) fn allow_turn(&self) {
        if !self.closed.load(Ordering::Acquire) {
            self.stopped.store(false, Ordering::Release);
            self.binding.allow_turn();
        }
    }
    pub(crate) fn cancel(&self) {
        self.stopped.store(true, Ordering::Release);
        self.binding.cancel();
    }
    pub(crate) fn respond(&self, id: &str, option: &str) -> bool {
        self.binding.respond(id, option)
    }
    pub(crate) fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.cancel();
        self.binding.close();
    }
    pub(crate) fn observe(&self, event: &Value) {
        if self.closed.load(Ordering::Acquire) {
            return;
        }
        let data = &event["data"];
        if data["sessionID"].as_str() != Some(self.session.as_str()) {
            return;
        }
        let scope = format!("opencode2:{}", self.session);
        match event["type"].as_str() {
            Some("session.execution.started") if !self.stopped.load(Ordering::Acquire) => self.binding.allow_turn(),
            Some("session.step.started") => self.binding.observe(&json!({"type":"message.updated","properties":{"info":{"sessionID":scope,"role":"assistant","id":data["assistantMessageID"]}}})),
            Some("session.tool.input.started") => {
                let (Some(message),Some(call),Some(name)) = (valid_id(&data["assistantMessageID"]),valid_id(&data["id"]),data["name"].as_str().and_then(|name|name.strip_prefix(&self.prefix)).filter(|name|super::accepts(name))) else { return; };
                let mut names = self.names.lock();
                let key = (message.into(),call.into());
                if let Some(old) = names.get(&key) { if old != name { drop(names);self.binding.cancel(); } }
                else if names.len()<2048 { names.insert(key,name.into()); }
            }
            Some("session.tool.input.ended" | "session.tool.called" | "session.tool.failed") => {
                let (Some(message),Some(call)) = (valid_id(&data["assistantMessageID"]),valid_id(&data["id"])) else { return; };
                let Some(name) = self.names.lock().get(&(message.into(),call.into())).cloned() else { return; };
                let arguments = if event["type"]=="session.tool.input.ended" { data["text"].as_str().and_then(|text|serde_json::from_str::<Value>(text).ok()).unwrap_or(Value::Null) } else { data["input"].clone() };
                let failed = event["type"]=="session.tool.failed" || data["executed"]==true;
                self.binding.observe(&json!({"type":"message.part.updated","properties":{"part":{"sessionID":scope,"messageID":message,"callID":call,"type":"tool","tool":format!("ion_teams_{name}"),"state":{"status":if failed{"error"}else{"running"},"input":arguments}}}}));
            }
            Some("session.execution.succeeded" | "session.execution.failed" | "session.execution.interrupted" | "session.deleted" | "session.idle") => self.binding.cancel(),
            _=>{}
        }
    }
}
fn valid_id(value: &Value) -> Option<&str> {
    value
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 256 && !s.chars().any(char::is_control))
}
impl Drop for Inner {
    fn drop(&mut self) {
        self.observer.close();
        self.hub.close();
        // The worker owns the final reference. No native process is killed and
        // stale teardown cannot revoke a replacement native plugin generation.
        let _ = rpc(
            &self.service.endpoint(),
            &self.directory,
            &self.id,
            "revoke",
            json!({"generation":self.generation}),
        );
    }
}

pub(crate) fn plugin(id: &str, prefix: &str) -> String {
    let definitions = super::tools()
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["function"].clone())
        .collect::<Vec<_>>();
    r#"const id=__ID__;
const prefix=__PREFIX__;
const definitions=__DEFINITIONS__;
const object={type:'object',additionalProperties:true};
const empty={type:'object',properties:{},additionalProperties:false};
export default {id,async setup(ctx){
 let binding;
 const clear=()=>{binding?.abort.abort();binding=undefined;};
 const registration=await ctx.tool.transform(editor=>{
  const selected=binding;
  if(!selected)return;
  for(const def of definitions)editor.add({name:prefix+def.name,description:def.description,input:def.parameters,options:{codemode:false},
   async execute(args,call){
    if(binding!==selected||call.sessionID!==selected.session||!call.messageID||!call.id||call.signal.aborted||selected.abort.signal.aborted)throw new Error('Ion Teams requires its live main-session call.');
    await ctx.permission.assert({sessionID:call.sessionID,agent:call.agent,source:{type:'tool',messageID:call.messageID,id:call.id},action:prefix+def.name,resources:['*'],save:['*']});
    const response=await fetch(selected.endpoint,{method:'POST',headers:{'Content-Type':'application/json',Authorization:'Bearer '+selected.credential},
     body:JSON.stringify({session_id:'opencode2:'+call.sessionID,message_id:call.messageID,call_id:call.id,name:def.name,arguments:args}),
     signal:AbortSignal.any([call.signal,selected.abort.signal,AbortSignal.timeout(340000)])});
    const reply=await response.json();
    if(!response.ok||!reply.ok)throw new Error(reply.text||'Ion team request did not complete.');
    return{content:reply.text};
   }
  });
 });
 const fields={generation:{type:'string'},session:{type:'string'},endpoint:{type:'string'},credential:{type:'string'}};
 await ctx.rpc.register({id,methods:{bind:{input:{type:'object',properties:fields,required:Object.keys(fields),additionalProperties:false},output:object},revoke:{input:{type:'object',properties:{generation:{type:'string'}},required:['generation'],additionalProperties:false},output:object},status:{input:empty,output:object}},events:{}},{
  bind:async(input)=>{
   const url=new URL(input.endpoint);
   if(url.protocol!=='http:'||url.hostname!=='127.0.0.1'||url.pathname!=='/call'||url.username||url.password||url.search||url.hash||!input.session||!input.generation||!/^[a-f0-9]{64}$/.test(input.credential))throw new Error('Invalid Ion source binding');
   if(!binding||['generation','session','endpoint','credential'].some(key=>binding[key]!==input[key])){clear();binding={...input,abort:new AbortController()};await ctx.tool.reload();}
   return{ready:true,generation:binding.generation,version:ctx.app.version};
  },
  revoke:async(input)=>{const revoked=binding?.generation===input.generation;if(revoked){clear();await ctx.tool.reload();}return{revoked};},
  status:async()=>({ready:!!binding,version:ctx.app.version,generation:binding?.generation??null})
 });
 return async()=>{clear();await registration.dispose();};
}};
"#.replace("__ID__",&json!(id).to_string()).replace("__PREFIX__",&json!(prefix).to_string()).replace("__DEFINITIONS__",&json!(definitions).to_string())
}
