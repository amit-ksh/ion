import { openDatabase, type DatabaseSource, type SqlDatabase, type Binding as SQLQueryBindings } from "./database";
import { portfolioId } from "../../waku-client/src/gateway-portfolio";
import {
  validateNotificationBatch,
  type GatewayNotificationBatch,
  type PushTopic,
} from "../../waku-client/src/gateway-notifications";
import type { DurableGatewayAuthority } from "./durable-authority";

const tokenPattern = /^(?:Expo|Exponent)PushToken\[[A-Za-z0-9_-]{20,256}\]$/;
const sendUrl = "https://exp.host/--/api/v2/push/send";
const receiptsUrl = "https://exp.host/--/api/v2/push/getReceipts";

type Ticket = {
  status: "ok" | "error";
  id?: string;
  details?: { error?: string };
};

/** Explicit phone opt-ins. New Inbox alerts send a fixed message and scope only;
 * producer/device/event identities stay in the gateway database. */
export class GatewayPush {
  private readonly db: SqlDatabase;
  private readonly timer?: ReturnType<typeof setInterval>;
  private busy = false;
  private writing = false;

  constructor(
    path: DatabaseSource,
    private readonly authority: DurableGatewayAuthority,
    private readonly send: typeof fetch = fetch,
    private readonly now: () => number = Date.now,
    periodic = true,
  ) {
    this.db = openDatabase(path);
    this.db.run("PRAGMA busy_timeout = 5000");
    this.db.run("PRAGMA synchronous = FULL");
    this.db.run(
      "CREATE TABLE IF NOT EXISTS gateway_push_subscriptions (device_id TEXT PRIMARY KEY, token TEXT NOT NULL)",
    );
    this.db.run(
      "CREATE TABLE IF NOT EXISTS gateway_push_jobs (id INTEGER PRIMARY KEY AUTOINCREMENT, device_id TEXT NOT NULL, event_key TEXT NOT NULL, task_id TEXT NOT NULL, status TEXT NOT NULL, ticket_id TEXT, submitted_at INTEGER, UNIQUE(device_id, event_key))",
    );
    const subscriptionColumns = new Set(
      this.all<{ name: string }>(
        "PRAGMA table_info(gateway_push_subscriptions)",
      ).map((r) => r.name),
    );
    if (!subscriptionColumns.has("task_enabled"))
      this.db.run(
        "ALTER TABLE gateway_push_subscriptions ADD COLUMN task_enabled INTEGER NOT NULL DEFAULT 1",
      );
    if (!subscriptionColumns.has("inbox_enabled"))
      this.db.run(
        "ALTER TABLE gateway_push_subscriptions ADD COLUMN inbox_enabled INTEGER NOT NULL DEFAULT 0",
      );
    const jobColumns = new Set(
      this.all<{ name: string }>("PRAGMA table_info(gateway_push_jobs)").map(
        (r) => r.name,
      ),
    );
    for (const [name, definition] of [
      ["topic", "TEXT NOT NULL DEFAULT 'taskRequests'"],
      ["publisher_id", "TEXT"],
      ["sequence", "INTEGER"],
      ["attention", "INTEGER"],
      ["expires_at", "INTEGER"],
      ["ticket_token", "TEXT"],
    ]) {
      if (!jobColumns.has(name!))
        this.db.run(
          `ALTER TABLE gateway_push_jobs ADD COLUMN ${name} ${definition}`,
        );
    }
    this.db.run(
      "CREATE TABLE IF NOT EXISTS gateway_inbox_push_publishers (publisher_id TEXT PRIMARY KEY,cursor INTEGER NOT NULL,last_batch TEXT NOT NULL,policy TEXT NOT NULL)",
    );
    this.db.run(
      "CREATE INDEX IF NOT EXISTS gateway_push_pending ON gateway_push_jobs(status,id)",
    );
    this.db.run(
      "CREATE INDEX IF NOT EXISTS gateway_push_expiry ON gateway_push_jobs(expires_at)",
    );
    this.db.run(
      "UPDATE gateway_push_jobs SET expires_at=?1 WHERE expires_at IS NULL",
      [this.now() + 15 * 60_000],
    );
    if (periodic) { this.timer = setInterval(() => void this.flush(), 5_000); this.timer.unref?.(); }
  }

  async close(): Promise<void> {
    clearInterval(this.timer);
    while (this.busy) await new Promise((resolve) => setTimeout(resolve, 20));
    this.db.close(true);
  }

