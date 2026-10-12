import { expect, test } from "bun:test";
import type {
  CloudBillingReport,
  ResponsePayload,
  Command,
  WakuClient,
} from "@waku/client";
import { readCloudBilling } from "./cloud-billing";
const scope = "e8e23910-655c-43aa-a47c-93e1d1cdf677";
const report: CloudBillingReport = {
  scopeId: scope,
  exportId: "export-test",
  period: "2026-09",
  currency: "USD",
  billedCost: "0.000000000000000001",
  effectiveCost: "0",
  contractedCost: "-0.5",
  rows: 1,
  unattributedResourceRows: 1,
  createdAt: "2026-10-10T00:00:00Z",
  fetchedAt: 1791763200,
  archiveSha256: "a".repeat(64),
  csvSha256: "b".repeat(64),
  current: true,
};
function transport(reply: ResponsePayload) {
  const commands: Command[] = [];
  const client: Pick<WakuClient, "request"> = {
    request: async (command) => {
      commands.push(command);
      return reply;
    },
  };
  return { client, commands };
}
test("account reads never submit a scope or sync a provider", async () => {
  const t = transport({
    type: "cloudBilling",
    page: { scopes: [], total: 0, nextCursor: null },
  });
  expect((await readCloudBilling(t.client, {})).kind).toBe("accounts");
  expect(t.commands).toEqual([
    { type: "loadCloudBilling", after: null, limit: 32 },
  ]);
});
test("history is scoped, bounded and preserves exact reported amounts", async () => {
  const t = transport({
    type: "cloudBillingReports",
    page: { scopeId: scope, reports: [report], total: 1, nextCursor: null },
  });
  const result = await readCloudBilling(t.client, {
    scope,
    after: "export-previous",
  });
  expect(result.kind).toBe("reports");
  if (result.kind === "reports")
    expect(result.page.reports[0].billedCost).toBe("0.000000000000000001");
  expect(t.commands).toEqual([
    {
      type: "loadCloudBillingReports",
      scopeId: scope,
      after: "export-previous",
      limit: 16,
    },
  ]);
});
test("charge reads refuse another account or original export identity", async () => {
  for (const wrong of [
    { ...report, scopeId: "a7da7f85-1f1e-48eb-88d9-faaf1d6c6472" },
    { ...report, exportId: "other-export" },
  ]) {
    const t = transport({
      type: "cloudBillingCharges",
      page: { report: wrong, charges: [], nextCursor: null },
    });
    await expect(
      readCloudBilling(t.client, { scope, exportId: "export-test" }),
    ).rejects.toThrow("another export");
  }
});
test("invalid route and cursor values reject before transport", async () => {
  for (const route of [
    { scope: "bad" },
    { after: "bad" },
    { scope, after: "../other" },
    { scope, exportId: "../../other" },
    { scope, exportId: "export-test", after: "100001" },
    { scope, exportId: "export-test", after: "1e3" },
  ]) {
    const t = transport({
      type: "cloudBilling",
      page: { scopes: [], total: 0, nextCursor: null },
    });
    await expect(readCloudBilling(t.client, route)).rejects.toThrow();
    expect(t.commands).toHaveLength(0);
  }
});
test("historical charges remain readable with original state and exact credit", async () => {
  const old = { ...report, current: false };
  const t = transport({
    type: "cloudBillingCharges",
    page: {
      report: old,
      charges: [
        {
          ordinal: 2,
          containerId: "project-original",
          resourceId: null,
          resourceType: null,
          category: "Credit",
          class: "Correction",
          chargePeriodStart: "2026-08-01T00:00:00Z",
          chargePeriodEnd: "2026-09-01T00:00:00Z",
          billedCost: "-0.00000001",
          effectiveCost: "0",
          contractedCost: "0",
        },
      ],
      nextCursor: null,
    },
  });
  const result = await readCloudBilling(t.client, {
    scope,
    exportId: "export-test",
    after: "1",
  });
  if (result.kind === "charges") {
    expect(result.page.report.current).toBe(false);
    expect(result.page.charges[0].billedCost).toBe("-0.00000001");
  } else throw new Error("Expected charge page");
});
