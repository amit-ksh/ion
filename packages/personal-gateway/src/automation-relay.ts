import { openDatabase, type DatabaseSource, type SqlDatabase, type Binding as SQLQueryBindings } from "./database";
import { createHash, randomUUID } from 'node:crypto';

import type { AutomationDefinition, AutomationRunSummary, AutomationProjection, AutomationAction, AutomationSubmission, AutomationCommand, AutomationOwner } from '../../waku-client/src/gateway-automation';
export type { AutomationDefinition, AutomationRunSummary, AutomationProjection, AutomationAction, AutomationSubmission, AutomationCommand, AutomationOwner } from '../../waku-client/src/gateway-automation';
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const HASH = /^[a-f0-9]{64}$/;
const statuses = ['running', 'waitingDecision', 'blocked', 'completed', 'canceled', 'skipped'];
function object(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}
function keys(value: unknown, names: string): boolean {
  return object(value) && Object.keys(value).sort().join(',') === names.split(',').sort().join(',');
}
function integer(value: unknown, max = Number.MAX_SAFE_INTEGER): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 && value <= max;
}
function id(value: unknown): value is string { return typeof value === 'string' && UUID.test(value); }
function text(value: unknown, max: number): value is string { return typeof value === 'string' && value.length <= max; }
function canonical(value: unknown): string {
  if (Array.isArray(value)) return '[' + value.map(canonical).join(',') + ']';
  if (object(value)) return '{' + Object.keys(value).sort().map((key) => JSON.stringify(key) + ':' + canonical(value[key])).join(',') + '}';
  return JSON.stringify(value);
}
function witness(publisherId: string, value: unknown): string {
  return createHash('sha256').update('ion-gateway-automation-v1:' + publisherId + ':' + canonical(value)).digest('hex');
}
export function validateProjection(value: unknown): asserts value is AutomationProjection {
  if (!keys(value, 'version,revision,projects,definitions,runs,truncated') || !object(value) ||
      value.version !== 1 || !integer(value.revision) || typeof value.truncated !== 'boolean' ||
      !Array.isArray(value.projects) || value.projects.length > 64 ||
      !Array.isArray(value.definitions) || value.definitions.length > 64 ||
      !Array.isArray(value.runs) || value.runs.length > 64 ||
      Buffer.byteLength(JSON.stringify(value)) > 60_000) throw new Error('Invalid automation projection');
  const projects = new Set<string>(); const definitions = new Set<string>(); const runs = new Set<string>();
  for (const project of value.projects) {
    if (!keys(project, 'id,name') || !id(project.id) || !text(project.name, 160) || projects.has(project.id)) throw new Error('Invalid project summary');
    projects.add(project.id);
  }
  for (const d of value.definitions) {
    if (!keys(d, 'id,projectId,name,kind,revision,ready,paused,nextRun,schedule,stageCount') ||
        !id(d.id) || !id(d.projectId) || !projects.has(d.projectId) || !text(d.name, 160) ||
        !['workflow', 'routine'].includes(d.kind) || d.revision !== value.revision ||
        typeof d.ready !== 'boolean' || !integer(d.stageCount, 24) || definitions.has(d.id) ||
        (d.kind === 'workflow' && (d.paused !== null || d.nextRun !== null || d.schedule !== null)) ||
        (d.kind === 'routine' && (typeof d.paused !== 'boolean' || !integer(d.nextRun) || !text(d.schedule, 500)))) throw new Error('Invalid automation definition');
    definitions.add(d.id);
  }
  for (const r of value.runs) {
    if (!keys(r, 'id,projectId,name,definitionId,revision,status,currentStage,stageCount,gate') ||
        !id(r.id) || !id(r.projectId) || !projects.has(r.projectId) || !id(r.definitionId) ||
        !text(r.name, 160) || !integer(r.revision) || !statuses.includes(r.status) ||
        !integer(r.stageCount, 24) || r.stageCount === 0 || !integer(r.currentStage, r.stageCount) || runs.has(r.id)) throw new Error('Invalid automation run');
    if (r.gate !== null && (!keys(r.gate, 'index,title,instructions,reviewComplete') ||
        r.status !== 'waitingDecision' || !integer(r.gate.index, r.stageCount - 1) || r.gate.index !== r.currentStage ||
        !text(r.gate.title, 160) || !text(r.gate.instructions, 2_000) || typeof r.gate.reviewComplete !== 'boolean')) throw new Error('Invalid stage review');
    if (r.status === 'waitingDecision' && r.gate === null) throw new Error('Missing stage review');
    runs.add(r.id);
  }
}
export function validateAction(value: unknown): asserts value is AutomationAction {
  if (!object(value) || !integer(value.expectedRevision) ||
      (value.kind === 'start' ? !keys(value, 'kind,definitionId,runId,expectedRevision') || !id(value.definitionId) || !id(value.runId)
      : value.kind === 'stop' ? !keys(value, 'kind,runId,expectedRevision') || !id(value.runId)
      : value.kind === 'gate' ? !keys(value, 'kind,runId,expectedRevision,stage,approved') || !id(value.runId) || !integer(value.stage, 23) || typeof value.approved !== 'boolean'
      : value.kind === 'routine' ? !keys(value, 'kind,definitionId,expectedRevision,paused') || !id(value.definitionId) || typeof value.paused !== 'boolean'
      : true)) throw new Error('Invalid automation action');
}

