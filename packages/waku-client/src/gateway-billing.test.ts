import { expect, test } from "bun:test";
import {
  portfolioReadGrant,
  validatePortfolioQuery,
  validatePortfolioResult,
  type PortfolioQuery,
} from "./gateway-portfolio";
import type { CloudBillingReport } from "./generated";
const scope = "00000000-0000-0000-0000-000000000001",
  other = "00000000-0000-0000-0000-000000000002";
const report: CloudBillingReport = {
  scopeId: scope,
  exportId: "export-1",
  period: "2026-09",
  currency: "USD",
  billedCost: "-0.000000000000000001",
  effectiveCost: "12345678901234567890.123456789012345678",
  contractedCost: "0",
  rows: 3,
  unattributedResourceRows: 1,
  createdAt: "2026-10-01T00:00:00.123456789Z",
  fetchedAt: 1,
  archiveSha256: "a".repeat(64),
  csvSha256: "b".repeat(64),
  current: true,
};
const accounts: PortfolioQuery = {
  kind: "cloudBilling",
  query: { after: null, limit: 32 },
};
const reports: PortfolioQuery = {
  kind: "cloudBillingReports",
  query: { scopeId: scope, after: null, limit: 16 },
};
const charges: PortfolioQuery = {
  kind: "cloudBillingCharges",
  query: { scopeId: scope, exportId: report.exportId, after: null, limit: 32 },
};
function accountResult() {
  return {
    kind: "cloudBilling",
    page: {
      scopes: [
        {
          id: scope,
          name: "Cloud costs",
          currency: "USD",
          revision: 1,
          reports: [{ ...report }],
        },
      ],
      nextCursor: null as string | null,
      total: 1,
    },
    servedAt: 1,
    pageTruncated: false,
  };
}
function reportResult() {
  return {
    kind: "cloudBillingReports",
    page: {
      scopeId: scope,
      reports: [{ ...report }],
      nextCursor: null as string | null,
      total: 1,
    },
    servedAt: 1,
    pageTruncated: false,
  };
}
function chargeResult() {
  return {
    kind: "cloudBillingCharges",
    page: {
      report: { ...report },
      charges: [
        {
          ordinal: 1,
          resourceType: "gpu",
          category: "Usage",
          class: null,
          chargePeriodStart: "2026-09-01T00:00:00Z",
          chargePeriodEnd: "2026-10-01T00:00:00Z",
          billedCost: "-0.000000000000000001",
          effectiveCost: "1.2",
          contractedCost: "0",
        },
      ],
      nextCursor: null,
    },
    servedAt: 1,
    pageTruncated: false,
  };
}
test("all saved billing queries require usageRead and cannot represent writes or connection fields", () => {
  for (const query of [accounts, reports, charges]) {
    validatePortfolioQuery(query);
    expect(portfolioReadGrant(query)).toBe("usageRead");
    expect(() =>
      validatePortfolioQuery({ ...query, token: "private" }),
    ).toThrow();
  }
  for (const query of [
    { kind: "syncCloudBilling", query: accounts.query },
    {
      ...reports,
      query: { ...reports.query, after: "https://archive.invalid" },
    },
    { ...charges, query: { ...charges.query, after: 0 } },
    { ...charges, query: { ...charges.query, after: 100001 } },
    { ...accounts, query: { after: null, limit: 33 } },
    { ...reports, query: { ...reports.query, contractId: "private" } },
  ])
    expect(() => validatePortfolioQuery(query)).toThrow();
});
test("exact signed decimals and report digests survive validated account export and charge pages", () => {
  for (const [value, query] of [
    [accountResult(), accounts],
    [reportResult(), reports],
    [chargeResult(), charges],
  ] as const)
    validatePortfolioResult(value, query);
  expect(reportResult().page.reports[0]!.effectiveCost).toBe(
    "12345678901234567890.123456789012345678",
  );
  expect(chargeResult().page.charges[0]!.billedCost).toBe(
    "-0.000000000000000001",
  );
});
test("connection or cloud resource identities cannot enter any billing gateway response", () => {
  for (const key of [
    "accountId",
    "contractId",
    "archiveOrigin",
    "token",
    "downloadUrl",
  ]) {
    const value = accountResult();
    Object.assign(value.page.scopes[0]!, { [key]: "private" });
    expect(() => validatePortfolioResult(value, accounts)).toThrow();
  }
  for (const key of ["containerId", "resourceId"]) {
    const value = chargeResult();
    Object.assign(value.page.charges[0]!, { [key]: "private" });
    expect(() => validatePortfolioResult(value, charges)).toThrow();
  }
  const value = reportResult();
  Object.assign(value.page.reports[0]!, {
    downloadUrl: "https://archive.invalid",
  });
  expect(() => validatePortfolioResult(value, reports)).toThrow();
});
test("response account export and selected-month identities stay within the requested scope", () => {
  const values = [reportResult(), reportResult()];
  values[0]!.page.scopeId = other;
  values[1]!.page.reports[0]!.scopeId = other;
  for (const value of values)
    expect(() => validatePortfolioResult(value, reports)).toThrow();
  const wrong = chargeResult();
  wrong.page.report.exportId = "export-2";
  expect(() => validatePortfolioResult(wrong, charges)).toThrow();
  const account = accountResult();
  account.page.scopes[0]!.reports[0]!.currency = "EUR";
  expect(() => validatePortfolioResult(account, accounts)).toThrow();
  const duplicate = accountResult();
  duplicate.page.scopes[0]!.reports.push({ ...report });
  expect(() => validatePortfolioResult(duplicate, accounts)).toThrow();
});
test("bounded pages require strictly advancing rows and continuation cursors without truncating evidence", () => {
  const value = reportResult();
  value.page.nextCursor = report.exportId;
  value.page.total = 2;
  validatePortfolioResult(value, reports);
  expect(() =>
    validatePortfolioResult(value, {
      kind: "cloudBillingReports",
      query: { scopeId: scope, after: report.exportId, limit: 16 },
    }),
  ).toThrow();
  const empty = reportResult();
  empty.page.reports = [];
  empty.page.nextCursor = report.exportId;
  expect(() => validatePortfolioResult(empty, reports)).toThrow();
  const noCursor = reportResult();
  noCursor.pageTruncated = true;
  expect(() => validatePortfolioResult(noCursor, reports)).toThrow();
  const ordinal = chargeResult();
  ordinal.page.charges[0]!.ordinal = 4;
  expect(() => validatePortfolioResult(ordinal, charges)).toThrow();
  const many = chargeResult();
  many.page.charges.push({ ...many.page.charges[0]! });
  expect(() => validatePortfolioResult(many, charges)).toThrow();
});
test("invalid digests fractional counts floats scientific notation and invalid calendar dates are rejected", () => {
  for (const patch of [
    { csvSha256: "bad" },
    { billedCost: 0.1 },
    { billedCost: "1e-2" },
    { rows: 1.5 },
    { unattributedResourceRows: 4 },
    { createdAt: "2026-02-30T00:00:00Z" },
    { period: "2026-13" },
  ]) {
    const value = reportResult();
    Object.assign(value.page.reports[0]!, patch);
    expect(() => validatePortfolioResult(value, reports)).toThrow();
  }
});
test("charge period comparison preserves sub-millisecond precision from native report evidence", () => {
  const value = chargeResult();
  value.page.charges[0]!.chargePeriodStart = "2026-09-01T00:00:00.123456788Z";
  value.page.charges[0]!.chargePeriodEnd = "2026-09-01T00:00:00.123456789Z";
  validatePortfolioResult(value, charges);
  value.page.charges[0]!.chargePeriodEnd =
    value.page.charges[0]!.chargePeriodStart;
  expect(() => validatePortfolioResult(value, charges)).toThrow();
});
