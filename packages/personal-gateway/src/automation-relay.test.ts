import { afterEach, expect, test } from 'bun:test';
import { mkdtempSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { randomUUID } from 'node:crypto';
import { DurableAutomationRelay, validateProjection, type AutomationProjection, type AutomationSubmission } from './automation-relay';
const directories: string[] = [];
afterEach(() => { for (const directory of directories.splice(0)) rmSync(directory, { recursive: true, force: true }); });
const publisher = randomUUID(), otherPublisher = randomUUID(), phone = randomUUID(), project = randomUUID(), definition = randomUUID(), run = randomUUID();
function fixture() {
  const directory = mkdtempSync(join(tmpdir(), 'ion-automation-relay-')); directories.push(directory);
  let now = 1_000; const path = join(directory, 'relay.sqlite');
  const relay = new DurableAutomationRelay(path, () => now);
  const projection: AutomationProjection = { version: 1, revision: 3, projects: [{ id: project, name: 'Ion' }], truncated: false,
    definitions: [{ id: definition, projectId: project, name: 'Release', kind: 'workflow', revision: 3, ready: true, paused: null, nextRun: null, schedule: null, stageCount: 2 }],
    runs: [{ id: run, projectId: project, definitionId: definition, name: 'Release', revision: 4, status: 'waitingDecision', currentStage: 1, stageCount: 2,
      gate: { index: 1, title: 'Deploy', instructions: 'Deploy the approved build.', reviewComplete: true } }] };
  relay.publish(publisher, projection);
  const owner = () => relay.owners(new Set([publisher]))[0]!;
  const input = (): AutomationSubmission => ({ publisherId: publisher, idempotencyKey: randomUUID(), fingerprint: owner().definitions[0]!.fingerprint,
    action: { kind: 'start', definitionId: definition, expectedRevision: 3, runId: randomUUID() } });
  return { relay, path, projection, owner, input, time: (value: number) => { now = value; } };
}
test('durable claim survives restart and never re-enters the queue', () => {
  const f = fixture(); let relay = f.relay;
  try {
    const input = f.input(); const command = relay.enqueue(phone, input);
    expect(relay.enqueue(phone, JSON.parse(JSON.stringify(input))).id).toBe(command.id);
    expect(relay.byKey(phone, input.idempotencyKey)?.id).toBe(command.id);
    expect(relay.pending(otherPublisher)).toEqual([]);
    expect(() => relay.claim(otherPublisher, command.id, true)).toThrow('another owner');
    expect(relay.claim(publisher, command.id, true).status).toBe('uncertain');
    relay.close(); relay = new DurableAutomationRelay(f.path, () => 1_001);
    expect(relay.pending(publisher)).toEqual([]);
    expect(relay.command(command.id)?.status).toBe('uncertain');
    expect(() => relay.claim(publisher, command.id, true)).toThrow('unused claim');
    expect(() => relay.settle(otherPublisher, command.id, true)).toThrow('another owner');
    expect(relay.settle(publisher, command.id, true).status).toBe('applied');
    expect(relay.settle(publisher, command.id, true).status).toBe('applied');
    expect(() => relay.settle(publisher, command.id, false)).toThrow('awaiting');
  } finally { relay.close(); }
});
test('owners cannot substitute the same resource identity and witness across publishers', () => {
  const f = fixture(); try {
    f.relay.publish(otherPublisher, f.projection);
    const own = f.owner().definitions[0]!; const other = f.relay.owners(new Set([otherPublisher]))[0]!.definitions[0]!;
    expect(own.fingerprint).not.toBe(other.fingerprint);
    expect(() => f.relay.enqueue(phone, { ...f.input(), publisherId: otherPublisher })).toThrow('changed');
    expect(f.relay.owners(new Set())).toEqual([]);
  } finally { f.relay.close(); }
});
test('same command key rejects changed intent even after its original snapshot changes', () => {
  const f = fixture(); try {
    const input = f.input(); const command = f.relay.enqueue(phone, input);
    f.projection.revision = 5; f.projection.definitions[0]!.revision = 5; f.relay.publish(publisher, f.projection);
    expect(f.relay.enqueue(phone, input).id).toBe(command.id);
    expect(() => f.relay.enqueue(phone, { ...input, action: { ...input.action, expectedRevision: 5 } })).toThrow('Idempotency');
    expect(f.relay.claim(publisher, command.id, true).status).toBe('rejected');
  } finally { f.relay.close(); }
});
test('offline, time reversal and expired commands fail before any claim is consumed', () => {
  const f = fixture(); try {
    const input = f.input(); const command = f.relay.enqueue(phone, input);
    f.time(61_001); expect(() => f.relay.enqueue(phone, { ...input, idempotencyKey: randomUUID() })).toThrow('stale');
    expect(f.relay.claim(publisher, command.id, true).status).toBe('rejected');
    f.time(1_000); f.relay.publish(publisher, f.projection); const late = f.relay.enqueue(phone, f.input());
    f.time(302_000); f.relay.publish(publisher, f.projection);
    expect(f.relay.claim(publisher, late.id, true).status).toBe('rejected');
    f.time(900); expect(() => f.relay.enqueue(phone, f.input())).toThrow('stale');
  } finally { f.relay.close(); }
});
test('revoked sender rejects a queued control and a partial gate cannot approve', () => {
  const f = fixture(); try {
    const command = f.relay.enqueue(phone, f.input()); expect(f.relay.claim(publisher, command.id, false).status).toBe('rejected');
    f.projection.runs[0]!.gate!.reviewComplete = false; f.relay.publish(publisher, f.projection);
    const input: AutomationSubmission = { publisherId: publisher, idempotencyKey: randomUUID(), fingerprint: f.owner().runs[0]!.fingerprint,
      action: { kind: 'gate', runId: run, expectedRevision: 4, stage: 1, approved: true } };
    expect(() => f.relay.enqueue(phone, input)).toThrow('incomplete');
    const decline = f.relay.enqueue(phone, { ...input, action: { kind: 'gate', runId: run, expectedRevision: 4, stage: 1, approved: false } });
    expect(f.relay.claim(publisher, decline.id, true).status).toBe('uncertain');
  } finally { f.relay.close(); }
});
test('gate revisions bind the exact stage and removed or finished runs reject queued commands', () => {
  const f = fixture(); try {
    const input: AutomationSubmission = { publisherId: publisher, idempotencyKey: randomUUID(), fingerprint: f.owner().runs[0]!.fingerprint,
      action: { kind: 'gate', runId: run, expectedRevision: 4, stage: 1, approved: true } };
    expect(() => f.relay.enqueue(phone, { ...input, action: { kind: 'gate', runId: run, expectedRevision: 4, stage: 0, approved: true } })).toThrow('changed');
    const c = f.relay.enqueue(phone, input); f.projection.runs[0]!.status = 'completed'; f.projection.runs[0]!.gate = null;
    f.relay.publish(publisher, f.projection); expect(f.relay.claim(publisher, c.id, true).status).toBe('rejected');
    expect(() => f.relay.enqueue(phone, { ...input, idempotencyKey: randomUUID(), action: { kind: 'stop', runId: run, expectedRevision: 4 } })).toThrow('changed');
  } finally { f.relay.close(); }
});
test('projection schema rejects unknown fields, unsafe counters, missing projects and oversized UTF-8', () => {
  const f = fixture(); try {
    expect(() => validateProjection({ ...f.projection, credentials: 'secret' })).toThrow();
    expect(() => validateProjection({ ...f.projection, revision: Number.MAX_SAFE_INTEGER + 1 })).toThrow();
    expect(() => validateProjection({ ...f.projection, projects: [] })).toThrow();
    expect(() => validateProjection({ ...f.projection, runs: [{ ...f.projection.runs[0], gate: null }] })).toThrow();
    expect(() => validateProjection({ ...f.projection, runs: Array.from({ length: 64 }, () => ({ ...f.projection.runs[0], id: randomUUID(), gate: { index: 1, title: '', instructions: '😀'.repeat(1000), reviewComplete: true } })) })).toThrow();
    expect(() => f.relay.enqueue(phone, { ...f.input(), action: { kind: 'start', definitionId: definition, runId: randomUUID(), expectedRevision: 3, command: 'arbitrary shell' } } as unknown as AutomationSubmission)).toThrow();
  } finally { f.relay.close(); }
});
test('routine control remains distinct from workflow control and stop has queue priority', () => {
  const f = fixture(); try {
    expect(() => f.relay.enqueue(phone, { ...f.input(), action: { kind: 'routine', definitionId: definition, expectedRevision: 3, paused: false } })).toThrow();
    const start = f.relay.enqueue(phone, f.input());
    const stop = f.relay.enqueue(phone, { publisherId: publisher, idempotencyKey: randomUUID(), fingerprint: f.owner().runs[0]!.fingerprint, action: { kind: 'stop', runId: run, expectedRevision: 4 } });
    expect(f.relay.pending(publisher).map((c) => c.id)).toEqual([stop.id, start.id]);
    const routineId = randomUUID(); f.projection.definitions.push({ ...f.projection.definitions[0]!, id: routineId, kind: 'routine', paused: true, schedule: 'Daily at 08:00 · Asia/Calcutta', nextRun: 0 });
    f.relay.publish(publisher, f.projection);
    const routine = f.owner().definitions[1]!;
    const schedule = f.relay.enqueue(phone, { publisherId: publisher, idempotencyKey: randomUUID(), fingerprint: routine.fingerprint, action: { kind: 'routine', definitionId: routineId, expectedRevision: 3, paused: false } });
    expect(f.relay.claim(publisher, schedule.id, true).status).toBe('uncertain');
  } finally { f.relay.close(); }
});
