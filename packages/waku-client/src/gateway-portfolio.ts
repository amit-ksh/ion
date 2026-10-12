import {
  isGatewayBilling,
  isGatewayBillingQuery,
  validateGatewayBillingQuery,
  validateGatewayBillingResult,
  type GatewayBillingQuery,
  type GatewayBillingResult,
} from "./gateway-billing";
export * from "./gateway-billing";
import {
  isWorkflowHistory,
  isWorkflowHistoryQuery,
  validateWorkflowHistoryQuery,
  validateWorkflowHistoryResult,
  type WorkflowHistoryQuery,
  type WorkflowHistoryResult,
} from "./gateway-history";
import type {
  InboxEntry,
  InboxPage,
  InboxQuery,
  UsageLedgerEntry,
  UsageLedgerPage,
  UsageLedgerQuery,
  UsageCost,
} from "./generated";

export interface GatewayCloudBudget {
  id: string;
  name: string;
  currency: string;
  limitMicros: number;
  paused: boolean;
  heldMicros: number;
  availableMicros: number;
  requests: number;
}
export type PortfolioQuery =
  | WorkflowHistoryQuery
  | GatewayBillingQuery
  | { kind: "cloudBudgets"; query: { after: string | null; limit: number } }
  | { kind: "inbox"; query: InboxQuery }
  | { kind: "usage"; query: UsageLedgerQuery }
  | { kind: "projects"; query: { after: string | null; limit: number } };
export type PortfolioResult =
  | WorkflowHistoryResult
  | GatewayBillingResult
  | {
      kind: "cloudBudgets";
      page: {
        budgets: GatewayCloudBudget[];
        nextCursor: string | null;
        total: number;
      };
      pageTruncated: boolean;
      servedAt: number;
    }
  | {
      kind: "inbox";
      page: Omit<InboxPage, "entries"> & {
        entries: Array<InboxEntry & { detailComplete: boolean }>;
      };
      pageTruncated: boolean;
      servedAt: number;
    }
  | {
      kind: "usage";
      page: Omit<UsageLedgerPage, "entries"> & {
        entries: Array<
          Omit<UsageLedgerEntry, "host" | "cost"> & {
            cost: Omit<UsageCost, "origin"> | null;
          }
        >;
      };
      pageTruncated: boolean;
      servedAt: number;
    }
  | {
      kind: "projects";
      page: {
        projects: Array<{ id: string; name: string }>;
        nextCursor: string | null;
        total: number;
      };
      pageTruncated: boolean;
      servedAt: number;
    };
export interface PortfolioSubmission {
  publisherId: string;
  idempotencyKey: string;
  query: PortfolioQuery;
}
export interface PortfolioJob extends PortfolioSubmission {
  id: string;
  deviceId: string;
  status: "queued" | "complete" | "rejected" | "expired";
  result: PortfolioResult | null;
  createdAt: number;
  updatedAt: number;
  expiresAt: number;
}
export interface PortfolioOwner {
  publisherId: string;
  name: string;
  publishedAt: number;
}

export const portfolioId = (v: unknown): v is string =>
  typeof v === "string" &&
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(v);
const obj = (v: unknown): v is Record<string, any> =>
  !!v && typeof v === "object" && !Array.isArray(v);
const shape = (v: unknown, fields: string): v is Record<string, any> =>
  obj(v) &&
  Object.keys(v).sort().join(",") === fields.split(",").sort().join(",");
const number = (v: unknown): v is number =>
  typeof v === "number" && Number.isSafeInteger(v) && v >= 0;
const nullable = (v: unknown, check: (value: unknown) => boolean) =>
  v === null || check(v);
const label = (v: unknown, max: number) =>
  typeof v === "string" && v.length <= max;
const tag = (v: unknown, values: string) =>
  typeof v === "string" && values.split(",").includes(v);
const bool = (v: unknown) => typeof v === "boolean";
const providers =
  "antigravity,amp,claude,codex,cursor,deepSeek,fx,openCode,openCode2,grok,kimi,nebius,ohMyPi,pi";
const kinds = "modelCall,remoteCommand,gpuJob,soloModelCall";
const inboxKinds =
  "proposal,decision,approval,question,issue,completed,artifact,update";
const cursor = (v: unknown) => nullable(v, (n) => number(n) && n > 0);
const uuid = (v: unknown) => nullable(v, portfolioId);
// Hermes/native runtimes do not all provide TextEncoder.
export function portfolioUtf8Bytes(value: string): number {
  let bytes = 0;
  for (const character of value) {
    const point = character.codePointAt(0)!;
    bytes += point <= 0x7f ? 1 : point <= 0x7ff ? 2 : point <= 0xffff ? 3 : 4;
  }
  return bytes;
}
const bytes = (v: unknown) => portfolioUtf8Bytes(JSON.stringify(v));

