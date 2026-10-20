import {expect,test} from "bun:test";
import {mkdtempSync,rmSync} from "node:fs";
import {tmpdir} from "node:os";
import {join} from "node:path";
import {AppActions} from "./app-actions";
import {ManagedApps,composioUser} from "./apps";
import {Budget} from "./budget";
import {ServiceStore} from "./service-store";
import type {Account} from "./auth";
const account=(c="a"):Account=>({workspaceId:c.repeat(64),subject:c,issuer:"https://issuer.example/",scopes:new Set(["ion:gateway"])});
const effect={accountId:"ca_own",action:"FIXTURE_READ",version:"20261020",arguments:{query:"exact"}};
const request=(path:string,body:unknown)=>new Request("https://api.example/v1/apps/actions/"+path,{method:"POST",body:JSON.stringify(body)});
async function fixture(run:(store:ServiceStore)=>Promise<void>){
 const directory=mkdtempSync(join(tmpdir(),"ion-actions-")),store=new ServiceStore(directory);
 try{await run(store);}finally{store.close();rmSync(directory,{recursive:true,force:true,maxRetries:5,retryDelay:100});}
}
function provider(){
 let posts=0,foreign=false,inactive=false,fail=false;
 const send=async(url:string,init:RequestInit)=>{
  expect((init.headers as any)["x-api-key"]).toBe("backend-key");expect(init.redirect).toBe("manual");
  const path=new URL(url).pathname;
  if(path.includes("/connected_accounts/"))return Response.json({id:"ca_own",user_id:foreign?composioUser(account("b")):composioUser(account()),toolkit:{slug:"fixture"},status:inactive?"REVOKED":"ACTIVE"});
  if(path.endsWith("/tools/FIXTURE_READ"))return Response.json({slug:"FIXTURE_READ",version:effect.version,toolkit:{slug:"fixture"},name:"Read",input_parameters:{type:"object"}});
  if(path.endsWith("/tools/execute/FIXTURE_READ")){
   posts++;const body=JSON.parse(init.body as string);
   expect(body).toEqual({connected_account_id:effect.accountId,user_id:composioUser(account()),version:effect.version,arguments:effect.arguments});
   if(fail)throw new Error("private provider error");
   return Response.json({successful:true,log_id:"log_real_fixture",data:{answer:"read result",nested:{access_token:"secret"}},session_info:{token:"private"}});
  }
  throw new Error("Unexpected fixture request");
 };
 return{send,get posts(){return posts;},foreign:()=>{foreign=true;},inactive:()=>{inactive=true;},fail:()=>{fail=true;}};
}
async function prepare(actions:AppActions){return ((await (await actions.handle(account(),request("prepare",effect))).json()) as { proposalId: string }).proposalId;}
test("owned app effects require exact approval, retain versions and return bounded receipts",()=>fixture(async store=>{
 const upstream=provider(),apps=new ManagedApps(store,"backend-key",upstream.send),budget=new Budget(store);
 let enabled=true;const actions=new AppActions(store,apps,budget,()=>enabled);
 const proposalId=await prepare(actions);
 await expect(actions.handle(account(),request("execute",{proposalId}))).rejects.toThrow("approval");expect(upstream.posts).toBe(0);
 await expect(actions.handle(account("b"),request("approve",{proposalId,allow:true}))).rejects.toThrow("unavailable");
 await actions.handle(account(),request("approve",{proposalId,allow:true}));
 await expect(actions.handle(account(),request("execute",{proposalId,arguments:{query:"changed"}}))).rejects.toThrow("saved approved payload");
 enabled=false;await expect(actions.handle(account(),request("execute",{proposalId}))).rejects.toThrow("OFF");enabled=true;
 const result=await (await actions.handle(account(),request("execute",{proposalId}))).json() as { data: { answer: string }; log_id: string };
 expect(result.data.answer).toBe("read result");expect(JSON.stringify(result)).not.toContain("secret");expect(JSON.stringify(result)).not.toContain("session_info");
 expect(result.log_id).toBe("log_real_fixture");expect(upstream.posts).toBe(1);
 const restarted=new AppActions(store,apps,budget,()=>true);
 expect(await (await restarted.handle(account(),request("execute",{proposalId}))).json()).toEqual(result);expect(upstream.posts).toBe(1);
 expect(budget.usage("composio",account().workspaceId).account).toEqual({spent:0,held:1000});
}));
test("denied, expired and reconnected/foreign app actions never dispatch",()=>fixture(async store=>{
 const upstream=provider(),apps=new ManagedApps(store,"backend-key",upstream.send);let now=1000;
 const actions=new AppActions(store,apps,new Budget(store),()=>true,()=>now);
 const denied=await prepare(actions);await actions.handle(account(),request("approve",{proposalId:denied,allow:false}));
 await expect(actions.handle(account(),request("execute",{proposalId:denied}))).rejects.toThrow("approval");
 const expired=await prepare(actions);now+=300_001;
 await expect(actions.handle(account(),request("approve",{proposalId:expired,allow:true}))).rejects.toThrow("expired");
 const changed=await prepare(actions);await actions.handle(account(),request("approve",{proposalId:changed,allow:true}));
 upstream.foreign();await expect(actions.handle(account(),request("execute",{proposalId:changed}))).rejects.toThrow("unavailable");expect(upstream.posts).toBe(0);
}));
test("uncertain provider dispatch keeps a reservation and cannot repeat after restart",()=>fixture(async store=>{
 const upstream=provider(),apps=new ManagedApps(store,"backend-key",upstream.send),budget=new Budget(store),actions=new AppActions(store,apps,budget,()=>true);
 const proposalId=await prepare(actions);await actions.handle(account(),request("approve",{proposalId,allow:true}));upstream.fail();
 await expect(actions.handle(account(),request("execute",{proposalId}))).rejects.toThrow("could not be reached");
 await expect(new AppActions(store,apps,budget,()=>true).handle(account(),request("execute",{proposalId}))).rejects.toThrow("uncertain");
 expect(upstream.posts).toBe(1);expect(budget.combined().total.held).toBe(1000);
 expect(((await (await actions.handle(account(),request("status",{proposalId}))).json()) as { state: string }).state).toBe("uncertain");
}));
test("arbitrary credentials, meta tools, instant accounts and unpinned versions are rejected",()=>fixture(async store=>{
 const upstream=provider(),actions=new AppActions(store,new ManagedApps(store,"backend-key",upstream.send),new Budget(store),()=>true);
 for(const input of [{...effect,action:"COMPOSIO_MULTI_EXECUTE_TOOL"},{...effect,version:"latest"},{...effect,accountId:"instant_account"},{...effect,user_id:"operator"},{...effect,arguments:[]}])
  await expect(actions.handle(account(),request("prepare",input))).rejects.toThrow("exact");
 expect(upstream.posts).toBe(0);
}));


test("disconnect after approval prevents dispatch and request limits persist",()=>fixture(async store=>{
 const upstream=provider(),apps=new ManagedApps(store,"backend-key",upstream.send),budget=new Budget(store),actions=new AppActions(store,apps,budget,()=>true);
 const proposalId=await prepare(actions);await actions.handle(account(),request("approve",{proposalId,allow:true}));upstream.inactive();
 await expect(actions.handle(account(),request("execute",{proposalId}))).rejects.toThrow("Reconnect");
 expect(upstream.posts).toBe(0);expect(budget.combined().total.held).toBe(0);
 store.put(account().workspaceId,"app-action-rate","minute",{minute:Math.floor(Date.now()/60_000),count:20});
 await expect(new AppActions(store,apps,budget,()=>true).handle(account(),request("status",{proposalId}))).rejects.toThrow("limit");
}));
