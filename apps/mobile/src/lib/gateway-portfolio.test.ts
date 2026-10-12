import { afterEach, expect, test } from "bun:test";
import { randomUUID } from "node:crypto";
import {
  portfolioUtf8Bytes,
  validatePortfolioQuery,
  validatePortfolioResult,
  type PortfolioJob,
  type PortfolioQuery,
  type PortfolioResult,
  type PortfolioSubmission,
} from "@waku/client";
import { loadGatewayPortfolio } from "./gateway-portfolio";
import {
  gatewayPortfolioOwners,
  readGatewayPortfolio,
  submitGatewayPortfolio,
  type GatewayPairing,
} from "./gateway-client";
const pairing: GatewayPairing = {
  baseUrl: "https://gateway.example",
  id: randomUUID(),
  token: "private-history-token",
  grants: ["inboxRead", "usageRead"],
  expiresAt: Date.now() + 120_000,
};
const publisherId = randomUUID(),
  nonce = randomUUID();
const query: PortfolioQuery = {
  kind: "projects",
  query: { after: null, limit: 32 },
};
const result: PortfolioResult = {
  kind: "projects",
  page: { projects: [], nextCursor: null, total: 0 },
  servedAt: Date.now(),
  pageTruncated: false,
};
function fixture() {
  const input: PortfolioSubmission = {
    publisherId,
    query,
    idempotencyKey: nonce,
  };
  const job: PortfolioJob = {
    ...input,
    id: randomUUID(),
    deviceId: pairing.id,
    status: "queued",
    result: null,
    createdAt: Date.now(),
    updatedAt: Date.now(),
    expiresAt: Date.now() + 120_000,
  };
  const calls: string[] = [];
  let current = true;
  const api = {
    submit: async (_: GatewayPairing, value: PortfolioSubmission) => {
      calls.push("submit");
      expect(value).toEqual(input);
      return job;
    },
    read: async (
      _: GatewayPairing,
      value: PortfolioSubmission,
    ): Promise<PortfolioJob | null> => {
      calls.push("read");
      expect(value).toEqual(input);
      return { ...job, status: "complete", result };
    },
    wait: async () => {
      calls.push("wait");
    },
  };
  const load = () =>
    loadGatewayPortfolio(
      pairing,
      publisherId,
      query,
      nonce,
      () => current,
      api,
    );
  return {
    api,
    calls,
    job,
    input,
    load,
    invalidate: () => {
      current = false;
    },
  };
}
test("queued owner reads poll the same nonce and return an exact complete page", async () => {
  const f = fixture();
  expect(await f.load()).toEqual(result);
  expect(f.calls).toEqual(["submit", "wait", "read"]);
});
test("a lost submit response reads the original receipt without submitting a second request", async () => {
  const f = fixture();
  f.api.submit = async () => {
    f.calls.push("submit");
    throw new Error("Network lost");
  };
  expect(await f.load()).toEqual(result);
  expect(f.calls).toEqual(["submit", "read"]);
});
test("confirmed absence after a dropped response preserves the original error", async () => {
  const f = fixture();
  f.api.submit = async () => {
    f.calls.push("submit");
    throw new Error("Network lost");
  };
  f.api.read = async () => {
    f.calls.push("read");
    return null;
  };
  await expect(f.load()).rejects.toThrow("Network lost");
  expect(f.calls).toEqual(["submit", "read"]);
});
test("focus, pairing or filter changes fence the first send and every subsequent poll", async () => {
  const f = fixture();
  f.invalidate();
  await expect(f.load()).rejects.toThrow("scope changed");
  expect(f.calls).toEqual([]);
  const waiting = fixture();
  waiting.api.wait = async () => {
    waiting.calls.push("wait");
    waiting.invalidate();
  };
  await expect(waiting.load()).rejects.toThrow("scope changed");
  expect(waiting.calls).toEqual(["submit", "wait"]);
  const dropped = fixture();
  dropped.api.submit = async () => {
    dropped.invalidate();
    throw new Error("Disconnected");
  };
  await expect(dropped.load()).rejects.toThrow("scope changed");
  expect(dropped.calls).toEqual([]);
});
test("responses from another owner, phone, nonce, query or result kind cannot populate a view", async () => {
  for (const change of [
    { publisherId: randomUUID() },
    { deviceId: randomUUID() },
    { idempotencyKey: randomUUID() },
    { query: { kind: "projects", query: { after: null, limit: 1 } } },
    { status: "complete", result: { ...result, kind: "usage" } },
  ]) {
    const f = fixture();
    f.api.submit = async () => ({ ...f.job, ...change }) as PortfolioJob;
    await expect(f.load()).rejects.toThrow();
    expect(f.calls).toEqual([]);
  }
});
test("rejected and expired reads stop polling and a pending queue has a bounded wait", async () => {
  for (const status of ["rejected", "expired"] as const) {
    const f = fixture();
    f.api.submit = async () => ({ ...f.job, status });
    await expect(f.load()).rejects.toThrow("expired or was rejected");
    expect(f.calls).toEqual([]);
  }
  const f = fixture();
  f.api.read = async () => {
    f.calls.push("read");
    return f.job;
  };
  await expect(f.load()).rejects.toThrow("taking longer");
  expect(f.calls.filter((v) => v === "read")).toHaveLength(60);
  expect(f.calls.filter((v) => v === "submit")).toHaveLength(1);
});
test("expired, task-only and wrong history grants cannot issue owner queries", async () => {
  const inbox: PortfolioQuery = {
    kind: "inbox",
    query: {
      projectId: null,
      taskId: null,
      kind: null,
      view: "all",
      status: "all",
      after: null,
      limit: 32,
    },
  };
  for (const credential of [
    { ...pairing, grants: ["taskRead", "automationRead"] },
    { ...pairing, grants: ["usageRead"] },
    { ...pairing, expiresAt: Date.now() - 1 },
  ]) {
    const f = fixture();
    await expect(
      loadGatewayPortfolio(
        credential,
        publisherId,
        inbox,
        nonce,
        () => true,
        f.api,
      ),
    ).rejects.toThrow();
    expect(f.calls).toEqual([]);
  }
  const f = fixture();
  await expect(
    loadGatewayPortfolio(
      pairing,
      publisherId,
      { kind: "projects", query: { after: null, limit: 33 } },
      nonce,
      () => true,
      f.api,
    ),
  ).rejects.toThrow();
  expect(f.calls).toEqual([]);
});
test("native response validation and Unicode sizing work without TextEncoder", () => {
  const encoder = globalThis.TextEncoder;
  try {
    globalThis.TextEncoder = undefined as unknown as typeof TextEncoder;
    expect(portfolioUtf8Bytes("aé文😀")).toBe(10);
    const exactModel: PortfolioQuery = {
      kind: "usage",
      query: {
        projectId: null,
        taskId: null,
        agentId: null,
        provider: null,
        environment: null,
        kind: null,
        model: "模型😀",
        since: null,
        after: null,
        limit: 32,
      },
    };
    expect(() => validatePortfolioQuery(exactModel)).not.toThrow();
    expect(() =>
      validatePortfolioQuery({
        ...exactModel,
        query: { ...exactModel.query, model: "😀".repeat(129) },
      }),
    ).toThrow();
    expect(() => validatePortfolioResult(result, query)).not.toThrow();
  } finally {
    globalThis.TextEncoder = encoder;
  }
});
const originalFetch = globalThis.fetch;
afterEach(() => {
  globalThis.fetch = originalFetch;
});
test("phone portfolio routes use only the device grant, no-store and redirect refusal", async () => {
  const f = fixture(),
    calls: { url: string; init: RequestInit }[] = [];
  globalThis.fetch = (async (
    url: Parameters<typeof fetch>[0],
    init: RequestInit = {},
  ) => {
    calls.push({ url: String(url), init });
    if (String(url).endsWith("/portfolio"))
      return Response.json({
        owners: [{ publisherId, name: "Owner", publishedAt: Date.now() }],
      });
    return Response.json(
      String(url).includes("?key=")
        ? { job: { ...f.job, status: "complete", result } }
        : f.job,
    );
  }) as unknown as typeof fetch;
  expect(await gatewayPortfolioOwners(pairing)).toHaveLength(1);
  expect((await submitGatewayPortfolio(pairing, f.input)).id).toBe(f.job.id);
  expect((await readGatewayPortfolio(pairing, f.input))?.result).toEqual(
    result,
  );
  expect(calls.map((c) => c.url)).toEqual([
    `${pairing.baseUrl}/v1/device/portfolio`,
    `${pairing.baseUrl}/v1/device/portfolio/queries`,
    `${pairing.baseUrl}/v1/device/portfolio/queries?key=${nonce}`,
  ]);
  expect(
    calls.every(
      (c) => c.init.redirect === "error" && c.init.cache === "no-store",
    ),
  ).toBe(true);
  expect(JSON.stringify(calls[1]?.init.headers)).toContain(pairing.token);
  expect(calls[1]?.init.body).toBe(JSON.stringify(f.input));
});
test("invalid owner directory and cross-scope HTTP replies fail before returning data", async () => {
  const f = fixture();
  const owner = { publisherId, name: "Owner", publishedAt: Date.now() };
  for (const owners of [
    [owner, owner],
    [{ ...owner, publisherId: "not-a-uuid" }],
    [{ ...owner, publishedAt: -1 }],
  ]) {
    globalThis.fetch = (async () =>
      Response.json({ owners })) as unknown as typeof fetch;
    await expect(gatewayPortfolioOwners(pairing)).rejects.toThrow(
      "owner directory",
    );
  }
  globalThis.fetch = (async () =>
    Response.json({
      ...f.job,
      publisherId: randomUUID(),
    })) as unknown as typeof fetch;
  await expect(submitGatewayPortfolio(pairing, f.input)).rejects.toThrow(
    "scope",
  );
  globalThis.fetch = (async () =>
    Response.json({
      job: { ...f.job, idempotencyKey: randomUUID() },
    })) as unknown as typeof fetch;
  await expect(readGatewayPortfolio(pairing, f.input)).rejects.toThrow("scope");
});

