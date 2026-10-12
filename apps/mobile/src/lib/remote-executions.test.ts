import { expect, test } from "bun:test";
import type {
  Command,
  RemoteExecution,
  RemoteExecutionQuery,
  RemoteExecutionCursor,
  WakuClient,
} from "@waku/client";
import { readRemoteExecutions } from "./remote-executions";
const id = "00000000-0000-0000-0000-000000000001";
const entry: RemoteExecution = {
  kind: "command",
  requestId: id,
  runId: id,
  rootTaskId: id,
  projectId: id,
  assignmentId: id,
  taskId: id,
  attemptId: id,
  agentId: id,
  teamName: "Team",
  objective: "Test",
  state: "submitting",
  remoteProject: "project-a",
  operationId: null,
  jobId: null,
  cancellationOperationId: null,
  cleanupState: null,
  stopRequested: false,
  settled: false,
};
const query: RemoteExecutionQuery = {
  projectId: null,
  kind: null,
  unresolvedOnly: true,
  after: null,
  limit: 32,
};
function transport(
  entries: RemoteExecution[],
  nextCursor: RemoteExecutionCursor | null = null,
) {
  const commands: Command[] = [];
  const client: Pick<WakuClient, "request"> = {
    request: async (command) => {
      commands.push(command);
      return { type: "remoteExecutions", page: { entries, nextCursor } };
    },
  };
  return { client, commands };
}
test("remote reads preserve original run and missing operation without inventing completion", async () => {
  const f = transport([entry]);
  const page = await readRemoteExecutions(f.client, query);
  expect(page.entries[0].operationId).toBeNull();
  expect(page.entries[0].settled).toBe(false);
  expect(f.commands).toEqual([{ type: "listRemoteExecutions", query }]);
});
test("invalid route and cursor kind fail before transport", async () => {
  const f = transport([]);
  for (const invalid of [
    { ...query, limit: 0 },
    { ...query, limit: 33 },
    { ...query, projectId: "not-a-uuid" },
    {
      ...query,
      kind: "gpu" as const,
      after: { kind: "command" as const, requestId: id },
    },
  ])
    await expect(readRemoteExecutions(f.client, invalid)).rejects.toThrow();
  expect(f.commands).toEqual([]);
});
test("remote response fences project kind and unresolved filter", async () => {
  for (const [requested, row] of [
    [{ ...query, projectId: "00000000-0000-0000-0000-000000000002" }, entry],
    [{ ...query, kind: "gpu" as const }, entry],
    [query, { ...entry, settled: true }],
  ] as [RemoteExecutionQuery, RemoteExecution][])
    await expect(
      readRemoteExecutions(transport([row]).client, requested),
    ).rejects.toThrow();
});
test("remote paging rejects duplicate unordered and oversized records", async () => {
  for (const entries of [
    [entry, entry],
    [{ ...entry, requestId: "00000000-0000-0000-0000-000000000002" }, entry],
    Array(33).fill(entry),
  ])
    await expect(
      readRemoteExecutions(transport(entries).client, query),
    ).rejects.toThrow();
});
test("next cursor must match final record on a full page", async () => {
  const cursor = { kind: "command" as const, requestId: id };
  await expect(
    readRemoteExecutions(transport([entry], cursor).client, query),
  ).rejects.toThrow();
  expect(
    (
      await readRemoteExecutions(transport([entry], cursor).client, {
        ...query,
        limit: 1,
      })
    ).nextCursor,
  ).toEqual(cursor);
  await expect(
    readRemoteExecutions(
      transport([entry], {
        ...cursor,
        requestId: "00000000-0000-0000-0000-000000000002",
      }).client,
      { ...query, limit: 1 },
    ),
  ).rejects.toThrow();
});
