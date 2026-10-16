// Native Cursor protocol research fixture. This does not register a Teams adapter.
import { expect, test } from "bun:test";
import { createHash } from "node:crypto";
import {
  mkdtempSync,
  writeFileSync,
  readFileSync,
  rmSync,
  realpathSync,
} from "node:fs";
import { join, resolve, dirname, sep } from "node:path";
import { tmpdir } from "node:os";
type Json = Record<string, any>;
const nativeHookTemplate = readFileSync(
  new URL(
    "../../crates/waku-core/src/driver/source_teams/cursor/hook.ps1",
    import.meta.url,
  ),
  "utf8",
);
const runtime = process.env.ION_TEST_CURSOR_RUNTIME;
const native = test.skipIf(!runtime || process.platform !== "win32");
const indexHash =
  "0e7e28c34e050c6b83fe574e1fefa8d95d7c80bea6c4a4d3ae2726e5191243ef";

async function run(mode: string): Promise<Json> {
  const dir = resolve(runtime!);
  expect(
    createHash("sha256")
      .update(readFileSync(join(dir, "index.js")))
      .digest("hex"),
  ).toBe(indexHash);
  const base = realpathSync(process.env.ION_NATIVE_FIXTURE_ROOT ?? tmpdir());
  const root = mkdtempSync(join(base, "cursor-source-contract-"));
  const host = join(root, "host.mjs");
  writeFileSync(host, controller);
  try {
    const p = Bun.spawn([join(dir, "node.exe"), host, mode], {
      cwd: root,
      env: {
        SystemRoot: process.env.SystemRoot,
        WINDIR: process.env.WINDIR,
        PATH: process.env.PATH,
        ION_TEST_CURSOR_RUNTIME: dir,
        ION_CURSOR_CONTRACT_TEMP: root,
        ION_CURSOR_HOOK_TEMPLATE: nativeHookTemplate,
      },
      stdout: "pipe",
      stderr: "pipe",
    });
    const [out, err, code] = await Promise.all([
      new Response(p.stdout).text(),
      new Response(p.stderr).text(),
      p.exited,
    ]);
    expect(err).toBe("");
    expect(code).toBe(0);
    const result = JSON.parse(out);
    expect(result.timedOut).toBe(false);
    expect(result.modelSelections).toEqual(["ion-native-fixture-model"]);
    return result;
  } finally {
    const target = realpathSync(root);
    if (dirname(target) !== base || !target.startsWith(base + sep))
      throw new Error("Fixture cleanup escaped its explicit parent");
    rmSync(target, { recursive: true, force: true });
  }
}
const calls = (r: Json) => r.mcp.filter((x: Json) => x.method === "tools/call");
const permissions = (r: Json) =>
  r.messages.filter((x: Json) => x.method === "session/request_permission");

native(
  "Cursor native ACP retains separate IDs for identical MCP calls and returns successful results",
  async () => {
    const r = await run("allow");
    const pc = permissions(r);
    const mc = calls(r);
    expect(pc.map((x: Json) => x.params.toolCall.toolCallId)).toEqual([
      "ion-native-call-1",
      "ion-native-call-2",
    ]);
    expect(mc).toHaveLength(2);
    expect(mc[0].params).toEqual(mc[1].params);
    expect(mc.every((x: Json) => x.params._meta === undefined)).toBe(true);
    expect(
      mc.every(
        (x: Json) => x.params.arguments.ion_native_token === "model-forged",
      ),
    ).toBe(true);
    expect(r.nativeResults).toHaveLength(2);
    expect(
      r.nativeResults.every((x: string) => x.includes("native-mcp-success")),
    ).toBe(true);
    const source = r.messages.filter(
      (x: Json) => x.params?.update?.sessionUpdate === "tool_call",
    );
    expect(source.map((x: Json) => x.params.update.toolCallId)).toEqual(
      pc.map((x: Json) => x.params.toolCall.toolCallId),
    );
    expect(
      source.every((x: Json) => x.params.sessionId === pc[0].params.sessionId),
    ).toBe(true);
  },
  40000,
);

