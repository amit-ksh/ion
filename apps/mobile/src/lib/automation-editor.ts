import { WakuRpcError, type ControlSnapshot, type Routine, type Workflow, type WorkflowStage, type WorkflowTarget } from '@waku/client';
import type { AutomationTransport, SubmissionStorage } from './automation-api';

export type AutomationDefinition = (Workflow & { kind: 'workflow' }) | (Routine & { kind: 'routine' });
export type DefinitionDraft = { version: 1; revision: number; record: AutomationDefinition; baseline: AutomationDefinition | null };
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const MAX_DRAFT_BYTES = 512 * 1024;
const writes = new Map<string, Promise<void>>();
const saves = new Map<string, Promise<ControlSnapshot>>();

export function definitionDraftKey(owner: string, slot: string): string {
  return `ion.mobile.automation-definition.v1.${encodeURIComponent(owner)}.${encodeURIComponent(slot)}`;
}
export function newDefinition(kind: 'workflow' | 'routine', id: string, projectId: string, teamId: string | null): AutomationDefinition {
  if (kind === 'workflow') return { kind, id, projectId, name: '', prompt: '', stages: [newStage(teamId)], limits: null };
  return { kind, id, projectId, name: '', instructions: '', intervalMinutes: 1440, paused: true, nextRun: 0,
    provider: 'nebius', teamId, workflowId: null, lastError: null, missedRunPolicy: 'skip', overlapPolicy: 'skip' };
}
export function newStage(teamId: string | null = null): WorkflowStage {
  return { title: '', instructions: '', teamId, requiresDecision: false, inheritWorkspace: false, requiredFormats: [], transition: { type: 'next' } };
}
export function readDefinitionDraft(raw: string): DefinitionDraft {
  if (utf8Bytes(raw) > MAX_DRAFT_BYTES) throw new Error('The saved draft is too large. Keep it for review before starting another draft.');
  let draft: DefinitionDraft;
  try { draft = JSON.parse(raw) as DefinitionDraft; } catch { throw new Error('The saved draft could not be read. It has been kept for review.'); }
  if (draft?.version !== 1 || !Number.isSafeInteger(draft.revision) || draft.revision < 0 || !validDefinition(draft.record)
    || (draft.baseline !== null && (!validDefinition(draft.baseline) || draft.baseline.id !== draft.record.id || draft.baseline.kind !== draft.record.kind))) {
    throw new Error('The saved draft is incompatible. It has been kept for review.');
  }
  return draft;
}
function validDefinition(value: unknown): value is AutomationDefinition {
  if (!value || typeof value !== 'object') return false;
  const record = value as AutomationDefinition;
  const id = (v: unknown) => typeof v === 'string' && UUID.test(v);
  const numeric = (v: unknown) => typeof v === 'number' && Number.isSafeInteger(v) && v >= 0;
  const target = (v: unknown) => !!v && typeof v === 'object' && ('type' in v) && (v.type === 'finish' || v.type === 'stage' && 'index' in v && numeric(v.index));
  const transition = (v: WorkflowStage['transition']) => v === undefined || !!v && (v.type === 'next'
    || v.type === 'goTo' && target(v.target)
    || v.type === 'documentCondition' && typeof v.document === 'string' && typeof v.pointer === 'string'
      && (v.equals === null || typeof v.equals === 'string' || typeof v.equals === 'boolean' || typeof v.equals === 'number' && Number.isFinite(v.equals)) && target(v.onMatch) && target(v.onMismatch));
  if (!id(record.id) || !id(record.projectId) || typeof record.name !== 'string') return false;
  if (record.kind === 'workflow') return typeof record.prompt === 'string' && Array.isArray(record.stages) && record.stages.length >= 1 && record.stages.length <= 24
    && record.stages.every((s) => s && typeof s.title === 'string' && typeof s.instructions === 'string' && typeof s.requiresDecision === 'boolean'
      && (s.teamId === null || id(s.teamId)) && (s.inheritWorkspace === undefined || typeof s.inheritWorkspace === 'boolean')
      && (s.requiredFormats === undefined || Array.isArray(s.requiredFormats) && s.requiredFormats.every((v) => typeof v === 'string'))
      && (s.retry === undefined || !!s.retry && numeric(s.retry.maxAttempts) && numeric(s.retry.delaySeconds)) && transition(s.transition))
    && (record.limits === null || !!record.limits && ['maxWorkItems', 'maxParallelTasks', 'maxDepth', 'maxAttempts', 'maxModelCalls', 'maxReworkRounds', 'timeoutSeconds']
      .every((k) => numeric(record.limits![k as keyof NonNullable<typeof record.limits>]))
      && typeof record.limits.requireChildReview === 'boolean' && (record.limits.maxTokens === null || numeric(record.limits.maxTokens)));
  if (record.kind !== 'routine' || typeof record.instructions !== 'string' || !numeric(record.intervalMinutes) || typeof record.paused !== 'boolean' || !numeric(record.nextRun)
    || typeof record.provider !== 'string' || record.teamId !== null && !id(record.teamId) || record.workflowId !== null && !id(record.workflowId)) return false;
  const c = record.calendar;
  return c === undefined || !!c && typeof c.timeZone === 'string' && numeric(c.hour) && numeric(c.minute) && !!c.recurrence
    && (c.recurrence.type === 'daily' || (c.recurrence.type === 'weekly' || c.recurrence.type === 'monthly') && Array.isArray(c.recurrence.days) && c.recurrence.days.every(numeric));
}
function utf8Bytes(value: string): number {
  let bytes = 0;
  for (let i = 0; i < value.length; i++) {
    const code = value.charCodeAt(i);
    if (code < 0x80) bytes++;
    else if (code < 0x800) bytes += 2;
    else if (code >= 0xd800 && code <= 0xdbff && i + 1 < value.length && value.charCodeAt(i + 1) >= 0xdc00 && value.charCodeAt(i + 1) <= 0xdfff) { bytes += 4; i++; }
    else bytes += 3;
  }
  return bytes;
}
export function cloneDefinition(record: AutomationDefinition): AutomationDefinition { return JSON.parse(JSON.stringify(record)) as AutomationDefinition; }
function serial(key: string, write: () => Promise<unknown>): Promise<void> {
  const operation = (writes.get(key) ?? Promise.resolve()).catch(() => {}).then(async () => { await write(); });
  writes.set(key, operation);
  const clear = () => { if (writes.get(key) === operation) writes.delete(key); };
  void operation.then(clear, clear);
  return operation;
}
export function persistDefinitionDraft(storage: SubmissionStorage, key: string, draft: DefinitionDraft): Promise<void> {
  const raw = JSON.stringify(draft);
  readDefinitionDraft(raw);
  return serial(key, () => storage.setItem(key, raw));
}
export function clearDefinitionDraft(storage: SubmissionStorage, key: string): Promise<void> {
  return serial(key, () => storage.removeItem(key));
}
// Owner defaults and timer-owned fields do not constitute an authoring conflict.
function content(record: AutomationDefinition): unknown {
  if (record.kind === 'routine') {
    const { nextRun: _next, lastError: _error, ...definition } = record;
    return { ...definition, calendar: record.calendar ?? null, missedRunPolicy: record.missedRunPolicy ?? 'skip', overlapPolicy: record.overlapPolicy ?? 'skip' };
  }
  return { ...record, limits: record.limits ?? null, stages: record.stages.map((s) => ({ ...s, inheritWorkspace: s.inheritWorkspace ?? false,
    requiredFormats: s.requiredFormats ?? [], transition: s.transition ?? { type: 'next' }, retry: s.retry ?? null })) };
}
function stable(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(stable).join(',')}]`;
  if (value !== null && typeof value === 'object') return `{${Object.entries(value).filter(([, v]) => v !== undefined).sort(([a], [b]) => a.localeCompare(b)).map(([k, v]) => `${JSON.stringify(k)}:${stable(v)}`).join(',')}}`;
  return JSON.stringify(value) ?? 'null';
}
export function sameDefinition(a: AutomationDefinition, b: AutomationDefinition): boolean {
  return stable(content(a)) === stable(content(b));
}
function find(snapshot: ControlSnapshot, record: AutomationDefinition): AutomationDefinition | null {
  const saved = snapshot.records.find((r) => r.id === record.id);
  if (!saved) return null;
  if (saved.kind !== record.kind) throw new Error('This definition identity now belongs to another record. Reload before editing.');
  return saved as AutomationDefinition;
}
export function rebaseDefinitionDraft(draft: DefinitionDraft, snapshot: ControlSnapshot): DefinitionDraft {
  const current = find(snapshot, draft.record);
  if (draft.baseline ? !current || !sameDefinition(current, draft.baseline) : current !== null) {
    throw new Error('This definition changed on another client. Load the latest definition to review it; your draft has been kept.');
  }
  return { ...draft, revision: snapshot.revision, baseline: current };
}
/** Routine edits save paused; enabling is an explicit existing owner action. */
export function definitionForSave(record: AutomationDefinition): AutomationDefinition {
  return record.kind === 'routine' ? { ...record, paused: true, nextRun: 0, lastError: null } : record;
}
export async function saveDefinitionDraft(transport: AutomationTransport, storage: SubmissionStorage, key: string, draft: DefinitionDraft): Promise<ControlSnapshot> {
  const pending = saves.get(key); if (pending) return pending;
  const operation = save(); saves.set(key, operation);
  try { return await operation; } finally { if (saves.get(key) === operation) saves.delete(key); }
  async function save(): Promise<ControlSnapshot> {
    const submitted = { ...draft, record: definitionForSave(draft.record) };
    await persistDefinitionDraft(storage, key, submitted);
    let snapshot: ControlSnapshot;
    try {
      const response = await transport.request({ type: 'saveWorkspaceRecord', expectedRevision: submitted.revision, record: submitted.record });
      if (response.type !== 'workspaceControl') throw new Error('Unexpected save acknowledgement.');
      const stored = find(response.snapshot, submitted.record);
      if (!stored || !sameDefinition(stored, submitted.record)) throw new Error('The save acknowledgement did not contain this definition.');
      snapshot = response.snapshot;
    } catch (cause) {
      // One read reconciles an unknown acknowledgement. Never resend a mutation
      // or silently advance a revision after another client's conflicting edit.
      const current = await transport.request({ type: 'loadWorkspaceControl' }).catch(() => null);
      const saved = current?.type === 'workspaceControl' ? find(current.snapshot, submitted.record) : null;
      if (!saved || !sameDefinition(saved, submitted.record)) {
        throw new Error(cause instanceof WakuRpcError ? `The definition was not saved: ${cause.message}` : 'Saving could not be confirmed. Your draft is kept. Refresh before retrying.');
      }
      snapshot = (current as Extract<typeof current, { type: 'workspaceControl' }>).snapshot;
    }
    try { await clearDefinitionDraft(storage, key); } catch { throw new Error('The definition was saved, but its local draft could not be cleared. Retry to reconcile the same definition.'); }
    return snapshot;
  }
}
export function removeWorkflowStage(workflow: Workflow, index: number): Workflow {
  if (workflow.stages.length <= 1) throw new Error('A workflow needs at least one stage.');
  if (!Number.isInteger(index) || index < 0 || index >= workflow.stages.length) throw new Error('Choose an existing stage.');
  function remap(target: WorkflowTarget): WorkflowTarget {
    if (target.type === 'finish') return target;
    if (target.index === index) throw new Error('Another stage routes here. Change that route before removing this stage.');
    return { type: 'stage', index: target.index > index ? target.index - 1 : target.index };
  }
  const stages = workflow.stages.filter((_, i) => i !== index).map((s, i) => {
    const t = s.transition;
    return { ...s, inheritWorkspace: i === 0 ? false : s.inheritWorkspace,
      transition: !t || t.type === 'next' ? t : t.type === 'goTo' ? { ...t, target: remap(t.target) }
        : { ...t, onMatch: remap(t.onMatch), onMismatch: remap(t.onMismatch) } };
  });
  return { ...workflow, stages };
}
