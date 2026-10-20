import { expect, test } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { ServiceStore } from "./service-store";
import { Budget } from "./budget";
import { Inference, inferencePolicy } from "./inference";
import { Compute, computePolicy, type VMState } from "./compute";
import type { Account } from "./auth";
async function fixture(run: (store: ServiceStore) => Promise<void>) {
  const directory = mkdtempSync(join(tmpdir(), "ion-spending-"));
  const store = new ServiceStore(directory);
  try { await run(store); } finally { store.close(); rmSync(directory, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 }); }
}
const account = (workspaceId = "a".repeat(64)): Account => ({ subject: "judge", issuer: "https://issuer.example/", workspaceId, scopes: new Set(["ion:gateway"]) });
const limits = { monthly: 100, lifetime: 200, accountLifetime: 80 };
test("spending reserves atomically across accounts, preserves unknown outcomes and settles only once", () => fixture(async store => {
  let now = Date.UTC(2026, 9, 20); const budget = new Budget(store, () => now);
  const one = budget.reserve("tokenFactory", "one", 70, limits);
  expect(() => budget.reserve("tokenFactory", "one", 11, limits)).toThrow("limit");
  expect(() => budget.reserve("tokenFactory", "two", 31, limits)).toThrow("limit");
  const two = budget.reserve("tokenFactory", "two", 30, limits);
  budget.settle(one, 20); budget.settle(one, 20);
  expect(() => budget.settle(one, 19)).toThrow("Conflicting");
  expect(budget.usage("tokenFactory").total).toEqual({ spent: 20, held: 30 });
  // A new budget instance retains the unknown request; advancing a month does not refund it.
  now = Date.UTC(2026, 10, 1);
  const restarted = new Budget(store, () => now);
  expect(restarted.usage("tokenFactory").total.held).toBe(30);
  expect(restarted.usage("tokenFactory").monthly).toEqual({ spent: 0, held: 0 });
  restarted.settle(two, 35); // A provider-reported overage is debt, not silently capped.
  expect(restarted.usage("tokenFactory").total.spent).toBe(55);
  expect(restarted.usage("tokenFactory").monthly.spent).toBe(0);
  expect(() => store.transaction(() => { restarted.reserve("cloud", "one", 10, limits); throw new Error("rollback"); })).toThrow("rollback");
  expect(restarted.usage("cloud").total).toEqual({ spent: 0, held: 0 });
}));
const inferenceConfig = inferencePolicy(JSON.stringify({ models: [{ id: "fixture-model", contextTokens: 4096, outputTokens: 10,
  inputMicroUSDPerMillion: 1_000_000, outputMicroUSDPerMillion: 1_000_000 }], limits: { monthly: 50_000, lifetime: 50_000, accountLifetime: 10_000 } }))!;
const completion = (body: unknown) => new Request("https://api.example/v1/inference/chat/completions", { method: "POST", body: JSON.stringify(body) });
const prompt = { model: "fixture-model", messages: [{ role: "user", content: "hello" }], stream: true };
test("shared proxy admits only approved requests, settles exact usage and emits compatible tool SSE", () => fixture(async store => {
  let enabled = false, calls = 0; const budget = new Budget(store);
  const proxy = new Inference(budget, () => enabled, "fixture-key", inferenceConfig, (async (url: any, options: any) => {
    calls++; expect(String(url)).toBe("https://api.tokenfactory.nebius.com/v1/chat/completions");
    expect(options.redirect).toBe("manual"); expect(options.headers.Authorization).toBe("Bearer fixture-key");
    const body = JSON.parse(options.body); expect(body.stream).toBe(true); expect(body.max_completion_tokens).toBe(10);
    const packet = { id: "fixture", model: "fixture-model", choices: [{ index: 0, delta: { role: "assistant", content: null,
      tool_calls: [{ index: 0, id: "call-1", type: "function", function: { name: "read_file", arguments: "{}" } }] }, finish_reason: "tool_calls" }],
      usage: { prompt_tokens: 2, completion_tokens: 3, total_tokens: 5 } };
    return new Response("data: " + JSON.stringify(packet) + "\n\ndata: [DONE]\n\n", { headers: { "Content-Type": "text/event-stream" } });
  }));
  await expect(proxy.handle(account(), completion(prompt))).rejects.toThrow("OFF");
  enabled = true;
  await expect(proxy.handle(account(), completion({ ...prompt, model: "other" }))).rejects.toThrow("approved");
  await expect(proxy.handle(account(), completion({ ...prompt, n: 2 }))).rejects.toThrow("approved");
  expect(calls).toBe(0);
  const response = await proxy.handle(account(), completion(prompt)); const sse = await response.text();
  expect(sse).toContain('"index":0'); expect(sse).toContain("[DONE]");
  expect(budget.usage("tokenFactory", account().workspaceId).account).toEqual({ spent: 5, held: 0 });
}));
test("missing usage and network failure keep the full hold and block subsequent spending", () => fixture(async store => {
  const budget = new Budget(store), configuration = { ...inferenceConfig, limits: { monthly: 4200, lifetime: 4200, accountLifetime: 4200 } };
  const proxy = new Inference(budget, () => true, "key", configuration, (async () => Response.json({ model: "fixture-model", choices: [] })));
  await expect(proxy.handle(account(), completion({ ...prompt, stream: false }))).rejects.toThrow("receipt");
  expect(budget.usage("tokenFactory").total.held).toBe(4106);
  await expect(proxy.handle(account(), completion(prompt))).rejects.toThrow("limit");
  const failing = new Inference(budget, () => true, "key", inferenceConfig, (async () => { throw new Error("secret upstream diagnostics"); }));
  await expect(failing.handle(account("b".repeat(64)), completion(prompt))).rejects.toThrow("uncertain");
  expect(budget.usage("tokenFactory").total.held).toBe(8212);
}));
const computeConfig = computePolicy(JSON.stringify({ instanceId: "computeinstance-fixture", projectId: "project-e04qppz9bc0026zt9y9ky7",
  microUSDPerHour: 1_000_000, maxRuntimeSeconds: 120, idleSeconds: 30, startupSeconds: 30, stopAllowanceSeconds: 120,
  limits: { monthly: 7_000_000, lifetime: 22_000_000, accountLifetime: 22_000_000 } }))!;
