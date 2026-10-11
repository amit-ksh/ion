import { Database } from "bun:sqlite";
import { randomUUID } from "node:crypto";
import {
  portfolioId,
  portfolioReadGrant,
  validatePortfolioQuery,
  validatePortfolioResult,
  type PortfolioSubmission,
  type PortfolioJob,
  type PortfolioQuery,
  type PortfolioResult,
} from "../../waku-client/src/gateway-portfolio";
export type {
  PortfolioSubmission,
  PortfolioJob,
  PortfolioQuery,
  PortfolioResult,
} from "../../waku-client/src/gateway-portfolio";

export const portfolioGrant = portfolioReadGrant;

function object(value: unknown): value is Record<string, any> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}
function exact(value: unknown, names: string): boolean {
  return (
    object(value) &&
    Object.keys(value).sort().join(",") === names.split(",").sort().join(",")
  );
}
function canonical(value: unknown): string {
  if (Array.isArray(value)) return "[" + value.map(canonical).join(",") + "]";
  if (object(value))
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

/** Read-only owner queries. They cannot contain an agent, app or notification
 * mutation. Retrying a lost owner answer reads state; it never executes work. */
export class DurablePortfolioRelay {
  private readonly db: Database;
  constructor(
    path: string,
    private readonly now: () => number = Date.now,
  ) {
    this.db = new Database(path, { create: true, strict: true });
    this.db.run("PRAGMA busy_timeout=5000");
    this.db.run("PRAGMA synchronous=FULL");
    this.db.run(
      "CREATE TABLE IF NOT EXISTS relay_portfolio_owners(publisher_id TEXT PRIMARY KEY,published_at INTEGER NOT NULL)",
    );
    this.db.run(
      "CREATE TABLE IF NOT EXISTS relay_portfolio_queries(id TEXT PRIMARY KEY,device_id TEXT NOT NULL,publisher_id TEXT NOT NULL,nonce TEXT NOT NULL,query TEXT NOT NULL,status TEXT NOT NULL,result TEXT,created_at INTEGER NOT NULL,updated_at INTEGER NOT NULL,expires_at INTEGER NOT NULL,UNIQUE(device_id,nonce))",
    );
    this.db.run(
      "CREATE INDEX IF NOT EXISTS relay_portfolio_queue ON relay_portfolio_queries(publisher_id,status,created_at,id)",
    );
    this.db.run(
      "CREATE INDEX IF NOT EXISTS relay_portfolio_device_queue ON relay_portfolio_queries(device_id,status)",
    );
    this.db.run(
      "CREATE INDEX IF NOT EXISTS relay_portfolio_expiry ON relay_portfolio_queries(expires_at)",
    );
  }
  close(): void {
    this.db.close(true);
  }
  private write<T>(fn: () => T): T {
    this.db.run("BEGIN IMMEDIATE");
    try {
      const result = fn();
      this.db.run("COMMIT");
      return result;
    } catch (error) {
      this.db.run("ROLLBACK");
      throw error;
    }
  }
  private prune(): void {
    const time = this.now();
    // Remove response bodies promptly even if the requesting device disappeared.
    this.db.run(
      "UPDATE relay_portfolio_queries SET status='expired',result=NULL,updated_at=?1 WHERE expires_at<=?1 AND status!='expired'",
      [time],
    );
    this.db.run("DELETE FROM relay_portfolio_queries WHERE expires_at<=?1", [
      time - 600_000,
    ]);
    this.db.run("DELETE FROM relay_portfolio_owners WHERE published_at<=?1", [
      time - 86_400_000,
    ]);
  }
  publish(publisherId: string): void {
    if (!portfolioId(publisherId)) throw new Error("Invalid portfolio owner");
    this.prune();
    this.db.run(
      "INSERT INTO relay_portfolio_owners VALUES(?1,?2) ON CONFLICT(publisher_id) DO UPDATE SET published_at=excluded.published_at",
      [publisherId, this.now()],
    );
  }
  owners(
    allowed: Set<string>,
  ): Array<{ publisherId: string; publishedAt: number }> {
    return [...allowed]
      .sort()
      .slice(0, 64)
      .flatMap((publisherId) => {
        const row = this.db
          .query<{ published_at: number }, [string]>(
            "SELECT published_at FROM relay_portfolio_owners WHERE publisher_id=?1",
          )
          .get(publisherId);
        return row ? [{ publisherId, publishedAt: row.published_at }] : [];
      });
  }
  private fresh(publisherId: string): boolean {
    const row = this.db
      .query<{ published_at: number }, [string]>(
        "SELECT published_at FROM relay_portfolio_owners WHERE publisher_id=?1",
      )
      .get(publisherId);
    return (
      !!row &&
      row.published_at <= this.now() &&
      this.now() - row.published_at <= 60_000
    );
  }
  enqueue(deviceId: string, value: unknown): PortfolioJob {
    if (
      !exact(value, "publisherId,idempotencyKey,query") ||
      !object(value) ||
      !portfolioId(deviceId) ||
      !portfolioId(value.publisherId) ||
      !portfolioId(value.idempotencyKey)
    )
      throw new Error("Invalid portfolio submission");
    validatePortfolioQuery(value.query);
    if (Buffer.byteLength(JSON.stringify(value)) > 4_000)
      throw new Error("Portfolio query too large");
    const input = value as unknown as PortfolioSubmission;
    return this.write(() => {
      this.prune();
      const existing = this.byKey(deviceId, input.idempotencyKey);
      if (existing) {
        if (
          existing.publisherId !== input.publisherId ||
          canonical(existing.query) !== canonical(input.query)
        )
          throw new Error("Portfolio request identity changed");
        return existing;
      }
      if (!this.fresh(input.publisherId))
        throw new Error("Registered owner is offline or stale");
      const count = this.db
        .query<{ count: number }, [string, string]>(
          "SELECT count(*) AS count FROM relay_portfolio_queries WHERE status='queued' AND (device_id=?1 OR publisher_id=?2)",
        )
        .get(deviceId, input.publisherId)!.count;
      const stored = this.db
        .query<{ count: number }, [string]>(
          "SELECT count(*) AS count FROM relay_portfolio_queries WHERE publisher_id=?1",
        )
        .get(input.publisherId)!.count;
      if (count >= 64 || stored >= 512)
        throw new Error("Owner read queue is full");
      const time = this.now(),
        id = randomUUID();
      this.db.run(
        "INSERT INTO relay_portfolio_queries VALUES(?1,?2,?3,?4,?5,'queued',NULL,?6,?6,?7)",
        [
          id,
          deviceId,
          input.publisherId,
          input.idempotencyKey,
          JSON.stringify(input.query),
          time,
          time + 120_000,
        ],
      );
      return this.job(id)!;
    });
  }
  job(id: string): PortfolioJob | null {
    if (!portfolioId(id)) throw new Error("Invalid portfolio request ID");
    const row = this.db
      .query<
        {
          id: string;
          device_id: string;
          publisher_id: string;
          nonce: string;
          query: string;
          status: PortfolioJob["status"];
          result: string | null;
          created_at: number;
          updated_at: number;
          expires_at: number;
        },
        [string]
      >("SELECT * FROM relay_portfolio_queries WHERE id=?1")
      .get(id);
    if (!row) return null;
    const expired = row.expires_at <= this.now();
    return {
      id: row.id,
      deviceId: row.device_id,
      publisherId: row.publisher_id,
      idempotencyKey: row.nonce,
      query: JSON.parse(row.query),
      status: expired ? "expired" : row.status,
      result: expired || row.result === null ? null : JSON.parse(row.result),
      createdAt: row.created_at,
      updatedAt: row.updated_at,
      expiresAt: row.expires_at,
    };
  }
  byKey(deviceId: string, key: string): PortfolioJob | null {
    if (!portfolioId(key)) throw new Error("Invalid portfolio request key");
    const row = this.db
      .query<{ id: string }, [string, string]>(
        "SELECT id FROM relay_portfolio_queries WHERE device_id=?1 AND nonce=?2",
      )
      .get(deviceId, key);
    return row ? this.job(row.id) : null;
  }
  pending(publisherId: string): PortfolioJob[] {
    this.prune();
    return this.db
      .query<{ id: string }, [string]>(
        "SELECT id FROM relay_portfolio_queries WHERE publisher_id=?1 AND status='queued' ORDER BY created_at,id LIMIT 16",
      )
      .all(publisherId)
      .map((row) => this.job(row.id)!);
  }
  reject(publisherId: string, id: string): PortfolioJob {
    const job = this.job(id);
    if (!job || job.publisherId !== publisherId)
      throw new Error("Portfolio request belongs to another owner");
    if (job.status === "queued")
      this.db.run(
        "UPDATE relay_portfolio_queries SET status='rejected',updated_at=?1 WHERE id=?2 AND status='queued'",
        [this.now(), id],
      );
    return this.job(id)!;
  }
  complete(publisherId: string, id: string, value: unknown): PortfolioJob {
    return this.write(() => {
      this.prune();
      const job = this.job(id);
      if (!job || job.publisherId !== publisherId)
        throw new Error("Portfolio request belongs to another owner");
      // A read response is immutable once stored; a repeated network response
      // cannot replace the original snapshot, even if owner state has moved on.
      if (job.status === "complete") return job;
      if (job.status !== "queued")
        throw new Error("Portfolio request is no longer pending");
      validatePortfolioResult(value, job.query);
      if (
        value.servedAt > this.now() + 30_000 ||
        this.now() - value.servedAt > 120_000
      )
        throw new Error("Invalid owner response time");
      this.db.run(
        "UPDATE relay_portfolio_queries SET status='complete',result=?1,updated_at=?2 WHERE id=?3 AND status='queued'",
        [JSON.stringify(value), this.now(), id],
      );
      return this.job(id)!;
    });
  }
}
