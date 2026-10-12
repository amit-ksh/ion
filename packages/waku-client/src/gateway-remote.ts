import type {
  RemoteExecutionCursor,
  RemoteExecutionKind,
  RemoteExecutionQuery,
} from "./generated";
export interface GatewayRemoteExecution {
  kind: RemoteExecutionKind;
  requestId: string;
  rootTaskId: string;
  projectId: string;
  teamName: string;
  objective: string;
  state: string;
  operationAvailable: boolean;
  jobAvailable: boolean;
  cancellationAvailable: boolean;
  cleanupState: string | null;
  stopRequested: boolean;
  settled: boolean;
}
export type GatewayRemoteQuery = {
  kind: "remoteExecutions";
  query: RemoteExecutionQuery;
};
export interface GatewayRemoteResult {
  kind: "remoteExecutions";
  page: {
    entries: GatewayRemoteExecution[];
    nextCursor: RemoteExecutionCursor | null;
  };
  pageTruncated: boolean;
  servedAt: number;
}
const kinds: RemoteExecutionKind[] = ["model", "command", "gpu"];
const object = (v: unknown): v is Record<string, any> =>
  !!v && typeof v === "object" && !Array.isArray(v);
const exact = (v: unknown, fields: string): v is Record<string, any> =>
  object(v) &&
  Object.keys(v).sort().join(",") === fields.split(",").sort().join(",");
const uuid = (v: unknown): v is string =>
  typeof v === "string" &&
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(v);
const kind = (v: unknown): v is RemoteExecutionKind =>
  typeof v === "string" && kinds.includes(v as RemoteExecutionKind);
const cursor = (v: unknown): v is RemoteExecutionCursor =>
  exact(v, "kind,requestId") &&
  kind(v.kind) &&
  uuid(v.requestId) &&
  v.requestId !== "00000000-0000-0000-0000-000000000000";
const key = (v: RemoteExecutionCursor) =>
  `${kinds.indexOf(v.kind)}:${v.requestId}`;
const label = (v: unknown, max = 160): v is string =>
  typeof v === "string" && v.length <= max;
export function validateGatewayRemoteQuery(
  v: unknown,
): asserts v is GatewayRemoteQuery {
  if (
    !exact(v, "kind,query") ||
    v.kind !== "remoteExecutions" ||
    !exact(v.query, "projectId,kind,unresolvedOnly,after,limit")
  )
    throw new Error("Invalid remote inventory query");
  const q = v.query;
  if (
    (q.projectId !== null && !uuid(q.projectId)) ||
    (q.kind !== null && !kind(q.kind)) ||
    typeof q.unresolvedOnly !== "boolean" ||
    !Number.isSafeInteger(q.limit) ||
    q.limit < 1 ||
    q.limit > 32 ||
    (q.after !== null &&
      (!cursor(q.after) || (q.kind !== null && q.kind !== q.after.kind)))
  )
    throw new Error("Invalid remote inventory filter or cursor");
}
export function validateGatewayRemoteResult(
  v: unknown,
  request: GatewayRemoteQuery,
): asserts v is GatewayRemoteResult {
  validateGatewayRemoteQuery(request);
  if (
    !exact(v, "kind,page,pageTruncated,servedAt") ||
    v.kind !== request.kind ||
    typeof v.pageTruncated !== "boolean" ||
    !Number.isSafeInteger(v.servedAt) ||
    v.servedAt < 0 ||
    !exact(v.page, "entries,nextCursor") ||
    !Array.isArray(v.page.entries) ||
    v.page.entries.length > request.query.limit ||
    (v.page.nextCursor !== null && !cursor(v.page.nextCursor))
  )
    throw new Error("Invalid remote inventory response");
  let previous = request.query.after ? key(request.query.after) : "";
  for (const row of v.page.entries) {
    if (
      !exact(
        row,
        "kind,requestId,rootTaskId,projectId,teamName,objective,state,operationAvailable,jobAvailable,cancellationAvailable,cleanupState,stopRequested,settled",
      ) ||
      !cursor({ kind: row.kind, requestId: row.requestId }) ||
      !uuid(row.rootTaskId) ||
      !uuid(row.projectId) ||
      !label(row.teamName) ||
      !label(row.objective, 1000) ||
      !label(row.state) ||
      !row.state.trim() ||
      (row.cleanupState !== null &&
        (!label(row.cleanupState) || !row.cleanupState.trim())) ||
      ![
        "operationAvailable",
        "jobAvailable",
        "cancellationAvailable",
        "stopRequested",
        "settled",
      ].every((k) => typeof row[k] === "boolean")
    )
      throw new Error("Invalid redacted remote receipt");
    const current = key({ kind: row.kind, requestId: row.requestId });
    if (
      current <= previous ||
      (request.query.projectId !== null &&
        row.projectId !== request.query.projectId) ||
      (request.query.kind !== null && row.kind !== request.query.kind) ||
      (request.query.unresolvedOnly && row.settled)
    )
      throw new Error("Remote receipt crossed filter or page scope");
    previous = current;
  }
  const next = v.page.nextCursor;
  if (
    (next !== null &&
      (!v.page.entries.length ||
        key(next) !== previous ||
        (!v.pageTruncated && v.page.entries.length !== request.query.limit))) ||
    (v.pageTruncated && next === null)
  )
    throw new Error("Invalid remote inventory continuation");
}
