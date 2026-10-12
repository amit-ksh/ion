import type { AutomationStatus, WorkflowTarget } from "./generated";
export type WorkflowHistoryQuery =
  | {
      kind: "workflowHistory";
      query: {
        projectId: string | null;
        definitionId: string | null;
        pendingOnly: boolean;
        after: string | null;
        limit: number;
      };
    }
  | { kind: "workflowRun"; query: { runId: string } }
  | {
      kind: "workflowEvidence";
      query: { runId: string; after: string | null; limit: number };
    }
  | {
      kind: "workflowDocument";
      query: {
        runId: string;
        artifactId: string;
        sha256: string;
        offset: number;
        limit: number;
      };
    };
export interface HistoricalRun {
  id: string;
  name: string;
  projectId: string;
  definitionId: string;
  revision: number;
  createdAt: number;
  scheduledAt: number | null;
  deadline: number;
  status: AutomationStatus;
  currentStage: number;
  stageCount: number;
}
export interface HistoricalStage {
  index: number;
  title: string;
  teamRunId: string;
  taskId: string;
  requiresDecision: boolean;
  approved: boolean;
  started: boolean;
  completed: boolean;
  skipped: boolean;
  workspaceFrom: string | null;
  route: {
    target: WorkflowTarget;
    artifactId: string | null;
    sha256: string | null;
    matched: boolean | null;
  } | null;
  review: { instructions: string; complete: boolean } | null;
}
export interface WorkflowDocument {
  id: string;
  name: string;
  format: string;
  projectId: string;
  taskId: string | null;
  stage: number;
  teamRunId: string;
  assignmentId: string;
  bytes: number;
  sha256: string;
  createdAt: number;
}
export type WorkflowHistoryResult =
  | {
      kind: "workflowHistory";
      page: {
        runs: HistoricalRun[];
        nextCursor: string | null;
        pendingCount: number;
      };
      pageTruncated: boolean;
      servedAt: number;
    }
  | {
      kind: "workflowRun";
      page: { run: HistoricalRun; stages: HistoricalStage[] };
      pageTruncated: false;
      servedAt: number;
    }
  | {
      kind: "workflowEvidence";
      page: {
        runId: string;
        revision: number;
        documents: WorkflowDocument[];
        nextCursor: string | null;
        total: number;
      };
      pageTruncated: boolean;
      servedAt: number;
    }
  | {
      kind: "workflowDocument";
      page: {
        runId: string;
        document: WorkflowDocument;
        offset: number;
        nextOffset: number | null;
        data: string;
      };
      pageTruncated: false;
      servedAt: number;
    };
const object = (v: unknown): v is Record<string, any> =>
  !!v && typeof v === "object" && !Array.isArray(v);
const shape = (v: unknown, fields: string): v is Record<string, any> =>
  object(v) &&
  Object.keys(v).sort().join(",") === fields.split(",").sort().join(",");
const id = (v: unknown): v is string =>
  typeof v === "string" &&
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(v);
const n = (v: unknown): v is number =>
  typeof v === "number" && Number.isSafeInteger(v) && v >= 0;
const nullable = (v: unknown, check: (value: unknown) => boolean) =>
  v === null || check(v);
const label = (v: unknown, max: number) =>
  typeof v === "string" && v.length <= max;
const sha = (v: unknown) => typeof v === "string" && /^[0-9a-f]{64}$/.test(v);
const bool = (v: unknown) => typeof v === "boolean";
export const isWorkflowHistory = (
  kind: unknown,
): kind is WorkflowHistoryQuery["kind"] =>
  [
    "workflowHistory",
    "workflowRun",
    "workflowEvidence",
    "workflowDocument",
  ].includes(kind as string);