export function validatePortfolioQuery(
  value: unknown,
): asserts value is PortfolioQuery {
  if (obj(value) && isGatewayBilling(value.kind)) {
    validateGatewayBillingQuery(value);
    return;
  }
  if (obj(value) && isWorkflowHistory(value.kind)) {
    validateWorkflowHistoryQuery(value);
    return;
  }
  if (
    !shape(value, "kind,query") ||
    !obj(value.query) ||
    !number(value.query.limit) ||
    value.query.limit < 1 ||
    value.query.limit > 32
  )
    throw new Error("Invalid portfolio query");
  const q = value.query;
  if (value.kind === "projects" || value.kind === "cloudBudgets") {
    if (!shape(q, "after,limit") || !uuid(q.after))
      throw new Error("Invalid project query");
  } else if (value.kind === "inbox") {
    if (
      !shape(q, "projectId,taskId,kind,view,status,after,limit") ||
      !uuid(q.projectId) ||
      !uuid(q.taskId) ||
      !nullable(q.kind, (v) => tag(v, inboxKinds)) ||
      !tag(q.view, "inbox,unread,acknowledged,archived,all") ||
      !tag(q.status, "all,needsAction,resolved") ||
      !cursor(q.after)
    )
      throw new Error("Invalid Inbox query");
  } else if (value.kind === "usage") {
    if (
      !shape(
        q,
        "projectId,taskId,agentId,provider,environment,kind,model,since,after,limit",
      ) ||
      !uuid(q.projectId) ||
      !uuid(q.taskId) ||
      !uuid(q.agentId) ||
      !nullable(q.provider, (v) => tag(v, providers)) ||
      !nullable(q.environment, (v) => tag(v, "local,nebiusSandbox")) ||
      !nullable(q.kind, (v) => tag(v, kinds)) ||
      !nullable(
        q.model,
        (v) =>
          typeof v === "string" && !!v.trim() && portfolioUtf8Bytes(v) <= 512,
      ) ||
      !nullable(q.since, number) ||
      !cursor(q.after)
    )
      throw new Error("Invalid usage query");
  } else throw new Error("Unknown portfolio query");
}
const money = (v: unknown) =>
  shape(v, "currency,micros") &&
  /^[A-Z]{3}$/.test(v.currency) &&
  number(v.micros);
const cost = (v: unknown) =>
  shape(v, "amount,quality,finality") &&
  money(v.amount) &&
  tag(v.quality, "estimated,reported") &&
  tag(v.finality, "provisional,final");
