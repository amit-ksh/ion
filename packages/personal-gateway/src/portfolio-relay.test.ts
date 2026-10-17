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

test("two maximum PDF pages retain bounded image receipts and compact nonce identity before expiry", () => {
  const f = fixture();
  let relay = f.relay;
  try {
    const expected = {
      id: randomUUID(),
      name: "Report.pdf",
      format: "pdf",
      projectId: randomUUID(),
      taskId: null,
      stage: 0,
      teamRunId: randomUUID(),
      assignmentId: randomUUID(),
      bytes: 100,
      sha256: "a".repeat(64),
      createdAt: 100,
    };
    const runId = randomUUID(),
      transferId = randomUUID();
    let original: ReturnType<typeof f.input> | null = null;
    for (let page = 0; page < 2; page++)
      for (let offset = 0; offset < 12 * 1024 * 1024; offset += 32768) {
        const query: PortfolioQuery = {
          kind: "workflowPdfPage",
          query: {
            runId,
            revision: 7,
            expected,
            page,
            width: 800,
            transferId: offset ? transferId : null,
            offset,
            limit: 32768,
          },
        };
        const input = {
          publisherId: publisher,
          idempotencyKey: randomUUID(),
          query,
        };
        if (!original) original = input;
        f.time(100000 + page * 384 + offset / 32768);
        relay.publish(publisher);
        const job = relay.enqueue(phone, input);
        const end = offset + 32768;
        relay.complete(publisher, job.id, {
          kind: "workflowPdfPage",
          page: {
            runId,
            revision: 7,
            document: expected,
            transferId,
            page,
            pages: 2,
            requestedWidth: 800,
            width: 800,
            height: 1000,
            bytes: 12 * 1024 * 1024,
            sha256: "b".repeat(64),
            expiresAt: 700000,
            offset,
            nextOffset: end < 12 * 1024 * 1024 ? end : null,
            data: Buffer.alloc(32768).toString("base64"),
          },
          pageTruncated: false,
          servedAt: 100000 + page * 384 + offset / 32768,
        });
      }
    const db = new (require("bun:sqlite").Database)(f.path);
    expect(
      db.query("SELECT count(*) AS n FROM relay_portfolio_queries").get().n,
    ).toBeLessThanOrEqual(65);
    expect(
      db.query("SELECT count(*) AS n FROM relay_pdf_tombstones").get().n,
    ).toBeLessThanOrEqual(704);
    db.close();
    relay.close();
    relay = new DurablePortfolioRelay(f.path, () => 100800);
    relay.publish(publisher);
    expect(() =>
      relay.enqueue(phone, {
        ...original!,
        query: {
          ...original!.query,
          query: { ...(original!.query as any).query, width: 1200 },
        },
      }),
    ).toThrow(/identity/);
    expect(() => relay.enqueue(phone, original!)).not.toThrow();
  } finally {
    relay.close();
  }
});

test("PDF transport WAL retains FULL durability and immutable receipts across close/reopen and rejected completion", () => {
  const f = fixture();
  let relay = f.relay;
  try {
    const db = (relay as any).db;
    expect(db.query("PRAGMA journal_mode").get().journal_mode).toBe("wal");
    expect(db.query("PRAGMA synchronous").get().synchronous).toBe(2);
    expect(db.query("PRAGMA busy_timeout").get().timeout).toBe(5000);
    const input = f.input(),
      job = relay.enqueue(phone, input);
    expect(() =>
      relay.complete(publisher, job.id, { ...result(), path: "/private" }),
    ).toThrow();
    expect(relay.job(job.id)?.status).toBe("queued");
    relay.complete(publisher, job.id, result());
    relay.close();
    relay = new DurablePortfolioRelay(f.path, () => 100001);
    expect(relay.byKey(phone, input.idempotencyKey)?.result).toEqual(result());
    expect(() => relay.enqueue(phone, { ...input, query: inbox })).toThrow(
      /identity/,
    );
  } finally {
    relay.close();
  }
});

test("a maximum HTML original and resource set cannot exhaust the relay catalogue", () => {
  const f = fixture();
  let relay = f.relay;
  try {
    const runId = randomUUID(),
      artifactId = randomUUID();
    const document = {
      id: artifactId,
      name: "Report.html",
      format: "html",
      projectId: randomUUID(),
      taskId: null,
      stage: 0,
      teamRunId: randomUUID(),
      assignmentId: randomUUID(),
      bytes: 16 * 1024 * 1024,
      sha256: "a".repeat(64),
      createdAt: 100,
    };
    let original: any = null;
    // 1024 original chunks plus 576 retained-source chunks under one live lease.
    for (let index = 0; index < 1600; index++) {
      const source = index >= 1024,
        offset = (source ? index - 1024 : index) * 16384;
      const query: PortfolioQuery = source
        ? {
            kind: "workflowHtmlSource",
            query: {
              runId,
              artifactId,
              sha256: document.sha256,
              source: {
                manifestSha256: "b".repeat(64),
                sourceIndex: 1,
                sourceSha256: "c".repeat(64),
                offset: offset % (2 * 1024 * 1024),
                limit: 16384,
              },
            },
          }
        : {
            kind: "workflowDocument",
            query: {
              runId,
              artifactId,
              sha256: document.sha256,
              offset,
              limit: 16384,
            },
          };
      const input = {
        publisherId: publisher,
        idempotencyKey: randomUUID(),
        query,
      };
      if (!original) original = input;
      f.time(100000 + index);
      relay.publish(publisher);
      const job = relay.enqueue(phone, input);
      const result: PortfolioResult = source
        ? {
            kind: "workflowHtmlSource",
            page: {
              runId,
              document,
              chunk: {
                artifactId,
                artifactSha256: document.sha256,
                manifestSha256: "b".repeat(64),
                sourceIndex: 1,
                sourceSha256: "c".repeat(64),
                offset: offset % (2 * 1024 * 1024),
                nextOffset:
                  (offset % (2 * 1024 * 1024)) + 16384 < 2 * 1024 * 1024
                    ? (offset % (2 * 1024 * 1024)) + 16384
                    : null,
                data: Buffer.alloc(16384).toString("base64"),
              },
            },
            pageTruncated: false,
            servedAt: 100000 + index,
          }
        : {
            kind: "workflowDocument",
            page: {
              runId,
              document,
              offset,
              nextOffset:
                offset + 16384 < document.bytes ? offset + 16384 : null,
              data: Buffer.alloc(16384).toString("base64"),
            },
            pageTruncated: false,
            servedAt: 100000 + index,
          };
      relay.complete(publisher, job.id, result);
    }
    const db = new (require("bun:sqlite").Database)(f.path);
    expect(
      db.query("SELECT count(*) AS n FROM relay_portfolio_queries").get().n,
    ).toBeLessThanOrEqual(65);
    expect(
      db.query("SELECT count(*) AS n FROM relay_pdf_tombstones").get().n,
    ).toBeLessThanOrEqual(1536);
    db.close();
    relay.close();
    relay = new DurablePortfolioRelay(f.path, () => 102000);
    relay.publish(publisher);
    expect(() =>
      relay.enqueue(phone, {
        ...original,
        query: {
          ...original.query,
          query: { ...original.query.query, sha256: "d".repeat(64) },
        },
      }),
    ).toThrow("identity changed");
    expect(relay.enqueue(phone, original).status).toBe("queued");
  } finally {
    relay.close();
  }
}, 30000);
