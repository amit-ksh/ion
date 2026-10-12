import { expect, test } from "bun:test";
import { randomUUID } from "node:crypto";
import type {
  GatewayBillingQuery,
  PortfolioJob,
  PortfolioSubmission,
} from "@waku/client";
import { gatewayBillingRequest } from "./gateway-billing";
import { loadGatewayPortfolio } from "./gateway-portfolio";
import type { GatewayPairing } from "./gateway-client";
const owner = randomUUID(),
  scope = randomUUID();
test("gateway billing navigation pins scope to owner and converts only valid bounded cursors", () => {
  expect(gatewayBillingRequest({})).toEqual({
    query: { kind: "cloudBilling", query: { after: null, limit: 32 } },
    ownerId: undefined,
  });
  expect(
    gatewayBillingRequest({ owner, scope, after: "export-1" }).query,
  ).toEqual({
    kind: "cloudBillingReports",
    query: { scopeId: scope, after: "export-1", limit: 16 },
  });
  expect(
    gatewayBillingRequest({ owner, scope, exportId: "export-1", after: "1" })
      .query,
  ).toEqual({
    kind: "cloudBillingCharges",
    query: { scopeId: scope, exportId: "export-1", after: 1, limit: 32 },
  });
  for (const route of [
    { scope },
    { owner: "invalid" },
    { owner: [owner] },
    { owner, scope, exportId: "../private" },
    { owner, scope, exportId: "export-1", after: "0" },
    { owner, scope, exportId: "export-1", after: "100001" },
    { owner, scope, exportId: "export-1", after: "1e3" },
    { owner, scope, exportId: "export-1", after: "01" },
    { exportId: "export-1" },
    { after: "" },
  ])
    expect(() => gatewayBillingRequest(route)).toThrow();
});
function fixture(query: GatewayBillingQuery) {
  const pairing: GatewayPairing = {
    baseUrl: "https://gateway.example",
    id: randomUUID(),
    token: "private",
    grants: ["usageRead"],
    expiresAt: Date.now() + 120000,
  };
  const input: PortfolioSubmission = {
    publisherId: owner,
    idempotencyKey: randomUUID(),
    query,
  };
  const report = {
    scopeId: scope,
    exportId: "export-1",
    period: "2026-09",
    currency: "USD",
    billedCost: "-0.000000000000000001",
    effectiveCost: "1.2",
    contractedCost: "0",
    rows: 1,
    unattributedResourceRows: 1,
    createdAt: "2026-10-01T00:00:00Z",
    fetchedAt: 1,
    archiveSha256: "a".repeat(64),
    csvSha256: "b".repeat(64),
    current: true,
  };
  const result = {
    kind: "cloudBillingCharges",
    page: { report, charges: [], nextCursor: null },
    pageTruncated: false,
    servedAt: Date.now(),
  };
  const job: PortfolioJob = {
    ...input,
    id: randomUUID(),
    deviceId: pairing.id,
    status: "complete",
    result: result as any,
    createdAt: Date.now(),
    updatedAt: Date.now(),
    expiresAt: Date.now() + 120000,
  };
  let active = true;
  const calls: string[] = [];
  const api = {
    submit: async (_: GatewayPairing, request: PortfolioSubmission) => {
      calls.push("submit");
      expect(request).toEqual(input);
      return job;
    },
    read: async (_: GatewayPairing, request: PortfolioSubmission) => {
      calls.push("read");
      expect(request).toEqual(input);
      return job;
    },
    wait: async () => {
      calls.push("wait");
    },
  };
  const load = () =>
    loadGatewayPortfolio(
      pairing,
      owner,
      query,
      input.idempotencyKey,
      () => active,
      api,
    );
  return {
    pairing,
    input,
    job,
    api,
    calls,
    load,
    invalidate: () => {
      active = false;
    },
  };
}
const query = () =>
  gatewayBillingRequest({ owner, scope, exportId: "export-1" }).query;
test("phone billing checks usageRead before sending and does not receive connection fields", async () => {
  const f = fixture(query());
  expect((await f.load()).kind).toBe("cloudBillingCharges");
  expect(f.calls).toEqual(["submit"]);
  const denied = fixture(query());
  denied.pairing.grants = ["inboxRead"];
  await expect(denied.load()).rejects.toThrow(
    "required owner history read access",
  );
  expect(denied.calls).toEqual([]);
});
test("billing owner or export changes and connection fields reject the full receipt", async () => {
  for (const patch of [
    { scopeId: randomUUID() },
    { exportId: "another-export" },
    { downloadUrl: "https://archive.invalid" },
  ]) {
    const f = fixture(query());
    Object.assign((f.job.result as any).page.report, patch);
    await expect(f.load()).rejects.toThrow();
  }
  const f = fixture(query());
  f.job.publisherId = randomUUID();
  await expect(f.load()).rejects.toThrow("request scope");
});
test("lost billing submit acknowledgements reconcile the same read and nonce", async () => {
  const f = fixture(query());
  f.api.submit = async () => {
    f.calls.push("submit");
    throw new Error("Dropped response");
  };
  expect((await f.load()).kind).toBe("cloudBillingCharges");
  expect(f.calls).toEqual(["submit", "read"]);
});
test("backgrounding or changing pairing suppresses late billing results and follow-up polls", async () => {
  const f = fixture(query());
  f.api.submit = async () => {
    f.calls.push("submit");
    f.invalidate();
    return f.job;
  };
  await expect(f.load()).rejects.toThrow("scope changed");
  expect(f.calls).toEqual(["submit"]);
  const pending = fixture(query());
  pending.job.status = "queued";
  pending.job.result = null;
  pending.api.wait = async () => {
    pending.calls.push("wait");
    pending.invalidate();
  };
  await expect(pending.load()).rejects.toThrow("scope changed");
  expect(pending.calls).toEqual(["submit", "wait"]);
});
