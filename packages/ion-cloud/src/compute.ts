import { Budget, money, type Limits } from "./budget";
import { CloudFailure } from "./auth";
import { ServiceStore, hash, readJson } from "./service-store";
export type VMState = "RUNNING" | "STOPPED" | "STARTING" | "STOPPING" | "UNKNOWN";
export interface ComputePolicy {
  instanceId: string; projectId: string; microUSDPerHour: number;
  maxRuntimeSeconds: number; idleSeconds: number; startupSeconds: number;
  stopAllowanceSeconds: number; limits: Limits;
}
interface State {
  observed: VMState; seen: number; controller?: string; demand: number;
  started?: number; stopAt?: number; reservation?: string; haltedAt?: number; confirmedStarted?: boolean;
  command?: { action: "start" | "stop"; issued: number };
}
export function computePolicy(raw?: string): ComputePolicy | undefined {
  if (!raw) return;
  const p = JSON.parse(raw) as ComputePolicy;
  if (!p || !/^computeinstance-[a-z0-9]+$/.test(p.instanceId) ||
      p.projectId !== "project-e04qppz9bc0026zt9y9ky7" || !p.limits ||
      p.limits.monthly > 7_000_000 || p.limits.lifetime > 22_000_000)
    throw new Error("Set the pilot instance and Cloud limits with a USD 3 non-runtime allowance");
  money(p.microUSDPerHour); Object.values(p.limits).forEach(money);
  if (!p.microUSDPerHour || !p.limits.monthly || !p.limits.lifetime || !p.limits.accountLifetime)
    throw new Error("Set conservative verified compute prices and positive limits");
  for (const [value, min, max] of [[p.maxRuntimeSeconds, 120, 3600], [p.idleSeconds, 30, 300],
      [p.startupSeconds, 30, 600], [p.stopAllowanceSeconds, 120, 1800]])
    if (!Number.isSafeInteger(value) || value! < min! || value! > max!) throw new Error("Invalid VM time bounds");
  return p;
}
/** The gateway keeps policy and receipts. Only a separate authenticated operator
 * controller owns gRPC credentials; never put those credentials on the app VM. */
