import { CloudFailure, type Account } from "./auth";
import { Budget, money, type Limits } from "./budget";
import { ServiceStore, hash, readJson } from "./service-store";

const base = "https://api.tokenfactory.nebius.com/sandboxes/v1";
const uuid = (v: unknown): v is string => typeof v === "string" && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(v) && !/^0{8}-0{4}-0{4}-0{4}-0{12}$/.test(v);
interface Image { target: string; image: string; label?: string; kind: "command" | "harness"; manifest: Record<string, unknown>; policy?: Record<string, unknown>; }
export interface SandboxPolicy {
  projectId: string;
  images: Image[];
  maximumOperationMicroUSD: number;
  maximumUploadMicroUSD?: number;
  maxRuntimeSeconds: number;
  pricingReference: string;
  limits: Limits;
}
interface Operation { workspace: string; project: string; request: string; body: any; signature: string; created: number;
  reservation: string; operation?: string; receipt?: any; cancelRequested?: number; }
interface FileRecord { sha256: string; size: number; uuid?: string; reservation?: string; }
const headers = { "Cache-Control": "no-store", "X-Content-Type-Options": "nosniff" };
const canonical = (value:any):string => JSON.stringify(value,(_,v)=>v && typeof v === "object" && !Array.isArray(v)
  ? Object.fromEntries(Object.keys(v).sort().map(k=>[k,v[k]])) : v);
