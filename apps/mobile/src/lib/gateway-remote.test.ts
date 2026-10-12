import { afterEach, expect, test } from "bun:test";
import { randomUUID } from "node:crypto";
import type {
  GatewayRemoteExecution,
  PortfolioJob,
  PortfolioSubmission,
} from "@waku/client";
import {
  gatewayPairingIdentity,
  remoteTaskDestination,
} from "./gateway-remote";
import {
  gatewayOwnerTask,
  enqueueGatewayTaskCommand,
  type GatewayPairing,
} from "./gateway-client";
import { loadGatewayPortfolio } from "./gateway-portfolio";
const fetchOriginal = globalThis.fetch;
afterEach(() => {
  globalThis.fetch = fetchOriginal;
});
const owner = randomUUID(),
  taskId = randomUUID(),
  project = randomUUID(),
  runtime = randomUUID();
const pairing: GatewayPairing = {
  baseUrl: "https://gateway.example",
  id: randomUUID(),
  token: "private",
  expiresAt: Date.now() + 120000,
  grants: ["usageRead", "taskRead", "taskControl"],
};
const row: GatewayRemoteExecution = {
  kind: "gpu",
  requestId: randomUUID(),
  rootTaskId: taskId,
  projectId: project,
  teamName: "Engineering",
  objective: "Train",
  state: "canceling",
  operationAvailable: false,
  jobAvailable: true,
  cancellationAvailable: true,
  cleanupState: "pending",
  stopRequested: true,
  settled: false,
};
const task = {
  id: taskId,
  projectId: project,
  projectName: "Ion",
  title: "Team task",
  provider: "nebius",
  status: "working",
  runtimeId: runtime,
  updatedAt: 1,
  publisherSeenAt: Date.now(),
};
test("original-task navigation requires distinct taskRead and pins owner and project", () => {
  expect(remoteTaskDestination(row, owner, pairing)).toEqual({
    pathname: "/gateway-task/[id]",
    params: { id: taskId, owner, project },
  });
  expect(() =>
    remoteTaskDestination(row, owner, { ...pairing, grants: ["usageRead"] }),
  ).toThrow();
  expect(() =>
    remoteTaskDestination(row, owner, { ...pairing, expiresAt: 1 }),
  ).toThrow();
});
test("replacement token grant expiry or gateway URL changes invalidate the captured pairing scope", () => {
  const original = gatewayPairingIdentity(pairing);
  for (const patch of [
    { token: "changed" },
    { baseUrl: "https://other.example" },
    { grants: ["usageRead"] },
    { expiresAt: 1 },
  ])
    expect(gatewayPairingIdentity({ ...pairing, ...patch })).not.toBe(original);
  expect(
    gatewayPairingIdentity({
      ...pairing,
      grants: [...pairing.grants].reverse(),
    }),
  ).toBe(original);
});
test("original-task lookup is owner-scoped and refuses extra fields or cross-owner replies", async () => {
  let calls = 0;
  globalThis.fetch = (async (
    input: Parameters<typeof fetch>[0],
    init?: Parameters<typeof fetch>[1],
  ) => {
    calls++;
    expect(String(input)).toBe(
      `${pairing.baseUrl}/v1/device/tasks/${taskId}?publisherId=${owner}`,
    );
    expect(init?.redirect).toBe("error");
    expect(init?.cache).toBe("no-store");
    return Response.json({ publisherId: owner, task });
  }) as unknown as typeof fetch;
  expect(await gatewayOwnerTask(pairing, owner, taskId)).toEqual(task);
  expect(calls).toBe(1);
  for (const value of [
    { publisherId: randomUUID(), task },
    { publisherId: owner, task: { ...task, operationId: "private" } },
    { publisherId: owner, task: { ...task, id: randomUUID() } },
    { publisherId: owner, task: { ...task, runtimeId: "invalid" } },
  ]) {
    globalThis.fetch = (async () =>
      Response.json(value)) as unknown as typeof fetch;
    await expect(gatewayOwnerTask(pairing, owner, taskId)).rejects.toThrow();
  }
  calls = 0;
  globalThis.fetch = (async () => {
    calls++;
    return Response.json({});
  }) as unknown as typeof fetch;
  await expect(
    gatewayOwnerTask({ ...pairing, grants: ["usageRead"] }, owner, taskId),
  ).rejects.toThrow();
  expect(calls).toBe(0);
});
test("scoped task controls retain original runtime nonce and reject a foreign owner receipt", async () => {
  const input = {
    idempotencyKey: randomUUID(),
    taskId,
    runtimeId: runtime,
    kind: "stop" as const,
    payload: {},
  };
  const receipt = {
    id: randomUUID(),
    deviceId: pairing.id,
    publisherId: owner,
    taskId,
    runtimeId: runtime,
    kind: "stop",
    status: "queued",
    createdAt: 1,
    updatedAt: 1,
  };
  globalThis.fetch = (async (
    url: Parameters<typeof fetch>[0],
    init?: Parameters<typeof fetch>[1],
  ) => {
    expect(String(url)).toBe(
      `${pairing.baseUrl}/v1/device/commands?publisherId=${owner}`,
    );
    expect(JSON.parse(String(init?.body))).toEqual(input);
    return Response.json(receipt);
  }) as unknown as typeof fetch;
  expect((await enqueueGatewayTaskCommand(pairing, input, owner)).status).toBe(
    "queued",
  );
  for (const patch of [
    { publisherId: randomUUID() },
    { runtimeId: randomUUID() },
    { deviceId: randomUUID() },
    { taskId: randomUUID() },
  ]) {
    globalThis.fetch = (async () =>
      Response.json({ ...receipt, ...patch })) as unknown as typeof fetch;
    await expect(
      enqueueGatewayTaskCommand(pairing, input, owner),
    ).rejects.toThrow("scope");
  }
});
test("late remote inventory receipts cannot populate a backgrounded or changed owner view", async () => {
  const query = {
    kind: "remoteExecutions" as const,
    query: {
      projectId: null,
      kind: null,
      unresolvedOnly: true,
      after: null,
      limit: 32,
    },
  };
  const input: PortfolioSubmission = {
    publisherId: owner,
    idempotencyKey: randomUUID(),
    query,
  };
  const job: PortfolioJob = {
    ...input,
    id: randomUUID(),
    deviceId: pairing.id,
    status: "complete",
    result: {
      kind: "remoteExecutions",
      page: { entries: [row], nextCursor: null },
      pageTruncated: false,
      servedAt: 1,
    },
    createdAt: 1,
    updatedAt: 1,
    expiresAt: Date.now() + 120000,
  };
  let active = true,
    calls = 0;
  const api = {
    submit: async () => {
      calls++;
      active = false;
      return job;
    },
    read: async () => {
      calls++;
      return job;
    },
    wait: async () => {},
  };
  await expect(
    loadGatewayPortfolio(
      pairing,
      owner,
      query,
      input.idempotencyKey,
      () => active,
      api,
    ),
  ).rejects.toThrow("scope changed");
  expect(calls).toBe(1);
  active = true;
  job.publisherId = randomUUID();
  api.submit = async () => job;
  await expect(
    loadGatewayPortfolio(
      pairing,
      owner,
      query,
      input.idempotencyKey,
      () => active,
      api,
    ),
  ).rejects.toThrow("request scope");
});
