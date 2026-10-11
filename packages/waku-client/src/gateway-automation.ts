// Personal gateway contract, distinct from the direct daemon RPC protocol.
export interface AutomationDefinition {
  id: string; projectId: string; name: string; kind: 'workflow' | 'routine';
  revision: number; ready: boolean; paused: boolean | null; nextRun: number | null;
  schedule: string | null; stageCount: number;
}
export interface AutomationRunSummary {
  id: string; projectId: string; name: string; definitionId: string; revision: number;
  status: 'running' | 'waitingDecision' | 'blocked' | 'completed' | 'canceled' | 'skipped';
  currentStage: number; stageCount: number;
  gate: { index: number; title: string; instructions: string; reviewComplete: boolean } | null;
}
export interface AutomationProjection {
  version: 1; revision: number;
  projects: Array<{ id: string; name: string }>;
  definitions: AutomationDefinition[]; runs: AutomationRunSummary[];
  truncated: boolean;
}
export type AutomationAction =
  | { kind: 'start'; definitionId: string; runId: string; expectedRevision: number }
  | { kind: 'stop'; runId: string; expectedRevision: number }
  | { kind: 'gate'; runId: string; expectedRevision: number; stage: number; approved: boolean }
  | { kind: 'routine'; definitionId: string; expectedRevision: number; paused: boolean };
export interface AutomationSubmission {
  publisherId: string; idempotencyKey: string; fingerprint: string; action: AutomationAction;
}
export interface AutomationCommand extends AutomationSubmission {
  id: string; deviceId: string; status: 'queued' | 'uncertain' | 'applied' | 'rejected';
  createdAt: number; updatedAt: number;
}
export interface AutomationOwner extends Omit<AutomationProjection, 'definitions' | 'runs'> {
  publisherId: string; publishedAt: number;
  definitions: Array<AutomationDefinition & { fingerprint: string }>;
  runs: Array<AutomationRunSummary & { fingerprint: string }>;
}