  private write<T>(operation: () => T): T {
    if (this.writing) return operation();
    return this.db.atomic(() => {
      this.writing = true;
      try { return operation(); } finally { this.writing = false; }
    });
  }
  private all<T>(sql: string, values: SQLQueryBindings[] = []): T[] {
    const q = this.db.prepare<T, SQLQueryBindings[]>(sql);
    try {
      return q.all(...values);
    } finally {
      q.finalize();
    }
  }
  private one<T>(sql: string, values: SQLQueryBindings[] = []): T | null {
    const q = this.db.prepare<T, SQLQueryBindings[]>(sql);
    try {
      return q.get(...values);
    } finally {
      q.finalize();
    }
  }
  register(
    deviceId: string,
    token: string,
    topic: PushTopic = "taskRequests",
  ): void {
    if (!tokenPattern.test(token)) throw new Error("Invalid Expo push token");
    this.write(() => {
      this.db.run(
        "INSERT INTO gateway_push_subscriptions(device_id,token,task_enabled,inbox_enabled) VALUES(?1,?2,0,0) ON CONFLICT(device_id) DO UPDATE SET token=excluded.token",
        [deviceId, token],
      );
      this.db.run(
        topic === "inbox"
          ? "UPDATE gateway_push_subscriptions SET inbox_enabled=1 WHERE device_id=?1"
          : "UPDATE gateway_push_subscriptions SET task_enabled=1 WHERE device_id=?1",
        [deviceId],
      );
    });
  }
  unregister(deviceId: string, topic?: PushTopic): void {
    this.write(() => {
      if (topic)
        this.db.run(
          topic === "inbox"
            ? "UPDATE gateway_push_subscriptions SET inbox_enabled=0 WHERE device_id=?1"
            : "UPDATE gateway_push_subscriptions SET task_enabled=0 WHERE device_id=?1",
          [deviceId],
        );
      else
        this.db.run(
          "DELETE FROM gateway_push_subscriptions WHERE device_id=?1",
          [deviceId],
        );
      this.db.run(
        "UPDATE gateway_push_jobs SET status='cancelled' WHERE device_id=?1 AND status='queued' AND (?2 IS NULL OR topic=?2)",
        [deviceId, topic ?? null],
      );
      this.db.run(
        "DELETE FROM gateway_push_subscriptions WHERE device_id=?1 AND task_enabled=0 AND inbox_enabled=0",
        [deviceId],
      );
    });
  }
  isRegistered(deviceId: string, topic: PushTopic = "taskRequests"): boolean {
    return !!this.one(
      topic === "inbox"
        ? "SELECT 1 FROM gateway_push_subscriptions WHERE device_id=?1 AND inbox_enabled=1"
        : "SELECT 1 FROM gateway_push_subscriptions WHERE device_id=?1 AND task_enabled=1",
      [deviceId],
    );
  }
  publicationCursor(publisherId: string): number | null {
    if (!portfolioId(publisherId)) throw new Error("Invalid publisher");
    return (
      this.one<{ cursor: number }>(
        "SELECT cursor FROM gateway_inbox_push_publishers WHERE publisher_id=?1",
        [publisherId],
      )?.cursor ?? null
    );
  }
  publishInbox(publisherId: string, value: unknown): { next: number } {
    if (!portfolioId(publisherId)) throw new Error("Invalid publisher");
    validateNotificationBatch(value);
    const batch: GatewayNotificationBatch = value;
    const signature = JSON.stringify({
      after: batch.after,
      next: batch.next,
      policy: batch.policy,
      events: batch.events.map((e) => ({
        sequence: e.sequence,
        needsAttention: e.needsAttention,
      })),
    });
    const result = this.write(() => {
      this.prune();
      const previous = this.one<{ cursor: number; last_batch: string }>(
        "SELECT cursor,last_batch FROM gateway_inbox_push_publishers WHERE publisher_id=?1",
        [publisherId],
      );
      if (previous?.last_batch === signature) return { next: previous.cursor };
      if ((previous?.cursor ?? null) !== batch.after)
        throw new Error(
          "Inbox publication cursor changed; refresh before publishing",
        );
      if (batch.policy !== "all")
        this.db.run(
          "UPDATE gateway_push_jobs SET status='cancelled' WHERE publisher_id=?1 AND topic='inbox' AND status='queued' AND (?2='off' OR attention=0)",
          [publisherId, batch.policy],
        );
      if (batch.events.length) {
        const attention = batch.events.some((e) => e.needsAttention),
          sequence = batch.events.at(-1)!.sequence;
        const allowed = new Set(
          this.authority
            .listDevices()
            .filter(
              (d) =>
                !d.revoked &&
                d.expiresAt > this.now() &&
                d.grants.includes("inboxRead"),
            )
            .map((d) => d.id),
        );
        for (const { device_id } of this.all<{ device_id: string }>(
          "SELECT device_id FROM gateway_push_subscriptions WHERE inbox_enabled=1",
        )) {
          if (!allowed.has(device_id)) {
            this.unregister(device_id, "inbox");
            continue;
          }
          const carried = this.one<{ attention: number }>(
            "SELECT coalesce(max(attention),0) AS attention FROM gateway_push_jobs WHERE device_id=?1 AND publisher_id=?2 AND topic='inbox' AND status='queued'",
            [device_id, publisherId],
          )!.attention;
          this.db.run(
            "UPDATE gateway_push_jobs SET status='superseded' WHERE device_id=?1 AND publisher_id=?2 AND topic='inbox' AND status='queued'",
            [device_id, publisherId],
          );
          const pending = this.one<{ n: number }>(
            "SELECT count(*) AS n FROM gateway_push_jobs WHERE status='queued'",
          )!.n;
          if (pending >= 4096) throw new Error("Push queue capacity reached");
          this.db.run(
            "INSERT OR IGNORE INTO gateway_push_jobs(device_id,event_key,task_id,status,topic,publisher_id,sequence,attention,expires_at) VALUES(?1,?2,'','queued','inbox',?3,?4,?5,?6)",
            [
              device_id,
              `inbox:${publisherId}:${sequence}`,
              publisherId,
              sequence,
              Number(attention || carried > 0),
              this.now() + 15 * 60_000,
            ],
          );
        }
      }
      this.db.run(
        "INSERT INTO gateway_inbox_push_publishers VALUES(?1,?2,?3,?4) ON CONFLICT(publisher_id) DO UPDATE SET cursor=excluded.cursor,last_batch=excluded.last_batch,policy=excluded.policy",
        [publisherId, batch.next, signature, batch.policy],
      );
      return { next: batch.next };
    });
    if (batch.events.length) void this.flush();
    return result;
  }
  private prune(): void {
    this.db.run(
      "UPDATE gateway_push_jobs SET status='expired' WHERE status='queued' AND expires_at<=?1",
      [this.now()],
    );
    this.db.run(
      "DELETE FROM gateway_push_jobs WHERE status NOT IN ('queued','submitted') AND expires_at<?1",
      [this.now() - 7 * 86400_000],
    );
  }

