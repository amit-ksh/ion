import { test, expect } from "bun:test";
import {
  formatBudgetAmount,
  parseBudgetAmount,
  validateCloudBudget,
} from "./cloud-budgets";
import {
  portfolioReadGrant,
  validatePortfolioQuery,
  validatePortfolioResult,
} from "./gateway-portfolio";
const id = "00000000-0000-4000-8000-000000000001";
test("money inputs retain six exact decimal places and reject coercion or rounding", () => {
  for (const [text, value] of [
    ["0.000001", 1],
    ["1.234567", 1_234_567],
    ["1000000", 1_000_000_000_000],
    [" 01.20 ", 1_200_000],
  ] as const) {
    expect(parseBudgetAmount(text)).toBe(value);
    expect(parseBudgetAmount(formatBudgetAmount(value))).toBe(value);
  }
  for (const text of [
    "0",
    "-1",
    "NaN",
    "1e6",
    ".5",
    "1.2.3",
    "1.0000001",
    "1000000.000001",
    "18446744073709551615",
  ]) {
    expect(() => parseBudgetAmount(text)).toThrow();
  }
});
test("owner budget validates exact target, currency, cap and wire shape", () => {
  const budget = {
    id,
    name: "Training",
    cloudProject: "project-test",
    currency: "USD",
    limitMicros: 10_000,
    paused: true,
    targets: ["training"],
  };
  validateCloudBudget(budget);
  for (const patch of [
    { limitMicros: 0 },
    { limitMicros: 0.1 },
    { limitMicros: Number.MAX_SAFE_INTEGER },
    { currency: "usd" },
    { cloudProject: "../project" },
    { targets: ["training", "training"] },
    { targets: [""] },
    { id: 0 },
    { name: "x".repeat(161) },
    { secret: "never" },
  ])
    expect(() => validateCloudBudget({ ...budget, ...patch })).toThrow();
});
test("gateway GPU totals are read-only, scoped to usage grant and strictly validated", () => {
  const query = {
    kind: "cloudBudgets",
    query: { after: null, limit: 32 },
  } as const;
  validatePortfolioQuery(query);
  expect(portfolioReadGrant(query)).toBe("usageRead");
  const row = {
    id,
    name: "GPU",
    currency: "USD",
    limitMicros: 10_000,
    heldMicros: 2_000,
    availableMicros: 8_000,
    paused: false,
    requests: 1,
  };
  const result = {
    kind: "cloudBudgets",
    page: { budgets: [row], nextCursor: null, total: 1 },
    pageTruncated: false,
    servedAt: 1000,
  };
  validatePortfolioResult(result, query);
  for (const patch of [
    { cloudProject: "private-project" },
    { targets: ["private-host"] },
    { availableMicros: 9_000 },
    { heldMicros: -1 },
    { currency: "usd" },
    { requests: 1.5 },
  ])
    expect(() =>
      validatePortfolioResult(
        {
          ...result,
          page: { ...result.page, budgets: [{ ...row, ...patch }] },
        },
        query,
      ),
    ).toThrow();
  expect(() =>
    validatePortfolioQuery({
      ...query,
      query: { ...query.query, activate: true },
    }),
  ).toThrow();
  expect(() =>
    validatePortfolioResult(
      {
        ...result,
        page: {
          ...result.page,
          nextCursor: "00000000-0000-4000-8000-000000000002",
        },
      },
      query,
    ),
  ).toThrow();
  expect(() =>
    validatePortfolioResult(result, {
      ...query,
      query: { after: id, limit: 32 },
    }),
  ).toThrow();
});
