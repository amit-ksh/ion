import type { CloudBillingReport } from "./generated";
export interface GatewayBillingScope {
  id: string;
  name: string;
  currency: string;
  revision: number;
  reports: CloudBillingReport[];
}
export interface GatewayBillingCharge {
  ordinal: number;
  resourceType: string | null;
  category: string;
  class: string | null;
  chargePeriodStart: string;
  chargePeriodEnd: string;
  billedCost: string;
  effectiveCost: string;
  contractedCost: string;
}
export type GatewayBillingQuery =
  | { kind: "cloudBilling"; query: { after: string | null; limit: number } }
  | {
      kind: "cloudBillingReports";
      query: { scopeId: string; after: string | null; limit: number };
    }
  | {
      kind: "cloudBillingCharges";
      query: {
        scopeId: string;
        exportId: string;
        after: number | null;
        limit: number;
      };
    };
type Envelope = { pageTruncated: boolean; servedAt: number };
export type GatewayBillingResult = Envelope &
  (
    | {
        kind: "cloudBilling";
        page: {
          scopes: GatewayBillingScope[];
          nextCursor: string | null;
          total: number;
        };
      }
    | {
        kind: "cloudBillingReports";
        page: {
          scopeId: string;
          reports: CloudBillingReport[];
          nextCursor: string | null;
          total: number;
        };
      }
    | {
        kind: "cloudBillingCharges";
        page: {
          report: CloudBillingReport;
          charges: GatewayBillingCharge[];
          nextCursor: number | null;
        };
      }
  );
const obj = (v: unknown): v is Record<string, any> =>
  !!v && typeof v === "object" && !Array.isArray(v);
const shape = (v: unknown, fields: string): v is Record<string, any> =>
  obj(v) &&
  Object.keys(v).sort().join(",") === fields.split(",").sort().join(",");
const count = (v: unknown): v is number =>
  typeof v === "number" && Number.isSafeInteger(v) && v >= 0;
const uuid = (v: unknown): v is string =>
  typeof v === "string" &&
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(v);
const identifier = (v: unknown): v is string =>
  typeof v === "string" && /^[A-Za-z0-9_-]{1,160}$/.test(v);
const label = (v: unknown): v is string =>
  typeof v === "string" && v.length <= 160;
const currency = (v: unknown): v is string =>
  typeof v === "string" && /^[A-Z]{3}$/.test(v);
const decimal = (v: unknown): v is string =>
  typeof v === "string" &&
  /^-?(?:0|[1-9][0-9]{0,38})(?:\.[0-9]{1,18})?$/.test(v);
function instant(v: unknown): { millis: number; remainder: string } | null {
  if (typeof v !== "string") return null;
  const m =
    /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.(\d{1,9}))?(Z|[+-](\d{2}):(\d{2}))$/.exec(
      v,
    );
  if (!m) return null;
  const year = Number(m[1]),
    month = Number(m[2]),
    day = Number(m[3]);
  const leap = year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0);
  const days = [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][
    month - 1
  ];
  if (
    !days ||
    day < 1 ||
    day > days ||
    Number(m[4]) > 23 ||
    Number(m[5]) > 59 ||
    Number(m[6]) > 59 ||
    Number(m[9] ?? 0) > 23 ||
    Number(m[10] ?? 0) > 59
  )
    return null;
  const millis = Date.parse(v);
  return Number.isFinite(millis)
    ? { millis, remainder: (m[7] ?? "").padEnd(9, "0").slice(3) }
    : null;
}
const timestamp = (v: unknown): v is string => instant(v) !== null;
function later(end: unknown, start: unknown): boolean {
  const e = instant(end),
    s = instant(start);
  return (
    !!e &&
    !!s &&
    (e.millis > s.millis ||
      (e.millis === s.millis && e.remainder > s.remainder))
  );
}
const digest = (v: unknown): v is string =>
  typeof v === "string" && /^[0-9a-f]{64}$/.test(v);