native(
  "Cursor native permission denial prevents the MCP invocation",
  async () => {
    const r = await run("deny");
    expect(permissions(r)).toHaveLength(1);
    expect(calls(r)).toHaveLength(0);
    expect(r.nativeResults[0]).toContain("User rejected");
  },
  40000,
);

native(
  "Cursor native Stop while approval is pending prevents the MCP invocation",
  async () => {
    const r = await run("stop");
    expect(permissions(r)).toHaveLength(1);
    expect(calls(r)).toHaveLength(0);
    const result = r.messages.find(
      (x: Json) => x.id === 3 && (x.result || x.error),
    );
    expect(result).toBeDefined();
    expect(result.result?.stopReason).toBe("cancelled");
  },
  40000,
);

native(
  "Cursor native private-profile missing hook fails closed before MCP execution",
  async () => {
    const r = await run("missing-hook");
    expect(calls(r)).toHaveLength(0);
    expect(permissions(r)).toHaveLength(0);
    expect(r.nativeResults[0]).toContain("fail closed");
  },
  40000,
);

native(
  "Cursor cached ACP plugin flag does not make its plugin hook an authority boundary",
  async () => {
    const r = await run("plugin");
    expect(calls(r)).toHaveLength(1);
    expect(permissions(r)).toHaveLength(1);
    expect(r.nativeResults[0]).toContain("native-mcp-success");
  },
  40000,
);

native(
  "Cursor native hook overwrites model input before the witnessed permission and MCP call",
  async () => {
    const r = await run("hook-update");
    expect(r.hooks).toHaveLength(1);
    expect(r.hooks[0].tool_input.ion_native_token).toBe("model-forged");
    expect(r.hooks[0].conversation_id).toBe(permissions(r)[0].params.sessionId);
    expect(r.hooks[0].tool_use_id).not.toBe(
      permissions(r)[0].params.toolCall.toolCallId,
    );
    expect(calls(r)).toHaveLength(1);
    expect(calls(r)[0].params.arguments.ion_native_token).toBe(
      r.hooks[0].ion_hook_nonce,
    );
    expect(permissions(r)).toHaveLength(1);
    expect(JSON.stringify(permissions(r)[0])).toContain(
      r.hooks[0].ion_hook_nonce,
    );
    expect(r.nativeResults[0]).toContain("native-mcp-success");
  },
  40000,
);

native(
  "Cursor cached ACP loads the workspace-cache hook without changing its user profile",
  async () => {
    const r = await run("project-hook");
    expect(r.hooks).toHaveLength(1);
    expect(calls(r)).toHaveLength(1);
    expect(calls(r)[0].params.arguments.ion_native_token).toBe(
      r.hooks[0].ion_hook_nonce,
    );
    expect(r.nativeResults[0]).toContain("native-mcp-success");
  },
  40000,
);

native(
  "Cursor rewrites identical calls with separate bindings witnessed under distinct native call IDs",
  async () => {
    const r = await run("project-hook-pair");
    expect(r.hooks).toHaveLength(2);
    expect(calls(r)).toHaveLength(2);
    const ids = permissions(r).map((p: Json) => p.params.toolCall.toolCallId);
    expect(ids).toEqual(["ion-native-call-1", "ion-native-call-2"]);
    expect(new Set(r.hooks.map((h: Json) => h.ion_hook_nonce)).size).toBe(2);
    expect(new Set(r.hooks.map((h: Json) => h.tool_use_id)).size).toBe(2);
    const source = r.messages.filter(
      (m: Json) => m.params?.update?.sessionUpdate === "tool_call",
    );
    for (let i = 0; i < 2; i++) {
      const hook = r.hooks[i];
      const request = permissions(r)[i];
      const rewritten = calls(r)[i].params.arguments;
      const text = request.params.toolCall.content[0].content.text;
      const nativeArgs = JSON.parse(
        text.slice(text.indexOf("\n") + 1, text.lastIndexOf("\n")),
      );
      expect(rewritten).toEqual({
        team: "ion-team-fixture",
        ion_native_token: hook.ion_hook_nonce,
      });
      expect(nativeArgs).toEqual(rewritten);
      expect(hook.tool_input).toEqual({
        team: "ion-team-fixture",
        ion_native_token: "model-forged",
      });
      expect(hook.conversation_id).toBe(request.params.sessionId);
      expect(hook.tool_use_id).not.toBe(ids[i]);
      expect(source[i].params.update.toolCallId).toBe(ids[i]);
      expect(JSON.stringify(source[i].params.update.rawInput)).toContain(
        "model-forged",
      );
    }
    expect(
      r.nativeResults.every((x: string) => x.includes("native-mcp-success")),
    ).toBe(true);
  },
  40000,
);
for (const mode of ["project-hook-deny", "project-hook-stop"]) {
  native(
    "Cursor rewritten call remains stopped by native " + mode,
    async () => {
      const r = await run(mode);
      expect(r.hooks).toHaveLength(1);
      expect(permissions(r)).toHaveLength(1);
      expect(calls(r)).toHaveLength(0);
      if (mode.endsWith("stop"))
        expect(
          r.messages.find((m: Json) => m.id === 3 && m.result)?.result
            .stopReason,
        ).toBe("cancelled");
      else expect(r.nativeResults[0]).toContain("User rejected");
    },
    40000,
  );
}

