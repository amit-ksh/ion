import type { CloudBudget, ControlRecord } from "./generated";
export function parseBudgetAmount(text: string): number {
  const value = text.trim();
  if (value.length > 20 || !/^\d+(?:\.\d{0,6})?$/.test(value))
    throw new Error("Enter an amount with at most six decimal places.");
  const [whole, fraction = ""] = value.split(".");
  const integer = Number(whole);
  if (integer > 1_000_000)
    throw new Error("GPU budget exceeds 1,000,000 currency units.");
  const micros = integer * 1_000_000 + Number(fraction.padEnd(6, "0"));
  if (!Number.isSafeInteger(micros) || micros < 1 || micros > 1_000_000_000_000)
    throw new Error(
      "Enter a positive GPU budget no greater than 1,000,000 currency units.",
    );
  return micros;
}
export function formatBudgetAmount(micros: number): string {
  if (!Number.isSafeInteger(micros) || micros < 0)
    throw new Error("Invalid monetary amount.");
  return `${Math.floor(micros / 1_000_000)}.${String(micros % 1_000_000).padStart(6, "0")}`;
}
const bytes = (s: string) => {
  let n = 0;
  for (const c of s) {
    const p = c.codePointAt(0)!;
    n += p <= 127 ? 1 : p <= 2047 ? 2 : p <= 65535 ? 3 : 4;
  }
  return n;
};
const label = (s: unknown) =>
  typeof s === "string" &&
  s === s.trim() &&
  !!s &&
  bytes(s) <= 160 &&
  !/[\u0000-\u001f\u007f-\u009f]/.test(s);
export function validateCloudBudget(
  value: unknown,
): asserts value is CloudBudget {
  const b = value as CloudBudget;
  if (
    !b ||
    typeof b !== "object" ||
    Array.isArray(b) ||
    Object.keys(b).sort().join(",") !==
      "cloudProject,currency,id,limitMicros,name,paused,targets" ||
    typeof b.id !== "string" ||
    !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(
      b.id,
    ) ||
    b.id === "00000000-0000-0000-0000-000000000000" ||
    !label(b.name) ||
    typeof b.cloudProject !== "string" ||
    !/^[A-Za-z0-9_-]{1,160}$/.test(b.cloudProject) ||
    typeof b.currency !== "string" ||
    !/^[A-Z]{3}$/.test(b.currency) ||
    !Number.isSafeInteger(b.limitMicros) ||
    b.limitMicros < 1 ||
    b.limitMicros > 1_000_000_000_000 ||
    typeof b.paused !== "boolean" ||
    !Array.isArray(b.targets) ||
    b.targets.length < 1 ||
    b.targets.length > 16 ||
    b.targets.some((t) => !label(t)) ||
    new Set(b.targets).size !== b.targets.length
  )
    throw new Error(
      "Enter a name, exact Nebius project ID, three-letter currency, positive cap and distinct approved target names.",
    );
}
export function sameCloudBudget(
  record: ControlRecord | undefined,
  budget: CloudBudget,
): boolean {
  if (record?.kind !== "cloudBudget") return false;
  const { kind: _, ...saved } = record;
  return (
    saved.id === budget.id &&
    saved.name === budget.name &&
    saved.cloudProject === budget.cloudProject &&
    saved.currency === budget.currency &&
    saved.limitMicros === budget.limitMicros &&
    saved.paused === budget.paused &&
    JSON.stringify(saved.targets) === JSON.stringify(budget.targets)
  );
}
