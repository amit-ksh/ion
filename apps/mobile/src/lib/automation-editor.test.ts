import { describe, expect, test } from 'bun:test';
import { WakuRpcError, type Command, type ControlSnapshot, type ResponsePayload } from '@waku/client';
import { cloneDefinition, definitionDraftKey, definitionForSave, newDefinition, newStage, persistDefinitionDraft, readDefinitionDraft, rebaseDefinitionDraft, removeWorkflowStage, sameDefinition, saveDefinitionDraft, type AutomationDefinition, type DefinitionDraft } from './automation-editor';
import type { SubmissionStorage } from './automation-api';
const ID = '00000000-0000-4000-8000-000000000001';
const PROJECT = '00000000-0000-4000-8000-000000000002';
const TEAM = '00000000-0000-4000-8000-000000000003';
const OTHER = '00000000-0000-4000-8000-000000000004';
function fixture(kind: 'workflow' | 'routine' = 'workflow') {
  const values = new Map<string, string>(); const calls: Command[] = [];
  const record = newDefinition(kind, ID, PROJECT, TEAM); record.name = 'Release';
  if (record.kind === 'workflow') { record.prompt = 'Ship the release'; record.stages[0].title = 'Review'; record.stages[0].instructions = 'Review changes'; }
  else record.instructions = 'Review the latest changes';
  const draft: DefinitionDraft = { version: 1, revision: 7, record, baseline: null };
  const storage: SubmissionStorage = { getItem: async (key) => values.get(key) ?? null, setItem: async (key, value) => { values.set(key, value); }, removeItem: async (key) => { values.delete(key); } };
  const key = definitionDraftKey(`owner-${Math.random()}`, ID);
  const snapshot = (definition: AutomationDefinition = definitionForSave(record)): ControlSnapshot => ({ revision: 8, records: [definition] });
  let answer: (command: Command) => Promise<ResponsePayload> = async () => ({ type: 'workspaceControl', snapshot: snapshot() });
  const transport = { request: async (command: Command) => { calls.push(command); return answer(command); } };
  return { values, calls, record, draft, storage, key, snapshot, transport, answer: (next: typeof answer) => { answer = next; }, save: () => saveDefinitionDraft(transport, storage, key, draft) };
}
describe('phone definition persistence and save recovery', () => {
  test('stable definition identity is persisted before mutation and exact acknowledgement clears it', async () => {
    const f = fixture();
    f.answer(async (command) => { expect(readDefinitionDraft(f.values.get(f.key)!).record.id).toBe(ID);
      expect(command).toEqual({ type: 'saveWorkspaceRecord', expectedRevision: 7, record: f.record });
      return { type: 'workspaceControl', snapshot: f.snapshot() }; });
    expect((await f.save()).revision).toBe(8); expect(f.values.size).toBe(0); expect(f.calls).toHaveLength(1);
  });
  test('storage failure sends no owner mutation', async () => {
    const f = fixture(); f.storage.setItem = async () => { throw new Error('Storage unavailable'); };
    await expect(f.save()).rejects.toThrow('Storage unavailable'); expect(f.calls).toHaveLength(0);
  });
  test('unknown acknowledgement reconciles by reading, without repeating save', async () => {
    const f = fixture(); f.answer(async (command) => { if (command.type === 'saveWorkspaceRecord') throw new Error('Lost connection');
      return { type: 'workspaceControl', snapshot: f.snapshot() }; });
    await f.save(); expect(f.calls.map((c) => c.type)).toEqual(['saveWorkspaceRecord', 'loadWorkspaceControl']); expect(f.values.size).toBe(0);
  });
  test('unconfirmed or conflicting response retains the same local draft and pinned revision', async () => {
    const f = fixture(); const conflicting = cloneDefinition(f.record); conflicting.name = 'Edited on desktop';
    f.answer(async (command) => { if (command.type === 'saveWorkspaceRecord') throw new WakuRpcError('Workspace changed');
      return { type: 'workspaceControl', snapshot: f.snapshot(conflicting) }; });
    await expect(f.save()).rejects.toThrow('Workspace changed'); expect(f.calls).toHaveLength(2);
    const saved = readDefinitionDraft(f.values.get(f.key)!); expect(saved.revision).toBe(7); expect(saved.record.id).toBe(ID);
    f.calls.length = 0; f.answer(async () => { throw new Error('Offline'); });
    await expect(f.save()).rejects.toThrow('could not be confirmed'); expect(f.calls).toHaveLength(2); expect(f.values.size).toBe(1);
  });
  test('wrong record acknowledgement cannot clear or rewrite the original intent', async () => {
    const f = fixture(); const wrong = cloneDefinition(f.record); wrong.id = OTHER;
    f.answer(async () => ({ type: 'workspaceControl', snapshot: f.snapshot(wrong) }));
    await expect(f.save()).rejects.toThrow('could not be confirmed'); expect(f.values.size).toBe(1);
    expect(f.calls.filter((c) => c.type === 'saveWorkspaceRecord')).toHaveLength(1);
  });
  test('local cleanup failure retains a reconcilable acknowledged identity', async () => {
    const f = fixture(); const remove = f.storage.removeItem; f.storage.removeItem = async () => { throw new Error('Phone storage failed'); };
    await expect(f.save()).rejects.toThrow('was saved'); expect(f.values.size).toBe(1);
    f.storage.removeItem = remove;
    f.answer(async (command) => { if (command.type === 'saveWorkspaceRecord') throw new WakuRpcError('Workspace changed');
      return { type: 'workspaceControl', snapshot: f.snapshot() }; });
    await f.save(); expect(f.values.size).toBe(0);
  });
  test('concurrent taps share one persisted mutation, while owner addresses use separate drafts', async () => {
    const f = fixture(); let release!: () => void; const pending = new Promise<void>((done) => { release = done; });
    f.answer(async () => { await pending; return { type: 'workspaceControl', snapshot: f.snapshot() }; });
    const first = f.save(); const second = f.save(); release(); await Promise.all([first, second]); expect(f.calls).toHaveLength(1);
    expect(definitionDraftKey('profile:address-a', ID)).not.toBe(definitionDraftKey('profile:address-b', ID));
    expect(definitionDraftKey('profile:address-a', ID)).not.toBe(definitionDraftKey('profile:address-a', OTHER));
  });
  test('draft writes serialize before save and acknowledgement cleanup', async () => {
    const f = fixture(); let release!: () => void; const pending = new Promise<void>((done) => { release = done; });
    const write = f.storage.setItem; let count = 0;
    f.storage.setItem = async (key, value) => { if (++count === 1) await pending; await write(key, value); };
    const stale = { ...f.draft, record: { ...f.record, name: 'Older edit' } };
    const first = persistDefinitionDraft(f.storage, f.key, stale); const save = f.save();
    expect(f.calls).toHaveLength(0); release(); await Promise.all([first, save]); expect(f.values.size).toBe(0);
    expect(f.calls).toHaveLength(1);
  });
  test('refresh can advance an unrelated revision but cannot overwrite changed or removed definitions', () => {
    const f = fixture(); const base = cloneDefinition(f.record); const editing = { ...f.draft, baseline: base, record: { ...f.record, name: 'My edit' } };
    const rebased = rebaseDefinitionDraft(editing, { revision: 99, records: [base] });
    expect(rebased.record.name).toBe('My edit'); expect(rebased.revision).toBe(99);
    expect(() => rebaseDefinitionDraft(editing, f.snapshot({ ...base, name: 'Another edit' }))).toThrow('another client');
    expect(() => rebaseDefinitionDraft(editing, { revision: 99, records: [] })).toThrow('another client');
    expect(() => rebaseDefinitionDraft(f.draft, f.snapshot())).toThrow('another client');
  });
  test('routine saves pause explicitly and timer updates alone are not authoring conflicts', async () => {
    const f = fixture('routine'); if (f.record.kind !== 'routine') throw new Error();
    f.record.paused = false; f.record.nextRun = 100; f.record.lastError = 'Old failure';
    const base = cloneDefinition(f.record); const editing: DefinitionDraft = { ...f.draft, baseline: base };
    const advanced = { ...f.record, nextRun: 200, lastError: null };
    expect(rebaseDefinitionDraft(editing, f.snapshot(advanced)).revision).toBe(8);
    f.answer(async (command) => { if (command.type !== 'saveWorkspaceRecord') throw new Error();
      expect(command.record).toMatchObject({ paused: true, nextRun: 0, lastError: null, teamId: TEAM, projectId: PROJECT });
      return { type: 'workspaceControl', snapshot: f.snapshot(command.record as AutomationDefinition) }; });
    await f.save(); expect(f.values.size).toBe(0);
  });
  test('malformed, oversized and structurally incompatible drafts are kept and rejected', () => {
    const f = fixture(); expect(() => readDefinitionDraft('{')).toThrow('kept');
    expect(() => readDefinitionDraft(JSON.stringify({ ...f.draft, version: 2 }))).toThrow('incompatible');
    expect(() => readDefinitionDraft(JSON.stringify({ ...f.draft, record: { ...f.record, stages: [null] } }))).toThrow('incompatible');
    expect(() => readDefinitionDraft(JSON.stringify({ ...f.draft, record: { ...f.record, name: '🌿'.repeat(150000) } }))).toThrow('too large');
    expect(readDefinitionDraft(JSON.stringify(f.draft)).record.id).toBe(ID);
  });
});
describe('workflow stage removal preserves authored routing', () => {
  test('remaining routes remap and the first stage cannot inherit code', () => {
    const f = fixture(); if (f.record.kind !== 'workflow') throw new Error();
    const workflow = { ...f.record, stages: [newStage(TEAM), { ...newStage(TEAM), inheritWorkspace: true }, newStage(TEAM), newStage(TEAM)] };
    workflow.stages[0].transition = { type: 'documentCondition', document: 'result.json', pointer: '/ready', equals: true, onMatch: { type: 'stage', index: 3 }, onMismatch: { type: 'finish' } };
    const removed = removeWorkflowStage(workflow, 2); expect(removed.stages[0].transition).toMatchObject({ onMatch: { type: 'stage', index: 2 } });
    expect(removeWorkflowStage(workflow, 0).stages[0].inheritWorkspace).toBe(false);
    expect(workflow.stages).toHaveLength(4);
  });
  test('referenced stages and the last remaining stage cannot be removed silently', () => {
    const f = fixture(); if (f.record.kind !== 'workflow') throw new Error(); const record = f.record;
    expect(() => removeWorkflowStage(record, 0)).toThrow('at least one');
    record.stages.push(newStage(TEAM)); record.stages[0].transition = { type: 'goTo', target: { type: 'stage', index: 1 } };
    expect(() => removeWorkflowStage(record, 1)).toThrow('routes here'); expect(f.record.stages).toHaveLength(2);
  });
  test('owner default omission does not fabricate a conflict or discard explicit limits', () => {
    const f = fixture(); if (f.record.kind !== 'workflow') throw new Error();
    const baseline = cloneDefinition(f.record); if (baseline.kind !== 'workflow') throw new Error();
    delete baseline.stages[0].inheritWorkspace; delete baseline.stages[0].transition; delete baseline.stages[0].requiredFormats;
    expect(sameDefinition(baseline, f.record)).toBe(true);
    const changed = cloneDefinition(f.record); if (changed.kind !== 'workflow') throw new Error();
    changed.stages[0].requiresDecision = true; expect(sameDefinition(changed, f.record)).toBe(false);
  });
});