native(
  "Cursor native allow-always can bypass the permission witness on a later rewritten call",
  async () => {
    const r = await run("project-hook-always");
    expect(r.hooks).toHaveLength(2);
    expect(calls(r)).toHaveLength(2);
    expect(permissions(r)).toHaveLength(1);
    expect(r.nativeResults).toHaveLength(2);
  },
  40000,
);

native(
  "Cursor native PowerShell hook can use an authenticated loopback callback and return rewritten input",
  async () => {
    const r = await run("project-hook-http");
    expect(r.hookRequests).toHaveLength(1);
    expect(r.hookRequests[0].tool_input.ion_native_token).toBe("model-forged");
    expect(calls(r)).toHaveLength(1);
    expect(calls(r)[0].params.arguments.ion_native_token).not.toBe(
      "model-forged",
    );
    expect(JSON.stringify(permissions(r)[0])).toContain(
      calls(r)[0].params.arguments.ion_native_token,
    );
    expect(r.nativeResults[0]).toContain("native-mcp-success");
  },
  40000,
);

native(
  "Cursor native loopback hook preserves UTF8 arguments and supplementary Unicode",
  async () => {
    const r = await run("project-hook-http-utf8");
    expect(r.hookRequests).toHaveLength(1);
    expect(r.hookRequests[0].tool_input.team).toBe(
      "ion-team-fixture — 研究 🚀",
    );
    expect(calls(r)[0].params.arguments.team).toBe(
      "ion-team-fixture — 研究 🚀",
    );
  },
  40000,
);
for (const mode of [
  "project-hook-http-deny",
  "project-hook-http-redirect",
  "project-hook-http-stall",
  "project-hook-http-malformed",
  "project-hook-http-oversized",
]) {
  native(
    "Cursor native hook refuses unsuccessful loopback response " + mode,
    async () => {
      const r = await run(mode);
      expect(r.hookRequests).toHaveLength(1);
      expect(calls(r)).toHaveLength(0);
      expect(permissions(r)).toHaveLength(0);
      expect(r.requestPaths).not.toContain("/_ion_unexpected_redirect");
    },
    40000,
  );
}

for (const mode of [
  "project-hook-http-missing-token",
  "project-hook-http-bad-token",
]) {
  native(
    "Cursor native hook refuses unavailable or invalid transport authority " +
      mode,
    async () => {
      const r = await run(mode);
      expect(r.hookRequests).toHaveLength(0);
      expect(calls(r)).toHaveLength(0);
      expect(permissions(r)).toHaveLength(0);
    },
    40000,
  );
}

