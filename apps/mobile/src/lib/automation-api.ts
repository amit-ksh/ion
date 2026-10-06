import { WakuRpcError, type AutomationRun, type Command, type ResponsePayload } from '@waku/client';

export interface AutomationTransport {
  request(command: Command): Promise<ResponsePayload>;
}
export interface SubmissionStorage {
  getItem(key: string): Promise<string | null>;
  setItem(key: string, value: string): Promise<unknown>;
  removeItem(key: string): Promise<unknown>;
}
interface Submission { id: string; revision: number }
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const starts = new Map<string, Promise<AutomationRun>>();

/** Persist before sending. An unknown response always retains the original ID;
 * only a completed rejection plus confirmed absence permits another submission.
 * The namespace identifies both the saved profile and its exact daemon address. */
export async function startSavedAutomation(
  transport: AutomationTransport, storage: SubmissionStorage, namespace: string,
  definition: string, revision: number, randomUUID: () => string,
): Promise<AutomationRun> {
  const key = `ion.mobile.automation-start.v1.${encodeURIComponent(namespace)}.${definition}`;
  const active = starts.get(key);
  if (active) return active;
  const operation = start();
  starts.set(key, operation);
  try { return await operation; }
  finally { if (starts.get(key) === operation) starts.delete(key); }

  async function start(): Promise<AutomationRun> {
    const saved = await storage.getItem(key);
    let submission: Submission;
    if (saved !== null) {
      try { submission = JSON.parse(saved) as Submission; }
      catch { throw new Error('Saved run identity could not be read. Keep it for recovery; no run was started.'); }
      if (!submission || !UUID.test(submission.id) || !Number.isSafeInteger(submission.revision) || submission.revision < 0) {
        throw new Error('Saved run identity is invalid. Keep it for recovery; no run was started.');
      }
    } else {
      submission = { id: randomUUID(), revision };
      if (!UUID.test(submission.id) || !Number.isSafeInteger(revision) || revision < 0) throw new Error('Invalid run submission');
      // A failed durable write must prevent the first potentially billable request.
      await storage.setItem(key, JSON.stringify(submission));
    }
    let run: AutomationRun;
    try {
      run = await runResponse(transport, { type: 'startAutomation', id: submission.id, definitionId: definition, expectedDefinitionRevision: submission.revision });
    } catch (cause) {
      let confirmed: ResponsePayload | undefined;
      try { confirmed = await transport.request({ type: 'getAutomationRun', id: submission.id }); }
      catch { /* Preserve the submitted ID if recovery is unavailable. */ }
      if (confirmed?.type === 'automationRun' && confirmed.run.id === submission.id) run = confirmed.run;
      else {
        if (cause instanceof WakuRpcError && confirmed?.type === 'automationMissing' && confirmed.id === submission.id) await storage.removeItem(key);
        throw cause;
      }
    }
    if (run.id !== submission.id) throw new Error('The daemon returned another run identity; retry recovery.');
    await storage.removeItem(key);
    return run;
  }
}

export async function runResponse(transport: AutomationTransport, command: Command): Promise<AutomationRun> {
  const response = await transport.request(command);
  if (response.type !== 'automationRun') throw new Error('Unexpected automation response. Refresh before trying again.');
  return response.run;
}

export function automationStatus(status: AutomationRun['status']): string {
  return { running: 'Running', waitingDecision: 'Decision required', blocked: 'Needs attention', completed: 'Completed', canceled: 'Stopped', skipped: 'Skipped overlap' }[status];
}
export function routineTime(seconds: number): string {
  if (!seconds) return 'Paused';
  const date = new Date(seconds * 1000);
  return Number.isFinite(date.getTime()) ? date.toISOString().replace('T', ' ').slice(0, 16) + ' UTC' : 'Unknown time';
}