export function validatePortfolioResult(
  value: unknown,
  request: PortfolioQuery,
): asserts value is PortfolioResult {
  validatePortfolioQuery(request);
  if (
    !shape(value, "kind,page,pageTruncated,servedAt") ||
    value.kind !== request.kind ||
    !number(value.servedAt) ||
    !bool(value.pageTruncated) ||
    !obj(value.page) ||
    bytes(value) > 60_000
  )
    throw new Error("Invalid portfolio result");
  if (isGatewayBillingQuery(request)) {
    validateGatewayBillingResult(value, request);
    return;
  }
  if (isWorkflowHistoryQuery(request)) {
    validateWorkflowHistoryResult(value, request);
    return;
  }
  const p = value.page;
  if (value.kind === "cloudBudgets") {
    if (
      request.kind !== "cloudBudgets" ||
      !shape(p, "budgets,nextCursor,total") ||
      !Array.isArray(p.budgets) ||
      p.budgets.length > request.query.limit ||
      !uuid(p.nextCursor) ||
      !number(p.total) ||
      p.total < p.budgets.length ||
      p.total > 64
    )
      throw new Error("Invalid GPU budget page");
    let previous = request.query.after;
    for (const row of p.budgets) {
      if (
        !shape(
          row,
          "id,name,currency,limitMicros,paused,heldMicros,availableMicros,requests",
        ) ||
        !portfolioId(row.id) ||
        !label(row.name, 160) ||
        typeof row.currency !== "string" ||
        !/^[A-Z]{3}$/.test(row.currency) ||
        !bool(row.paused) ||
        !["limitMicros", "heldMicros", "availableMicros", "requests"].every(
          (k) => number(row[k]),
        ) ||
        row.limitMicros < 1 ||
        row.limitMicros > 1_000_000_000_000 ||
        row.heldMicros + row.availableMicros !== row.limitMicros ||
        (previous !== null && row.id <= previous)
      )
        throw new Error("Invalid GPU budget row");
      previous = row.id;
    }
    if (
      p.nextCursor !== null &&
      (!p.budgets.length || p.nextCursor !== previous)
    )
      throw new Error("Invalid GPU budget cursor");
    return;
  }
  if (value.kind === "projects") {
    if (
      !shape(p, "projects,nextCursor,total") ||
      !Array.isArray(p.projects) ||
      p.projects.length > request.query.limit ||
      !uuid(p.nextCursor) ||
      !number(p.total) ||
      p.total < p.projects.length
    )
      throw new Error("Invalid project page");
    let previous = request.kind === "projects" ? request.query.after : null;
    for (const row of p.projects) {
      if (
        !shape(row, "id,name") ||
        !portfolioId(row.id) ||
        !label(row.name, 160) ||
        (previous !== null && row.id <= previous)
      )
        throw new Error("Invalid project row");
      previous = row.id;
    }
    if (
      p.nextCursor !== null &&
      (p.projects.length === 0 || p.nextCursor !== previous)
    )
      throw new Error("Invalid project cursor");
    return;
  }
  if (
    !Array.isArray(p.entries) ||
    p.entries.length > request.query.limit ||
    !number(p.revision) ||
    !cursor(p.nextCursor)
  )
    throw new Error("Invalid history page");
  let previous: number | null =
    request.query.after === null ? null : (request.query.after as number);
  for (const row of p.entries) {
    if (
      !obj(row) ||
      !number(row.sequence) ||
      row.sequence < 1 ||
      (previous !== null && row.sequence >= previous)
    )
      throw new Error("Invalid history order");
    previous = row.sequence;
  }
  if (
    p.nextCursor !== null &&
    (p.entries.length === 0 || p.nextCursor !== previous)
  )
    throw new Error("Invalid history cursor");
  if (value.kind === "inbox") {
    if (
      !shape(
        p,
        "revision,historyRevision,notificationPolicy,entries,nextCursor,matchingCount,unreadCount,needsActionCount,taskHistorySyncing",
      ) ||
      !number(p.historyRevision) ||
      !tag(p.notificationPolicy, "all,issuesOnly,off") ||
      !["matchingCount", "unreadCount", "needsActionCount"].every((k) =>
        number(p[k]),
      ) ||
      !bool(p.taskHistorySyncing) ||
      p.matchingCount < p.entries.length
    )
      throw new Error("Invalid Inbox counts");
    for (const row of p.entries) {
      if (
        !shape(
          row,
          "sequence,source,sourceId,projectId,taskId,runId,kind,title,detail,detailComplete,occurredAt,needsAction,state",
        ) ||
        !portfolioId(row.sourceId) ||
        !["projectId", "taskId", "runId"].every((k) => uuid(row[k])) ||
        !tag(
          row.source,
          "task,workspaceAction,artifact,teamAssignment,teamDecision,automation",
        ) ||
        !tag(row.kind, inboxKinds) ||
        !label(row.title, 160) ||
        !label(row.detail, 1_000) ||
        !bool(row.detailComplete) ||
        !number(row.occurredAt) ||
        !bool(row.needsAction) ||
        !shape(row.state, "key,read,acknowledged,archived") ||
        !label(row.state.key, 100) ||
        !["read", "acknowledged", "archived"].every((k) => bool(row.state[k]))
      )
        throw new Error("Invalid Inbox row");
      if (
        request.kind === "inbox" &&
        ((request.query.status === "needsAction" && !row.needsAction) ||
          (request.query.status === "resolved" && row.needsAction) ||
          (request.query.view === "inbox" && row.state.archived) ||
          (request.query.view === "unread" &&
            (row.state.archived || row.state.read)) ||
          (request.query.view === "archived" && !row.state.archived) ||
          (request.query.view === "acknowledged" &&
            (row.state.archived || !row.state.acknowledged)))
      )
        throw new Error("Inbox row crossed view scope");
      if (
        request.kind === "inbox" &&
        ((request.query.projectId !== null &&
          row.projectId !== request.query.projectId) ||
          (request.query.taskId !== null &&
            row.taskId !== request.query.taskId) ||
          (request.query.kind !== null && row.kind !== request.query.kind))
      )
        throw new Error("Inbox row crossed query scope");
    }
  } else {
    if (
      !shape(p, "revision,entries,nextCursor,summary") ||
      !shape(
        p.summary,
        "records,modelCalls,inputTokens,outputTokens,reservedTokens,unknownCalls,unknownCostRecords,elapsedMilliseconds,costs",
      ) ||
      ![
        "records",
        "modelCalls",
        "inputTokens",
        "outputTokens",
        "reservedTokens",
        "unknownCalls",
        "unknownCostRecords",
        "elapsedMilliseconds",
      ].every((k) => number(p.summary[k])) ||
      !Array.isArray(p.summary.costs) ||
      p.summary.costs.length > 512 ||
      !p.summary.costs.every(cost) ||
      p.summary.records < p.entries.length ||
      p.summary.modelCalls > p.summary.records ||
      p.summary.unknownCostRecords > p.summary.records ||
      p.summary.unknownCalls > p.summary.modelCalls
    )
      throw new Error("Invalid usage totals");
    for (const row of p.entries) {
      if (
        !shape(
          row,
          "sequence,kind,sourceId,projectId,taskId,runId,assignmentId,attemptId,agentId,agentName,provider,model,environment,observedAt,startedAt,updatedAt,status,inputTokens,outputTokens,reservedTokens,elapsedMilliseconds,cost,spendingAllowance",
        ) ||
        !tag(row.kind, kinds) ||
        !["sourceId", "projectId", "taskId"].every((k) =>
          portfolioId(row[k]),
        ) ||
        !["runId", "assignmentId", "attemptId", "agentId"].every((k) =>
          uuid(row[k]),
        ) ||
        !nullable(row.agentName, (v) => label(v, 160)) ||
        !nullable(row.provider, (v) => tag(v, providers)) ||
        !nullable(row.model, (v) => label(v, 1_024)) ||
        !nullable(row.environment, (v) => tag(v, "local,nebiusSandbox")) ||
        !["observedAt", "updatedAt", "reservedTokens"].every((k) =>
          number(row[k]),
        ) ||
        ![
          "startedAt",
          "inputTokens",
          "outputTokens",
          "elapsedMilliseconds",
        ].every((k) => nullable(row[k], number)) ||
        !label(row.status, 160) ||
        !nullable(row.cost, cost) ||
        !nullable(row.spendingAllowance, money)
      )
        throw new Error("Invalid usage row");
      if (
        request.kind === "usage" &&
        [
          "projectId",
          "taskId",
          "agentId",
          "provider",
          "model",
          "environment",
          "kind",
        ].some(
          (k) =>
            request.query[k as keyof UsageLedgerQuery] !== null &&
            row[k] !== request.query[k as keyof UsageLedgerQuery],
        )
      )
        throw new Error("Usage row crossed query scope");
      if (
        request.kind === "usage" &&
        request.query.since !== null &&
        (row.startedAt ?? row.observedAt) < request.query.since
      )
        throw new Error("Usage row crossed time scope");
    }
  }
}

