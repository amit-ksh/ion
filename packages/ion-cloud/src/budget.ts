import { CloudFailure } from "./auth";
import { ServiceStore, secret } from "./service-store";
export type Meter = "cloud" | "tokenFactory";
export interface Limits { monthly: number; lifetime: number; accountLifetime: number }
interface Counter { spent: number; held: number }
interface Reservation { meter: Meter; workspace: string; month: string; maximum: number; settled?: number }
const scope = "_operator";
const zero = (): Counter => ({ spent: 0, held: 0 });
export function money(value: number): number {
  if (!Number.isSafeInteger(value) || value < 0 || value > 1_000_000_000)
    throw new Error("Invalid micro-USD amount");
  return value;
}
/** Micro-USD reservations are charged before dispatch. Unknown outcomes retain
 * their full reservation forever; only a trusted provider receipt can settle it. */
export class Budget {
  constructor(private store: ServiceStore, private clock = Date.now) {}
  private counter(id: string): Counter {
    const c = this.store.get<Counter>(scope, "budget", id) ?? zero();
    money(c.spent); money(c.held); return c;
  }
  private keys(meter: Meter, workspace: string, month: string) {
    return [meter + ":total", meter + ":month:" + month, meter + ":account:" + workspace];
  }
  reserve(meter: Meter, workspace: string, maximum: number, limits: Limits): string {
    money(maximum); Object.values(limits).forEach(money);
    if (!maximum) throw new Error("A positive reservation is required");
    const month = new Date(this.clock()).toISOString().slice(0, 7);
    return this.store.transaction(() => {
      const keys = this.keys(meter, workspace, month), ceilings = [limits.lifetime, limits.monthly, limits.accountLifetime];
      for (let i = 0; i < keys.length; i++) {
        const row = this.counter(keys[i]!);
        if (row.spent + row.held + maximum > ceilings[i]!)
          throw new CloudFailure(429, "Shared " + meter + " spending limit reached");
      }
      const id = secret();
      for (const key of keys) {
        const row = this.counter(key); row.held += maximum;
        this.store.put(scope, "budget", key, row);
      }
      this.store.put(scope, "reservation", id, { meter, workspace, month, maximum } satisfies Reservation);
      return id;
    });
  }
  settle(id: string, actual: number) {
    money(actual);
    this.store.transaction(() => {
      const r = this.store.get<Reservation>(scope, "reservation", id);
      if (!r) throw new Error("Unknown spending reservation");
      if (r.settled !== undefined) {
        if (r.settled !== actual) throw new Error("Conflicting spending receipt");
        return;
      }
      // Actual overages are debt, never discarded or a reason to release funds.
      for (const key of this.keys(r.meter, r.workspace, r.month)) {
        const row = this.counter(key); row.held -= r.maximum; row.spent += actual;
        this.store.put(scope, "budget", key, row);
      }
      r.settled = actual; this.store.put(scope, "reservation", id, r);
    });
  }
  usage(meter: Meter, workspace?: string) {
    const month = new Date(this.clock()).toISOString().slice(0, 7);
    return { unit: "microUSD", month, total: this.counter(meter + ":total"),
      monthly: this.counter(meter + ":month:" + month),
      ...(workspace ? { account: this.counter(meter + ":account:" + workspace) } : {}) };
  }
}
export function tokenCost(tokens: number, microUSDPerMillion: number): number {
  money(tokens); money(microUSDPerMillion);
  return Number((BigInt(tokens) * BigInt(microUSDPerMillion) + 999_999n) / 1_000_000n);
}