export class Compute {
  constructor(private store: ServiceStore, private budget: Budget,
    private ownerEnabled: () => boolean, private ownerUpdated: () => number,
    private policy?: ComputePolicy, private token?: string, private clock = Date.now) {}
  get configured() { return !!this.policy && !!this.token && this.token.length >= 43; }
  private read(): State { return this.store.get<State>("_operator", "compute", "vm") ?? { observed: "UNKNOWN", seen: 0, demand: 0 }; }
  private save(s: State) { this.store.put("_operator", "compute", "vm", s); }
  get available() { const s = this.read(); return this.configured && this.clock() - s.seen < 45_000; }
  permitted = () => {
    const s = this.read();
    return this.available && this.ownerEnabled() && !(s.reservation && !s.confirmedStarted && s.haltedAt) && (!s.haltedAt || this.ownerUpdated() > s.haltedAt);
  };
  status() {
    const s = this.read();
    return { configured: this.configured, controllerOnline: this.available, state: this.available ? s.observed : "UNKNOWN",
      stopConfirmed: this.available && s.observed === "STOPPED", hardStopAt: s.stopAt ?? null,
      paused: !!s.haltedAt && (this.ownerUpdated() <= s.haltedAt || !!(s.reservation && !s.confirmedStarted)),
      note: s.reservation && !s.confirmedStarted && s.haltedAt
        ? "An uncertain start remains reserved. Operator reconciliation is required before more Cloud work."
        : null };
  }
  demand() {
    if (!this.permitted()) throw new CloudFailure(503, "Cloud is OFF, paused, or its operator controller is offline");
    const s = this.read(); s.demand = this.clock(); this.save(s);
    return this.available && s.observed === "RUNNING";
  }
  /** Called only after the app worker validates its own token. */
  workerLease(activeBuild: boolean) {
    if (activeBuild && this.permitted()) this.demand();
  }
  async tick(request: Request): Promise<Response> {
    if (!this.configured || hash(request.headers.get("authorization") ?? "") !== hash("Bearer " + this.token))
      throw new CloudFailure(401, "Compute controller authorization required");
    if (request.method !== "POST") throw new CloudFailure(405, "Method not allowed");
    const body = await readJson(request, 2000), p = this.policy!, now = this.clock();
    if (body.instanceId !== p.instanceId || body.projectId !== p.projectId ||
        !/^[A-Za-z0-9_-]{24,80}$/.test(body.controllerId) ||
        !["RUNNING", "STOPPED", "STARTING", "STOPPING", "UNKNOWN"].includes(body.state))
      throw new CloudFailure(400, "Invalid compute observation");
    return this.store.transaction(() => {
      const s = this.read();
      if (s.controller && s.controller !== body.controllerId && now - s.seen < 45_000)
        throw new CloudFailure(409, "Another controller owns the live lease");
      // A provider-confirmed stop, not a local shutdown/timeout, releases a hold.
      if (body.state === "STOPPED" && s.reservation && s.confirmedStarted && body.operationSettled === true) {
        const actualSeconds = Math.max(0, Math.ceil((now - s.started!) / 1000));
        this.budget.settle(s.reservation, Math.ceil(actualSeconds * p.microUSDPerHour / 3600));
        delete s.reservation; delete s.started; delete s.stopAt; delete s.command; delete s.confirmedStarted;
      }
      if (["STARTING", "RUNNING", "STOPPING"].includes(body.state) && s.reservation) s.confirmedStarted = true;
      s.observed = body.state; s.seen = now; s.controller = body.controllerId;
      if (s.haltedAt && this.ownerEnabled() && this.ownerUpdated() > s.haltedAt) delete s.haltedAt;
      if (["RUNNING", "STARTING"].includes(body.state) && !s.reservation) {
        // An unexpected externally-started VM is stopped, never adopted as free runtime.
        s.haltedAt = now;
      }
      const hard = !!s.stopAt && now >= s.stopAt;
      const startupFailed = body.state === "STARTING" && s.started !== undefined && now - s.started >= p.startupSeconds * 1000;
      if (hard || startupFailed) s.haltedAt = now;
      const buildDemand = this.store.db.query<{ value: string }, []>("SELECT value FROM service_objects WHERE kind='release'").all().some(row => {
        const r = JSON.parse(row.value);
        return ["queued", "building"].includes(r.status) && now - r.created < 600_000;
      });
      if (buildDemand && !s.haltedAt && this.ownerEnabled()) s.demand = now;
      const wants = this.ownerEnabled() && !s.haltedAt && now - s.demand < p.idleSeconds * 1000;
      // A start that never became observable retains its hold and latches the
      // session after the startup bound. It cannot silently reserve/start again.
      if (s.reservation && !s.confirmedStarted && now - s.started! >= p.startupSeconds * 1000) s.haltedAt = now;
      let action: "start" | "stop" | "observe" = "observe";
      if ((!wants || hard || startupFailed || !!s.haltedAt) && ["RUNNING", "STARTING", "UNKNOWN"].includes(body.state)) action = "stop";
      if (wants && body.state === "STOPPED" && !s.reservation && body.operationSettled === true) {
        const nextMonth = Date.UTC(new Date(now).getUTCFullYear(), new Date(now).getUTCMonth() + 1, 1);
        const duration = Math.min(p.maxRuntimeSeconds * 1000, nextMonth - now - 120_000);
        if (duration < 120_000) { s.haltedAt = now; }
        else {
          try {
            const maximum = Math.ceil((duration / 1000 + p.stopAllowanceSeconds) * p.microUSDPerHour / 3600);
            // Already inside the same synchronous SQL transaction.
            const id = this.budget.reserve("cloud", "_compute", maximum, p.limits);
            s.reservation = id; s.started = now; s.stopAt = now + duration; action = "start";
          } catch (error) { if (!(error instanceof CloudFailure)) throw error; s.haltedAt = now; }
        }
      }
      // Coalesce commands for 30s. The controller observes actual provider state
      // before any retry and the SDK has mutation retries disabled.
      if (action !== "observe") {
        if (s.command?.action === action && now - s.command.issued < 30_000) action = "observe";
        else s.command = { action, issued: now };
      }
      this.save(s);
      return Response.json({ action, instanceId: p.instanceId, projectId: p.projectId,
        stopAt: s.stopAt ?? null, pollSeconds: 10 }, { headers: { "Cache-Control": "no-store" } });
    });
  }
}
