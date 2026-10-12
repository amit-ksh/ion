import { expect, test } from "bun:test";
import {
  portfolioReadGrant,
  validatePortfolioQuery,
  validatePortfolioResult,
  type GatewayRemoteExecution,
  type GatewayRemoteQuery,
  type GatewayRemoteResult,
} from "./gateway-portfolio";
const uuid = (n: number) =>
  `00000000-0000-0000-0000-${String(n).padStart(12, "0")}`;
const query = (
  patch: Partial<GatewayRemoteQuery["query"]> = {},
): GatewayRemoteQuery => ({
  kind: "remoteExecutions",
  query: {
    projectId: null,
    kind: null,
    unresolvedOnly: true,
    after: null,
    limit: 32,
    ...patch,
  },
});
const row = (
  patch: Partial<GatewayRemoteExecution> = {},
): GatewayRemoteExecution => ({
  kind: "model",
  requestId: uuid(1),
  rootTaskId: uuid(2),
  projectId: uuid(3),
  teamName: "Engineering",
  objective: "Check a change",
  state: "submitting",
  operationAvailable: false,
  jobAvailable: false,
  cancellationAvailable: false,
  cleanupState: null,
  stopRequested: true,
  settled: false,
  ...patch,
});
const result = (
  entries: GatewayRemoteExecution[] = [row()],
): GatewayRemoteResult => ({
  kind: "remoteExecutions",
  page: { entries, nextCursor: null },
  pageTruncated: false,
  servedAt: 1,
});
test("remote inventory is a typed bounded Usage read with no control/provider fields", () => {
  validatePortfolioQuery(query());
  expect(portfolioReadGrant(query())).toBe("usageRead");
  for (const bad of [
    { ...query(), stop: true },
    query({ unresolvedOnly: "true" as any }),
    query({ limit: 33 }),
    query({ kind: "gpu", after: { kind: "model", requestId: uuid(1) } }),
    query({ after: { kind: "gpu", requestId: uuid(0) } }),
    { ...query(), query: { ...query().query, operationId: "provider" } },
  ])
    expect(() => validatePortfolioQuery(bad)).toThrow();
});
test("model command and GPU receipt states preserve unknown handles stop and cleanup distinctions", () => {
  const page = result([
    row(),
    row({
      kind: "command",
      requestId: uuid(4),
      state: "canceling",
      operationAvailable: true,
    }),
    row({
      kind: "gpu",
      requestId: uuid(5),
      state: "canceling",
      jobAvailable: true,
      cancellationAvailable: true,
      cleanupState: "pending",
    }),
  ]);
  validatePortfolioResult(page, query());
  expect(page.page.entries[0]!.settled).toBe(false);
  expect(page.page.entries[2]!.cleanupState).toBe("pending");
  const settled = result([
    row({
      kind: "gpu",
      cleanupState: "verified",
      settled: true,
      state: "cleaned",
    }),
  ]);
  validatePortfolioResult(settled, query({ unresolvedOnly: false }));
  expect(() => validatePortfolioResult(settled, query())).toThrow();
});
test("remote provider projects handles inputs and internal assignment identities never enter a response", () => {
  for (const field of [
    "remoteProject",
    "operationId",
    "jobId",
    "cancellationOperationId",
    "runId",
    "assignmentId",
    "attemptId",
    "agentId",
    "request",
    "command",
    "environment",
    "token",
  ]) {
    const page = result();
    Object.assign(page.page.entries[0]!, { [field]: "private" });
    expect(() => validatePortfolioResult(page, query())).toThrow();
  }
});
test("remote pages reject cross-project types duplicate or reordered identities and forged continuations", () => {
  expect(() =>
    validatePortfolioResult(result(), query({ projectId: uuid(9) })),
  ).toThrow();
  expect(() =>
    validatePortfolioResult(result(), query({ kind: "gpu" })),
  ).toThrow();
  expect(() =>
    validatePortfolioResult(result([row(), row()]), query()),
  ).toThrow();
  expect(() =>
    validatePortfolioResult(
      result([row({ kind: "gpu" }), row({ kind: "command" })]),
      query(),
    ),
  ).toThrow();
  const page = result();
  page.page.nextCursor = { kind: "model", requestId: uuid(9) };
  expect(() => validatePortfolioResult(page, query({ limit: 1 }))).toThrow();
  page.page.nextCursor = { kind: "model", requestId: uuid(1) };
  validatePortfolioResult(page, query({ limit: 1 }));
  expect(() =>
    validatePortfolioResult(
      page,
      query({ after: page.page.nextCursor, limit: 1 }),
    ),
  ).toThrow();
});
test("truncated pages preserve whole rows and only the last returned immutable cursor", () => {
  const page = result();
  page.pageTruncated = true;
  page.page.nextCursor = { kind: "model", requestId: uuid(1) };
  validatePortfolioResult(page, query());
  page.page.nextCursor = null;
  expect(() => validatePortfolioResult(page, query())).toThrow();
  page.page.entries = [];
  page.page.nextCursor = { kind: "model", requestId: uuid(1) };
  expect(() => validatePortfolioResult(page, query())).toThrow();
});
test("invalid receipt booleans oversized labels and additional cursor fields are rejected", () => {
  for (const patch of [
    { settled: "false" },
    { operationAvailable: 1 },
    { state: "" },
    { objective: "x".repeat(1001) },
  ])
    expect(() =>
      validatePortfolioResult(result([row(patch as any)]), query()),
    ).toThrow();
  expect(() =>
    validatePortfolioQuery(
      query({
        after: { kind: "model", requestId: uuid(1), project: "private" } as any,
      }),
    ),
  ).toThrow();
});
