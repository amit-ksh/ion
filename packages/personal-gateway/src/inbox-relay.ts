import { portfolioId } from "../../waku-client/src/gateway-portfolio";
import { openDatabase, type DatabaseSource, type SqlDatabase, type Binding as SQLQueryBindings } from "./database";
import { randomUUID } from "node:crypto";
import {
  validateInboxSubmission,
  inboxSubmission,
  sameInboxSubmission,
  type InboxCommand,
} from "../../waku-client/src/gateway-inbox";
/** Notification state only. A claimed request has a durable owner-side receipt;
 * it cannot approve decisions, invoke agents or mutate a workflow. */
export class DurableInboxRelay {
  private readonly db: SqlDatabase;
  constructor(
    path: DatabaseSource,
    private readonly now: () => number = Date.now,
  ) {
    this.db = openDatabase(path);
    this.db.run("PRAGMA busy_timeout=5000");
    this.db.run("PRAGMA synchronous=FULL");
    this.db.run(
      "CREATE TABLE IF NOT EXISTS relay_inbox_commands(id TEXT PRIMARY KEY,device_id TEXT NOT NULL,publisher_id TEXT NOT NULL,nonce TEXT NOT NULL,status TEXT NOT NULL,expires_at INTEGER NOT NULL,body TEXT NOT NULL,UNIQUE(device_id,nonce))",
    );
    this.db.run(
      "CREATE INDEX IF NOT EXISTS relay_inbox_queue ON relay_inbox_commands(publisher_id,status,expires_at)",
    );
    this.db.run(
      "CREATE INDEX IF NOT EXISTS relay_inbox_expiry ON relay_inbox_commands(status,expires_at)",
    );
  }
  close(): void {
    this.db.close(true);
  }
  private write<T>(fn: () => T): T {
    return this.db.atomic(fn);
  }
  private expireQueued(): void {
    // A queued operation was never claimed. Atomically making its expired
    // deadline terminal lets the phone clear it without waiting for an owner.
    // Claimed operations keep uncertainty until their owner receipt is recovered.
    this.db.run(
      "UPDATE relay_inbox_commands SET status='rejected',body=json_set(body,'$.status','rejected','$.updatedAt',?1) WHERE status='queued' AND expires_at<=?1",
      [this.now()],
    );
  }
  command(id: string): InboxCommand | null {
    this.expireQueued();
    const row = this.db
      .query<{ body: string }, [string]>(
        "SELECT body FROM relay_inbox_commands WHERE id=?1",
      )
      .get(id);
    return row ? (JSON.parse(row.body) as InboxCommand) : null;
  }
  byKey(deviceId: string, key: string): InboxCommand | null {
    this.expireQueued();
    const row = this.db
      .query<{ body: string }, [string, string]>(
        "SELECT body FROM relay_inbox_commands WHERE device_id=?1 AND nonce=?2",
      )
      .get(deviceId, key);
    return row ? (JSON.parse(row.body) as InboxCommand) : null;
  }
  private save(command: InboxCommand): InboxCommand {
    this.db.run(
      "UPDATE relay_inbox_commands SET status=?1,body=?2 WHERE id=?3",
      [command.status, JSON.stringify(command), command.id],
    );
    return command;
  }
  enqueue(deviceId: string, value: unknown): InboxCommand {
    validateInboxSubmission(value);
    if (!portfolioId(deviceId)) throw new Error("Invalid Inbox device");
    return this.write(() => {
      const previous = this.byKey(deviceId, value.idempotencyKey);
      if (previous) {
        if (!sameInboxSubmission(inboxSubmission(previous), value))
          throw new Error("Inbox idempotency key changed");
        return previous;
      }
      const now = this.now();
      if (value.expiresAt <= now || value.expiresAt > now + 120_000)
        throw new Error("Inbox request expired or invalid deadline");
      // The pinned deadline makes old submissions inert even after tombstone pruning.
      this.db.run("DELETE FROM relay_inbox_commands WHERE expires_at<?1", [
        now - 7 * 86400_000,
      ]);
      const count = this.db
        .query<{ count: number }, [string]>(
          "SELECT count(*) AS count FROM relay_inbox_commands WHERE publisher_id=?1",
        )
        .get(value.publisherId)!.count;
      if (count >= 512) throw new Error("Inbox owner receipt queue is full");
      const command: InboxCommand = {
        ...value,
        id: randomUUID(),
        deviceId,
        status: "queued",
        createdAt: now,
        updatedAt: now,
      };
      this.db.run(
        "INSERT INTO relay_inbox_commands VALUES(?1,?2,?3,?4,?5,?6,?7)",
        [
          command.id,
          deviceId,
          value.publisherId,
          value.idempotencyKey,
          command.status,
          value.expiresAt,
          JSON.stringify(command),
        ],
      );
      return command;
    });
  }
  pending(publisherId: string): InboxCommand[] {
    this.expireQueued();
    return this.db
      .query<{ body: string }, [string]>(
        "SELECT body FROM relay_inbox_commands WHERE publisher_id=?1 AND status IN ('queued','uncertain') ORDER BY expires_at,id LIMIT 16",
      )
      .all(publisherId)
      .map((row) => JSON.parse(row.body));
  }
  claim(publisherId: string, id: string, authorized: boolean): InboxCommand {
    return this.write(() => {
      const command = this.command(id);
      if (!command || command.publisherId !== publisherId)
        throw new Error("Unknown Inbox command");
      if (command.status === "applied" || command.status === "rejected")
        return command;
      if (
        !authorized ||
        (command.status === "queued" && command.expiresAt <= this.now())
      )
        return this.save({
          ...command,
          status: "rejected",
          updatedAt: this.now(),
        });
      return this.save({
        ...command,
        status: "uncertain",
        updatedAt: this.now(),
      });
    });
  }
  settle(publisherId: string, id: string, applied: boolean): InboxCommand {
    return this.write(() => {
      const command = this.command(id);
      if (!command || command.publisherId !== publisherId)
        throw new Error("Unknown Inbox command");
      const status = applied ? "applied" : "rejected";
      if (command.status === status) return command;
      if (command.status !== "uncertain")
        throw new Error("Inbox command is not claimed");
      return this.save({ ...command, status, updatedAt: this.now() });
    });
  }
}
