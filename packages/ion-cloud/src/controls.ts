import { CloudFailure, type Account } from "./auth";
import type { Admission } from "./admission";
import { readJson, ServiceStore } from "./service-store";
export interface ServiceControls {
  version: 1;
  cloud: boolean;
  tokenFactory: boolean;
  appActions?: boolean;
  updated: number;
  cloudUpdated?: number;
}
const scope = "_operator";
/** Shared owner controls survive restarts and default to no paid work. */
export class Controls {
  constructor(
    private readonly store: ServiceStore,
    private readonly admission: Admission,
    private readonly available: { cloud:boolean; tokenFactory:boolean; appActions?:boolean } = { cloud: false, tokenFactory: false },
  ) {}
  get(): ServiceControls {
    const saved = this.store.get<ServiceControls>(
      scope,
      "controls",
      "services",
    );
    if (!saved)
      return { version: 1, cloud: false, tokenFactory: false, appActions: false, updated: 0 };
    if (
      saved.version !== 1 ||
      typeof saved.cloud !== "boolean" ||
      typeof saved.tokenFactory !== "boolean" ||
      !Number.isSafeInteger(saved.updated) ||
      saved.updated < 0
    )
      throw new CloudFailure(503, "Owner service controls need repair");
    if(saved.appActions!==undefined && typeof saved.appActions!=="boolean")throw new CloudFailure(503,"Owner app controls need repair");
    return {...saved,appActions:saved.appActions??false};
  }
  get cloudAvailable() {
    return this.available.cloud;
  }
  computeEnabled = () => this.available.cloud && this.get().cloud;
  async handle(account: Account, request: Request): Promise<Response> {
    if (request.method === "GET")
      return Response.json(
        {
          ...this.get(),
          cloudAvailable: this.available.cloud,
          tokenFactoryAvailable: this.available.tokenFactory,
          appActionsAvailable: this.available.appActions??false,
          canManage: this.admission.admit(account) === "owner",
        },
        { headers: { "Cache-Control": "no-store" } },
      );
    this.admission.owner(account);
    if (request.method !== "PUT")
      throw new CloudFailure(405, "Method not allowed");
    const input = await readJson(request, 1024);
    if (
      !input ||
      typeof input !== "object" ||
      Array.isArray(input) ||
      !Object.keys(input).length ||
      Object.keys(input).some(
        (k) =>
          !["cloud", "tokenFactory", "appActions"].includes(k) ||
          typeof input[k] !== "boolean",
      )
    )
      throw new CloudFailure(400, "Set boolean shared service switches");
    if (input.cloud === true && !this.available.cloud)
      throw new CloudFailure(
        503,
        "Automatic VM lifecycle and spending controls are not configured yet",
      );
    if (input.tokenFactory === true && !this.available.tokenFactory)
      throw new CloudFailure(
        503,
        "Shared Token Factory inference and spending controls are not configured yet",
      );
    const next = this.store.transaction(() => {
      if(input.appActions===true&&!this.available.appActions)throw new CloudFailure(503,"Managed app actions are not configured");
      const saved = this.get();
      const next: ServiceControls = {
        ...saved,
        ...input,
        version: 1,
        updated: Date.now(),
        cloudUpdated: typeof input.cloud === "boolean" && input.cloud !== saved.cloud ? Date.now() : (saved.cloudUpdated ?? 0),
      };
      this.store.put(scope, "controls", "services", next);
      return next;
    });
    // This gates new work and runtime routing. It does not assert that a VM has stopped.
    return Response.json(
      {
        ...next,
        cloudAvailable: this.available.cloud,
        tokenFactoryAvailable: this.available.tokenFactory,
        appActionsAvailable: this.available.appActions??false,
        canManage: true,
        computeStopPending: !next.cloud,
      },
      { headers: { "Cache-Control": "no-store" } },
    );
  }
}
