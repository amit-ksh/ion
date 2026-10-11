import { test, expect } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { randomUUID } from "node:crypto";
import { DurableInboxRelay } from "./inbox-relay";
function fixture() {
  const dir = mkdtempSync(join(tmpdir(), "ion-inbox-relay-"));
  let now = 1_000;
  const path = join(dir, "relay.sqlite");
  return {
    dir,
    path,
    time: () => now,
    set: (t: number) => {
      now = t;
    },
  };
}
test("notification relay restarts claimed receipts, preserves nonce and rejects changed identity and settlement", () => {
  const f = fixture();
  let relay = new DurableInboxRelay(f.path, f.time);
  try {
    const device = randomUUID(),
      publisher = randomUUID();
    const input = {
      publisherId: publisher,
      idempotencyKey: randomUUID(),
      sequence: 1,
      expectedRevision: 0,
      action: "archive",
      expiresAt: 121_000,
    };
    const command = relay.enqueue(device, input);
    relay.claim(publisher, command.id, true);
    relay.close();
    relay = new DurableInboxRelay(f.path, f.time);
    expect(relay.enqueue(device, input).status).toBe("uncertain");
    expect(relay.pending(publisher)[0]!.id).toBe(command.id);
    expect(() =>
      relay.enqueue(device, { ...input, action: "restore" }),
    ).toThrow("changed");
    expect(() => relay.settle(randomUUID(), command.id, true)).toThrow(
      "Unknown",
    );
    relay.settle(publisher, command.id, true);
    expect(relay.settle(publisher, command.id, true).status).toBe("applied");
    expect(() => relay.settle(publisher, command.id, false)).toThrow(
      "not claimed",
    );
    expect(relay.byKey(randomUUID(), input.idempotencyKey)).toBeNull();
  } finally {
    relay.close();
    rmSync(f.dir, { recursive: true, force: true });
  }
});
test("notification deadlines and receipt capacity stay bounded; expiry never renews a request", () => {
  const f = fixture(),
    relay = new DurableInboxRelay(f.path, f.time);
  try {
    const device = randomUUID(),
      publisher = randomUUID();
    const input = {
      publisherId: publisher,
      idempotencyKey: randomUUID(),
      sequence: 1,
      expectedRevision: 0,
      action: "archive",
      expiresAt: 121_000,
    };
    expect(() =>
      relay.enqueue(device, { ...input, expiresAt: 121_001 }),
    ).toThrow("deadline");
    const command = relay.enqueue(device, input);
    f.set(121_000);
    expect(relay.byKey(device, input.idempotencyKey)!.status).toBe("rejected");
    expect(relay.pending(publisher)).toEqual([]);
    expect(relay.claim(publisher, command.id, true).status).toBe("rejected");
    expect(() =>
      relay.enqueue(device, { ...input, idempotencyKey: randomUUID() }),
    ).toThrow("expired");
    expect(() =>
      relay.enqueue(device, { ...input, expiresAt: 241_000 }),
    ).toThrow("changed");
    f.set(1_000);
    for (let i = 1; i < 512; i++)
      relay.enqueue(device, { ...input, idempotencyKey: randomUUID() });
    expect(() =>
      relay.enqueue(device, { ...input, idempotencyKey: randomUUID() }),
    ).toThrow("full");
    expect(relay.pending(publisher)).toHaveLength(16);
  } finally {
    relay.close();
    rmSync(f.dir, { recursive: true, force: true });
  }
});