const providerBody = (record:Operation) => ({...record.body,hostname:"ion-"+record.workspace.slice(0,22)+"-"+record.request});
export function sandboxPolicy(raw?: string): SandboxPolicy | undefined {
  if (!raw) return;
  const p = JSON.parse(raw) as SandboxPolicy;
  if (!p || typeof p.projectId !== "string" || !/^(?:aiproject|project)-[a-z0-9]{8,80}$/.test(p.projectId) || !Array.isArray(p.images) || !p.images.length || p.images.length > 16 ||
      !Number.isInteger(p.maxRuntimeSeconds) || p.maxRuntimeSeconds < 10 || p.maxRuntimeSeconds > 180 ||
      !/^https:\/\//.test(p.pricingReference) || !p.limits)
    throw new Error("Verify sandbox images, price ceiling and runtime bounds before enabling execution");
  money(p.maximumOperationMicroUSD); Object.values(p.limits).forEach(money);
  if(p.maximumUploadMicroUSD!==undefined){money(p.maximumUploadMicroUSD);if(!p.maximumUploadMicroUSD||p.maximumUploadMicroUSD>100_000)throw new Error("Verify the bounded upload and retention price ceiling");}
  if (!p.maximumOperationMicroUSD || p.maximumOperationMicroUSD > 100_000 || !p.limits.monthly || !p.limits.lifetime || !p.limits.accountLifetime)
    throw new Error("Set a positive sandbox price ceiling within the pilot budget");
  const seen = new Set<string>(), images = new Set<string>();
  for (const i of p.images) {
    if (!uuid(i.target) || !uuid(i.image) || seen.has(i.target+":"+i.kind) || images.has(i.image) || !["command", "harness"].includes(i.kind) ||
        (i.label!==undefined && (typeof i.label!=="string" || i.label.length>160)) || (i.kind==="harness" && i.target!==i.image) ||
        !i.manifest || typeof i.manifest !== "object" || JSON.stringify(i).length > 64_000 || (i.kind === "command" && (!i.policy || i.policy.target!==i.target || i.policy.image!==i.image)))
      throw new Error("Register exact immutable sandbox images and reviewed manifests");
    if(i.kind==="command" && (i.policy?.credential_free!==true || !Number.isInteger(i.policy.uid)||Number(i.policy.uid)<1||!Number.isInteger(i.policy.gid)||Number(i.policy.gid)<1||
        !i.policy.resources || i.manifest.version!==1 || i.manifest.worker!=="ion-job-worker/1" || i.manifest.credential_free!==true || i.manifest.network!=="disabled" ||
        i.manifest.child!=="nonroot" || i.manifest.network_boundary!=="linux-seccomp-socket-deny/v1" || i.manifest.resource_boundary!=="linux-rlimit/v1"))throw new Error("Command images must enforce the reviewed credential-free nonroot child boundary");
    seen.add(i.target+":"+i.kind); images.add(i.image);
  }
  return structuredClone(p);
}
/** Restricted authenticated REST bridge for the existing native worker protocol.
 * Project credentials and immutable image registration are operator-owned. */
export class Sandbox {
  private active = new Set<string>();
  private uploadActive = false;
  constructor(private store: ServiceStore, private budget: Budget, private enabled: () => boolean,
    private key?: string, private policy?: SandboxPolicy,
    private send: (input: string, init: RequestInit) => Promise<Response> = (input, init) => globalThis.fetch(input, init),
    private clock = Date.now) {}
  get configured() { return !!this.key && !!this.policy; }
  get pending() { return this.records().some(r => !r.receipt && !!r.operation); }
  private records() { return this.store.all<Operation>("_operator", "sandbox-request"); }
  private save(record: Operation) {
    const id=record.workspace+":"+record.request, old=this.store.get<Operation>("_operator","sandbox-request",id);
    this.store.put("_operator", "sandbox-request",id,{...record,...(old?.receipt?{receipt:old.receipt}:{})});
  }
  private async call(path: string, method = "GET", body?: RequestInit["body"], type = "application/json", project=this.policy?.projectId) {
    if(!this.key || !project)throw new CloudFailure(503,"Restore the original sandbox credentials before reconciliation");
    try {
      return await this.send(base + path, { method, body, redirect: "manual", signal: AbortSignal.timeout(20_000),
        headers: { Authorization: "Bearer " + this.key!, Project: project, "Content-Type": type } });
    } catch { throw new CloudFailure(503, "Sandbox response unavailable; inspect the saved request before retrying"); }
  }
  private async json(response: Response, maximum = 1_500_000): Promise<any> {
    if (!response.ok) { await response.body?.cancel(); throw new CloudFailure(response.status === 403 ? 403 : 502, "Sandbox permission or provider request failed"); }
    return readJson(new Request("https://response.invalid", {method: "POST", body: response.body, duplex: "half"} as RequestInit), maximum);
  }
  private async access() {
    const value = await this.json(await this.call("/whoami"), 24_000);
    return value.permissions?.spawn_disposable === true && value.permissions?.list === true && value.permissions?.cancel === true;
  }
  private rate(workspace: string) {
    const minute = Math.floor(this.clock() / 60_000);
    this.store.transaction(() => {
      for (const [scope, max] of [[workspace,300],["_operator",900]] as const) {
        const old = this.store.get<{minute:number; count:number}>(scope,"sandbox-rate","minute");
        const count = old?.minute === minute ? old.count : 0;
        if (count >= max) throw new CloudFailure(429,"Sandbox request limit reached");
        this.store.put(scope,"sandbox-rate","minute",{minute,count:count+1});
      }
    });
  }
  async handle(account: Account, request: Request): Promise<Response> {
    if (!/^[a-f0-9]{64}$/.test(account.workspaceId)) throw new CloudFailure(401,"Sign in required");
    this.store.admit(account.workspaceId); this.rate(account.workspaceId);
    if (this.active.has(account.workspaceId) || this.active.size >= 4) throw new CloudFailure(429,"Another sandbox request is active");
    this.active.add(account.workspaceId);
    try { return await this.dispatch(account,request); } finally { this.active.delete(account.workspaceId); }
  }
  private async dispatch(account: Account, request: Request): Promise<Response> {
    const url = new URL(request.url), path = url.pathname.slice("/v1/sandbox".length);
    if (path === "/config" && request.method === "GET") {
      const available = this.configured && await this.access();
      return Response.json({configured:this.configured,available,enabled:this.enabled(),
        ...(available ? {project:this.policy!.projectId,images:this.policy!.images.map(i=>({target:i.target,image:i.image,kind:i.kind,label:i.label??"Approved sandbox",
          providers:i.kind==="harness"&&Array.isArray(i.manifest.capabilities)?[...new Set(i.manifest.capabilities.map((c:any)=>c.provider).filter((p:any)=>typeof p==="string"&&/^[a-zA-Z0-9_-]{1,64}$/.test(p)))]:[]}))} : {}),
        reason: available ? null : "Sandbox activation, permissions, reviewed images and verified pricing are required"},{headers});
    }
    if(path==="/recover" && request.method==="POST") {
      const input=await readJson(request,1024);
      if(!uuid(input?.requestId)||!uuid(input.operationId)||Object.keys(input).some(k=>!["requestId","operationId"].includes(k)))throw new CloudFailure(400,"Choose the original sandbox request and operation");
      const record=this.store.get<Operation>("_operator","sandbox-request",account.workspaceId+":"+input.requestId);
      if(!record || (record.operation && record.operation!==input.operationId))throw new CloudFailure(404,"Original sandbox request unavailable");
      const collision=this.records().find(r=>r.operation===input.operationId && (r.workspace!==account.workspaceId||r.request!==input.requestId));
      if(collision)throw new CloudFailure(404,"Original sandbox request unavailable");
      const observed=await this.json(await this.call("/operations/"+input.operationId,"GET",undefined,"application/json",record.project));
      if(observed.uuid!==input.operationId||observed.kind!=="instance"||observed.image_uuid!==record.body.image||
         !observed.metadata||!Object.keys(record.body).every(k=>canonical(observed.metadata[k])===canonical(providerBody(record)[k])))
        throw new CloudFailure(404,"Operation does not prove the original sandbox request");
      record.operation=input.operationId;this.save(record);
      return Response.json({uuid:record.operation},{headers});
    }
    const match = /^\/operations\/([0-9a-f-]{36})$/.exec(path);
    if (match && ["GET","DELETE"].includes(request.method)) {
      const record = this.records().find(r=>r.workspace === account.workspaceId && r.operation === match[1]);
      if (!record) throw new CloudFailure(404,"Sandbox operation unavailable");
      if (request.method === "DELETE") { await this.cancel(record); return new Response(null,{status:202,headers}); }
      return Response.json(await this.observe(record),{headers});
    }
    if (!this.configured) throw new CloudFailure(503,"Managed sandbox activation, images and pricing are not configured");
    if (path.startsWith("/images/") && request.method === "GET") {
      const image = this.policy!.images.find(i=>i.target === path.slice(8) && i.kind === "command");
      if (!image) throw new CloudFailure(404,"Registered sandbox image unavailable");
      return Response.json(image.policy ?? image.manifest,{headers});
    }
    const inspect = /^\/inspect\/([0-9a-f-]{36})\/download$/.exec(path);
    if (inspect && request.method === "GET") {
      const image = this.policy!.images.find(i=>i.image === inspect[1]), name = url.searchParams.get("path");
      if (!image || !["/ion/host.json","/ion/job-host.json"].includes(name ?? "") || [...url.searchParams.keys()].some(k=>k!=="path"))
        throw new CloudFailure(404,"Registered sandbox manifest unavailable");
      // Compare actual image bytes to the reviewed registry before declaring readiness.
      const manifest = await this.json(await this.call(path+"?"+new URLSearchParams({path:name!})),64_000);
      if (canonical(manifest) !== canonical(image.manifest)) throw new CloudFailure(409,"Registered sandbox manifest changed");
      return Response.json(manifest,{headers});
    }
    if (path === "/files" && request.method === "POST") return this.upload(account,request);
    if (/^\/files\/[a-f0-9]{64}$/.test(path) && request.method === "GET") {
      const file = this.store.get<FileRecord>(account.workspaceId,"sandbox-file",path.slice(7));
      if (!file) throw new CloudFailure(404,"Uploaded file unavailable");
      if (!file.uuid) throw new CloudFailure(409,"Upload acknowledgement is uncertain; reconcile before continuing");
      return Response.json({sha256:file.sha256,size:file.size,uuid:file.uuid},{headers});
    }
    if (path === "/instances" && request.method === "POST") return this.spawn(account,request);
    throw new CloudFailure(404,"Unsupported managed sandbox operation");
  }
  private async upload(account: Account, request: Request) {
    if(this.uploadActive)throw new CloudFailure(429,"Another project upload is active");
    this.uploadActive=true;
    try{return await this.uploadFile(account,request);}finally{this.uploadActive=false;}
  }
  private async uploadFile(account: Account, request: Request) {
    if(!this.policy!.maximumUploadMicroUSD)throw new CloudFailure(503,"Sandbox upload retention and pricing require operator verification");
    if (!this.enabled()) throw new CloudFailure(403,"Shared Token Factory is OFF");
    if (!await this.access()) throw new CloudFailure(403,"Nebius sandbox permission or project activation is missing");
    const reader=request.body?.getReader(); if(!reader)throw new CloudFailure(400,"Provide a bounded project archive");
    const chunks:Uint8Array[]=[];let length=0;
    while(true){const part=await reader.read();if(part.done)break;length+=part.value.length;if(length>20_000_000){await reader.cancel();throw new CloudFailure(413,"Sandbox project archive exceeds its limit");}chunks.push(part.value);}
    const bytes=Buffer.concat(chunks),digest=hash(bytes);
    const old=this.store.get<FileRecord>(account.workspaceId,"sandbox-file",digest);
    if(old?.uuid)return Response.json({sha256:old.sha256,size:old.size,uuid:old.uuid},{status:201,headers});
    if(old)throw new CloudFailure(409,"Upload outcome requires reconciliation; no repeated upload");
    if(this.store.all(account.workspaceId,"sandbox-file").length>=8)throw new CloudFailure(429,"Sandbox upload history needs owner review");
    if(!this.enabled())throw new CloudFailure(403,"Shared Token Factory is OFF");
    const reservation=this.store.transaction(()=>{
      const existing=this.store.all<FileRecord>(account.workspaceId,"sandbox-file");
      if(existing.reduce((n,f)=>n+f.size,0)+bytes.length>20_000_000)throw new CloudFailure(429,"Sandbox transfer storage limit reached");
      const reservation=this.budget.reserve("sandbox",account.workspaceId,this.policy!.maximumUploadMicroUSD!,this.policy!.limits);
      this.store.put(account.workspaceId,"sandbox-file",digest,{sha256:digest,size:bytes.length,reservation} satisfies FileRecord);
      return reservation;
    });
    const file=await this.json(await this.call("/files","POST",bytes,"application/octet-stream"),16_000);
    if(!uuid(file.uuid)||file.sha256!==digest||file.size!==bytes.length)throw new CloudFailure(502,"Uploaded project identity changed; reconcile before continuing");
    const saved={sha256:digest,size:bytes.length,uuid:file.uuid};this.store.put(account.workspaceId,"sandbox-file",digest,{...saved,reservation});
    return Response.json(saved,{status:201,headers});
  }
  private async spawn(account:Account,request:Request) {
    if (!this.enabled()) throw new CloudFailure(403,"Shared Token Factory is OFF");
    const body=await readJson(request,800_000), image=this.policy!.images.find(i=>i.image===body?.image);
    const id=typeof body?.hostname === "string" ? /^ion-(?:job-)?([0-9a-f-]{36})$/.exec(body.hostname)?.[1] : undefined;
    if(!image || !uuid(id) || body.disposable!==true || body.shell!==false || body.preserve_env!==false ||
       !Number.isInteger(body.timeout)||body.timeout<1||body.timeout>this.policy!.maxRuntimeSeconds ||
       !Number.isInteger(body.truncate_output_at)||body.truncate_output_at<1||body.truncate_output_at>524288 ||
       !body.stdin||body.stdin.encoding!=="base64"||body.stdin.close!==true||typeof body.stdin.value!=="string"||body.stdin.value.length>700000 ||
       !body.resources_limits||!Number.isInteger(body.resources_limits.max_layer_bytes)||body.resources_limits.max_layer_bytes<1||body.resources_limits.max_layer_bytes>1073741824 ||
       Object.keys(body).some(k=>!["image","hostname","command","args","shell","env","cwd","uid","gid","stdin","timeout","truncate_output_at","disposable","preserve_env","resources_limits","networking","files"].includes(k)))
      throw new CloudFailure(400,"Use the exact bounded registered sandbox worker request");
    if(Object.keys(body.stdin).some(k=>!["value","encoding","close"].includes(k))||Object.keys(body.resources_limits).some(k=>k!=="max_layer_bytes")||
       Object.keys(body.networking??{}).some(k=>k!=="enabled")||!/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(body.stdin.value))
      throw new CloudFailure(400,"Invalid sandbox input envelope");
    let input:any;
    try {input=JSON.parse(Buffer.from(body.stdin.value,"base64").toString("utf8"));}catch{throw new CloudFailure(400,"Invalid sandbox worker input");}
    if(input?.version!==1||input.request_id!==id)throw new CloudFailure(400,"Sandbox worker request identity changed");
    if(image.kind==="command") {
      if(body.command!=="/usr/local/bin/ion-job-worker" || body.cwd!=="/ion" || body.uid!==0 || body.gid!==0 || body.networking?.enabled!==false ||
         !body.env || Object.keys(body.env).some(k=>!["PATH","ION_JOB_ISOLATED","ION_JOB_CHILD_UID","ION_JOB_CHILD_GID","ION_JOB_RESOURCE_LIMITS"].includes(k)) ||
         body.env.PATH!=="/usr/local/bin:/usr/bin:/bin"||body.env.ION_JOB_ISOLATED!=="1" ||
         body.env.ION_JOB_CHILD_UID!==String(image.policy?.uid)||body.env.ION_JOB_CHILD_GID!==String(image.policy?.gid)||
         typeof body.env.ION_JOB_RESOURCE_LIMITS!=="string") throw new CloudFailure(400,"Command image isolation policy changed");
      let resources:any;try{resources=JSON.parse(body.env.ION_JOB_RESOURCE_LIMITS);}catch{throw new CloudFailure(400,"Invalid command resource policy");}
      if(canonical(resources)!==canonical(image.policy?.resources)||!Array.isArray(input.argv)||input.argv.length<1||input.argv.length>64||
         !Array.isArray(image.policy?.executables)||!image.policy.executables.includes(input.argv[0])||input.argv.some((a:any)=>typeof a!=="string"||a.includes("\0")||a.length>8192)||
         !Number.isInteger(input.timeout_seconds)||input.timeout_seconds<1||input.timeout_seconds>Number(image.policy?.max_seconds)||body.timeout!==input.timeout_seconds+10||
         !Array.isArray(input.inputs)||input.inputs.length>32||!Array.isArray(input.output_paths)||input.output_paths.length>32||
         body.resources_limits.max_layer_bytes!==image.policy?.max_layer_bytes)throw new CloudFailure(400,"Command request exceeds its registered policy");
    } else {
      if(body.command!=="/usr/local/bin/ion-harness-worker"||body.cwd!=="/ion/work"||body.uid!==1000||body.gid!==1000||body.env?.HOME!=="/home/ion"||
         body.env?.PATH!=="/usr/local/bin:/usr/bin:/bin"||Object.keys(body.env).some(k=>!["PATH","HOME"].includes(k))||body.networking?.enabled!==true)
        throw new CloudFailure(400,"Harness image identity policy changed");
      const capabilities=image.manifest.capabilities;
      if(Buffer.from(body.stdin.value,"base64").length>131072 || !Array.isArray(capabilities) ||
         !capabilities.some((cap:any)=>canonical(cap)===canonical(input.capability)) ||
         !Number.isInteger(input.capability?.input_cap)||input.capability.input_cap<1||input.capability.input_cap>131072 ||
         !Number.isInteger(input.capability?.output_cap)||input.capability.output_cap<1||input.capability.output_cap>4096 ||
         canonical(input.execution)!==canonical(image.manifest.execution)||typeof input.system!=="string"||input.system.length>32768 ||
         !Array.isArray(input.messages)||input.messages.length>256||!Array.isArray(input.tools)||input.tools.length>64)
        throw new CloudFailure(400,"Harness request differs from its reviewed model and usage capability");
    }
    if(!Array.isArray(body.args)||JSON.stringify(body.args)!==JSON.stringify(body.files?["--project-file","/ion/job-project.ion"] : []))throw new CloudFailure(400,"Worker arguments changed");
    if(body.files && image.kind!=="command")throw new CloudFailure(400,"Harness workers cannot receive project-file arguments");
    if(body.files){
      const files=this.store.all<FileRecord>(account.workspaceId,"sandbox-file");
      if(Object.keys(body.files).length!==1||!files.some(f=>f.uuid&&f.uuid===body.files["/ion/job-project.ion"]?.uuid)||
         body.files["/ion/job-project.ion"].uid!==0||body.files["/ion/job-project.ion"].gid!==0||body.files["/ion/job-project.ion"].mode!=="0400")throw new CloudFailure(403,"Project upload is not owned by this account");
    }
    const signature=hash(canonical(body)),old=this.store.get<Operation>("_operator","sandbox-request",account.workspaceId+":"+id);
    if(old){if(old.signature!==signature)throw new CloudFailure(409,"Sandbox request payload changed");if(!old.operation)throw new CloudFailure(409,"Sandbox launch acknowledgement is uncertain; do not resubmit");return Response.json({uuid:old.operation},{status:201,headers});}
    if(!await this.access())throw new CloudFailure(403,"Nebius sandbox permission or project activation is missing");
    let record!:Operation;
    this.store.transaction(()=>{
      if(!this.enabled())throw new CloudFailure(403,"Shared Token Factory is OFF");
      const records=this.records(),pending=records.filter(r=>!r.receipt);
      if(records.length>=128||records.filter(r=>r.workspace===account.workspaceId).length>=32||pending.length>=3||pending.some(r=>r.workspace===account.workspaceId))throw new CloudFailure(429,"Sandbox concurrency or history limit reached");
      const reservation=this.budget.reserve("sandbox",account.workspaceId,this.policy!.maximumOperationMicroUSD,this.policy!.limits);
      record={workspace:account.workspaceId,project:this.policy!.projectId,request:id!,signature,body,created:this.clock(),reservation};this.save(record);
    });
    const response=await this.call("/instances","POST",JSON.stringify(providerBody(record))),value=await this.json(response,800_000);
    if(response.status!==201||!uuid(value.uuid))throw new CloudFailure(502,"Sandbox launch acknowledgement is uncertain; no fallback or resubmission");
    if(this.records().some(r=>r.operation===value.uuid))throw new CloudFailure(502,"Sandbox operation identity already exists; owner reconciliation is required");
    record.operation=value.uuid;this.save(record);
    return Response.json({uuid:record.operation},{status:201,headers});
  }
  private async cancel(record:Operation) {
    if(record.receipt)return;
    record.cancelRequested=this.clock();this.save(record);
    const response=await this.call("/operations/"+record.operation,"DELETE",undefined,"application/json",record.project);
    if(![202,409].includes(response.status)){await response.body?.cancel();throw new CloudFailure(503,"Sandbox stop is not confirmed; inspect the original operation");}
    await response.body?.cancel();
  }
  private async observe(record:Operation):Promise<any> {
    if(record.receipt)return record.receipt;
    const value=await this.json(await this.call("/operations/"+record.operation,"GET",undefined,"application/json",record.project));
    if(value.uuid!==record.operation||value.kind!=="instance"||value.image_uuid!==record.body.image||
       !["PENDING","ASSIGNED","EXECUTING","SUCCESS","FAILED","CANCELLED"].includes(value.status))throw new CloudFailure(502,"Sandbox operation identity changed");
    if(value.metadata?.hostname!==undefined && value.metadata.hostname!==providerBody(record).hostname)throw new CloudFailure(502,"Sandbox operation account witness changed");
    // Preserve the exact request witness for native recovery, never a provider credential.
    delete value.error;
    if(["SUCCESS","FAILED","CANCELLED"].includes(value.status)) {
      if(value.metadata?.hostname!==providerBody(record).hostname)throw new CloudFailure(502,"Sandbox terminal account witness is missing");
      if(value.result_image_uuid || value.result?.image)throw new CloudFailure(502,"Disposable sandbox unexpectedly retained an image; owner review required");
      // Native protocols use the logical hostname; the provider witness has
      // already been bound to this authenticated namespace above.
      value.metadata.hostname=record.body.hostname;record.receipt=value;this.save(record);
      // Raw operation cost has unspecified units. Keep the full hold until billing reconciliation.
    }
    if(value.metadata)value.metadata.hostname=record.body.hostname;
    return value;
  }
  async maintenance() {
    if(!this.key)return;
    for(const record of this.records().filter(r=>!r.receipt&&r.operation)) {
      try {
        if(!this.enabled()||this.clock()>=record.created+(record.body.timeout+15)*1000)await this.cancel(record);
        await this.observe(record);
      }catch{ /* Preserve the durable request and its full hold until confirmed. */ }
    }
  }
}
