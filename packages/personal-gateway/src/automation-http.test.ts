import { afterEach, expect, test } from 'bun:test';
import { mkdtempSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { randomUUID } from 'node:crypto';
import { startGateway } from './server';
import type { AutomationProjection, AutomationCommand, AutomationOwner } from './automation-relay';
const directories: string[] = [];
afterEach(() => { for (const directory of directories.splice(0)) rmSync(directory, { recursive: true, force: true }); });
const ownerToken = 'owner_' + 'g'.repeat(48);
function setup() {
  const directory = mkdtempSync(join(tmpdir(), 'ion-workflow-http-')); directories.push(directory);
  const gateway = startGateway({ databasePath: join(directory, 'authority.sqlite'), ownerToken, allowedTools: [] });
  async function call(path: string, method = 'GET', identity?: { id: string; token: string }, data?: unknown) {
    const headers = new Headers(); if (identity) { headers.set('Authorization', `Bearer ${identity.token}`); headers.set('X-Waku-Device-Id', identity.id); }
    if (data !== undefined) headers.set('Content-Type', 'application/json');
    return fetch(new URL(path, gateway.server.url), { method, headers, body: data === undefined ? undefined : JSON.stringify(data), redirect: 'error' });
  }
  const owner = { id: '', token: ownerToken };
  async function pair(grants: string[], name: string) {
    const invite = await (await call('/v1/owner/invites', 'POST', owner, { grants })).json() as { invite: string };
    return (await (await call('/v1/pair', 'POST', undefined, { invite: invite.invite, name })).json()) as { id: string; token: string };
  }
  const project = randomUUID(), definition = randomUUID();
  const projection: AutomationProjection = { version: 1, revision: 1, projects: [{ id: project, name: 'Ion' }], truncated: false, runs: [], definitions: [{ id: definition, projectId: project, name: 'Release', kind: 'workflow', revision: 1, ready: true, paused: null, nextRun: null, schedule: null, stageCount: 1 }] };
  return { gateway, call, pair, owner, projection, definition };
}
test('HTTP grants separate publication, workflow reading and workflow controls, with exact owner routing', async () => {
  const f = setup(); try {
    const desktop = await f.pair(['publish'], 'Home desktop'); const other = await f.pair(['publish'], 'Cloud owner');
    const reader = await f.pair(['automationRead'], 'Read phone'); const phone = await f.pair(['automationRead','automationControl'], 'Control phone');
    const taskPhone = await f.pair(['taskRead','taskControl'], 'Legacy phone');
    expect((await f.call('/v1/device/automations', 'PUT', desktop, { projection: f.projection })).status).toBe(200);
    expect((await f.call('/v1/device/automations', 'PUT', reader, { projection: f.projection })).status).toBe(401);
    expect((await f.call('/v1/device/automations', 'GET', desktop)).status).toBe(401);
    expect((await f.call('/v1/device/automations', 'GET', taskPhone)).status).toBe(401);
    expect((await f.call('/v1/device/session', 'GET', reader)).status).toBe(200);
    const result = await f.call('/v1/device/automations', 'GET', reader);
    expect(result.headers.get('cache-control')).toBe('no-store');
    const feed = await result.json() as { owners: (AutomationOwner & { name: string })[] };
    expect(feed.owners[0]!.publisherId).toBe(desktop.id); expect(feed.owners[0]!.name).toBe('Home desktop');
    const input = { publisherId: desktop.id, idempotencyKey: randomUUID(), fingerprint: feed.owners[0]!.definitions[0]!.fingerprint, action: { kind: 'start', definitionId: f.definition, runId: randomUUID(), expectedRevision: 1 } };
    expect((await f.call('/v1/device/automations/commands', 'POST', reader, input)).status).toBe(401);
    expect((await f.call('/v1/device/automations/commands', 'POST', taskPhone, input)).status).toBe(401);
    const queued = await f.call('/v1/device/automations/commands', 'POST', phone, input); expect(queued.status).toBe(201);
    const command = await queued.json() as AutomationCommand;
    const repeated = await (await f.call('/v1/device/automations/commands', 'POST', phone, input)).json() as AutomationCommand; expect(repeated.id).toBe(command.id);
    const otherQueue = await (await f.call('/v1/device/automations/queue', 'GET', other)).json(); expect(otherQueue).toEqual({ commands: [] });
    expect((await f.call(`/v1/device/automations/commands/${command.id}/claim`, 'POST', other, {})).status).toBe(400);
    expect((await f.call(`/v1/device/automations/commands/${command.id}/claim`, 'POST', phone, {})).status).toBe(401);
    const claimed = await (await f.call(`/v1/device/automations/commands/${command.id}/claim`, 'POST', desktop, {})).json() as AutomationCommand; expect(claimed.status).toBe('uncertain');
    expect((await f.call(`/v1/device/automations/commands/${command.id}/settle`, 'POST', other, { applied: true })).status).toBe(400);
    const recovery = await (await f.call('/v1/device/automations/uncertain', 'GET', desktop)).json() as { commands: AutomationCommand[] }; expect(recovery.commands[0]!.id).toBe(command.id);
    expect(await (await f.call('/v1/device/automations/uncertain', 'GET', other)).json()).toEqual({ commands: [] });
    expect((await f.call(`/v1/device/automations/commands/${command.id}/settle`, 'POST', desktop, { applied: true })).status).toBe(200);
    const known = await (await f.call(`/v1/device/automations/commands?key=${input.idempotencyKey}`, 'GET', phone)).json() as { command: AutomationCommand }; expect(known.command.status).toBe('applied');
    expect((await f.call(`/v1/device/automations/commands?key=${input.idempotencyKey}`, 'GET', reader)).status).toBe(401);
    const anotherPhone = await f.pair(['automationControl'], 'Other phone');
    expect(await (await f.call(`/v1/device/automations/commands?key=${input.idempotencyKey}`, 'GET', anotherPhone)).json()).toEqual({ command: null });
    expect((await f.call('/v1/owner/devices', 'GET', phone)).status).toBe(401);
  } finally { await f.gateway.close(); }
});
test('revocation hides the owner projection and rejects already queued phone commands before effects', async () => {
  const f = setup(); try {
    const desktop = await f.pair(['publish'], 'Desktop'); const phone = await f.pair(['automationRead','automationControl'], 'Phone');
    await f.call('/v1/device/automations', 'PUT', desktop, { projection: f.projection });
    const feed = await (await f.call('/v1/device/automations', 'GET', phone)).json() as { owners: AutomationOwner[] };
    const input = { publisherId: desktop.id, idempotencyKey: randomUUID(), fingerprint: feed.owners[0]!.definitions[0]!.fingerprint, action: { kind: 'start', definitionId: f.definition, runId: randomUUID(), expectedRevision: 1 } };
    const command = await (await f.call('/v1/device/automations/commands', 'POST', phone, input)).json() as AutomationCommand;
    await f.call(`/v1/owner/devices/${phone.id}/revoke`, 'POST', f.owner);
    const rejected = await (await f.call(`/v1/device/automations/commands/${command.id}/claim`, 'POST', desktop, {})).json() as AutomationCommand; expect(rejected.status).toBe('rejected');
    const reader = await f.pair(['automationRead','automationControl'], 'Reader');
    await f.call(`/v1/owner/devices/${desktop.id}/revoke`, 'POST', f.owner);
    expect(await (await f.call('/v1/device/automations', 'GET', reader)).json()).toEqual({ owners: [] });
    expect((await f.call('/v1/device/automations/commands', 'POST', reader, { ...input, idempotencyKey: randomUUID() })).status).toBe(409);
    expect((await f.call('/v1/device/automations', 'PUT', desktop, { projection: f.projection })).status).toBe(401);
  } finally { await f.gateway.close(); }
});
test('the original solo task queue is also isolated to its registered publisher', async () => {
  const f = setup(); try {
    const desktop = await f.pair(['publish'], 'Desktop'); const other = await f.pair(['publish'], 'Other desktop'); const phone = await f.pair(['taskControl'], 'Phone');
    const task = { id: randomUUID(), projectId: f.projection.projects[0]!.id, projectName: 'Ion', title: 'Task', provider: 'codex', status: 'running', runtimeId: randomUUID(), updatedAt: 100 };
    expect((await f.call(`/v1/device/tasks/${task.id}`, 'PUT', desktop, { task })).status).toBe(200);
    const command = await (await f.call('/v1/device/commands', 'POST', phone, { taskId: task.id, runtimeId: task.runtimeId, idempotencyKey: randomUUID(), kind: 'stop', payload: {} })).json() as { id: string };
    expect(await (await f.call('/v1/device/commands', 'GET', other)).json()).toEqual({ commands: [] });
    expect((await f.call(`/v1/device/commands/${command.id}/claim`, 'POST', other, {})).status).toBe(400);
    expect((await f.call(`/v1/device/commands/${command.id}/claim`, 'POST', desktop, {})).status).toBe(200);
    expect((await f.call(`/v1/device/commands/${command.id}/settle`, 'POST', other, { applied: true })).status).toBe(400);
    expect((await f.call(`/v1/device/commands/${command.id}/settle`, 'POST', desktop, { applied: true })).status).toBe(200);
  } finally { await f.gateway.close(); }
});