test("compute coalesces wake, reserves before start, idles/stops and settles only a confirmed provider stop", () => fixture(async store => {
  let now = Date.UTC(2026, 9, 20), enabled = false, updated = now;
  const budget = new Budget(store, () => now), compute = new Compute(store, budget, () => enabled, () => updated, computeConfig, "t".repeat(43), () => now);
  const tick = async (state: VMState, operationSettled = true, controllerId = "controller-" + "x".repeat(24)) => {
    const response = await compute.tick(new Request("https://api.example/internal/compute/tick", { method: "POST", headers: { Authorization: "Bearer " + "t".repeat(43) },
      body: JSON.stringify({ instanceId: computeConfig.instanceId, projectId: computeConfig.projectId, controllerId, state, operationSettled }) }));
    return response.json() as Promise<{ action: string }>;
  };
  expect(compute.available).toBe(false); await tick("STOPPED"); expect(compute.available).toBe(true);
  enabled = true; compute.demand();
  expect((await tick("STOPPED")).action).toBe("start");
  const held = budget.usage("cloud").total.held; expect(held).toBeGreaterThan(0);
  expect((await tick("STOPPED", false)).action).toBe("observe"); expect(budget.usage("cloud").total.held).toBe(held);
  await expect(tick("RUNNING", true, "foreign-controller-" + "x".repeat(24))).rejects.toThrow("Another controller");
  now += 1000; await tick("RUNNING"); now += 31_000;
  expect((await tick("RUNNING")).action).toBe("stop"); expect(compute.status().stopConfirmed).toBe(false);
  now += 1000; await tick("STOPPED"); expect(budget.usage("cloud").total.held).toBe(0);
  expect(compute.status().stopConfirmed).toBe(true);
  compute.demand(); expect((await tick("STOPPED")).action).toBe("start");
  now += 1000; await tick("RUNNING"); now += 121_000;
  expect((await tick("RUNNING")).action).toBe("stop"); expect(compute.status().paused).toBe(true);
  now += 1; updated = now; await tick("STOPPED"); expect(compute.status().paused).toBe(false);
  now += 46_000; expect(compute.available).toBe(false); expect(() => compute.demand()).toThrow("offline");
}));
test("ambiguous startup retains its hold and never silently starts a second session", () => fixture(async store => {
  let now = Date.UTC(2026, 9, 20); const budget = new Budget(store, () => now);
  const compute = new Compute(store, budget, () => true, () => 0, computeConfig, "t".repeat(43), () => now);
  const tick = async () => (await compute.tick(new Request("https://api.example/internal/compute/tick", { method: "POST", headers: { Authorization: "Bearer " + "t".repeat(43) },
    body: JSON.stringify({ instanceId: computeConfig.instanceId, projectId: computeConfig.projectId, controllerId: "c".repeat(24), state: "STOPPED", operationSettled: true }) }))).json() as Promise<{ action: string }>;
  await tick(); compute.demand(); expect((await tick()).action).toBe("start");
  const hold = budget.usage("cloud").total.held; now += 31_000;
  expect((await tick()).action).toBe("observe"); expect(compute.status().paused).toBe(true);
  expect(budget.usage("cloud").total.held).toBe(hold);
}));

test("a live stream stays single-flight and incomplete usage never emits successful completion", () => fixture(async store => {
  const budget = new Budget(store); let source!: ReadableStreamDefaultController<Uint8Array>;
  const proxy = new Inference(budget, () => true, "key", inferenceConfig, async () => new Response(new ReadableStream<Uint8Array>({ start(c) { source = c; } }), { headers: { "Content-Type": "text/event-stream" } }));
  const response = await proxy.handle(account(), completion(prompt));
  await expect(proxy.handle(account(), completion(prompt))).rejects.toThrow("already active");
  source.enqueue(new TextEncoder().encode('data: {"model":"fixture-model","choices":[{"index":0,"delta":{"content":"partial"},"finish_reason":"stop"}]}\n\ndata: [DONE]\n\n'));
  source.close(); const text = await response.text();
  expect(text).toContain("partial"); expect(text).toContain('"error"'); expect(text).not.toContain("data: [DONE]");
  expect(budget.usage("tokenFactory").total.held).toBe(4106);
}));
