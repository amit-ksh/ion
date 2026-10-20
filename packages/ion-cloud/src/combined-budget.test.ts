import { expect, test } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { Budget, pilotEnvelope } from "./budget";
import { ServiceStore } from "./service-store";
test("combined envelope covers all dispatch types, old counters, unknown holds and restarts", () => {
  const directory = mkdtempSync(join(tmpdir(), "ion-envelope-")), store = new ServiceStore(directory);
  try {
    let time = Date.UTC(2026,9,20);
    const envelope = { monthly: 100, lifetime: 150 }, limits = { monthly: 1000, lifetime: 1000, accountLifetime: 1000 };
    const budget = new Budget(store, () => time, envelope);
    const inference = budget.reserve("tokenFactory", "owner", 30, limits);
    budget.reserve("cloud", "_compute", 30, limits);
    const sandbox = budget.reserve("sandbox", "judge", 30, limits);
    expect(() => budget.reserve("composio", "judge", 11, limits)).toThrow("Combined");
    budget.reserve("composio", "judge", 10, limits);
    expect(budget.combined().total).toEqual({ spent:0, held:100 });
    budget.settle(inference,10); budget.settle(inference,10);
    expect(budget.combined().total).toEqual({ spent:10, held:70 });
    const restarted = new Budget(store, () => time, envelope);
    expect(restarted.combined().total).toEqual(budget.combined().total);
    time=Date.UTC(2026,10,1);
    expect(restarted.combined().monthly).toEqual({ spent:0, held:0 });
    expect(() => restarted.reserve("sandbox","owner",71,limits)).toThrow("Combined");
    restarted.reserve("sandbox","owner",70,limits);
    restarted.settle(sandbox,35); // provider overage remains debt and blocks new dispatch
    expect(restarted.combined().total).toEqual({spent:45,held:110});
    expect(() => restarted.reserve("cloud","_compute",1,limits)).toThrow("Combined");
    // Envelope configuration cannot be widened after constructing the service.
    envelope.lifetime=1000;
    expect(restarted.combined().limits.lifetime).toBe(150);
    expect(() => new Budget(store, Date.now, {...pilotEnvelope,lifetime:15_000_001})).toThrow("authorization");
    expect(() => new Budget(store, Date.now, {...pilotEnvelope,monthly:pilotEnvelope.monthly+1})).toThrow("authorization");
    expect(() => new Budget(store, Date.now, {...pilotEnvelope,lifetime:pilotEnvelope.lifetime+1})).toThrow("authorization");
  } finally { store.close(); rmSync(directory,{recursive:true,force:true,maxRetries:5,retryDelay:100}); }
});
test("combined envelope picks up counters written before deployment and rolls back rejected multi-service reservations", () => {
  const directory=mkdtempSync(join(tmpdir(),"ion-envelope-")),store=new ServiceStore(directory);
  try {
    const month=new Date().toISOString().slice(0,7);
    store.put("_operator","budget","tokenFactory:total",{spent:40,held:40});
    store.put("_operator","budget","tokenFactory:month:"+month,{spent:40,held:40});
    const budget=new Budget(store,Date.now,{monthly:100,lifetime:100});
    expect(()=>store.transaction(()=>{
      budget.reserve("cloud","_compute",15,{monthly:100,lifetime:100,accountLifetime:100});
      budget.reserve("sandbox","judge",10,{monthly:100,lifetime:100,accountLifetime:100});
    })).toThrow("Combined");
    expect(budget.combined().total).toEqual({spent:40,held:40});
    expect(budget.usage("cloud").total).toEqual({spent:0,held:0});
  } finally { store.close();rmSync(directory,{recursive:true,force:true,maxRetries:5,retryDelay:100}); }
});
