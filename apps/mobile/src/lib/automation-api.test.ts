import { describe, expect, test } from 'bun:test';
import { WakuRpcError, type AutomationRun, type Command, type ResponsePayload } from '@waku/client';
import { intervalLabel, startSavedAutomation, requestMobileAutomation, type SubmissionStorage } from './automation-api';
const ID = '00000000-0000-4000-8000-000000000001';
const OTHER = '00000000-0000-4000-8000-000000000002';
const DEFINITION = '00000000-0000-4000-8000-000000000003';
function run(id = ID): AutomationRun {
  return { id, name: 'Release', projectId: DEFINITION, workflowId: DEFINITION, routineId: null,
    definitionRevision: 7, revision: 1, createdAt: 1, scheduledAt: null, deadline: 300,
    status: 'running', reason: null, currentStage: 0, stages: [] };
}
function fixture() {
  const values = new Map<string, string>();
  const calls: Command[] = [];
  const storage: SubmissionStorage = {
    getItem: async (key) => values.get(key) ?? null,
    setItem: async (key, value) => { values.set(key, value); },
    removeItem: async (key) => { values.delete(key); },
  };
  let answer: (command: Command) => Promise<ResponsePayload> = async () => ({ type: 'automationRun', run: run() });
  const transport = { request: async (command: Command): Promise<ResponsePayload> => {
    if (command.type === 'loadWorkspaceControl') return { type: 'workspaceControl', snapshot: localSnapshot() };
    calls.push(command); return answer(command);
  } };
  return { values, calls, storage, transport, answer: (next: typeof answer) => { answer = next; },
    start: (namespace = 'owner', revision = 7, uuid = () => ID) => startSavedAutomation(transport, storage, namespace, DEFINITION, revision, uuid) };
}
describe('phone automation identity and reconciliation', () => {
  test('persists before transport and clears only an acknowledged exact run', async () => {
    const f = fixture();
    f.answer(async (command) => {
      expect(f.values.size).toBe(1);
      expect(command).toEqual({ type: 'startAutomation', id: ID, definitionId: DEFINITION, expectedDefinitionRevision: 7 });
      return { type: 'automationRun', run: run() };
    });
    expect((await f.start()).id).toBe(ID);
    expect(f.values.size).toBe(0);
  });
  test('failed acknowledgement cleanup keeps the run identity for the next retry', async () => {
    const f = fixture(); const clear = f.storage.removeItem;
    f.storage.removeItem = async () => { throw new Error('Storage unavailable'); };
    await expect(f.start()).rejects.toThrow('Storage unavailable'); expect(f.values.size).toBe(1);
    f.storage.removeItem = clear; f.calls.length = 0;
    f.answer(async (command) => {
      expect(command).toMatchObject({ id: ID, expectedDefinitionRevision: 7 });
      return { type: 'automationRun', run: run() };
    });
    await f.start('owner', 20, () => OTHER); expect(f.values.size).toBe(0);
  });
  test('another returned run cannot clear this submission identity', async () => {
    const f = fixture();
    f.answer(async () => ({ type: 'automationRun', run: run(OTHER) }));
    await expect(f.start()).rejects.toThrow('another run identity'); expect(f.values.size).toBe(1);
  });
  test('failed persistence sends no request', async () => {
    const f = fixture(); f.storage.setItem = async () => { throw new Error('Disk unavailable'); };
    await expect(f.start()).rejects.toThrow('Disk unavailable'); expect(f.calls).toHaveLength(0);
  });
  test('lost acknowledgement recovers the actual run without another start', async () => {
    const f = fixture();
    f.answer(async (command) => {
      if (command.type === 'startAutomation') throw new Error('timeout');
      return { type: 'automationRun', run: run() };
    });
    expect((await f.start()).id).toBe(ID);
    expect(f.calls.map((c) => c.type)).toEqual(['startAutomation', 'getAutomationRun']);
    expect(f.values.size).toBe(0);
  });
  test('unknown outcome plus missing lookup retains ID and pinned revision after restart', async () => {
    const f = fixture();
    f.answer(async (command) => {
      if (command.type === 'startAutomation') throw new Error('disconnected');
      return { type: 'automationMissing', id: ID };
    });
    await expect(f.start()).rejects.toThrow('disconnected'); expect(f.values.size).toBe(1);
    f.calls.length = 0;
    f.answer(async (command) => {
      expect(command).toMatchObject({ type: 'startAutomation', id: ID, expectedDefinitionRevision: 7 });
      return { type: 'automationRun', run: run() };
    });
    await f.start('owner', 20, () => OTHER);
    expect(f.calls).toHaveLength(1);
  });
  test('completed rejection and exact absence permit a corrected fresh submission', async () => {
    const f = fixture();
    f.answer(async (command) => {
      if (command.type === 'startAutomation') throw new WakuRpcError('definitions changed');
      return { type: 'automationMissing', id: ID };
    });
    await expect(f.start()).rejects.toThrow('definitions changed'); expect(f.values.size).toBe(0);
    f.calls.length = 0;
    f.answer(async (command) => {
      expect(command).toMatchObject({ id: OTHER, expectedDefinitionRevision: 20 });
      return { type: 'automationRun', run: run(OTHER) };
    });
    await f.start('owner', 20, () => OTHER);
  });
  test('unavailable or mismatched recovery keeps the original identity', async () => {
    for (const mismatch of [false, true]) {
      const f = fixture();
      f.answer(async (command) => {
        if (command.type === 'startAutomation') throw new WakuRpcError('rejected');
        if (!mismatch) throw new Error('lookup unavailable');
        return { type: 'automationMissing', id: OTHER };
      });
      await expect(f.start()).rejects.toThrow('rejected'); expect(f.values.size).toBe(1);
    }
  });
  test('concurrent taps share one request and same durable run', async () => {
    const f = fixture(); let release!: () => void;
    const hold = new Promise<void>((resolve) => { release = resolve; });
    f.answer(async () => { await hold; return { type: 'automationRun', run: run() }; });
    const first = f.start(); const second = f.start('owner', 40, () => OTHER);
    release(); expect((await first).id).toBe(ID); expect((await second).id).toBe(ID);
    expect(f.calls).toHaveLength(1);
  });
  test('another owner address uses a separate identity', async () => {
    const f = fixture();
    f.answer(async () => { throw new Error('connection lost'); });
    await expect(f.start()).rejects.toThrow('connection lost');
    f.answer(async (command) => {
      expect(command).toMatchObject({ id: OTHER, expectedDefinitionRevision: 9 });
      return { type: 'automationRun', run: run(OTHER) };
    });
    await f.start('other-owner-address', 9, () => OTHER);
    expect(f.values.size).toBe(1);
  });
  test('corrupt saved identity is retained and cannot initiate another run', async () => {
    const f = fixture();
    f.answer(async () => { throw new Error('connection lost'); });
    await expect(f.start()).rejects.toThrow('connection lost');
    const key = [...f.values.keys()][0]; f.values.set(key, '{'); f.calls.length = 0;
    await expect(f.start()).rejects.toThrow('could not be read');
    expect(f.calls).toHaveLength(0); expect(f.values.get(key)).toBe('{');
  });
});

