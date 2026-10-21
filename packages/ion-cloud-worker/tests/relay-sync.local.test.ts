import { expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { randomUUID } from "node:crypto";
import { existsSync, writeFileSync } from "node:fs";
import { createGatewayRuntime } from "../../personal-gateway/src/server";
import { durableDatabase } from "../src/database";

// Actual HTTP services + production DO SQL adapter. SQLite changes are local
// changed rows, NOT Cloudflare cursor rowsWritten (which also counts indexes).
function fixture() {
  const raw = new Database(":memory:");
  const counts = { requests: 0, sql: 0, selectCalls: 0, mutationCalls: 0, returnedRows: 0, changedRows: 0 };
  const database = durableDatabase({
    sql: { exec(sql: string, ...bindings: any[]) {
      counts.sql++;
      if (/^\s*(SELECT|WITH)\b/i.test(sql)) counts.selectCalls++;
      if (/^\s*(INSERT|UPDATE|DELETE|REPLACE)\b/i.test(sql)) counts.mutationCalls++;
      let rows: any[] = [];
      if (/^\s*(SELECT|WITH|PRAGMA)\b/i.test(sql)) rows = raw.query(sql).all(...bindings);
      else counts.changedRows += raw.run(sql, bindings).changes;
      counts.returnedRows += rows.length;
      return { toArray: () => rows };
    } },
    transactionSync(operation: () => unknown) { return raw.transaction(operation)(); },
  } as any);
  const owner = { id: "", token: "local_owner_" + "x".repeat(48) };
  const runtime = createGatewayRuntime({ databasePath: database, ownerToken: owner.token,
    allowedTools: [], periodicPush: false,
    pushFetch: async () => { throw new Error("Unexpected external request"); } });
  async function call(path: string, method = "GET", identity?: typeof owner, body?: unknown) {
    counts.requests++;
    const headers = new Headers();
    if (identity) { headers.set("Authorization", "Bearer " + identity.token); headers.set("X-Waku-Device-Id", identity.id); }
    if (body !== undefined) headers.set("Content-Type", "application/json");
    const response = await runtime.fetch(new Request("https://local.invalid" + path,
      { method, headers, body: body === undefined ? undefined : JSON.stringify(body) }));
    await runtime.flush(); runtime.hasPendingPush();
    return { status: response.status, body: await response.json() as any };
  }
  async function pair(grants: string[]) {
    const invite = await call("/v1/owner/invites", "POST", owner, { grants });
    const result = await call("/v1/pair", "POST", undefined, { invite: invite.body.invite, name: "Local fixture" });
    expect(result.status).toBe(201);
    return result.body as typeof owner;
  }
  return { counts, raw, owner, call, pair, reset() { for (const key of Object.keys(counts)) counts[key as keyof typeof counts] = 0; },
    async close() { await runtime.close(); raw.close(); } };
}
const projection = () => ({ version: 1, revision: 1, projects: [], definitions: [], runs: [], truncated: false });

test("desktop task bridge baseline bounds unchanged presence writes", async () => {
  const originalNow = Date.now; let now = originalNow(); Date.now = () => now;
  const f = fixture();
  try {
    const publisher = await f.pair(["publish"]);
    await f.call("/v1/device/heartbeat", "POST", publisher, {});
    f.reset();
    for (let i = 0; i < 20; i++) {
      now += 5_000;
      await f.call("/v1/device/session", "GET", publisher);
      await f.call("/v1/device/commands", "GET", publisher);
      await f.call("/v1/device/tasks/reconcile", "POST", publisher, { activeIds: [] });
      await f.call("/v1/device/heartbeat", "POST", publisher, {});
    }
    if (process.env.ION_RELAY_COUNTER_REPORT === "1" && !existsSync(new URL("../../../.waku-cache/ion-task-bridge-before.json", import.meta.url)))
      writeFileSync(new URL("../../../.waku-cache/ion-task-bridge-before.json", import.meta.url), JSON.stringify({ method: "source-derived desktop idle inventory, real HTTP and durable SQL adapter, local SQLite", simulatedSeconds: 100, counters: f.counts }, null, 2) + "\n");
    expect(f.counts.requests).toBe(80);
    expect(f.counts.changedRows).toBeLessThanOrEqual(4); // Presence plus initial task-set receipt.
  } finally { await f.close(); Date.now = originalNow; }
});

test("batched task bridge has bounded hour-long idle traffic and retains durable command ownership", async () => {
  const originalNow = Date.now; let now = originalNow(); Date.now = () => now;
  const f = fixture();
  try {
    const publisher = await f.pair(["publish"]), other = await f.pair(["publish"]);
    const phone = await f.pair(["taskRead", "taskControl"]);
    const first = await f.call("/v1/device/tasks/sync", "GET", publisher);
    expect(first.body.snapshotPresent).toBe(false);
    await f.call("/v1/device/tasks/reconcile", "POST", publisher, { activeIds: [] });
    f.reset();
    for (let i = 0; i < 20; i++) { now += 5_000; await f.call("/v1/device/tasks/sync", "GET", publisher); }
    const idle20 = { ...f.counts };
    expect(idle20.requests).toBe(20); expect(idle20.changedRows).toBe(3);
    f.reset();
    for (let i = 0; i < 360; i++) { now += 10_000; await f.call("/v1/device/tasks/sync", "GET", publisher); }
    const idleHour = { ...f.counts };
    expect(idleHour.requests).toBe(360); expect(idleHour.changedRows).toBe(120);
    expect((await f.call("/v1/device/tasks/sync", "GET", publisher)).body.snapshotPresent).toBe(true);
    const task = { id: randomUUID(), projectId: randomUUID(), projectName: "Ion", title: "Review", provider: "codex", status: "running", runtimeId: randomUUID(), updatedAt: now };
    f.reset();
    await f.call("/v1/device/tasks/sync", "GET", publisher);
    expect((await f.call(`/v1/device/tasks/${task.id}`, "PUT", publisher, { task })).body.changed).toBe(true);
    await f.call("/v1/device/tasks/reconcile", "POST", publisher, { activeIds: [task.id] });
    const changedTask = { ...f.counts };
    // Content replay and unchanged reconciliation cannot emit status again.
    f.reset();
    expect((await f.call(`/v1/device/tasks/${task.id}`, "PUT", publisher, { task })).body.changed).toBe(false);
    await f.call("/v1/device/tasks/reconcile", "POST", publisher, { activeIds: [task.id] });
    expect(f.counts.changedRows).toBe(0);
    const input = { taskId: task.id, runtimeId: task.runtimeId, kind: "stop", payload: {}, idempotencyKey: randomUUID() };
    const queued = await f.call("/v1/device/commands", "POST", phone, input);
    expect(queued.status).toBe(201);
    expect((await f.call("/v1/device/commands", "POST", phone, input)).body.id).toBe(queued.body.id);
    expect((await f.call("/v1/device/tasks/sync", "GET", other)).body.commands).toEqual([]);
    expect((await f.call("/v1/device/tasks/sync", "GET", publisher)).body.commands[0].id).toBe(queued.body.id);
    expect((await f.call(`/v1/device/commands/${queued.body.id}/claim`, "POST", publisher, {})).body.status).toBe("uncertain");
    expect((await f.call("/v1/device/tasks/sync", "GET", publisher)).body.commands).toEqual([]);
    expect((await f.call(`/v1/device/commands/${queued.body.id}/claim`, "POST", publisher, {})).status).toBe(400);
    expect((await f.call("/v1/device/tasks/sync", "GET", phone)).status).toBe(401);
    await f.call(`/v1/owner/devices/${publisher.id}/revoke`, "POST", f.owner);
    f.reset();
    expect((await f.call("/v1/device/tasks/sync", "GET", publisher)).status).toBe(401);
    expect((await f.call(`/v1/device/commands/${queued.body.id}/settle`, "POST", publisher, { applied: true })).status).toBe(401);
    expect(f.counts.changedRows).toBe(0);
    if (process.env.ION_RELAY_COUNTER_REPORT === "1")
      writeFileSync(new URL("../../../.waku-cache/ion-task-bridge-after.json", import.meta.url), JSON.stringify({ method: "real HTTP/durable adapter; local SQLite and simulated time; native UI thread not executed", idle20Seconds: 100, idle20, idleHourSeconds: 3600, idleHour, changedTask, commandIsolationAndNoReplay: true }, null, 2) + "\n");
  } finally { await f.close(); Date.now = originalNow; }
});

test("batched unchanged cycles bound requests/writes and changed content is immediately published", async () => {
  const originalNow = Date.now; let now = originalNow(); Date.now = () => now;
  const f = fixture();
  try {
    const publisher = await f.pair(["publish"]);
    expect((await f.call("/v1/device/sync", "GET", publisher)).body.projectionDigest).toBe(null);
    const body = projection();
    expect((await f.call("/v1/device/automations", "PUT", publisher, { projection: body })).status).toBe(200);
    f.reset();
    for (let i = 0; i < 20; i++) {
      now += 5_000;
      const result = await f.call("/v1/device/sync", "GET", publisher);
      expect(result.status).toBe(200);
      expect(result.body).toMatchObject({ publisherId: publisher.id, commands: [], uncertain: [], jobs: [], inbox: [], after: null });
      expect(result.body.projectionDigest).toMatch(/^[a-f0-9]{64}$/);
    }
    expect(f.counts.requests).toBe(20);
    expect(f.counts.changedRows).toBe(6); // 2 small presence timestamps, at 30/60/90s.
    body.revision++;
    f.reset();
    await f.call("/v1/device/automations", "PUT", publisher, { projection: body });
    expect(f.counts.changedRows).toBe(1);
    const stored = f.raw.query<{ body: string }, []>("SELECT body FROM relay_automation_owners").get()!;
    expect(JSON.parse(stored.body).revision).toBe(2);
    // Equal publications (including property order changes) cannot rewrite content.
    f.reset();
    for (let i = 0; i < 20; i++) await f.call("/v1/device/automations", "PUT", publisher, { projection: { ...body, projects: [] } });
    expect(f.counts.changedRows).toBe(0);
  } finally { await f.close(); Date.now = originalNow; }
});

test("one hour idle stays bounded, then a changed projection synchronizes on the next cycle", async () => {
  const originalNow = Date.now; let now = originalNow(); Date.now = () => now;
  const f = fixture();
  try {
    const publisher = await f.pair(["publish"]), body = projection();
    f.reset();
    await f.call("/v1/device/sync", "GET", publisher);
    await f.call("/v1/device/automations", "PUT", publisher, { projection: body });
    await f.call("/v1/device/session", "GET", publisher); // Fresh pre-publication grant.
    await f.call("/v1/device/inbox/publication", "PUT", publisher, { batch: { after: null, next: 0, policy: "all", events: [] } });
    const first = { ...f.counts };
    f.reset();
    for (let i = 0; i < 20; i++) { now += 5_000; await f.call("/v1/device/sync", "GET", publisher); }
    const idle20 = { ...f.counts };
    expect(idle20.requests).toBe(20); expect(idle20.changedRows).toBe(6);
    f.reset(); now += 5_000;
    await f.call("/v1/device/sync", "GET", publisher);
    body.revision++;
    await f.call("/v1/device/automations", "PUT", publisher, { projection: body });
    const changedProjection = { ...f.counts };
    expect(changedProjection.requests).toBe(2); expect(changedProjection.changedRows).toBe(1);
    f.reset();
    for (let i = 0; i < 360; i++) { now += 10_000; await f.call("/v1/device/sync", "GET", publisher); }
    const idleHour = { ...f.counts };
    expect(idleHour.requests).toBe(360);
    expect(idleHour.changedRows).toBeLessThanOrEqual(242);
    const state = await f.call("/v1/device/sync", "GET", publisher);
    expect(state.body.notificationPolicy).toBe("all");
    const report = { method: "Real HTTP handlers + durableDatabase backed by isolated Bun SQLite; AccountRelay flush checks replayed; simulated time", nativeOwnerThreadExecuted: false,
      liveCloudflareAttributionProven: false, externalProviderRequests: 0,
      metrics: "Local changed rows exclude indexes and are not CF rowsWritten; returned rows are not CF rowsRead; handler requests are not live billed counters",
      first, idle20, idle20Seconds: 100, changedProjection, idleHour, idleHourSeconds: 3600 };
    if (process.env.ION_RELAY_COUNTER_REPORT === "1") {
      writeFileSync(new URL("../../../.waku-cache/ion-relay-sync-after.json", import.meta.url), JSON.stringify(report, null, 2) + "\n");
      console.log(JSON.stringify(report));
    }
  } finally { await f.close(); Date.now = originalNow; }
});

test("batch is publisher-only, freshly authorized and scoped to its publisher", async () => {
  const f = fixture();
  try {
    const publisher = await f.pair(["publish"]), other = await f.pair(["publish"]), phone = await f.pair(["automationRead", "automationControl"]);
    expect((await f.call("/v1/device/sync", "GET", phone)).status).toBe(401);
    expect((await f.call("/v1/device/sync", "GET", { ...publisher, token: other.token })).status).toBe(401);
    const project = randomUUID(), definition = randomUUID(), value = projection();
    const active = { ...value, projects: [{ id: project, name: "Ion" }], definitions: [{ id: definition, projectId: project, name: "Review", kind: "workflow", revision: 1, ready: true, paused: null, nextRun: null, schedule: null, stageCount: 1 }] };
    await f.call("/v1/device/automations", "PUT", publisher, { projection: active });
    const owners = await f.call("/v1/device/automations", "GET", phone);
    const input = { publisherId: publisher.id, idempotencyKey: randomUUID(), fingerprint: owners.body.owners[0].definitions[0].fingerprint,
      action: { kind: "start", definitionId: definition, runId: randomUUID(), expectedRevision: 1 } };
    const queued = await f.call("/v1/device/automations/commands", "POST", phone, input);
    expect(queued.status).toBe(201);
    expect((await f.call("/v1/device/automations/commands", "POST", phone, input)).body.id).toBe(queued.body.id);
    expect((await f.call("/v1/device/sync", "GET", other)).body.commands).toEqual([]);
    const batch = await f.call("/v1/device/sync", "GET", publisher);
    expect(batch.body.commands[0].id).toBe(queued.body.id);
    expect((await f.call(`/v1/device/automations/commands/${queued.body.id}/claim`, "POST", publisher, {})).body.status).toBe("uncertain");
    expect((await f.call("/v1/device/sync", "GET", publisher)).body.uncertain[0].id).toBe(queued.body.id);
    expect((await f.call(`/v1/device/automations/commands/${queued.body.id}/claim`, "POST", publisher, {})).status).toBe(400);
    await f.call(`/v1/owner/devices/${publisher.id}/revoke`, "POST", f.owner);
    f.reset();
    expect((await f.call("/v1/device/sync", "GET", publisher)).status).toBe(401);
    expect((await f.call(`/v1/device/automations/commands/${queued.body.id}/settle`, "POST", publisher, { applied: true })).status).toBe(401);
    expect(f.counts.changedRows).toBe(0);
  } finally { await f.close(); }
});