// Runs the pinned CLI's real Connect HTTP/2 agent protocol. No alternative local-agent provider.
const controller = `import {mkdtempSync,mkdirSync,writeFileSync,readFileSync,existsSync} from "node:fs";
import {createServer as createH2} from "node:http2";
import {createServer} from "node:http";
import {gunzipSync,brotliDecompressSync} from "node:zlib";
import {tmpdir} from "node:os";
import {join} from "node:path";
import {spawn} from "node:child_process";
const cache=(process.env.ION_CURSOR_CONTRACT_TEMP ?? process.env.ION_NATIVE_FIXTURE_ROOT);const mode=process.argv[2]??"allow";const runtime=process.env.ION_TEST_CURSOR_RUNTIME;
const root=mkdtempSync(join(cache,"cursor-acp-probe-"));
for(const n of ["home","work","appdata","local","config","data","tmp"])mkdirSync(join(root,n));
const guard=join(root,"guard.cjs"),guardLog=join(root,"guard.jsonl");
writeFileSync(guard,\`const net=require('node:net'),tls=require('node:tls');
const fail=message=>{require('node:fs').appendFileSync(\${JSON.stringify(guardLog)},JSON.stringify(message)+'\\\\n');throw Error(message)};
const check=(raw)=>{let a=raw;if(Array.isArray(a[0]))a=a[0];let h;if(typeof a[0]==='object'){if(a[0].path)fail('fixture named pipe denied '+a[0].path);h=a[0].host??a[0].hostname??'localhost';}else if(typeof a[0]==='number')h=typeof a[1]==='string'?a[1]:'localhost';else fail('fixture network path denied '+a[0]);if(!['localhost','127.0.0.1','::1'].includes(h))fail('fixture blocked nonlocal host '+h);};
for(const obj of [net.Socket.prototype]){const f=obj.connect;obj.connect=function(...a){check(a);return f.apply(this,a)}}
const f=tls.connect;tls.connect=function(...a){check(a);return f.apply(this,a)};
\`);
const vi=(n)=>{const a=[];while(n>127){a.push((n&127)|128);n=Math.floor(n/128)}a.push(n);return Buffer.from(a)};
const fld=(n,v)=>{const b=typeof v==="string"?Buffer.from(v):Buffer.from(v);return Buffer.concat([vi(n*8+2),vi(b.length),b])};
const model=fld(1,"ion-native-fixture-model");
const token=Buffer.from(JSON.stringify({alg:"none"})).toString("base64url")+"."+Buffer.from(JSON.stringify({sub:"ion-contract-fixture",exp:4102444800,email:"ion-fixture@example.invalid"})).toString("base64url")+".fixture";
const requests=[],hookRequests=[];const hookToken=(await import("node:crypto")).randomBytes(32).toString("hex");
const server=createServer((req,res)=>{const chunks=[];req.on("data",c=>chunks.push(c));req.on("end",()=>{const body=Buffer.concat(chunks),path=req.url;requests.push({path,type:req.headers["content-type"],bytes:body.length,prefix:[...body.slice(0,50)]});
if(path==="/hook"){
 if(req.headers.authorization!=="Bearer "+hookToken){res.writeHead(403);res.end("{}");return;}
 const event=JSON.parse(body.toString("utf8"));hookRequests.push(event);
 if(mode==="project-hook-http-deny"){res.writeHead(403);res.end("{}");return;}
 if(mode==="project-hook-http-redirect"){res.writeHead(302,{Location:"/_ion_unexpected_redirect"});res.end();return;}
 if(mode==="project-hook-http-stall")return;
 if(mode==="project-hook-http-malformed"){res.writeHead(200,{"Content-Type":"application/json"});res.end("not-json");return;}
 if(mode==="project-hook-http-oversized"){res.writeHead(200,{"Content-Type":"application/json"});res.end("x".repeat(128*1024));return;}

 res.writeHead(200,{"Content-Type":"application/json"});res.end(JSON.stringify({permission:"allow",updated_input:{ion_native_token:hookToken+"-"+hookRequests.length}}));return;
}
let data=Buffer.alloc(0),type=req.headers["content-type"]??"application/proto";
if(path==="/auth/exchange_user_api_key"){data=Buffer.from(JSON.stringify({accessToken:token,refreshToken:"fixture-refresh-token"}));type="application/json"}
if(path.endsWith("/AvailableModels"))data=Buffer.concat([fld(1,"ion-native-fixture-model"),fld(2,Buffer.concat([model,Buffer.from([16,1,40,1])]))]);
if(path.endsWith("/GetUsableModels")||path.endsWith("/GetDefaultModelForCli"))data=fld(1,model);
res.writeHead(200,{"Content-Type":type});res.end(data);
});});await new Promise(r=>server.listen(0,"127.0.0.1",r));
const uv=(n,v)=>Buffer.concat([vi(n*8),vi(v)]);
const msg=(...v)=>Buffer.concat(v);
const fields=(b)=>{const a=[];let i=0;const read=()=>{let n=0,m=1;while(i<b.length){let v=b[i++];n+=(v&127)*m;if(!(v&128))return n;m*=128}throw Error("short varint")};while(i<b.length){const key=read(),n=key>>>3,w=key&7;if(w===2){const len=read(),v=b.subarray(i,i+len);i+=len;a.push({n,w,v})}else if(w===0)a.push({n,w,v:read()});else if(w===1){a.push({n,w,v:b.subarray(i,i+8)});i+=8}else if(w===5){a.push({n,w,v:b.subarray(i,i+4)});i+=4}else throw Error("wire "+w)}return a};
const get=(b,n)=>fields(b).find(x=>x.n===n)?.v;
const frame=(b)=>msg(Buffer.from([0]),Buffer.from([b.length>>>24,b.length>>>16&255,b.length>>>8&255,b.length&255]),b);
const mcpLog=join(root,"mcp.jsonl"),helper=join(root,"mcp.cjs");
writeFileSync(helper,\`require('readline').createInterface({input:process.stdin}).on('line',line=>{const x=JSON.parse(line);require('fs').appendFileSync(\${JSON.stringify(mcpLog)},JSON.stringify(x)+'\\\\n');if(x.id===undefined)return;let result={};if(x.method==='initialize')result={protocolVersion:'2024-11-05',capabilities:{tools:{}},serverInfo:{name:'ion-native-contract',version:'1'}};if(x.method==='tools/list')result={tools:[{name:'ion_invoke_team',description:'Invoke a team',annotations:{readOnlyHint:false,destructiveHint:true},inputSchema:{type:'object',properties:{team:{type:'string'},ion_native_token:{type:'string'}},required:['team']}}]};if(x.method==='tools/call')result={content:[{type:'text',text:'native-mcp-success'}],structuredContent:{ok:true},isError:false};console.log(JSON.stringify({jsonrpc:'2.0',id:x.id,result}));});\`);
const nodeExe=join(runtime,"node.exe");
const plugin=join(root,"plugin");mkdirSync(plugin);mkdirSync(join(plugin,".cursor-plugin"));
const hooks={hooks:{preToolUse:[{matcher:"^MCP:ion_invoke_team$",command:"ion-missing-native-fixture-hook",failClosed:true,timeout:1}]}};
writeFileSync(join(plugin,".cursor-plugin/plugin.json"),JSON.stringify({name:"ion-native-contract",version:"1.0.0",hooks}));
if(mode==="missing-hook"){mkdirSync(join(root,"home/.cursor"));writeFileSync(join(root,"home/.cursor/hooks.json"),JSON.stringify({version:1,...hooks}));}
const hookLog=join(root,"hook.jsonl");
if(mode==="hook-update" || mode.startsWith("project-hook")){
 const hookDirectory=mode.startsWith("project-hook")?join(root,"data/projects",join(root,"work").replace(/[^a-zA-Z0-9]/g,"-").replace(/-+/g,"-").replace(/^-+|-+$/g,""),".cursor"):join(root,"home/.cursor");
 mkdirSync(hookDirectory,{recursive:true});
 let hookCommand="& { process { $IonNativeInput=ConvertFrom-Json -InputObject ([string]$_); $IonNativeNonce=[guid]::NewGuid().ToString('N'); $IonNativeInput | Add-Member -NotePropertyName ion_hook_nonce -NotePropertyValue $IonNativeNonce; [System.IO.File]::AppendAllText('"+hookLog.replaceAll("'","''")+"',($IonNativeInput | ConvertTo-Json -Depth 30 -Compress)+[Environment]::NewLine); [Console]::WriteLine((@{ permission='allow'; updated_input=@{ ion_native_token=$IonNativeNonce } } | ConvertTo-Json -Depth 4 -Compress)); exit 0 } }";
 if(mode.startsWith("project-hook-http"))hookCommand=process.env.ION_CURSOR_HOOK_TEMPLATE.replaceAll("__ION_HOOK_PORT__",String(server.address().port));
 writeFileSync(join(hookDirectory,"hooks.json"),JSON.stringify({version:1,hooks:{preToolUse:[{matcher:"^MCP:ion_invoke_team$",command:hookCommand,failClosed:true,timeout:5}]}}));
}

const nativeResults=[],modelSelections=[];
let nativeResult,callNumber=1;
const sessions=new Set();const h2=createH2();h2.on("session",x=>{sessions.add(x);x.on("close",()=>sessions.delete(x))});h2.on("stream",(stream,headers)=>{const entry={path:headers[":path"],h2:true,events:[]};requests.push(entry);stream.respond({":status":200,"content-type":"application/connect+proto"});let buf=Buffer.alloc(0),chosen,callArgs;
const send=(b)=>stream.write(frame(b));
const exec=(field,payload,id)=>send(fld(2,msg(uv(1,id),fld(15,\`ion-native-exec-\${id}\`),fld(field,payload))));
const interact=(f,p)=>send(fld(1,fld(f,p)));
const invoke=()=>{const mapArg=(k,v)=>fld(2,msg(fld(1,k),fld(2,fld(3,v))));callArgs=msg(fld(1,chosen.name),mapArg('team',mode.endsWith('utf8')?'ion-team-fixture — 研究 🚀':'ion-team-fixture'),mapArg('ion_native_token','model-forged'),fld(3,'ion-native-call-'+callNumber),fld(4,chosen.provider),fld(5,chosen.tool));interact(2,msg(fld(1,'ion-native-call-'+callNumber),fld(2,fld(15,fld(1,callArgs)))));exec(11,callArgs,callNumber+1);};
stream.on("data",b=>{buf=msg(buf,b);while(buf.length>=5&&buf.length>=5+buf.readUInt32BE(1)){const flag=buf[0],len=buf.readUInt32BE(1);let data=buf.subarray(5,5+len);buf=buf.subarray(5+len);if(flag&2)continue;if(flag&1){const encoding=headers["connect-content-encoding"];if(encoding==="gzip")data=gunzipSync(data);else if(encoding==="br")data=brotliDecompressSync(data);else throw Error("unsupported fixture envelope compression "+encoding)}const fs=fields(data);entry.events.push(fs.map(x=>x.n));if(fs.some(x=>x.n===1)){const rr=get(data,1);modelSelections.push(get(get(rr,9),1).toString());exec(36,fld(1,"ion-teams"),1);continue;}const client=get(data,2);if(!client)continue;
const state=get(client,36);if(state){const ok=get(state,1);if(!ok){entry.stateError=state.toString('hex');continue;}const servers=fields(ok).filter(x=>x.n===1);entry.servers=servers.map(x=>fields(x.v).filter(x=>x.n===1||x.n===2).map(x=>x.v.toString()));for(const ss of servers){const defs=fields(ss.v).filter(x=>x.n===5);for(const dd of defs){const name=get(dd.v,5)?.toString();if(name==='ion_invoke_team')chosen={name:get(dd.v,1).toString(),provider:get(dd.v,4).toString(),tool:name};}}if(!chosen){entry.noTool=true;continue;}entry.chosen=chosen;invoke();continue;}
const result=get(client,11);if(result){nativeResult=result.toString('utf8');nativeResults.push(nativeResult);entry.resultHex=result.toString('hex');interact(3,msg(fld(1,'ion-native-call-'+callNumber),fld(2,fld(15,msg(fld(1,callArgs),fld(2,result))))));if((mode==="allow"||mode==="project-hook-pair"||mode==="project-hook-always")&&callNumber===1){callNumber++;invoke();continue;}interact(14,Buffer.alloc(0));stream.end(Buffer.from([2,0,0,0,2,123,125]));}
}});stream.on('error',e=>entry.error=e.message);});await new Promise(r=>h2.listen(0,"127.0.0.1",r));
const exe=join(runtime,"node.exe"), index=join(runtime,"index.js");
const env={SystemRoot:process.env.SystemRoot,WINDIR:process.env.WINDIR,PATH:runtime+";"+process.env.PATH,HOME:join(root,"home"),USERPROFILE:join(root,"home"),APPDATA:join(root,"appdata"),LOCALAPPDATA:join(root,"local"),ION_CURSOR_TEAM_HOOK_TOKEN:mode==="project-hook-http-missing-token"?"":mode==="project-hook-http-bad-token"?"f".repeat(64):hookToken,CURSOR_CONFIG_DIR:join(root,"config"),CURSOR_DATA_DIR:join(root,"data"),XDG_CONFIG_HOME:join(root,"config"),XDG_CACHE_HOME:join(root,"local"),TMP:join(root,"tmp"),TEMP:join(root,"tmp"),CI:"true",TERM:"dumb",NODE_OPTIONS:\`--require=\${guard}\`};
const cp=spawn(exe,[index,"--api-key","ion-contract-fixture-only","--endpoint",\`http://127.0.0.1:\${server.address().port}\`,"--agent-endpoint",\`http://127.0.0.1:\${h2.address().port}\`,"--model","ion-native-fixture-model",...(mode==="plugin"?["--plugin-dir",plugin]:[]),"acp"],{cwd:join(root,"work"),env,stdio:["pipe","pipe","pipe"]});
let out="",err="",sent=false,prompt=false,sessionId,timedOut=false;let pending="";
cp.stdout.on("data",b=>{out+=b.toString();pending+=b.toString();while(pending.includes("\\n")){const end=pending.indexOf("\\n"),line=pending.slice(0,end);pending=pending.slice(end+1);if(!line)continue;const q=JSON.parse(line);
if(q.method==="session/request_permission"){
if(mode==="stop"||mode==="project-hook-stop"){cp.stdin.write(JSON.stringify({jsonrpc:"2.0",method:"session/cancel",params:{sessionId}})+"\\n");cp.stdin.write(JSON.stringify({jsonrpc:"2.0",id:q.id,result:{outcome:{outcome:"cancelled"}}})+"\\n");}
else cp.stdin.write(JSON.stringify({jsonrpc:"2.0",id:q.id,result:{outcome:{outcome:"selected",optionId:(mode==="deny"||mode==="project-hook-deny")?"reject-once":mode==="project-hook-always"?"allow-always":"allow-once"}}})+"\\n");
}
if(q.id===1&&q.result&&!sent){sent=true;cp.stdin.write(JSON.stringify({jsonrpc:"2.0",id:2,method:"session/new",params:{cwd:join(root,"work"),mcpServers:[{name:"ion-teams",command:nodeExe,args:[helper],env:[]}]}})+"\\n");}
if(q.id===2&&q.result&&!prompt){prompt=true;sessionId=q.result.sessionId;cp.stdin.write(JSON.stringify({jsonrpc:"2.0",id:3,method:"session/prompt",params:{sessionId,prompt:[{type:"text",text:"Call the private Ion team once."}]}})+"\\n");}
if(q.id===3&&(q.result||q.error))setTimeout(()=>cp.kill(),250);
}});
cp.stderr.on("data",b=>err+=b.toString());
cp.stdin.write(JSON.stringify({jsonrpc:"2.0",id:1,method:"initialize",params:{protocolVersion:1,clientCapabilities:{},clientInfo:{name:"ion-contract",version:"1"}}})+"\\n");
const timer=setTimeout(()=>{timedOut=true;cp.kill()},30000);
await new Promise(r=>cp.on("exit",r));clearTimeout(timer);server.closeAllConnections?.();server.close();for(const x of sessions)x.destroy();h2.close();
const mcp=existsSync(mcpLog)?readFileSync(mcpLog,"utf8").split("\\n").filter(Boolean).map(x=>JSON.parse(x)):[];
const hookInputs=existsSync(hookLog)?readFileSync(hookLog,"utf8").split("\\n").filter(Boolean).map(x=>JSON.parse(x)):[];
const networkRefusals=existsSync(guardLog)?readFileSync(guardLog,"utf8").split("\\n").filter(Boolean).map(x=>JSON.parse(x)):[];
console.log(JSON.stringify({root,mode,timedOut,networkRefusals,hookRequests,requestPaths:requests.map(x=>x.path),modelSelections,nativeResults,mcp,hooks:hookInputs,messages:out.split("\\n").filter(Boolean).map(x=>JSON.parse(x)),err}));
`;