test("usage time filters match the owner start-time rule and unknown costs remain unknown", () => {
  const usage: PortfolioQuery = {
    kind: "usage",
    query: {
      projectId: null,
      taskId: null,
      agentId: null,
      provider: null,
      environment: null,
      kind: null,
      model: null,
      since: 2_000,
      after: null,
      limit: 32,
    },
  };
  const row = {
    sequence: 1,
    kind: "modelCall",
    sourceId: randomUUID(),
    projectId: randomUUID(),
    taskId: randomUUID(),
    runId: null,
    assignmentId: null,
    attemptId: null,
    agentId: null,
    agentName: null,
    provider: "nebius",
    model: null,
    environment: "local",
    observedAt: 2_100,
    startedAt: 1_900,
    updatedAt: 2_100,
    status: "completed",
    inputTokens: null,
    outputTokens: null,
    reservedTokens: 0,
    elapsedMilliseconds: null,
    cost: null,
    spendingAllowance: null,
  };
  const page = {
    kind: "usage",
    page: {
      revision: 1,
      entries: [row],
      nextCursor: null,
      summary: {
        records: 1,
        modelCalls: 1,
        inputTokens: 0,
        outputTokens: 0,
        reservedTokens: 0,
        unknownCalls: 1,
        unknownCostRecords: 1,
        elapsedMilliseconds: 0,
        costs: [],
      },
    },
    pageTruncated: false,
    servedAt: Date.now(),
  };
  expect(() => validatePortfolioResult(page, usage)).toThrow("time scope");
  page.page.entries[0]!.startedAt = 2_000;
  expect(() => validatePortfolioResult(page, usage)).not.toThrow();
  expect(page.page.entries[0]!.cost).toBeNull();
  expect(page.page.summary.unknownCostRecords).toBe(1);
});
test("project totals cannot claim fewer projects than a returned page", () => {
  const page = {
    ...result,
    page: {
      projects: [{ id: randomUUID(), name: "Project" }],
      nextCursor: null,
      total: 0,
    },
  };
  expect(() => validatePortfolioResult(page, query)).toThrow("project page");
});

test("faster PDF polling extends the bounded poll count while preserving exact request identity and deadline", async () => {
  const f = fixture();
  let reads = 0;
  const api = {
    ...f.api,
    maxPolls: 240,
    read: async () =>
      ++reads < 80 ? f.job : { ...f.job, status: "complete" as const, result },
  };
  expect(
    await loadGatewayPortfolio(
      pairing,
      publisherId,
      query,
      nonce,
      () => true,
      api,
    ),
  ).toEqual(result);
  expect(reads).toBe(80);
  for (const maxPolls of [0, 241, Number.NaN])
    await expect(
      loadGatewayPortfolio(pairing, publisherId, query, nonce, () => true, {
        ...api,
        maxPolls,
      }),
    ).rejects.toThrow(/poll bound/);
});
