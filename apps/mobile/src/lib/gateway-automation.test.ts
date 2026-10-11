import { expect, test } from 'bun:test';
import { randomUUID } from 'node:crypto';
import type { AutomationCommand, AutomationSubmission } from '@waku/client';
import { submitGatewayAutomation, gatewayOwnerFresh } from './gateway-automation';
import { GatewayRequestError, type GatewayPairing } from './gateway-client';
const pairing: GatewayPairing = { baseUrl: 'https://gateway.example', id: randomUUID(), token: 'private-pairing-secret', expiresAt: Date.now() + 100_000, grants: ['automationControl'] };
const publisher = randomUUID(), definition = randomUUID(), fingerprint = 'a'.repeat(64);
const intent = { kind: 'start' as const, definitionId: definition, expectedRevision: 5 };
function fixture() {
  const values = new Map<string, string>(); const order: string[] = [];
  let recorded: AutomationCommand | null = null; let enqueues = 0; let lookups = 0;
  const storage = { getItem: async (key: string) => values.get(key) ?? null, setItem: async (key: string, value: string) => { order.push('persist'); values.set(key, value); }, removeItem: async (key: string) => { order.push('clear'); values.delete(key); } };
  const transport = { lookup: async () => { order.push('read'); lookups += 1; return recorded; }, enqueue: async (_: GatewayPairing, input: AutomationSubmission) => { order.push('send'); enqueues += 1; recorded = { ...input, id: randomUUID(), deviceId: pairing.id, status: 'queued', createdAt: Date.now(), updatedAt: Date.now() }; return recorded; } };
  const submit = () => submitGatewayAutomation(pairing, storage, publisher, fingerprint, intent, randomUUID, transport);
  return { values, order, storage, transport, submit, enqueues: () => enqueues, lookups: () => lookups, recorded: () => recorded, set: (value: AutomationCommand | null) => { recorded = value; } };
}
test('persists the command and run IDs before sending and concurrent taps share one submission', async () => {
  const f = fixture(); const [a,b] = await Promise.all([f.submit(), f.submit()]);
  expect(f.order).toEqual(['persist','send']); expect(a.command.id).toBe(b.command.id); expect(f.enqueues()).toBe(1);
  expect(a.command.action.kind).toBe('start'); expect(f.values.size).toBe(1); expect([...f.values.values()][0]).not.toContain(pairing.token);
});
test('failed storage prevents any consequential request', async () => {
  const f = fixture(); f.storage.setItem = async () => { throw new Error('Storage unavailable'); };
  await expect(f.submit()).rejects.toThrow('Storage unavailable'); expect(f.enqueues()).toBe(0); expect(f.lookups()).toBe(0);
});
test('lost acknowledgement is recovered by identity with no second send', async () => {
  const f = fixture(); const original = f.transport.enqueue;
  f.transport.enqueue = async (...args) => { await original(...args); throw new Error('Disconnected'); };
  const result = await f.submit(); expect(result.command.id).toBe(f.recorded()!.id); expect(f.order).toEqual(['persist','send','read']);
  expect(f.enqueues()).toBe(1); expect(f.values.size).toBe(1);
});
test('unknown and queued effects recover by read after restart instead of being submitted again', async () => {
  const f = fixture(); await f.submit(); f.set({ ...f.recorded()!, status: 'uncertain' });
  const recovered = await f.submit(); expect(recovered.recovered).toBe(true); expect(recovered.command.status).toBe('uncertain');
  expect(f.enqueues()).toBe(1); expect(f.values.size).toBe(1);
  f.set({ ...f.recorded()!, status: 'applied' }); const applied = await f.submit(); expect(applied.command.status).toBe('applied'); expect(f.values.size).toBe(0);
  expect(f.enqueues()).toBe(1);
});
test('a failed status read preserves the original IDs and sends no replay', async () => {
  const f = fixture(); await f.submit(); const saved = [...f.values.values()][0];
  f.transport.lookup = async () => { throw new Error('Cannot read'); };
  await expect(f.submit()).rejects.toThrow('Cannot read'); expect([...f.values.values()][0]).toBe(saved); expect(f.enqueues()).toBe(1);
});
test('changing a saved decision does not silently send the original approval', async () => {
  const f = fixture(); const runId = randomUUID(); const gate = { kind: 'gate' as const, runId, expectedRevision: 4, stage: 0, approved: true };
  f.transport.enqueue = async () => { throw new Error('Network lost before commit'); };
  await expect(submitGatewayAutomation(pairing, f.storage, publisher, fingerprint, gate, randomUUID, f.transport)).rejects.toThrow('Network');
  let sends = 0; f.transport.enqueue = async () => { sends += 1; throw new Error('Must not send'); };
  await expect(submitGatewayAutomation(pairing, f.storage, publisher, fingerprint, { ...gate, approved: false }, randomUUID, f.transport)).rejects.toThrow('differs');
  expect(sends).toBe(0); expect(f.values.size).toBe(1);
});
test('changing foreground or credential context during persistence fences the first send', async () => {
  const f = fixture(); let authorized = true; const save = f.storage.setItem;
  f.storage.setItem = async (...args) => { await save(...args); authorized = false; };
  await expect(submitGatewayAutomation(pairing, f.storage, publisher, fingerprint, intent, randomUUID, f.transport, () => authorized)).rejects.toThrow('authorization changed');
  expect(f.enqueues()).toBe(0); expect(f.values.size).toBe(1);
});
test('a different command acknowledgement cannot clear or replace saved intent', async () => {
  const f = fixture(); f.transport.enqueue = async (_pairing, input) => ({ ...input, id: randomUUID(), deviceId: randomUUID(), status: 'applied', createdAt: 1, updatedAt: 1 });
  await expect(f.submit()).rejects.toThrow('another command'); expect(f.values.size).toBe(1);
});
test('only confirmed gateway rejection and confirmed absence clear an uncommitted intent', async () => {
  const f = fixture(); f.transport.enqueue = async () => { throw new GatewayRequestError(409); };
  await expect(f.submit()).rejects.toThrow('409'); expect(f.values.size).toBe(0);
  f.transport.lookup = async () => { throw new Error('Unknown lookup'); };
  await expect(f.submit()).rejects.toThrow('409'); expect(f.values.size).toBe(1);
});
test('corrupt saved state blocks sending, and an expired or read-only pairing cannot control workflows', async () => {
  const f = fixture(); await f.submit(); const key = [...f.values.keys()][0]!; f.values.set(key, '{broken');
  await expect(f.submit()).rejects.toThrow('unreadable'); expect(f.enqueues()).toBe(1);
  for (const p of [{ ...pairing, grants: ['automationRead'] }, { ...pairing, expiresAt: Date.now() - 1 }]) {
    await expect(submitGatewayAutomation(p, f.storage, publisher, fingerprint, intent, randomUUID, f.transport)).rejects.toThrow('grant');
  }
  expect(f.enqueues()).toBe(1);
});
test('foreground retry can send the exact persisted request if the gateway confirms it has no receipt', async () => {
  const f = fixture(); const original = f.transport.enqueue; let fail = true;
  f.transport.enqueue = async (...args) => { if (fail) throw new Error('Network failed'); return original(...args); };
  await expect(f.submit()).rejects.toThrow(); const saved = JSON.parse([...f.values.values()][0]!); fail = false;
  const result = await f.submit(); expect(result.command.idempotencyKey).toBe(saved.idempotencyKey); expect(result.command.action).toEqual(saved.action);
  expect(f.enqueues()).toBe(1);
});
test('owner freshness rejects missing, future and stale timestamps', () => {
  expect(gatewayOwnerFresh({ publishedAt: 1_000 }, 61_000)).toBe(true);
  expect(gatewayOwnerFresh({ publishedAt: 1_000 }, 61_001)).toBe(false);
  expect(gatewayOwnerFresh({ publishedAt: 1_001 }, 1_000)).toBe(false);
  expect(gatewayOwnerFresh({ publishedAt: NaN })).toBe(false);
});