export function isGatewayBilling(kind: unknown): boolean {
  return (
    kind === "cloudBilling" ||
    kind === "cloudBillingReports" ||
    kind === "cloudBillingCharges"
  );
}
export function isGatewayBillingQuery(query: {
  kind: string;
}): query is GatewayBillingQuery {
  return isGatewayBilling(query.kind);
}
export function validateGatewayBillingQuery(
  value: unknown,
): asserts value is GatewayBillingQuery {
  if (
    !shape(value, "kind,query") ||
    !obj(value.query) ||
    !count(value.query.limit) ||
    value.query.limit < 1 ||
    value.query.limit > 32
  )
    throw new Error("Invalid billing query page");
  const q = value.query;
  if (value.kind === "cloudBilling") {
    if (!shape(q, "after,limit") || (q.after !== null && !uuid(q.after)))
      throw new Error("Invalid billing account cursor");
  } else if (value.kind === "cloudBillingReports") {
    if (
      !shape(q, "scopeId,after,limit") ||
      !uuid(q.scopeId) ||
      (q.after !== null && !identifier(q.after))
    )
      throw new Error("Invalid billing export query");
  } else if (value.kind === "cloudBillingCharges") {
    if (
      !shape(q, "scopeId,exportId,after,limit") ||
      !uuid(q.scopeId) ||
      !identifier(q.exportId) ||
      (q.after !== null &&
        (!count(q.after) || q.after < 1 || q.after > 100_000))
    )
      throw new Error("Invalid billing charge query");
  } else throw new Error("Unknown billing query");
}
function report(
  value: unknown,
  scopeId: string,
  expectedCurrency?: string,
): asserts value is CloudBillingReport {
  if (
    !shape(
      value,
      "scopeId,exportId,period,currency,billedCost,effectiveCost,contractedCost,rows,unattributedResourceRows,createdAt,fetchedAt,archiveSha256,csvSha256,current",
    ) ||
    value.scopeId !== scopeId ||
    !identifier(value.exportId) ||
    typeof value.period !== "string" ||
    !/^\d{4}-(?:0[1-9]|1[0-2])$/.test(value.period) ||
    !currency(value.currency) ||
    (expectedCurrency !== undefined && value.currency !== expectedCurrency) ||
    !["billedCost", "effectiveCost", "contractedCost"].every((k) =>
      decimal(value[k]),
    ) ||
    !count(value.rows) ||
    value.rows < 1 ||
    value.rows > 100_000 ||
    !count(value.unattributedResourceRows) ||
    value.unattributedResourceRows > value.rows ||
    !timestamp(value.createdAt) ||
    !count(value.fetchedAt) ||
    !digest(value.archiveSha256) ||
    !digest(value.csvSha256) ||
    typeof value.current !== "boolean"
  )
    throw new Error("Invalid billing report provenance");
}
function cursor(
  next: unknown,
  last: unknown,
  length: number,
  truncated: boolean,
) {
  if (
    (next !== null && (!length || next !== last)) ||
    (truncated && next === null)
  )
    throw new Error("Invalid billing page cursor");
}
export function validateGatewayBillingResult(
  value: unknown,
  request: GatewayBillingQuery,
): asserts value is GatewayBillingResult {
  validateGatewayBillingQuery(request);
  if (
    !shape(value, "kind,page,pageTruncated,servedAt") ||
    value.kind !== request.kind ||
    !count(value.servedAt) ||
    typeof value.pageTruncated !== "boolean" ||
    !obj(value.page)
  )
    throw new Error("Invalid billing response");
  const p = value.page;
  if (request.kind === "cloudBilling") {
    if (
      !shape(p, "scopes,nextCursor,total") ||
      !Array.isArray(p.scopes) ||
      p.scopes.length > request.query.limit ||
      !count(p.total) ||
      p.total > 64 ||
      p.total < p.scopes.length ||
      (p.nextCursor !== null && !uuid(p.nextCursor))
    )
      throw new Error("Invalid billing account page");
    let previous = request.query.after;
    for (const row of p.scopes) {
      if (
        !shape(row, "id,name,currency,revision,reports") ||
        !uuid(row.id) ||
        !label(row.name) ||
        !currency(row.currency) ||
        !count(row.revision) ||
        !Array.isArray(row.reports) ||
        row.reports.length > 12 ||
        (previous !== null && row.id <= previous)
      )
        throw new Error("Invalid billing account row");
      let priorPeriod: string | null = null;
      for (const r of row.reports) {
        report(r, row.id, row.currency);
        if (!r.current || (priorPeriod !== null && r.period >= priorPeriod))
          throw new Error("Invalid selected billing snapshots");
        priorPeriod = r.period;
      }
      previous = row.id;
    }
    cursor(p.nextCursor, previous, p.scopes.length, value.pageTruncated);
  } else if (request.kind === "cloudBillingReports") {
    if (
      !shape(p, "scopeId,reports,nextCursor,total") ||
      p.scopeId !== request.query.scopeId ||
      !Array.isArray(p.reports) ||
      p.reports.length > request.query.limit ||
      !count(p.total) ||
      p.total > 4096 ||
      p.total < p.reports.length ||
      (p.nextCursor !== null && !identifier(p.nextCursor))
    )
      throw new Error("Billing exports crossed account scope");
    let previous = request.query.after;
    for (const r of p.reports) {
      report(r, request.query.scopeId);
      if (previous !== null && r.exportId <= previous)
        throw new Error("Invalid billing export order");
      previous = r.exportId;
    }
    cursor(p.nextCursor, previous, p.reports.length, value.pageTruncated);
  } else {
    if (
      !shape(p, "report,charges,nextCursor") ||
      !Array.isArray(p.charges) ||
      p.charges.length > request.query.limit ||
      (p.nextCursor !== null &&
        (!count(p.nextCursor) || p.nextCursor < 1 || p.nextCursor > 100_000))
    )
      throw new Error("Invalid billing charges page");
    report(p.report, request.query.scopeId);
    if (p.report.exportId !== request.query.exportId)
      throw new Error("Billing charges crossed export scope");
    let previous = request.query.after ?? 0;
    for (const row of p.charges) {
      if (
        !shape(
          row,
          "ordinal,resourceType,category,class,chargePeriodStart,chargePeriodEnd,billedCost,effectiveCost,contractedCost",
        ) ||
        !count(row.ordinal) ||
        row.ordinal <= previous ||
        row.ordinal > p.report.rows ||
        !label(row.category) ||
        (row.class !== null && !label(row.class)) ||
        (row.resourceType !== null && !label(row.resourceType)) ||
        !timestamp(row.chargePeriodStart) ||
        !timestamp(row.chargePeriodEnd) ||
        !later(row.chargePeriodEnd, row.chargePeriodStart) ||
        !["billedCost", "effectiveCost", "contractedCost"].every((k) =>
          decimal(row[k]),
        )
      )
        throw new Error("Invalid billing charge evidence");
      previous = row.ordinal;
    }
    cursor(p.nextCursor, previous, p.charges.length, value.pageTruncated);
  }
}