  enqueue(taskId: string, eventKey: string): void {
    if (!/^[0-9a-f-]{36}$/.test(taskId) || !eventKey || eventKey.length > 200)
      return;
    const allowed = new Set(
      this.authority
        .listDevices()
        .filter(
          (device) =>
            !device.revoked &&
            device.expiresAt > this.now() &&
            device.grants.includes("taskRead"),
        )
        .map((device) => device.id),
    );
    const rows = this.all<{ device_id: string }>(
      "SELECT device_id FROM gateway_push_subscriptions WHERE task_enabled=1",
    );
    for (const { device_id } of rows) {
      if (!allowed.has(device_id)) {
        this.unregister(device_id, "taskRequests");
        continue;
      }
      this.db.run(
        "INSERT OR IGNORE INTO gateway_push_jobs (device_id, event_key, task_id, status,expires_at) VALUES (?1, ?2, ?3, 'queued',?4)",
        [device_id, eventKey, taskId, this.now() + 15 * 60_000],
      );
    }
    void this.flush();
  }

  /** No recurring timer is needed once all jobs have reached a terminal state. */
  hasPending(): boolean {
    return !!this.one<{ n: number }>("SELECT count(*) AS n FROM gateway_push_jobs WHERE status IN ('queued','submitted')")?.n;
  }
  async flush(): Promise<void> {
    if (this.busy) return;
    this.busy = true;
    try {
      this.prune();
      const jobs = this.all<{
        id: number;
        device_id: string;
        task_id: string;
        token: string;
        topic: PushTopic;
        publisher_id: string | null;
        sequence: number | null;
        attention: number | null;
      }>(
        "SELECT j.id,j.device_id,j.task_id,s.token,j.topic,j.publisher_id,j.sequence,j.attention FROM gateway_push_jobs j JOIN gateway_push_subscriptions s ON s.device_id=j.device_id WHERE j.status='queued' AND ((j.topic='taskRequests' AND s.task_enabled=1) OR (j.topic='inbox' AND s.inbox_enabled=1)) ORDER BY j.id LIMIT 100",
      );
      if (jobs.length) {
        const devices = new Map(
          this.authority.listDevices().map((d) => [d.id, d]),
        );
        const sending = jobs.filter((job) => {
          const device = devices.get(job.device_id),
            grant = job.topic === "inbox" ? "inboxRead" : "taskRead";
          if (
            !device ||
            device.revoked ||
            device.expiresAt <= this.now() ||
            !device.grants.includes(grant)
          ) {
            this.unregister(job.device_id, job.topic);
            return false;
          }
          if (job.topic === "inbox") {
            const publisher = devices.get(job.publisher_id ?? "");
            const policy = this.one<{ policy: string }>(
              "SELECT policy FROM gateway_inbox_push_publishers WHERE publisher_id=?1",
              [job.publisher_id ?? ""],
            )?.policy;
            if (
              !publisher ||
              publisher.revoked ||
              publisher.expiresAt <= this.now() ||
              !publisher.grants.includes("publish") ||
              policy === "off" ||
              (policy === "issuesOnly" && !job.attention)
            ) {
              this.db.run(
                "UPDATE gateway_push_jobs SET status='cancelled' WHERE id=?1",
                [job.id],
              );
              return false;
            }
          }
          return true;
        });
        // Mark before network I/O: a lost response cannot send the same alert twice.
        for (const job of sending)
          this.db.run(
            "UPDATE gateway_push_jobs SET status = 'uncertain' WHERE id = ?1",
            [job.id],
          );
        if (sending.length) {
          try {
            const response = await this.send(sendUrl, {
              method: "POST",
              headers: {
                "Content-Type": "application/json",
                Accept: "application/json",
              },
              body: JSON.stringify(
                sending.map((job) => ({
                  to: job.token,
                  title:
                    job.topic === "inbox"
                      ? job.attention
                        ? "Inbox needs attention"
                        : "Inbox updated"
                      : "Task needs input",
                  body:
                    job.topic === "inbox"
                      ? "Open ion to view your Inbox."
                      : "Open ion to review the pending request.",
                  // Generic Inbox pushes carry no internal identities or user content to Expo.
                  data:
                    job.topic === "inbox"
                      ? { version: 1, scope: "inbox" }
                      : { taskId: job.task_id },
                  channelId: job.topic === "inbox" ? "inbox" : "tasks",
                  sound: "default",
                })),
              ),
              signal: AbortSignal.timeout(8_000),
            });
            if (response.ok) {
              const result = (await response.json()) as {
                data?: Ticket[] | Ticket;
              };
              const tickets = Array.isArray(result.data)
                ? result.data
                : [result.data];
              sending.forEach((job, index) => {
                const ticket = tickets[index];
                if (ticket?.status === "ok" && ticket.id) {
                  this.db.run(
                    "UPDATE gateway_push_jobs SET status = 'submitted', ticket_id = ?2, submitted_at = ?3, ticket_token=?4 WHERE id = ?1",
                    [job.id, ticket.id, this.now(), job.token],
                  );
                } else if (ticket?.status === "error") {
                  this.db.run(
                    "UPDATE gateway_push_jobs SET status = 'failed' WHERE id = ?1",
                    [job.id],
                  );
                  if (ticket.details?.error === "DeviceNotRegistered")
                    this.unregisterToken(job.device_id, job.token);
                }
              });
            }
          } catch {
            /* An uncertain send is never replayed. */
          }
        }
      }
      await this.checkReceipts();
    } finally {
      this.busy = false;
    }
  }

