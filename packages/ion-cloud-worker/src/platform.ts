import { DurableObject } from "cloudflare:workers";
import { Admission } from "../../ion-cloud/src/admission";
import { Budget } from "../../ion-cloud/src/budget";
import { Compute, computePolicy } from "../../ion-cloud/src/compute";
import { Inference, inferencePolicy } from "../../ion-cloud/src/inference";
import { Controls } from "../../ion-cloud/src/controls";
import { ManagedApps } from "../../ion-cloud/src/apps";
import { Hosting } from "../../ion-cloud/src/hosting";
import { ServiceStore } from "../../ion-cloud/src/service-store";
import { CloudFailure, type Account } from "../../ion-cloud/src/auth";
import { ionAuth0 } from "../../ion-cloud/src/config";
import { durableDatabase } from "./database";
import { R2Blobs } from "./blobs";
import type { Env } from "./env";
/** Shared controls/hosting metadata; all data and capabilities retain account scope. */
export class Platform extends DurableObject<Env> {
  private readonly store: ServiceStore;
  private readonly controls: Controls;
  private readonly apps: ManagedApps;
  private readonly budget: Budget;
  private readonly compute: Compute;
  private readonly inference: Inference;
  private readonly hosting?: Hosting;
  private readonly admission: Admission;
  constructor(ctx: DurableObjectState, env: Env) {
    super(ctx, env);
    this.admission = new Admission(JSON.parse(env.MEMBERS_JSON ?? "[]"));
    this.store = new ServiceStore(durableDatabase(ctx.storage));
    this.apps = new ManagedApps(this.store, env.COMPOSIO_PROJECT_KEY);
    this.budget = new Budget(this.store);
    const self = this;
    this.controls = new Controls(this.store, this.admission, {
      get cloud() { return !!env.OBJECTS && (self.compute?.available ?? false); },
      get tokenFactory() { return self.inference?.available ?? false; },
    });
    this.compute = new Compute(this.store, this.budget, () => this.controls.get().cloud,
      () => this.controls.get().cloudUpdated ?? 0, computePolicy(env.COMPUTE_POLICY_JSON), env.COMPUTE_CONTROLLER_TOKEN);
    this.inference = new Inference(this.budget, () => this.controls.get().tokenFactory,
      env.TOKEN_FACTORY_KEY, inferencePolicy(env.TOKEN_FACTORY_POLICY_JSON));
    this.hosting = env.OBJECTS ? new Hosting(
      this.store,
      new R2Blobs(env.OBJECTS),
      {
        contentDomain: env.CONTENT_DOMAIN,
        workerToken: env.HOSTING_WORKER_TOKEN,
        workerOrigin: env.HOSTING_WORKER_ORIGIN,
        secretsKey: env.HOSTING_SECRETS_KEY,
      },
      this.compute.permitted,
      false,
      () => this.compute.demand(),
    ) : undefined;
  }
  override async fetch(request: Request): Promise<Response> {
    try {
      this.store.expire();
      const url = new URL(request.url);
      const content = await this.hosting?.content(request);
      if (content) return content;
      if (url.origin !== this.env.PUBLIC_ORIGIN)
        throw new CloudFailure(404, "Unknown service host");
      if (url.pathname === "/internal/compute/tick") {
        if (!this.hosting) throw new CloudFailure(503, "App storage is not configured yet");
        return await this.compute.tick(request);
      }
      if (url.pathname === "/internal/platform/capabilities")
        return Response.json({ managedApps: this.apps.available, htmlPublishing: !!this.hosting,
          appDeployment: !!this.hosting && this.compute.available, sharedInference: this.inference.available });
      if (url.pathname.startsWith("/internal/hosting/")) {
        if (!this.hosting) throw new CloudFailure(503, "App storage is not configured yet");
        const response = await this.hosting.worker(request);
        if (url.pathname === "/internal/hosting/desired" && response.ok) {
          const desired = await response.clone().json() as { jobs: { building: boolean }[] };
          this.compute.workerLease(desired.jobs.some(job => job.building));
        }
        return response;
      }
      // This header is overwritten by the public Worker. Objects have no public URL.
      const subject = request.headers.get("x-ion-account-subject"),
        workspaceId = request.headers.get("x-ion-workspace");
      if (!subject || !workspaceId || !/^[a-f0-9]{64}$/.test(workspaceId))
        throw new CloudFailure(401, "Sign in required");
      const account: Account = {
        subject,
        workspaceId,
        issuer: ionAuth0.issuer,
        scopes: new Set(["ion:gateway"]),
      };
      this.admission.admit(account);
      if (url.pathname.startsWith("/v1/apps/")) return await this.apps.handle(account, request);
      if (url.pathname === "/v1/services/controls") return await this.controls.handle(account, request);
      if (url.pathname === "/v1/services/usage") return Response.json({
        combined: this.budget.combined(),
        cloud: this.budget.usage("cloud", account.workspaceId), tokenFactory: this.budget.usage("tokenFactory", account.workspaceId),
        compute: this.compute.status(), owner: this.admission.admit(account) === "owner",
      }, { headers: { "Cache-Control": "no-store" } });
      if (url.pathname.startsWith("/v1/inference/")) return await this.inference.handle(account, request);
      this.store.admit(account.workspaceId);
      if (url.pathname.startsWith("/v1/hosting/")) {
        if (!this.hosting) throw new CloudFailure(503, "Publishing storage is not configured yet");
        return await this.hosting.handle(account, request);
      }
      throw new CloudFailure(404, "Not found");
    } catch (error) {
      return Response.json(
        {
          error:
            error instanceof CloudFailure
              ? error.message
              : "Request could not be completed",
        },
        {
          status: error instanceof CloudFailure ? error.status : 503,
          headers: { "Cache-Control": "no-store" },
        },
      );
    }
  }
}
