import { test } from "node:test";
import assert from "node:assert/strict";
import { build } from "esbuild";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, basename, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { Miniflare, convertV4MiniflareOptions } from "miniflare";

test("workerd SQL cursors bound idle writes and preserve changed task publication", async () => {
  const directory = mkdtempSync(join(tmpdir(), "ion-relay-sql-"));
  let mf;
  try {
    const result = await build({
      entryPoints: [fileURLToPath(new URL("./fixtures/relay-counters.ts", import.meta.url))],
      bundle: true, write: false, format: "esm", platform: "node", target: "es2022",
      external: ["cloudflare:workers", "node:*"],
      alias: { "bun:sqlite": fileURLToPath(new URL("../src/sqlite-unavailable.ts", import.meta.url)) },
    });
    const start = () => new Miniflare({
      ...convertV4MiniflareOptions({
        name: "ion-relay-counters", modules: true, script: result.outputFiles[0].text,
        compatibilityDate: "2026-10-19", compatibilityFlags: ["nodejs_compat"],
        durableObjects: { COUNTERS: { className: "CounterRelay", useSQLite: true } },
        outboundService: () => { throw new Error("Unexpected external request"); },
      }),
      resourcePersistencePath: directory, isolatedResourcePersistencePath: directory,
      telemetry: { enabled: false },
    });
    mf = start();
    let now = 1_800_000_000_000;
    const owner = { token: "local_counter_owner_" + "x".repeat(48), id: "" };
    const call = async (path, method = "GET", identity, body) => {
      const headers = { "x-test-now": String(now) };
      if (identity) {
        headers.Authorization = "Bearer " + identity.token;
        headers["X-Waku-Device-Id"] = identity.id;
      }
      if (body !== undefined) headers["Content-Type"] = "application/json";
      const response = await mf.dispatchFetch("https://fixture.invalid" + path, {
        method, headers, body: body === undefined ? undefined : JSON.stringify(body),
      });
      assert.equal(response.ok, true, `local SQL fixture ${method} ${path}: ${response.status}`);
      return response.json();
    };
    const invite = await call("/v1/owner/invites", "POST", owner, { grants: ["publish"] });
    const publisher = await call("/v1/pair", "POST", undefined, { invite: invite.invite, name: "SQL fixture" });
    const phoneInvite = await call("/v1/owner/invites", "POST", owner, { grants: ["taskRead", "taskControl"] });
    const phone = await call("/v1/pair", "POST", undefined, { invite: phoneInvite.invite, name: "SQL phone fixture" });
    await call("/v1/device/sync", "GET", publisher);
    await call("/v1/device/automations", "PUT", publisher, { projection: { version: 1, revision: 1, projects: [], definitions: [], runs: [], truncated: false } });
    await call("/v1/device/inbox/publication", "PUT", publisher, { batch: { after: null, next: 0, policy: "all", events: [] } });
    await call("/v1/device/tasks/sync", "GET", publisher);
    await call("/v1/device/tasks/reconcile", "POST", publisher, { activeIds: [] });
    await call("/__counters", "DELETE");
    for (let i = 0; i < 20; i++) {
      now += 5_000;
      await call("/v1/device/sync", "GET", publisher);
      await call("/v1/device/tasks/sync", "GET", publisher);
    }
    const idle100Seconds = await call("/__counters");
    assert.equal(idle100Seconds.requests, 40);
    assert.ok(idle100Seconds.rowsWritten <= 36, JSON.stringify(idle100Seconds));
    await call("/__counters", "DELETE");
    for (let i = 0; i < 360; i++) {
      now += 10_000;
      await call("/v1/device/sync", "GET", publisher);
      await call("/v1/device/tasks/sync", "GET", publisher);
    }
    const idleHour = await call("/__counters");
    assert.equal(idleHour.requests, 720);
    // Three presence rows every 30 seconds. Workerd additionally counts indexes;
    // use its real cursor accounting, not SQLite changes() or returned row count.
    assert.ok(idleHour.rowsWritten > 0 && idleHour.rowsWritten <= 1_440, JSON.stringify(idleHour));
    assert.ok(idleHour.rowsRead > 0 && idleHour.rowsRead < 100_000, JSON.stringify(idleHour));
    await call("/__counters", "DELETE");
    const task = { id: crypto.randomUUID(), projectId: crypto.randomUUID(), projectName: "Ion", title: "Changed", provider: "codex", status: "running", runtimeId: crypto.randomUUID(), updatedAt: now };
    const published = await call(`/v1/device/tasks/${task.id}`, "PUT", publisher, { task });
    assert.equal(published.changed, true);
    await call("/v1/device/tasks/reconcile", "POST", publisher, { activeIds: [task.id] });
    const changedTask = await call("/__counters");
    assert.ok(changedTask.rowsWritten > 0);
    await call("/__counters", "DELETE");
    assert.equal((await call(`/v1/device/tasks/${task.id}`, "PUT", publisher, { task })).changed, false);
    await call("/v1/device/tasks/reconcile", "POST", publisher, { activeIds: [task.id] });
    const unchangedReplay = await call("/__counters");
    assert.equal(unchangedReplay.rowsWritten, 0);
    await call("/__counters", "DELETE");
    const stop = await call("/v1/device/commands", "POST", phone, {
      taskId: task.id, runtimeId: task.runtimeId, kind: "stop", payload: {}, idempotencyKey: crypto.randomUUID(),
    });
    const queued = await call("/v1/device/tasks/sync", "GET", publisher);
    assert.equal(queued.commands[0].id, stop.id);
    assert.equal((await call(`/v1/device/commands/${stop.id}/claim`, "POST", publisher, {})).status, "uncertain");
    await call("/v1/device/session", "GET", publisher); // Independent effect-time grant.
    assert.equal((await call(`/v1/device/commands/${stop.id}/settle`, "POST", publisher, { applied: true })).status, "applied");
    assert.deepEqual((await call("/v1/device/tasks/sync", "GET", publisher)).commands, []);
    const stopLifecycle = await call("/__counters");
    await call("/__counters", "DELETE");
    await call(`/v1/device/tasks/${task.id}`, "PUT", publisher, { task: { ...task, status: "waiting", updatedAt: now + 1 } });
    const interaction = await call("/v1/device/interactions", "POST", publisher, { interaction: {
      taskId: task.id, runtimeId: task.runtimeId, requestId: "local-permission", kind: "permission",
      title: "Review file change", detail: "Exact permission snapshot", questions: [],
      options: [{ id: "allow-once", label: "Allow once", allow: true }, { id: "deny", label: "Deny", allow: false }],
    } });
    const permission = await call("/v1/device/commands", "POST", phone, {
      taskId: task.id, runtimeId: task.runtimeId, kind: "permission", idempotencyKey: crypto.randomUUID(),
      payload: { interactionId: interaction.id, fingerprint: interaction.fingerprint, optionId: "deny" },
    });
    assert.equal((await call("/v1/device/tasks/sync", "GET", publisher)).commands[0].id, permission.id);
    assert.equal((await call(`/v1/device/interactions/${interaction.id}`, "GET", publisher)).requestId, "local-permission");
    assert.equal((await call(`/v1/device/commands/${permission.id}/claim`, "POST", publisher, {})).status, "uncertain");
    await call("/v1/device/session", "GET", publisher);
    assert.equal((await call(`/v1/device/commands/${permission.id}/settle`, "POST", publisher, { applied: true })).status, "applied");
    assert.deepEqual((await call(`/v1/device/tasks/${task.id}/interactions`, "GET", phone)).interactions, []);
    const approvalLifecycle = await call("/__counters");
    // Real workerd SQL: remember an answer even when its first snapshot has
    // not arrived. Retrying it and replaying the snapshot must not rewrite rows.
    const early = { taskId: task.id, runtimeId: task.runtimeId, requestId: "answered-before-publication" };
    await call("/__counters", "DELETE");
    await call("/v1/device/interactions/resolve", "POST", publisher, early);
    const earlyResolution = await call("/__counters");
    assert.ok(earlyResolution.rowsWritten > 0);
    await call("/__counters", "DELETE");
    for (let i = 0; i < 20; i++) await call("/v1/device/interactions/resolve", "POST", publisher, early);
    const resolutionRetries = await call("/__counters");
    assert.equal(resolutionRetries.requests, 20);
    assert.equal(resolutionRetries.rowsWritten, 0);
    await mf.dispose();
    mf = start();
    await call("/__counters", "DELETE");
    const lateInput = { ...early, kind: "permission", title: "Already answered", detail: "", questions: [],
      options: [{ id: "deny", label: "Deny", allow: false }] };
    const late = await call("/v1/device/interactions", "POST", publisher, { interaction: lateInput });
    assert.equal(late.status, "resolved");
    assert.deepEqual((await call(`/v1/device/tasks/${task.id}/interactions`, "GET", phone)).interactions, []);
    const latePublication = await call("/__counters");
    await call("/__counters", "DELETE");
    assert.equal((await call("/v1/device/interactions", "POST", publisher, { interaction: lateInput })).created, false);
    const lateReplay = await call("/__counters");
    assert.equal(lateReplay.rowsWritten, 0);
    // A genuinely new request still reaches the phone after recovery.
    const fresh = await call("/v1/device/interactions", "POST", publisher,
      { interaction: { ...lateInput, requestId: "new-after-recovery" } });
    assert.equal(fresh.status, "pending");
    assert.deepEqual((await call(`/v1/device/tasks/${task.id}/interactions`, "GET", phone)).interactions.map(value => value.id), [fresh.id]);
    await call("/v1/device/interactions/resolve", "POST", publisher,
      { ...early, requestId: "new-after-recovery" });
    const unfinishedInput = { taskId: task.id, runtimeId: task.runtimeId, kind: "stop", payload: {}, idempotencyKey: crypto.randomUUID() };
    const unfinished = await call("/v1/device/commands", "POST", phone, unfinishedInput);
    assert.equal((await call(`/v1/device/commands/${unfinished.id}/claim`, "POST", publisher, {})).status, "uncertain");
    const stillQueued = await call("/v1/device/commands", "POST", phone, { ...unfinishedInput, idempotencyKey: crypto.randomUUID() });
    await call("/__reopen-legacy-schema", "POST");
    const restored = await call("/v1/device/tasks/sync", "GET", publisher);
    assert.equal(restored.snapshotPresent, false);
    assert.deepEqual(restored.commands.map(command => command.id), [stillQueued.id]);
    assert.equal((await call(`/v1/device/commands/${unfinished.id}`, "GET", phone)).status, "uncertain");
    assert.equal((await call("/v1/device/commands", "POST", phone, unfinishedInput)).id, unfinished.id);
    const kept = await call(`/v1/device/tasks/${task.id}?publisherId=${publisher.id}`, "GET", phone);
    assert.equal(kept.task.title, task.title);
    assert.equal(kept.task.runtimeId, task.runtimeId);
    await call("/v1/device/tasks/reconcile", "POST", publisher, { activeIds: [task.id] });
    assert.equal((await call("/v1/device/tasks/sync", "GET", publisher)).snapshotPresent, true);
    const receipt = { method: "local workerd, actual Durable Object SQL cursor rowsRead/rowsWritten, real HTTP handlers; simulated clock; one publisher and one phone; no live billing attribution; stop/permission receipts are simulated local daemon acknowledgements, not real device effects", idle100Seconds, idleHour, changedTask, unchangedReplay, stopLifecycle, approvalLifecycle,
      resolutionRecovery: { earlyResolution, resolutionRetries, latePublication, lateReplay, workerRestartRetainedResolution: true, freshPendingRequestDelivered: true },
      legacySchemaRecovery: { taskAndGrantsRetained: true, queuedCommandRetained: true, uncertainCommandNotRequeued: true, idempotencyRetained: true, missingReceiptRepublished: true } };
    console.log(JSON.stringify(receipt));
    if (process.env.ION_RELAY_SQL_REPORT)
      writeFileSync(resolve(process.env.ION_RELAY_SQL_REPORT), JSON.stringify(receipt, null, 2) + "\n");
  } finally {
    await mf?.dispose();
    assert.equal(dirname(resolve(directory)), resolve(tmpdir()));
    assert.ok(basename(directory).startsWith("ion-relay-sql-"));
    rmSync(directory, { recursive: true, force: true });
  }
});