function localSnapshot(agent: 'local' | 'nebius' | 'missing' = 'local'): import('@waku/client').ControlSnapshot {
  return { revision: 7, records: [
    { kind: 'workflow', id: DEFINITION, stages: [{ teamId: OTHER }] },
    { kind: 'routine', id: 'routine', teamId: OTHER, workflowId: null },
    { kind: 'team', id: OTHER, members: [{ agentId: ID }] },
    ...(agent === 'missing' ? [] : [{ kind: 'agent', id: ID, provider: agent === 'nebius' ? 'nebius' : 'codex',
      execution: { environment: agent === 'nebius' ? 'nebiusSandbox' : 'local' } }]),
  ] } as import('@waku/client').ControlSnapshot;
}
test('mobile automation checks target profiles before starting or enabling, while stop and decline remain available', async () => {
  const calls: Command[] = [];
  const transport = { request: async (command: Command): Promise<ResponsePayload> => {
    calls.push(command);
    if (command.type === 'loadWorkspaceControl') return { type: 'workspaceControl', snapshot: localSnapshot('missing') };
    return { type: 'automationRun', run: run() };
  } };
  await expect(requestMobileAutomation(transport, { type: 'startAutomation', id: ID, definitionId: DEFINITION, expectedDefinitionRevision: 7 })).rejects.toThrow('Team agent unavailable');
  expect(calls.map(command => command.type)).toEqual(['loadWorkspaceControl']);
  await expect(requestMobileAutomation(transport, { type: 'setRoutinePaused', id: 'routine', expectedRevision: 7, paused: false })).rejects.toThrow('Team agent unavailable');
  expect(calls.map(command => command.type)).toEqual(['loadWorkspaceControl', 'loadWorkspaceControl']);
  await expect(requestMobileAutomation(transport, { type: 'generateWorkflow', projectId: ID, prompt: 'Build' })).rejects.toThrow('desktop');
  await expect(requestMobileAutomation(transport, { type: 'resolveAutomationGate', id: ID, stage: 0, expectedRevision: 1, approved: true })).rejects.toThrow('pinned execution');
  await requestMobileAutomation(transport, { type: 'stopAutomation', id: ID, expectedRevision: 1 });
  await requestMobileAutomation(transport, { type: 'resolveAutomationGate', id: ID, stage: 0, expectedRevision: 1, approved: false });
  await requestMobileAutomation(transport, { type: 'setRoutinePaused', id: 'routine', expectedRevision: 7, paused: true });
  expect(calls.slice(2).map(command => command.type)).toEqual(['stopAutomation', 'resolveAutomationGate', 'setRoutinePaused']);
});

for (const agent of ['local', 'nebius'] as const) test(`mobile can start and enable verified ${agent} automation teams`, async () => {
  const calls: Command[] = [];
  const transport = { request: async (command: Command): Promise<ResponsePayload> => {
    calls.push(command);
    return command.type === 'loadWorkspaceControl'
      ? { type: 'workspaceControl', snapshot: localSnapshot(agent) } : { type: 'automationRun', run: run() };
  } };
  await requestMobileAutomation(transport, { type: 'startAutomation', id: ID, definitionId: DEFINITION, expectedDefinitionRevision: 7 });
  await requestMobileAutomation(transport, { type: 'setRoutinePaused', id: 'routine', expectedRevision: 7, paused: false });
  expect(calls.map(command => command.type)).toEqual(['loadWorkspaceControl', 'startAutomation', 'loadWorkspaceControl', 'setRoutinePaused']);
});

test('routine intervals read in the largest whole unit', () => {
  expect(intervalLabel(1)).toBe('Every minute');
  expect(intervalLabel(45)).toBe('Every 45 minutes');
  expect(intervalLabel(60)).toBe('Every hour');
  expect(intervalLabel(240)).toBe('Every 4 hours');
  expect(intervalLabel(90)).toBe('Every 90 minutes');
  expect(intervalLabel(1_440)).toBe('Every day');
  expect(intervalLabel(4_320)).toBe('Every 3 days');
  expect(intervalLabel(10_080)).toBe('Every week');
  expect(intervalLabel(20_160)).toBe('Every 2 weeks');
});
