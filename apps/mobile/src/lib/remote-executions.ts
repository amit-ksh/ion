import type {
  WakuClient,
  RemoteExecutionQuery,
  RemoteExecutionPage,
  RemoteExecutionCursor,
  RemoteExecutionKind,
} from "@waku/client";
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const kinds: RemoteExecutionKind[] = ["model", "command", "gpu"];
const cursorValid = (cursor: RemoteExecutionCursor) =>
  kinds.includes(cursor.kind) && UUID.test(cursor.requestId);
const key = (cursor: RemoteExecutionCursor) =>
  `${kinds.indexOf(cursor.kind)}:${cursor.requestId}`;
export async function readRemoteExecutions(
  client: Pick<WakuClient, "request">,
  query: RemoteExecutionQuery,
): Promise<RemoteExecutionPage> {
  if (
    !Number.isInteger(query.limit) ||
    query.limit < 1 ||
    query.limit > 32 ||
    (query.projectId !== null && !UUID.test(query.projectId)) ||
    (query.kind !== null && !kinds.includes(query.kind)) ||
    (query.after !== null &&
      (!cursorValid(query.after) ||
        (query.kind !== null && query.after.kind !== query.kind)))
  )
    throw new Error("Invalid remote execution page");
  const reply = await client.request({ type: "listRemoteExecutions", query });
  if (
    reply.type !== "remoteExecutions" ||
    reply.page.entries.length > query.limit
  )
    throw new Error("Invalid remote execution response");
  let previous = query.after ? key(query.after) : "";
  for (const row of reply.page.entries) {
    const current = key({ kind: row.kind, requestId: row.requestId });
    if (
      !cursorValid({ kind: row.kind, requestId: row.requestId }) ||
      current <= previous ||
      [
        row.runId,
        row.rootTaskId,
        row.projectId,
        row.assignmentId,
        row.taskId,
        row.attemptId,
        row.agentId,
      ].some((id) => !UUID.test(id)) ||
      (query.projectId !== null && row.projectId !== query.projectId) ||
      (query.kind !== null && row.kind !== query.kind) ||
      (query.unresolvedOnly && row.settled)
    )
      throw new Error("Remote execution response belongs to another page");
    previous = current;
  }
  const next = reply.page.nextCursor;
  if (
    next !== null &&
    (!cursorValid(next) ||
      reply.page.entries.length !== query.limit ||
      key(next) !== previous)
  )
    throw new Error("Invalid remote execution cursor");
  return reply.page;
}
