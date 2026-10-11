import { afterEach, expect, test } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { randomUUID } from "node:crypto";
import {
  DurablePortfolioRelay,
  portfolioGrant,
  type PortfolioQuery,
  type PortfolioResult,
} from "./portfolio-relay";
import {
  validatePortfolioQuery,
  validatePortfolioResult,
} from "../../waku-client/src/gateway-portfolio";
const dirs: string[] = [];
afterEach(() => {
  for (const dir of dirs.splice(0))
    rmSync(dir, { recursive: true, force: true });
});
const publisher = randomUUID(),
  phone = randomUUID(),
  other = randomUUID();
const projects: PortfolioQuery = {
  kind: "projects",
  query: { after: null, limit: 32 },
};
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
    since: null,
    after: null,
    limit: 32,
  },
};
const result = (
  now = 100_000,
): Extract<PortfolioResult, { kind: "projects" }> => ({
  kind: "projects",
  page: { projects: [], nextCursor: null, total: 0 },
  pageTruncated: false,
  servedAt: now,
});
function fixture() {
  const dir = mkdtempSync(join(tmpdir(), "ion-portfolio-test-"));
  dirs.push(dir);
  const path = join(dir, "relay.sqlite");
  let now = 100_000;
  const relay = new DurablePortfolioRelay(path, () => now);
  relay.publish(publisher);
  return {
    relay,
    path,
    time: (n: number) => {
      now = n;
    },
    input: () => ({
      publisherId: publisher,
      idempotencyKey: randomUUID(),
      query: structuredClone(projects),
    }),
  };
}
test("read request and immutable first response survive restart and recover lost acknowledgements", () => {
  const f = fixture();
  let relay = f.relay;
  try {
    const input = f.input(),
      job = relay.enqueue(phone, input);
    expect(relay.enqueue(phone, input).id).toBe(job.id);
    relay.close();
    relay = new DurablePortfolioRelay(f.path, () => 100_000);
    expect(relay.pending(publisher)[0]?.id).toBe(job.id);
    const completed = relay.complete(publisher, job.id, result());
    expect(completed.status).toBe("complete");
    expect(relay.pending(publisher)).toEqual([]);
    const changed = result();
    changed.page.total = 12;
    expect(relay.complete(publisher, job.id, changed).result).toEqual(
      completed.result,
    );
    expect(relay.byKey(phone, input.idempotencyKey)?.id).toBe(job.id);
  } finally {
    relay.close();
  }
});
test("publishers, requester nonces and changed query identities remain isolated", () => {
  const f = fixture();
  try {
    f.relay.publish(other);
    const input = f.input(),
      job = f.relay.enqueue(phone, input);
    expect(f.relay.pending(other)).toEqual([]);
    expect(f.relay.byKey(other, input.idempotencyKey)).toBeNull();
    expect(() => f.relay.complete(other, job.id, result())).toThrow(
      "another owner",
    );
    expect(() => f.relay.reject(other, job.id)).toThrow("another owner");
    expect(() =>
      f.relay.enqueue(phone, { ...input, publisherId: other }),
    ).toThrow("identity");
    expect(() => f.relay.enqueue(phone, { ...input, query: inbox })).toThrow(
      "identity",
    );
    expect(f.relay.owners(new Set([publisher]))).toEqual([
      { publisherId: publisher, publishedAt: 100_000 },
    ]);
  } finally {
    f.relay.close();
  }
});
test("expired bodies are withheld, cleared and later pruned and stale owners cannot receive new reads", () => {
  const f = fixture();
  try {
    const input = f.input(),
      job = f.relay.enqueue(phone, input);
    f.relay.complete(publisher, job.id, result());
    f.time(220_000);
    expect(f.relay.job(job.id)?.status).toBe("expired");
    expect(f.relay.job(job.id)?.result).toBeNull();
    expect(() => f.relay.enqueue(phone, f.input())).toThrow("stale");
    f.relay.publish(publisher);
    expect(f.relay.enqueue(phone, input).status).toBe("expired");
    f.time(820_000);
    f.relay.publish(publisher);
    expect(f.relay.byKey(phone, input.idempotencyKey)).toBeNull();
  } finally {
    f.relay.close();
  }
});
test("only typed read queries and bounded pages enter the relay", () => {
  const f = fixture();
  try {
    const input = f.input();
    for (const query of [
      { kind: "stop", runId: randomUUID() },
      { kind: "projects", query: { after: null, limit: 33 } },
      { kind: "projects", query: { after: null, limit: 1, path: "/private" } },
      { ...usage, query: { ...usage.query, provider: "opencode" } },
    ])
      expect(() => validatePortfolioQuery(query)).toThrow();
    expect(() =>
      f.relay.enqueue(phone, { ...input, command: "shell" }),
    ).toThrow();
    expect(portfolioGrant(inbox)).toBe("inboxRead");
    expect(portfolioGrant(usage)).toBe("usageRead");
    expect(portfolioGrant(projects)).toBeNull();
    const job = f.relay.enqueue(phone, input);
    const leaking = { ...result(), privatePath: "/private" };
    expect(() => f.relay.complete(publisher, job.id, leaking)).toThrow();
    expect(f.relay.job(job.id)?.status).toBe("queued");
    expect(() =>
      f.relay.complete(publisher, job.id, { ...result(), servedAt: 131_001 }),
    ).toThrow("time");
  } finally {
    f.relay.close();
  }
});
test("queue and page counts stay bounded without executing or losing another owner read", () => {
  const f = fixture();
  try {
    for (let i = 0; i < 64; i++) f.relay.enqueue(phone, f.input());
    expect(f.relay.pending(publisher)).toHaveLength(16);
    expect(() => f.relay.enqueue(phone, f.input())).toThrow("full");
    f.relay.publish(other);
    expect(
      f.relay.enqueue(randomUUID(), { ...f.input(), publisherId: other })
        .status,
    ).toBe("queued");
  } finally {
    f.relay.close();
  }
});
test("response cursors, view scope, totals, unknown money and source privacy are validated", () => {
  const page: any = {
    kind: "inbox",
    page: {
      revision: 1,
      historyRevision: 3,
      notificationPolicy: "all",
      entries: [
        {
          sequence: 3,
          source: "task",
          sourceId: randomUUID(),
          projectId: null,
          taskId: null,
          runId: null,
          kind: "completed",
          title: "Finished",
          detail: "Task completed",
          detailComplete: true,
          occurredAt: 1000,
          needsAction: false,
          state: {
            key: "ion:v20000000000000003",
            read: false,
            acknowledged: false,
            archived: false,
          },
        },
      ],
      nextCursor: null,
      matchingCount: 50,
      unreadCount: 25,
      needsActionCount: 2,
      taskHistorySyncing: false,
    },
    pageTruncated: false,
    servedAt: 100_000,
  };
  validatePortfolioResult(page, inbox);
  if (page.kind !== "inbox") throw new Error("Expected Inbox page");
  const wrong: any = structuredClone(page);
  wrong.page.entries[0].path = "/private";
  expect(() => validatePortfolioResult(wrong, inbox)).toThrow();
  const bad = structuredClone(page);
  bad.page.nextCursor = 4;
  expect(() => validatePortfolioResult(bad, inbox)).toThrow("cursor");
  expect(() =>
    validatePortfolioResult(page, {
      kind: "inbox",
      query: { ...inbox.query, status: "needsAction" },
    }),
  ).toThrow("scope");
  const totals: any = {
    kind: "usage",
    page: {
      revision: 1,
      entries: [],
      nextCursor: null,
      summary: {
        records: 40,
        modelCalls: 30,
        inputTokens: 52,
        outputTokens: 15,
        reservedTokens: 0,
        unknownCalls: 4,
        unknownCostRecords: 40,
        elapsedMilliseconds: 0,
        costs: [],
      },
    },
    pageTruncated: false,
    servedAt: 100_000,
  };
  validatePortfolioResult(totals, usage);
  if (totals.kind !== "usage") throw new Error("Expected usage page");
  totals.page.summary.unknownCostRecords = 41;
  expect(() => validatePortfolioResult(totals, usage)).toThrow("totals");
});