/** A bounded metadata mirror. Only the registered publisher can claim a command;
 * claiming commits uncertainty before the daemon sees any potentially billable action. */
export class DurableAutomationRelay {
  private readonly db: SqlDatabase;
  constructor(path: DatabaseSource, private readonly now: () => number = Date.now) {
    this.db = openDatabase(path);
    this.db.run('PRAGMA busy_timeout = 5000'); this.db.run('PRAGMA synchronous = FULL');
    this.db.run('CREATE TABLE IF NOT EXISTS relay_automation_owners (publisher_id TEXT PRIMARY KEY, body TEXT NOT NULL, published_at INTEGER NOT NULL)');
    this.db.run('CREATE TABLE IF NOT EXISTS relay_automation_commands (id TEXT PRIMARY KEY, device_id TEXT NOT NULL, publisher_id TEXT NOT NULL, idempotency_key TEXT NOT NULL, body TEXT NOT NULL, status TEXT NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, UNIQUE(device_id, idempotency_key))');
    this.db.run('CREATE INDEX IF NOT EXISTS relay_automation_queue ON relay_automation_commands(publisher_id,status,created_at)');
  }
  close(): void { this.db.close(true); }
  private write<T>(operation: () => T): T {
    return this.db.atomic(operation);
  }
  publish(publisherId: string, value: unknown): void {
    if (!id(publisherId)) throw new Error('Invalid owner');
    validateProjection(value);
    this.db.run('INSERT INTO relay_automation_owners VALUES (?1,?2,?3) ON CONFLICT(publisher_id) DO UPDATE SET body=excluded.body,published_at=excluded.published_at', [publisherId, JSON.stringify(value), this.now()]);
  }
  private owner(publisherId: string): AutomationOwner | null {
    const row = this.db.query<{ body: string; published_at: number }, [string]>('SELECT body,published_at FROM relay_automation_owners WHERE publisher_id=?1').get(publisherId);
    if (!row) return null;
    const projection = JSON.parse(row.body) as AutomationProjection;
    return { ...projection, publisherId, publishedAt: row.published_at,
      definitions: projection.definitions.map((d) => ({ ...d, fingerprint: witness(publisherId, d) })),
      runs: projection.runs.map((r) => ({ ...r, fingerprint: witness(publisherId, r) })) };
  }
  owners(allowedPublishers: Set<string>): AutomationOwner[] {
    return [...allowedPublishers].sort().slice(0, 64).map((id) => this.owner(id)).filter((owner): owner is AutomationOwner => owner !== null);
  }
  private current(input: AutomationSubmission): void {
    const owner = this.owner(input.publisherId);
    const time = this.now();
    if (!owner || owner.publishedAt > time || time - owner.publishedAt > 60_000) throw new Error('Owner projection is offline or stale');
    const action = input.action;
    if (action.kind === 'start' || action.kind === 'routine') {
      const definition = owner.definitions.find((d) => d.id === action.definitionId);
      if (!definition || definition.revision !== action.expectedRevision || definition.fingerprint !== input.fingerprint ||
          (action.kind === 'start' && !definition.ready) || (action.kind === 'routine' && (definition.kind !== 'routine' || (!action.paused && !definition.ready)))) throw new Error('Definition changed or cannot run');
    } else {
      const run = owner.runs.find((r) => r.id === action.runId);
      if (!run || run.revision !== action.expectedRevision || run.fingerprint !== input.fingerprint ||
          ['completed', 'canceled', 'skipped'].includes(run.status) ||
          (action.kind === 'gate' && (run.status !== 'waitingDecision' || run.gate?.index !== action.stage || (action.approved && !run.gate.reviewComplete)))) throw new Error('Run changed or review is incomplete');
    }
  }
  enqueue(deviceId: string, input: AutomationSubmission): AutomationCommand {
    if (!id(deviceId) || !keys(input, 'publisherId,idempotencyKey,fingerprint,action') || !id(input.publisherId) || !id(input.idempotencyKey) || !HASH.test(input.fingerprint)) throw new Error('Invalid automation command');
    validateAction(input.action);
    return this.write(() => {
      const existing = this.byKey(deviceId, input.idempotencyKey);
      if (existing) {
        if (canonical(input) !== canonical({ publisherId: existing.publisherId, idempotencyKey: existing.idempotencyKey, fingerprint: existing.fingerprint, action: existing.action })) throw new Error('Idempotency key changed');
        return existing;
      }
      this.current(input);
      if (this.db.query<{ count: number }, [string]>("SELECT count(*) AS count FROM relay_automation_commands WHERE publisher_id=?1 AND status IN ('queued','uncertain')").get(input.publisherId)!.count >= 256) throw new Error('Owner command queue is full');
      const time = this.now(); const command: AutomationCommand = { ...input, id: randomUUID(), deviceId, status: 'queued', createdAt: time, updatedAt: time };
      this.db.run('INSERT INTO relay_automation_commands VALUES (?1,?2,?3,?4,?5,?6,?7,?8)', [command.id, deviceId, input.publisherId, input.idempotencyKey, JSON.stringify(command), command.status, time, time]);
      return command;
    });
  }
  command(id: string): AutomationCommand | null {
    const row = this.db.query<{ body: string; status: AutomationCommand['status']; updated_at: number }, [string]>('SELECT body,status,updated_at FROM relay_automation_commands WHERE id=?1').get(id);
    return row ? { ...JSON.parse(row.body), status: row.status, updatedAt: row.updated_at } : null;
  }
  byKey(deviceId: string, key: string): AutomationCommand | null {
    if (!id(key)) throw new Error('Invalid command key');
    const row = this.db.query<{ id: string }, [string, string]>('SELECT id FROM relay_automation_commands WHERE device_id=?1 AND idempotency_key=?2').get(deviceId, key);
    return row ? this.command(row.id) : null;
  }
  pending(publisherId: string): AutomationCommand[] {
    return this.db.query<{ id: string }, [string]>("SELECT id FROM relay_automation_commands WHERE publisher_id=?1 AND status='queued' ORDER BY CASE WHEN json_extract(body,'$.action.kind')='stop' THEN 0 ELSE 1 END,created_at,id LIMIT 32").all(publisherId).map((row) => this.command(row.id)!);
  }
  uncertain(publisherId: string): AutomationCommand[] {
    return this.db.query<{ id: string }, [string]>("SELECT id FROM relay_automation_commands WHERE publisher_id=?1 AND status='uncertain' ORDER BY created_at,id LIMIT 32").all(publisherId).map((row) => this.command(row.id)!);
  }
  recent(deviceId: string): AutomationCommand[] {
    return this.db.query<{ id: string }, [string]>('SELECT id FROM relay_automation_commands WHERE device_id=?1 ORDER BY created_at DESC,id DESC LIMIT 32').all(deviceId).map((row) => this.command(row.id)!);
  }
  claim(publisherId: string, commandId: string, senderAllowed: boolean): AutomationCommand {
    return this.write(() => {
      const command = this.command(commandId);
      if (!command || command.publisherId !== publisherId) throw new Error('Command belongs to another owner');
      if (command.status !== 'queued') throw new Error('Command has no unused claim');
      let valid = senderAllowed && this.now() >= command.createdAt && this.now() - command.createdAt <= 300_000;
      try { this.current(command); } catch { valid = false; }
      this.db.run('UPDATE relay_automation_commands SET status=?2,updated_at=?3 WHERE id=?1', [commandId, valid ? 'uncertain' : 'rejected', this.now()]);
      return this.command(commandId)!;
    });
  }
  settle(publisherId: string, commandId: string, applied: boolean): AutomationCommand {
    return this.write(() => {
      const command = this.command(commandId); const status = applied ? 'applied' : 'rejected';
      if (!command || command.publisherId !== publisherId) throw new Error('Command belongs to another owner');
      if (command.status === status) return command;
      if (command.status !== 'uncertain') throw new Error('Command is not awaiting a result');
      this.db.run('UPDATE relay_automation_commands SET status=?2,updated_at=?3 WHERE id=?1', [commandId, status, this.now()]);
      return this.command(commandId)!;
    });
  }
}