function canonical(value: unknown): string {
  if (Array.isArray(value)) return "[" + value.map(canonical).join(",") + "]";
  if (obj(value))
    return (
      "{" +
      Object.keys(value)
        .sort()
        .map((key) => JSON.stringify(key) + ":" + canonical(value[key]))
        .join(",") +
      "}"
    );
  return JSON.stringify(value);
}
export function validatePortfolioJob(
  value: unknown,
  deviceId: string,
  input: PortfolioSubmission,
): asserts value is PortfolioJob {
  validatePortfolioQuery(input.query);
  if (
    !shape(
      value,
      "id,deviceId,publisherId,idempotencyKey,query,status,result,createdAt,updatedAt,expiresAt",
    ) ||
    !portfolioId(value.id) ||
    value.deviceId !== deviceId ||
    value.publisherId !== input.publisherId ||
    value.idempotencyKey !== input.idempotencyKey ||
    canonical(value.query) !== canonical(input.query) ||
    !tag(value.status, "queued,complete,rejected,expired") ||
    !["createdAt", "updatedAt", "expiresAt"].every((key) =>
      number(value[key]),
    ) ||
    value.updatedAt < value.createdAt ||
    value.expiresAt < value.createdAt
  )
    throw new Error("Portfolio response crossed request scope");
  if (value.status === "complete")
    validatePortfolioResult(value.result, input.query);
  else if (value.result !== null)
    throw new Error("Unexpected pending portfolio data");
}

export function portfolioReadGrant(
  query: PortfolioQuery,
): "inboxRead" | "usageRead" | "automationRead" | "evidenceRead" | null {
  validatePortfolioQuery(query);
  return query.kind === "inbox"
    ? "inboxRead"
    : query.kind === "usage" ||
        query.kind === "cloudBudgets" ||
        isGatewayBilling(query.kind)
      ? "usageRead"
      : query.kind === "workflowHistory" || query.kind === "workflowRun"
        ? "automationRead"
        : query.kind === "workflowEvidence" || query.kind === "workflowDocument"
          ? "evidenceRead"
          : null;
}
export const portfolioDirectoryGrants = [
  "inboxRead",
  "usageRead",
  "automationRead",
  "evidenceRead",
];
