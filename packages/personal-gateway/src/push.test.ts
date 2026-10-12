import { afterEach, test, expect } from "bun:test";
import { Database } from "bun:sqlite";
import { mkdtempSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { DurableGatewayAuthority } from "./durable-authority";
import { GatewayPush } from "./push";
const dirs: string[] = [];
afterEach(() => {
  for (const d of dirs.splice(0)) rmSync(d, { recursive: true, force: true });
});
function row(db: Database, sql: string): any {
  const q = db.prepare(sql);
  try {
    return q.get();
  } finally {
    q.finalize();
  }
}
const token = "ExpoPushToken[" + "x".repeat(32) + "]";
async function idle() {
  for (let i = 0; i < 8; i++) await new Promise((r) => setTimeout(r, 1));
}
function fixture(send?: (body: any) => Promise<Response>) {
  const dir = mkdtempSync(join(tmpdir(), "ion-inbox-push-"));
  dirs.push(dir);
  const path = join(dir, "state.sqlite");
  const authority = new DurableGatewayAuthority(path, []);
  const publisher = authority.pair(
      authority.createInvite(["publish"]),
      "owner",
    ),
    phone = authority.pair(
      authority.createInvite(["inboxRead", "taskRead"]),
      "phone",
    );
  let now = Date.now();
  const bodies: any[] = [];
  const transport = (async (_url: any, init: any) => {
    const body = JSON.parse(init.body);
    bodies.push(body);
    return send
      ? send(body)
      : Response.json({
          data: body.map((_: any, i: number) => ({
            status: "ok",
            id: `ticket-${bodies.length}-${i}`,
          })),
        });
  }) as typeof fetch;
  let push = new GatewayPush(path, authority, transport, () => now);
  const db = new Database(path, { strict: true });
  return {
    path,
    authority,
    publisher,
    phone,
    bodies,
    db,
    get push() {
      return push;
    },
    setNow(n: number) {
      now = n;
    },
    async reopen() {
      await push.close();
      push = new GatewayPush(path, authority, transport, () => now);
    },
    async close() {
      await push.close();
      db.close(true);
      authority.close();
    },
  };
}
const baseline = (next = 10) => ({
  after: null,
  next,
  policy: "all",
  events: [],
});
const batch = (after = 10, next = 11, attention = true) => ({
  after,
  next,
  policy: "all",
  events: [{ sequence: next, needsAttention: attention }],
});
test("first owner publication is silent and exact replay/restart queues one generic alert", async () => {
  const f = fixture();
  try {
    f.push.register(f.phone.id, token, "inbox");
    expect(f.push.isRegistered(f.phone.id)).toBe(false);
    expect(f.push.publicationCursor(f.publisher.id)).toBeNull();
    f.push.publishInbox(f.publisher.id, baseline());
    await idle();
    expect(f.bodies).toHaveLength(0);
    f.push.publishInbox(f.publisher.id, batch());
    await idle();
    expect(f.bodies).toHaveLength(1);
    const message = f.bodies[0][0];
    expect(message.data).toEqual({ version: 1, scope: "inbox" });
    expect(JSON.stringify(message)).not.toContain(f.publisher.id);
    expect(JSON.stringify(message)).not.toContain(f.phone.id);
    expect(Object.keys(message.data).sort()).toEqual(["scope", "version"]);
    f.push.publishInbox(f.publisher.id, {
      events: [{ needsAttention: true, sequence: 11 }],
      policy: "all",
      next: 11,
      after: 10,
    });
    await idle();
    expect(f.bodies).toHaveLength(1);
    await f.reopen();
    expect(f.push.publicationCursor(f.publisher.id)).toBe(11);
    f.push.publishInbox(f.publisher.id, batch());
    await idle();
    expect(f.bodies).toHaveLength(1);
    expect(() =>
      f.push.publishInbox(f.publisher.id, {
        ...batch(),
        next: 12,
        events: [{ sequence: 12, needsAttention: true }],
      }),
    ).toThrow("cursor changed");
  } finally {
    await f.close();
  }
});
test("lost Expo acknowledgment remains uncertain and is never sent again", async () => {
  const f = fixture(async () => {
    throw new Error("lost response");
  });
  try {
    f.push.register(f.phone.id, token, "inbox");
    f.push.publishInbox(f.publisher.id, baseline());
    f.push.publishInbox(f.publisher.id, batch());
    await idle();
    expect(row(f.db, "SELECT status FROM gateway_push_jobs")).toEqual({
      status: "uncertain",
    });
    await f.reopen();
    await f.push.flush();
    expect(f.bodies).toHaveLength(1);
    f.push.publishInbox(f.publisher.id, batch());
    await idle();
    expect(f.bodies).toHaveLength(1);
  } finally {
    await f.close();
  }
});
test("separate opt-ins and independent grants preserve existing task alerts", async () => {
  const f = fixture();
  try {
    f.push.register(f.phone.id, token);
    expect(f.push.isRegistered(f.phone.id, "inbox")).toBe(false);
    f.push.publishInbox(f.publisher.id, baseline());
    f.push.publishInbox(f.publisher.id, batch());
    await idle();
    expect(f.bodies).toHaveLength(0);
    f.push.register(f.phone.id, token, "inbox");
    f.push.unregister(f.phone.id, "inbox");
    expect(f.push.isRegistered(f.phone.id)).toBe(true);
    const task = "00000000-0000-4000-8000-000000000001";
    f.push.enqueue(task, "event");
    await idle();
    expect(f.bodies[0][0].data).toEqual({ taskId: task });
    f.push.register(f.phone.id, token, "inbox");
    f.authority.revokeDevice(f.phone.id);
    f.push.publishInbox(f.publisher.id, batch(11, 12));
    await idle();
    expect(f.bodies).toHaveLength(1);
    expect(f.push.isRegistered(f.phone.id, "inbox")).toBe(false);
  } finally {
    await f.close();
  }
});
test("publisher revocation cancels unsent generic alerts before external transport", async () => {
  const f = fixture();
  try {
    f.push.register(f.phone.id, token, "inbox");
    f.push.publishInbox(f.publisher.id, baseline());
    f.authority.revokeDevice(f.publisher.id);
    f.push.publishInbox(f.publisher.id, batch());
    await idle();
    expect(f.bodies).toHaveLength(0);
    expect(row(f.db, "SELECT status FROM gateway_push_jobs")).toEqual({
      status: "cancelled",
    });
  } finally {
    await f.close();
  }
});
test("owner mute cancels queued Inbox work while batch progress remains durable", async () => {
  let release!: (r: Response) => void;
  const f = fixture(
    () =>
      new Promise<Response>((r) => {
        release = r;
      }),
  );
  try {
    f.push.register(f.phone.id, token, "inbox");
    f.push.publishInbox(f.publisher.id, baseline());
    f.push.publishInbox(f.publisher.id, batch());
    await idle();
    f.push.publishInbox(f.publisher.id, batch(11, 12));
    f.push.publishInbox(f.publisher.id, {
      after: 12,
      next: 12,
      policy: "off",
      events: [],
    });
    expect(
      row(f.db, "SELECT status FROM gateway_push_jobs WHERE sequence=12"),
    ).toEqual({ status: "cancelled" });
    expect(f.push.publicationCursor(f.publisher.id)).toBe(12);
    release(Response.json({ data: [{ status: "ok", id: "sent" }] }));
    await idle();
    await f.push.flush();
    expect(f.bodies).toHaveLength(1);
  } finally {
    await f.close();
  }
});
test("backlog coalescing keeps one pending alert per owner and expires stale work", async () => {
  let release!: (r: Response) => void;
  let calls = 0;
  const f = fixture(() =>
    ++calls === 1
      ? new Promise<Response>((r) => {
          release = r;
        })
      : Promise.resolve(
          Response.json({ data: [{ status: "ok", id: "later" }] }),
        ),
  );
  try {
    f.push.register(f.phone.id, token, "inbox");
    f.push.publishInbox(f.publisher.id, baseline());
    f.push.publishInbox(f.publisher.id, batch());
    await idle();
    f.push.publishInbox(f.publisher.id, batch(11, 12));
    f.push.publishInbox(f.publisher.id, batch(12, 13, false));
    expect(
      row(
        f.db,
        "SELECT count(*) AS n FROM gateway_push_jobs WHERE status='queued'",
      ),
    ).toEqual({ n: 1 });
    expect(
      row(
        f.db,
        "SELECT attention FROM gateway_push_jobs WHERE status='queued'",
      ),
    ).toEqual({ attention: 1 });
    f.setNow(Date.now() + 16 * 60_000);
    release(Response.json({ data: [{ status: "ok", id: "first" }] }));
    await idle();
    await f.push.flush();
    expect(f.bodies).toHaveLength(1);
    expect(
      row(f.db, "SELECT status FROM gateway_push_jobs WHERE sequence=13"),
    ).toEqual({ status: "expired" });
  } finally {
    await f.close();
  }
});

test("an invalid old token receipt cannot unregister a newly rotated token", async () => {
  let release!: (r: Response) => void;
  const f = fixture(
    () =>
      new Promise<Response>((r) => {
        release = r;
      }),
  );
  try {
    f.push.register(f.phone.id, token, "inbox");
    f.push.publishInbox(f.publisher.id, baseline());
    f.push.publishInbox(f.publisher.id, batch());
    await idle();
    f.push.register(
      f.phone.id,
      "ExpoPushToken[" + "n".repeat(32) + "]",
      "inbox",
    );
    release(
      Response.json({
        data: [{ status: "error", details: { error: "DeviceNotRegistered" } }],
      }),
    );
    await idle();
    expect(f.push.isRegistered(f.phone.id, "inbox")).toBe(true);
  } finally {
    await f.close();
  }
});
test("older task-only schema migrates without opting the phone into Inbox alerts", async () => {
  const dir = mkdtempSync(join(tmpdir(), "ion-push-migration-"));
  dirs.push(dir);
  const path = join(dir, "state.sqlite"),
    authority = new DurableGatewayAuthority(path, []);
  const phone = authority.pair(
    authority.createInvite(["taskRead", "inboxRead"]),
    "phone",
  );
  const db = new Database(path);
  db.run(
    "CREATE TABLE gateway_push_subscriptions (device_id TEXT PRIMARY KEY,token TEXT NOT NULL)",
  );
  db.run("INSERT INTO gateway_push_subscriptions VALUES(?1,?2)", [
    phone.id,
    token,
  ]);
  db.close(true);
  const push = new GatewayPush(path, authority, (async () =>
    Response.json({ data: [] })) as unknown as typeof fetch);
  try {
    expect(push.isRegistered(phone.id)).toBe(true);
    expect(push.isRegistered(phone.id, "inbox")).toBe(false);
  } finally {
    await push.close();
    authority.close();
  }
});

test("receipt checks retain a rotated token and stop querying tickets after their provider window", async () => {
  const f = fixture(async (body) =>
    Array.isArray(body)
      ? Response.json({ data: [{ status: "ok", id: "receipt-ticket" }] })
      : Response.json({
          data: {
            "receipt-ticket": {
              status: "error",
              details: { error: "DeviceNotRegistered" },
            },
          },
        }),
  );
  try {
    f.push.register(f.phone.id, token, "inbox");
    f.push.publishInbox(f.publisher.id, baseline());
    f.push.publishInbox(f.publisher.id, batch());
    await idle();
    f.push.register(
      f.phone.id,
      "ExpoPushToken[" + "r".repeat(32) + "]",
      "inbox",
    );
    f.setNow(Date.now() + 16 * 60_000);
    await f.push.flush();
    expect(f.push.isRegistered(f.phone.id, "inbox")).toBe(true);
    expect(f.bodies[1]).toEqual({ ids: ["receipt-ticket"] });
    f.push.publishInbox(f.publisher.id, batch(11, 12));
    await idle();
    f.setNow(Date.now() + 2 * 86400_000);
    const sent = f.bodies.length;
    await f.push.flush();
    expect(f.bodies).toHaveLength(sent);
    expect(
      row(f.db, "SELECT status FROM gateway_push_jobs WHERE sequence=12"),
    ).toEqual({ status: "receiptExpired" });
  } finally {
    await f.close();
  }
});