export function validateWorkflowHistoryQuery(
  value: unknown,
): asserts value is WorkflowHistoryQuery {
  if (!shape(value, "kind,query") || !object(value.query))
    throw new Error("Invalid workflow history query");
  const q = value.query;
  if (value.kind === "workflowHistory") {
    if (
      !shape(q, "projectId,definitionId,pendingOnly,after,limit") ||
      !nullable(q.projectId, id) ||
      !nullable(q.definitionId, id) ||
      !nullable(q.after, id) ||
      !bool(q.pendingOnly) ||
      !n(q.limit) ||
      q.limit < 1 ||
      q.limit > 32
    )
      throw new Error("Invalid workflow history filters");
  } else if (value.kind === "workflowRun") {
    if (!shape(q, "runId") || !id(q.runId))
      throw new Error("Invalid workflow run query");
  } else if (value.kind === "workflowEvidence") {
    if (
      !shape(q, "runId,after,limit") ||
      !id(q.runId) ||
      !nullable(q.after, id) ||
      !n(q.limit) ||
      q.limit < 1 ||
      q.limit > 32
    )
      throw new Error("Invalid workflow evidence query");
  } else if (value.kind === "workflowDocument") {
    if (
      !shape(q, "runId,artifactId,sha256,offset,limit") ||
      !id(q.runId) ||
      !id(q.artifactId) ||
      !sha(q.sha256) ||
      !n(q.offset) ||
      q.offset >= 16 * 1024 * 1024 ||
      !n(q.limit) ||
      q.limit < 1 ||
      q.limit > 16_384
    )
      throw new Error("Invalid workflow document query");
  } else throw new Error("Unknown workflow history query");
}
function run(value: unknown): value is HistoricalRun {
  return (
    shape(
      value,
      "id,name,projectId,definitionId,revision,createdAt,scheduledAt,deadline,status,currentStage,stageCount",
    ) &&
    ["id", "projectId", "definitionId"].every((k) => id(value[k])) &&
    label(value.name, 160) &&
    ["revision", "createdAt", "deadline", "currentStage", "stageCount"].every(
      (k) => n(value[k]),
    ) &&
    nullable(value.scheduledAt, n) &&
    value.stageCount >= 1 &&
    value.stageCount <= 24 &&
    value.currentStage <= value.stageCount &&
    [
      "running",
      "waitingDecision",
      "blocked",
      "completed",
      "canceled",
      "skipped",
    ].includes(value.status)
  );
}
function document(value: unknown): value is WorkflowDocument {
  return (
    shape(
      value,
      "id,name,format,projectId,taskId,stage,teamRunId,assignmentId,bytes,sha256,createdAt",
    ) &&
    ["id", "projectId", "teamRunId", "assignmentId"].every((k) =>
      id(value[k]),
    ) &&
    nullable(value.taskId, id) &&
    label(value.name, 160) &&
    label(value.format, 64) &&
    n(value.stage) &&
    value.stage < 24 &&
    n(value.bytes) &&
    value.bytes > 0 &&
    value.bytes <= 16 * 1024 * 1024 &&
    sha(value.sha256) &&
    n(value.createdAt)
  );
}
export function validateWorkflowDocument(
  value: unknown,
): asserts value is WorkflowDocument {
  if (!document(value)) throw new Error("Invalid accepted workflow document.");
}
function target(value: unknown): boolean {
  return (
    (shape(value, "type") && value.type === "finish") ||
    (shape(value, "type,index") &&
      value.type === "stage" &&
      n(value.index) &&
      value.index < 24)
  );
}
/** Decode canonical base64 without Node, atob or native Buffer dependencies. */
export function workflowChunkBytes(
  value: string,
  maxBytes = 16_384,
): Uint8Array {
  if (
    !Number.isSafeInteger(maxBytes) ||
    maxBytes < 1 ||
    maxBytes > 32_768 ||
    value.length > Math.ceil(maxBytes / 3) * 4 ||
    value.length % 4 !== 0 ||
    !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(
      value,
    )
  )
    throw new Error("Invalid document chunk encoding");
  const alphabet =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  const padding = value.endsWith("==") ? 2 : value.endsWith("=") ? 1 : 0;
  const count = (value.length / 4) * 3 - padding;
  if (count > maxBytes)
    throw new Error("Document chunk exceeds its byte bound.");
  const bytes = new Uint8Array(count);
  let out = 0;
  for (let i = 0; i < value.length; i += 4) {
    const a = alphabet.indexOf(value[i]!),
      b = alphabet.indexOf(value[i + 1]!),
      c = value[i + 2] === "=" ? 0 : alphabet.indexOf(value[i + 2]!),
      d = value[i + 3] === "=" ? 0 : alphabet.indexOf(value[i + 3]!);
    if (
      i + 4 === value.length &&
      ((padding === 2 && (b & 15) !== 0) || (padding === 1 && (c & 3) !== 0))
    )
      throw new Error("Noncanonical document chunk");
    const word = (a << 18) | (b << 12) | (c << 6) | d;
    if (out < bytes.length) bytes[out++] = word >> 16;
    if (out < bytes.length) bytes[out++] = word >> 8;
    if (out < bytes.length) bytes[out++] = word;
  }
  return bytes;
}
export function validateWorkflowHistoryResult(
  value: unknown,
  query: WorkflowHistoryQuery,
): asserts value is WorkflowHistoryResult {
  validateWorkflowHistoryQuery(query);
  if (
    !shape(value, "kind,page,pageTruncated,servedAt") ||
    value.kind !== query.kind ||
    !object(value.page) ||
    !bool(value.pageTruncated) ||
    !n(value.servedAt)
  )
    throw new Error("Invalid workflow history result");
  const p = value.page;
  if (query.kind === "workflowHistory") {
    if (
      !shape(p, "runs,nextCursor,pendingCount") ||
      !Array.isArray(p.runs) ||
      p.runs.length > query.query.limit ||
      !nullable(p.nextCursor, id) ||
      !n(p.pendingCount)
    )
      throw new Error("Invalid workflow history page");
    let previous: HistoricalRun | null = null;
    const seen = new Set<string>();
    for (const row of p.runs) {
      if (
        !run(row) ||
        (previous !== null &&
          (row.createdAt > previous.createdAt ||
            (row.createdAt === previous.createdAt && row.id >= previous.id))) ||
        seen.has(row.id) ||
        row.id === query.query.after ||
        (query.query.projectId !== null &&
          row.projectId !== query.query.projectId) ||
        (query.query.definitionId !== null &&
          row.definitionId !== query.query.definitionId) ||
        (query.query.pendingOnly &&
          !["waitingDecision", "blocked"].includes(row.status))
      )
        throw new Error("Workflow run crossed history scope");
      previous = row;
      seen.add(row.id);
    }
    if (
      p.nextCursor !== null &&
      (!p.runs.length || p.nextCursor !== previous?.id)
    )
      throw new Error("Invalid workflow history cursor");
  } else if (query.kind === "workflowRun") {
    if (
      !shape(p, "run,stages") ||
      !run(p.run) ||
      p.run.id !== query.query.runId ||
      !Array.isArray(p.stages) ||
      p.stages.length !== p.run.stageCount ||
      value.pageTruncated
    )
      throw new Error("Invalid workflow run detail");
    const teams = new Set();
    p.stages.forEach((s: any, index: number) => {
      if (
        !shape(
          s,
          "index,title,teamRunId,taskId,requiresDecision,approved,started,completed,skipped,workspaceFrom,route,review",
        ) ||
        s.index !== index ||
        !label(s.title, 160) ||
        !id(s.teamRunId) ||
        teams.has(s.teamRunId) ||
        !id(s.taskId) ||
        ![
          "requiresDecision",
          "approved",
          "started",
          "completed",
          "skipped",
        ].every((k) => bool(s[k])) ||
        !nullable(s.workspaceFrom, id) ||
        !nullable(
          s.route,
          (r) =>
            shape(r, "target,artifactId,sha256,matched") &&
            target(r.target) &&
            nullable(r.artifactId, id) &&
            nullable(r.sha256, sha) &&
            nullable(r.matched, bool),
        ) ||
        !nullable(
          s.review,
          (r) =>
            shape(r, "instructions,complete") &&
            label(r.instructions, 2000) &&
            bool(r.complete),
        )
      )
        throw new Error("Invalid workflow stage detail");
      if (
        s.review !== null &&
        (p.run.status !== "waitingDecision" || p.run.currentStage !== index)
      )
        throw new Error("Review belongs to another stage");
      teams.add(s.teamRunId);
    });
  } else if (query.kind === "workflowEvidence") {
    if (
      !shape(p, "runId,revision,documents,nextCursor,total") ||
      p.runId !== query.query.runId ||
      !n(p.revision) ||
      !Array.isArray(p.documents) ||
      p.documents.length > query.query.limit ||
      !nullable(p.nextCursor, id) ||
      !n(p.total) ||
      p.total < p.documents.length
    )
      throw new Error("Invalid workflow evidence page");
    let previous = query.query.after;
    for (const row of p.documents) {
      if (!document(row) || (previous !== null && row.id <= previous))
        throw new Error("Invalid workflow document order");
      previous = row.id;
    }
    if (
      p.nextCursor !== null &&
      (!p.documents.length || p.nextCursor !== previous)
    )
      throw new Error("Invalid workflow evidence cursor");
  } else {
    if (
      !shape(p, "runId,document,offset,nextOffset,data") ||
      p.runId !== query.query.runId ||
      !document(p.document) ||
      p.document.id !== query.query.artifactId ||
      p.document.sha256 !== query.query.sha256 ||
      p.offset !== query.query.offset ||
      !nullable(p.nextOffset, n) ||
      typeof p.data !== "string" ||
      value.pageTruncated
    )
      throw new Error("Document response crossed request scope");
    const size = workflowChunkBytes(p.data).length,
      end = p.offset + size;
    if (
      size < 1 ||
      size > query.query.limit ||
      end > p.document.bytes ||
      p.nextOffset !== (end < p.document.bytes ? end : null)
    )
      throw new Error("Invalid document chunk cursor");
  }
}

export const isWorkflowHistoryQuery = (query: {
  kind: string;
}): query is WorkflowHistoryQuery => isWorkflowHistory(query.kind);
