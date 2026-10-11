import type { AutomationAction, AutomationCommand, AutomationOwner, AutomationSubmission } from '@waku/client';
import type { SubmissionStorage } from './automation-api';
import { gatewayAutomationCommandByKey, gatewayEnqueueAutomation, GatewayRequestError, type GatewayPairing } from './gateway-client';

export type AutomationIntent = Exclude<AutomationAction, { kind: 'start' }> | Omit<Extract<AutomationAction, { kind: 'start' }>, 'runId'>;
interface Transport {
  lookup(pairing: GatewayPairing, key: string): Promise<AutomationCommand | null>;
  enqueue(pairing: GatewayPairing, input: AutomationSubmission): Promise<AutomationCommand>;
}
const actual: Transport = { lookup: gatewayAutomationCommandByKey, enqueue: gatewayEnqueueAutomation };
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const HASH = /^[a-f0-9]{64}$/;
const active = new Map<string, Promise<{ command: AutomationCommand; recovered: boolean }>>();
export function gatewayOwnerFresh(owner: Pick<AutomationOwner, 'publishedAt'>, now = Date.now()): boolean {
  return Number.isSafeInteger(owner.publishedAt) && owner.publishedAt <= now && now - owner.publishedAt <= 60_000;
}
function canonical(value: unknown): string {
  if (Array.isArray(value)) return '[' + value.map(canonical).join(',') + ']';
  if (value && typeof value === 'object') return '{' + Object.keys(value).sort().map((k) => JSON.stringify(k) + ':' + canonical((value as Record<string, unknown>)[k])).join(',') + '}';
  return JSON.stringify(value);
}
function validAction(action: AutomationAction): boolean {
  if (!action || !Number.isSafeInteger(action.expectedRevision) || action.expectedRevision < 0) return false;
  const keys = Object.keys(action).sort().join(',');
  return action.kind === 'start' ? keys === 'definitionId,expectedRevision,kind,runId' && UUID.test(action.definitionId) && UUID.test(action.runId)
    : action.kind === 'stop' ? keys === 'expectedRevision,kind,runId' && UUID.test(action.runId)
    : action.kind === 'routine' ? keys === 'definitionId,expectedRevision,kind,paused' && UUID.test(action.definitionId) && typeof action.paused === 'boolean'
    : action.kind === 'gate' ? keys === 'approved,expectedRevision,kind,runId,stage' && UUID.test(action.runId) && Number.isInteger(action.stage) && action.stage >= 0 && action.stage <= 23 && typeof action.approved === 'boolean'
    : false;
}
function validSubmission(value: AutomationSubmission): boolean {
  return !!value && Object.keys(value).sort().join(',') === 'action,fingerprint,idempotencyKey,publisherId' && UUID.test(value.publisherId) && UUID.test(value.idempotencyKey) && HASH.test(value.fingerprint) && validAction(value.action);
}
function verifyCommand(command: AutomationCommand, pairing: GatewayPairing, input: AutomationSubmission): void {
  if (!command || !UUID.test(command.id) || command.deviceId !== pairing.id ||
      !['queued', 'uncertain', 'applied', 'rejected'].includes(command.status) ||
      canonical({ publisherId: command.publisherId, idempotencyKey: command.idempotencyKey, fingerprint: command.fingerprint, action: command.action }) !== canonical(input)) {
    throw new Error('Gateway returned another command identity. Keep the saved request for recovery.');
  }
}
/** Persist exactly one request before sending. On another tap, recover that
 * request first; a changed current selection never overwrites an unknown effect. */
export async function submitGatewayAutomation(
  pairing: GatewayPairing, storage: SubmissionStorage, publisherId: string, fingerprint: string,
  intent: AutomationIntent, randomUUID: () => string, transport: Transport = actual, authorized: () => boolean = () => true,
): Promise<{ command: AutomationCommand; recovered: boolean }> {
  if (!pairing.grants.includes('automationControl') || pairing.expiresAt <= Date.now()) throw new Error('Workflow control grant is unavailable.');
  const target = 'definitionId' in intent ? intent.definitionId : intent.runId;
  const key = `ion.mobile.gateway-automation.v1.${encodeURIComponent(pairing.baseUrl)}.${pairing.id}.${publisherId}.${intent.kind}.${target}`;
  const pending = active.get(key); if (pending) return pending;
  const operation = perform(); active.set(key, operation);
  try { return await operation; } finally { if (active.get(key) === operation) active.delete(key); }
  async function perform() {
    const saved = await storage.getItem(key); let input: AutomationSubmission;
    if (saved !== null) {
      try { input = JSON.parse(saved); } catch { throw new Error('Saved request is unreadable. Keep it for recovery; no new action was sent.'); }
      if (!validSubmission(input) || input.publisherId !== publisherId || input.action.kind !== intent.kind ||
          ('definitionId' in input.action ? input.action.definitionId : input.action.runId) !== target) throw new Error('Saved request identity is invalid. Keep it for recovery.');
    } else {
      input = { publisherId, fingerprint, idempotencyKey: randomUUID(), action: intent.kind === 'start' ? { ...intent, runId: randomUUID() } : intent };
      if (!validSubmission(input)) throw new Error('Invalid workflow request');
      await storage.setItem(key, JSON.stringify(input));
    }
    let command: AutomationCommand | null = null;
    if (saved !== null) {
      // If status lookup fails, stop here. It is not evidence of absence.
      command = await transport.lookup(pairing, input.idempotencyKey);
      if (command) verifyCommand(command, pairing, input);
    }
    if (!command) {
      const originalIntent = input.action.kind === 'start' ? { kind: input.action.kind, definitionId: input.action.definitionId, expectedRevision: input.action.expectedRevision } : input.action;
      if (canonical(originalIntent) !== canonical(intent)) throw new Error('Earlier saved request differs from this action. Keep it for owner recovery; no new action was sent.');
      if (!authorized()) throw new Error('Workflow authorization changed. Keep the saved request for recovery; no action was sent.');
      try { command = await transport.enqueue(pairing, input); verifyCommand(command, pairing, input); }
      catch (error) {
        // Recover a lost or invalid acknowledgement without replaying it.
        const recorded = await transport.lookup(pairing, input.idempotencyKey).catch(() => undefined);
        if (recorded) { verifyCommand(recorded, pairing, input); command = recorded; }
        else {
          if (recorded === null && error instanceof GatewayRequestError && [400,409,422].includes(error.status)) await storage.removeItem(key);
          throw error;
        }
      }
    }
    if (command.status === 'applied' || command.status === 'rejected') await storage.removeItem(key);
    return { command, recovered: saved !== null };
  }
}
