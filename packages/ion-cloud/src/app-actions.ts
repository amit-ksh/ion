import { CloudFailure, type Account } from "./auth";
import { Budget } from "./budget";
import { ManagedApps, composioUser } from "./apps";
import { readJson, secret, ServiceStore } from "./service-store";
const id = (v: unknown): v is string => typeof v === "string" && /^[A-Za-z0-9_-]{1,160}$/.test(v);
const tool = (v: unknown): v is string => typeof v === "string" && /^[A-Z][A-Z0-9_]{0,127}$/.test(v) && !v.startsWith("COMPOSIO_");
const version = (v: unknown): v is string => id(v) && v !== "latest";
interface Effect { accountId: string; action: string; version: string; arguments: Record<string, unknown> }
interface Proposal { effect: Effect; state: "pending" | "approved" | "denied" | "uncertain" | "complete"; expires: number; receipt?: unknown; reservation?: string }
function safeTool(raw: any) {
  if (!tool(raw.slug) || !version(raw.version) || !id(raw.toolkit?.slug) || raw.is_deprecated === true ||
      !raw.input_parameters || typeof raw.input_parameters !== "object" || Array.isArray(raw.input_parameters) ||
      JSON.stringify(raw.input_parameters).length > 24_000) throw new CloudFailure(502,"App action metadata is unavailable");
  return { slug:raw.slug, version:raw.version, toolkit:{slug:raw.toolkit.slug}, name:String(raw.name??raw.slug).slice(0,160),
    description:String(raw.description??"").slice(0,2048), input_parameters:raw.input_parameters };
}
function receipt(raw: any) {
  if (raw?.successful !== true || raw.error || !id(raw.log_id)) throw new CloudFailure(502,"App action outcome needs reconciliation; do not repeat it");
  const clean = (v: any, depth=0): any => {
    if (depth>32) return null;
    if (Array.isArray(v)) return v.map(item=>clean(item,depth+1));
    if (v && typeof v==="object") return Object.fromEntries(Object.entries(v).map(([key,value])=>[key,
      /^(access_token|refresh_token|oauth_token|oauth_token_secret|api_key|authorization|client_secret|password|credentials|auth_config|session_info)$/i.test(key)?"[redacted]":clean(value,depth+1)]));
    return v;
  };
  let data=clean(raw.data??null),truncated=false;
  if (JSON.stringify(data).length>40_000) { data=null;truncated=true; }
  return {successful:true,data,log_id:raw.log_id,truncated};
}
/** Exact approved legacy actions retain their existing published tool versions.
 * Connections/discovery use per-user sessions; no Instant/hosted account,
 * session meta-tool, raw proxy or provider auth override can execute here. */