  private unregisterToken(deviceId: string, token: string): void {
    if (
      this.one<{ token: string }>(
        "SELECT token FROM gateway_push_subscriptions WHERE device_id=?1",
        [deviceId],
      )?.token === token
    )
      this.unregister(deviceId);
  }
  private async checkReceipts(): Promise<void> {
    this.db.run(
      "UPDATE gateway_push_jobs SET status='receiptExpired' WHERE status='submitted' AND submitted_at<?1",
      [this.now() - 86400_000],
    );
    const due = this.all<{
      id: number;
      device_id: string;
      ticket_id: string;
      ticket_token: string;
    }>(
      "SELECT id, device_id, ticket_id,ticket_token FROM gateway_push_jobs WHERE status = 'submitted' AND submitted_at <= ?1 ORDER BY id LIMIT 100",
      [this.now() - 15 * 60_000],
    );
    if (!due.length) return;
    try {
      const response = await this.send(receiptsUrl, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          Accept: "application/json",
        },
        body: JSON.stringify({ ids: due.map((job) => job.ticket_id) }),
        signal: AbortSignal.timeout(8_000),
      });
      if (!response.ok) return;
      const result = (await response.json()) as {
        data?: Record<string, Ticket>;
      };
      for (const job of due) {
        const receipt = result.data?.[job.ticket_id];
        if (!receipt) continue;
        this.db.run("UPDATE gateway_push_jobs SET status = ?2 WHERE id = ?1", [
          job.id,
          receipt.status === "ok" ? "delivered" : "failed",
        ]);
        if (receipt.details?.error === "DeviceNotRegistered")
          this.unregisterToken(job.device_id, job.ticket_token);
      }
    } catch {
      /* Check again after the next interval. */
    }
  }
}
