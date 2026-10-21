import { assertMobileTeam } from './mobile-capabilities';
import { WakuRpcError, type ControlSnapshot, type AutomationRun, type CalendarSchedule, type RoutineTrigger, type WorkflowTarget, type WorkflowTransition, type Command, type ResponsePayload } from '@waku/client';

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

/** All positive execution transitions cross the same mobile capability gate.
 * Run history, stopping, declining and pausing remain available. */
export async function requestMobileAutomation(transport: AutomationTransport, command: Command): Promise<ResponsePayload> {
  if (command.type === 'generateWorkflow') throw new Error('Generate workflow stages on desktop.');
  if (command.type === 'resolveAutomationGate' && command.approved) {
    // The public run projection does not expose the pinned future-stage profiles.
    // Current definitions cannot prove what this existing run will execute.
    throw new Error('Approve workflow stages on desktop, where pinned execution can be verified.');
  }
  if (command.type === 'startAutomation' || command.type === 'setRoutinePaused' && !command.paused) {
    const response = await transport.request({ type: 'loadWorkspaceControl' });
    if (response.type !== 'workspaceControl') throw new Error('Refresh to verify this automation before running.');
    assertMobileAutomationDefinition(response.snapshot, command.type === 'startAutomation' ? command.definitionId : command.id);
  }
  return transport.request(command);
}

export function assertMobileAutomationDefinition(snapshot: ControlSnapshot, id: string) {
  const record = snapshot.records.find(record => record.id === id);
  if (!record || record.kind !== 'workflow' && record.kind !== 'routine') throw new Error('Automation unavailable. Refresh before running.');
  const workflowId = record.kind === 'routine' ? record.workflowId : null;
  const workflow = record.kind === 'workflow' ? record : workflowId
    ? snapshot.records.find(record => record.id === workflowId && record.kind === 'workflow') : null;
  const teamIds = workflow?.kind === 'workflow' ? workflow.stages.map(stage => stage.teamId)
    : record.kind === 'routine' && !record.workflowId ? [record.teamId] : [];
  if (!teamIds.length) throw new Error('Automation target unavailable. Refresh before running.');
  for (const teamId of new Set(teamIds)) {
    const team = snapshot.records.find(record => record.id === teamId && record.kind === 'team');
    if (!team || team.kind !== 'team') throw new Error('Automation team unavailable. Refresh before running.');
    assertMobileTeam(team, snapshot);
  }
}

export async function runResponse(transport: AutomationTransport, command: Command): Promise<AutomationRun> {
  const response = await requestMobileAutomation(transport, command);
  if (response.type !== 'automationRun') throw new Error('Unexpected automation response. Refresh before trying again.');
  return response.run;
}

export function automationStatus(status: AutomationRun['status']): string {
  return { running: 'Running', waitingDecision: 'Decision required', blocked: 'Needs attention', completed: 'Completed', canceled: 'Stopped', skipped: 'Skipped' }[status];
}
export function routineTime(seconds: number): string {
  if (!seconds) return 'Paused';
  const date = new Date(seconds * 1000);
  return Number.isFinite(date.getTime()) ? date.toISOString().replace('T', ' ').slice(0, 16) + ' UTC' : 'Unknown time';
}

export function scheduleLabel(interval: number, calendar?: CalendarSchedule | null): string {
  if (!calendar) return `Every ${interval} minutes`;
  const weekdays = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
  const recurrence = calendar.recurrence.type === 'daily' ? 'Daily'
    : calendar.recurrence.type === 'weekly' ? calendar.recurrence.days.map((day) => weekdays[day]).filter(Boolean).join(', ')
    : `Monthly on ${calendar.recurrence.days.join(', ')}`;
  return `${recurrence} at ${String(calendar.hour).padStart(2, '0')}:${String(calendar.minute).padStart(2, '0')} · ${calendar.timeZone}`;
}
export function triggerLabel(trigger: RoutineTrigger): string {
  return `${scheduleLabel(trigger.intervalMinutes, trigger.calendar)} · ${trigger.missedRunPolicy === 'skip' ? 'Skip missed times' : 'Run latest missed time'} · ${trigger.overlapPolicy === 'allow' ? 'Allow overlap' : 'Skip overlap'}`;
}

export function workflowTarget(target: WorkflowTarget): string {
  return target.type === 'finish' ? 'Finish workflow' : `Stage ${target.index + 1}`;
}
export function workflowTransition(transition?: WorkflowTransition): string {
  if (!transition || transition.type === 'next') return 'Next stage';
  if (transition.type === 'goTo') return workflowTarget(transition.target);
  return `${transition.document} ${transition.pointer} = ${JSON.stringify(transition.equals)} → ${workflowTarget(transition.onMatch)}; otherwise ${workflowTarget(transition.onMismatch)}`;
}