export class AppActions {
  private inFlight=new Set<string>();
  constructor(private store:ServiceStore,private apps:ManagedApps,private budget:Budget,
    private enabled:()=>boolean,private clock=Date.now) {}
  private effect(body:any):Effect {
    if (!body || !id(body.accountId) || ["instant_account","hosted_account"].includes(body.accountId) ||
        !tool(body.action) || !version(body.version) || !body.arguments || typeof body.arguments!=="object" || Array.isArray(body.arguments) ||
        JSON.stringify(body.arguments).length>4096 || Object.keys(body).some(k=>!["accountId","action","version","arguments"].includes(k)))
      throw new CloudFailure(400,"Choose an exact account, app action version and bounded arguments");
    return {accountId:body.accountId,action:body.action,version:body.version,arguments:body.arguments};
  }
  private async verified(account:Account, effect:Pick<Effect,"accountId"|"action"|"version">) {
    const connection=await this.apps.connection(account,effect.accountId);
    if (connection.status!=="ACTIVE") throw new CloudFailure(409,"Reconnect this exact app account before continuing");
    const metadata=safeTool(await this.apps.provider("/tools/"+effect.action+"?"+new URLSearchParams({version:effect.version})));
    if (metadata.slug!==effect.action || metadata.version!==effect.version || metadata.toolkit.slug!==connection.toolkit)
      throw new CloudFailure(409,"The approved app action identity changed");
    return metadata;
  }
  async handle(account:Account,request:Request) {
    if(this.inFlight.size>=4||this.inFlight.has(account.workspaceId))throw new CloudFailure(429,"An app action request is already active; retry after it completes");
    composioUser(account);
    const minute=Math.floor(this.clock()/60_000);
    this.store.transaction(()=>{
      for(const [scope,max] of [[account.workspaceId,20],["_operator",120]] as const) {
        const saved=this.store.get<{minute:number;count:number}>(scope,"app-action-rate","minute");
        const count=saved?.minute===minute?saved.count:0;
        if(count>=max)throw new CloudFailure(429,"App action request limit reached");
        this.store.put(scope,"app-action-rate","minute",{minute,count:count+1});
      }
    });
    this.inFlight.add(account.workspaceId);
    try{return await this.dispatch(account,request);}finally{this.inFlight.delete(account.workspaceId);}
  }
  private async dispatch(account:Account,request:Request) {
    composioUser(account);this.store.admit(account.workspaceId);
    if (!this.apps.available) throw new CloudFailure(503,"Managed apps are not configured");
    if (request.method!=="POST") throw new CloudFailure(405,"Use an authenticated app operation");
    const input=await readJson(request,12_000),path=new URL(request.url).pathname;
    if (!input || typeof input!=="object" || Array.isArray(input)) throw new CloudFailure(400,"Invalid app action");
    let result:unknown;
    if (path==="/v1/apps/actions/tools") {
      if (!id(input.accountId) || typeof input.query!=="string" || input.query.length>256 ||
          (input.cursor!==undefined && (typeof input.cursor!=="string" || input.cursor.length>1024)) ||
          Object.keys(input).some(k=>!["accountId","query","cursor"].includes(k))) throw new CloudFailure(400,"Choose an owned app account");
      const connection=await this.apps.connection(account,input.accountId);
      if (connection.status!=="ACTIVE") throw new CloudFailure(409,"Reconnect this app account");
      const query=new URLSearchParams({toolkit_slug:connection.toolkit,query:input.query,include_deprecated:"false",limit:"20"});
      if(input.cursor)query.set("cursor",input.cursor);
      const page=await this.apps.provider("/tools?"+query);
      if (!Array.isArray(page.items)||page.items.length>20 || (page.next_cursor!=null && (typeof page.next_cursor!=="string" || page.next_cursor.length>1024))) throw new CloudFailure(502,"Invalid action catalog");
      const items=page.items.map(safeTool);
      if(items.some((item:any)=>item.toolkit.slug!==connection.toolkit))throw new CloudFailure(502,"App toolkit changed");
      result={items,next_cursor:page.next_cursor??null};
    } else if(path==="/v1/apps/actions/describe") {
      if(!id(input.accountId)||!tool(input.action)||!version(input.version)||Object.keys(input).some(k=>!["accountId","action","version"].includes(k)))throw new CloudFailure(400,"Choose an exact app action");
      result=await this.verified(account,input);
    } else if(path==="/v1/apps/actions/prepare") {
      const effect=this.effect(input);await this.verified(account,effect);
      if(this.store.all(account.workspaceId,"app-action").length>=256)throw new CloudFailure(429,"App action history needs owner review");
      const proposalId=secret();
      this.store.put(account.workspaceId,"app-action",proposalId,{effect,state:"pending",expires:this.clock()+300_000} satisfies Proposal);
      result={proposalId,expiresAt:this.clock()+300_000};
    } else {
      if(typeof input.proposalId!=="string"||!/^[A-Za-z0-9_-]{43}$/.test(input.proposalId))throw new CloudFailure(400,"Choose an app action proposal");
      const proposal=this.store.get<Proposal>(account.workspaceId,"app-action",input.proposalId);
      if(!proposal)throw new CloudFailure(404,"App action unavailable");
      if(path==="/v1/apps/actions/status") {
        if(Object.keys(input).some(k=>k!=="proposalId"))throw new CloudFailure(400,"Invalid app status request");
        result={state:proposal.state,expiresAt:proposal.expires,...(proposal.state==="complete"?{receipt:proposal.receipt}:{})};
      } else if(path==="/v1/apps/actions/approve") {
        if(typeof input.allow!=="boolean"||Object.keys(input).some(k=>!["proposalId","allow"].includes(k)))throw new CloudFailure(400,"Approve or deny the exact saved app action");
        if(proposal.expires<=this.clock()||proposal.state!=="pending")throw new CloudFailure(409,"App approval expired or already resolved");
        proposal.state=input.allow?"approved":"denied";
        this.store.put(account.workspaceId,"app-action",input.proposalId,proposal);result={state:proposal.state};
      } else if(path==="/v1/apps/actions/execute") {
        if(Object.keys(input).some(k=>k!=="proposalId"))throw new CloudFailure(400,"Execute only the saved approved payload");
        if(!this.enabled())throw new CloudFailure(403,"Shared app actions are OFF");
        if(proposal.state==="complete")result=proposal.receipt;
        else {
          if(proposal.state!=="approved"||proposal.expires<=this.clock())throw new CloudFailure(409,"Exact app approval required; uncertain outcomes must be reconciled");
          await this.verified(account,proposal.effect);
          // Claim and reserve in one commit after asynchronous ownership checks.
          this.store.transaction(()=>{
            const current=this.store.get<Proposal>(account.workspaceId,"app-action",input.proposalId);
            if(!this.enabled()||current?.state!=="approved"||current.expires<=this.clock())throw new CloudFailure(409,"App action no longer approved");
            // Conservative ordinary-call rate-card ceiling: USD .001. Keep its
            // hold until a billing receipt is reconciled; absent charge != free.
            current.reservation=this.budget.reserve("composio",account.workspaceId,1000,{monthly:2_000_000,lifetime:2_000_000,accountLifetime:500_000});
            current.state="uncertain";this.store.put(account.workspaceId,"app-action",input.proposalId,current);
          });
          const effect=proposal.effect;
          const raw=await this.apps.provider("/tools/execute/"+effect.action,"POST",{
            connected_account_id:effect.accountId,user_id:composioUser(account),version:effect.version,arguments:effect.arguments,
          });
          result=receipt(raw);
          const current=this.store.get<Proposal>(account.workspaceId,"app-action",input.proposalId)!;
          current.state="complete";current.receipt=result;this.store.put(account.workspaceId,"app-action",input.proposalId,current);
        }
      } else throw new CloudFailure(404,"Unknown app action operation");
    }
    return Response.json(result,{headers:{"Cache-Control":"no-store","X-Content-Type-Options":"nosniff"}});
  }
}
