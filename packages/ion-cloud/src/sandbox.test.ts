import {expect,test} from "bun:test";
import {mkdtempSync,rmSync} from "node:fs";
import {tmpdir} from "node:os";
import {join} from "node:path";
import {Sandbox,sandboxPolicy,type SandboxPolicy} from "./sandbox";
import {ServiceStore,hash} from "./service-store";
import {Budget} from "./budget";
import type {Account} from "./auth";
const image="10000000-0000-4000-8000-000000000001",operation="20000000-0000-4000-8000-000000000001",requestId="30000000-0000-4000-8000-000000000001";
const account=(c="a"):Account=>({workspaceId:c.repeat(64),subject:c,issuer:"https://issuer/",scopes:new Set(["ion:gateway"])});
const policy:SandboxPolicy={projectId:"project-fixture123",images:[{target:image,image,kind:"harness",manifest:{version:2,capabilities:[]}}],maximumOperationMicroUSD:10000,maximumUploadMicroUSD:1000,maxRuntimeSeconds:150,pricingReference:"https://pricing.fixture.invalid/verified",limits:{monthly:100000,lifetime:100000,accountLifetime:50000}};
const body=()=>({image,hostname:"ion-"+requestId,command:"/usr/local/bin/ion-harness-worker",args:[],shell:false,env:{PATH:"/usr/local/bin:/usr/bin:/bin",HOME:"/home/ion"},cwd:"/ion/work",uid:1000,gid:1000,stdin:{value:Buffer.from(JSON.stringify({version:1,request_id:requestId})).toString("base64"),encoding:"base64",close:true},timeout:150,truncate_output_at:65536,disposable:true,preserve_env:false,resources_limits:{max_layer_bytes:268435456},networking:{enabled:true}});
const request=(path:string,method="GET",data?:unknown)=>new Request("https://api.example/v1/sandbox"+path,{method,...(data?{body:JSON.stringify(data)}:{})});
async function fixture(run:(store:ServiceStore)=>Promise<void>){const dir=mkdtempSync(join(tmpdir(),"ion-sandbox-")),store=new ServiceStore(dir);try{await run(store);}finally{store.close();rmSync(dir,{recursive:true,force:true,maxRetries:5,retryDelay:100});}}
function provider(){
 let launches=0,cancels=0,fail=false,permission=true,terminal=false,payload:any;
 const send=async(url:string,init:RequestInit)=>{
  expect((init.headers as any).Authorization).toBe("Bearer server-only");expect((init.headers as any).Project).toBe(policy.projectId);
  const path=new URL(url).pathname.replace("/sandboxes/v1","");
  if(path==="/whoami")return Response.json({permissions:{spawn_disposable:permission,list:permission,cancel:permission}});
  if(path==="/instances"){launches++;payload=JSON.parse(init.body as string);if(fail)throw new Error("private transport error");return Response.json({uuid:operation},{status:201});}
  if(path==="/operations/"+operation){
   if(init.method==="DELETE"){cancels++;terminal=true;return new Response(null,{status:202});}
   return Response.json({uuid:operation,kind:"instance",status:terminal?"CANCELLED":"EXECUTING",image_uuid:image,result_image_uuid:null,metadata:{...payload,result:terminal?{state:{exit_code:0},stdout:{encoding:"base64",value:"",truncated:false}}:null}});
  }
  if(path==="/files")return Response.json({uuid:operation,sha256:hash(init.body as any),size:(init.body as Uint8Array).length},{status:201});
  throw new Error("Unexpected fixture route "+path);
 };
 return {send,get launches(){return launches;},get cancels(){return cancels;},fail(){fail=true;},deny(){permission=false;}};
}
test("sandbox is unavailable without verified operator configuration or provider permissions",()=>fixture(async store=>{
 const budget=new Budget(store),upstream=provider();
 expect(await(await new Sandbox(store,budget,()=>false).handle(account(),request("/config"))).json()).toMatchObject({configured:false,available:false});
 upstream.deny();const sandbox=new Sandbox(store,budget,()=>true,"server-only",sandboxPolicy(JSON.stringify(policy)),upstream.send);
 expect((await(await sandbox.handle(account(),request("/config"))).json()).available).toBe(false);
 await expect(sandbox.handle(account(),request("/instances","POST",body()))).rejects.toThrow("activation");
 expect(upstream.launches).toBe(0);expect(budget.combined().total.held).toBe(0);
}));
test("sandbox launch binds identity, reserves before dispatch and does not repeat across restart",()=>fixture(async store=>{
 const budget=new Budget(store),upstream=provider();let enabled=true;
 const make=()=>new Sandbox(store,budget,()=>enabled,"server-only",policy,upstream.send);
 const sandbox=make();
 expect((await sandbox.handle(account(),request("/instances","POST",body()))).status).toBe(201);
 expect(budget.combined().total.held).toBe(10000);
 await expect(make().handle(account("b"),request("/operations/"+operation))).rejects.toThrow("unavailable");
 expect((await make().handle(account(),request("/instances","POST",body()))).status).toBe(201);expect(upstream.launches).toBe(1);
 await expect(make().handle(account(),request("/instances","POST",{...body(),timeout:149}))).rejects.toThrow("changed");
 enabled=false;await make().maintenance();expect(upstream.cancels).toBe(1);
 expect((await(await make().handle(account(),request("/operations/"+operation))).json()).status).toBe("CANCELLED");
 expect(make().pending).toBe(false);expect(budget.combined().total.held).toBe(10000);
}));
test("uncertain sandbox acknowledgement keeps the spending hold and cannot retry or use another account",()=>fixture(async store=>{
 const budget=new Budget(store),upstream=provider();upstream.fail();const make=()=>new Sandbox(store,budget,()=>true,"server-only",policy,upstream.send);
 await expect(make().handle(account(),request("/instances","POST",body()))).rejects.toThrow("saved request");
 await expect(make().handle(account(),request("/instances","POST",body()))).rejects.toThrow("uncertain");
 expect(upstream.launches).toBe(1);expect(budget.combined().total.held).toBe(10000);
}));
test("sandbox rejects arbitrary images, root harnesses, retained state, injected env and files",()=>fixture(async store=>{
 const upstream=provider(),sandbox=new Sandbox(store,new Budget(store),()=>true,"server-only",policy,upstream.send);
 for(const changed of [{image:"tag:latest"},{uid:0},{disposable:false},{shell:true},{preserve_env:true},{env:{...body().env,API_KEY:"client-injected"}},{files:{"/ion/job-project.ion":{uuid:operation,uid:0,gid:0,mode:"0400"}}}])
  await expect(sandbox.handle(account(),request("/instances","POST",{...body(),...changed}))).rejects.toThrow();
 expect(upstream.launches).toBe(0);
}));
test("bounded uploaded files have account ownership; repeated confirmed upload is served from receipt",()=>fixture(async store=>{
 const upstream=provider(),sandbox=new Sandbox(store,new Budget(store),()=>true,"server-only",policy,upstream.send),bytes=Buffer.from("project archive"),digest=hash(bytes);
 const upload=()=>new Request("https://api.example/v1/sandbox/files",{method:"POST",body:bytes});
 expect((await sandbox.handle(account(),upload())).status).toBe(201);
 expect((await(await sandbox.handle(account(),request("/files/"+digest))).json()).sha256).toBe(digest);
 await expect(sandbox.handle(account("b"),request("/files/"+digest))).rejects.toThrow("unavailable");
 expect((await sandbox.handle(account(),upload())).status).toBe(201);
}));
