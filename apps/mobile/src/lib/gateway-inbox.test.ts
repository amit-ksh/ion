import { test, expect, afterEach } from "bun:test";
import { randomUUID } from "node:crypto";
import { submitGatewayInbox, type InboxIntent } from "./gateway-inbox";
import {
  gatewayEnqueueInbox,
  gatewayInboxByKey,
  GatewayRequestError,
  type GatewayPairing,
} from "./gateway-client";
import type { InboxCommand, InboxSubmission } from "@waku/client";
const originalFetch = globalThis.fetch;
afterEach(() => {
  globalThis.fetch = originalFetch;
});
function fixture() {
  const pairing: GatewayPairing = {
    baseUrl: "https://gateway.invalid",
    id: randomUUID(),
    token: "private-device-token",
    expiresAt: Date.now() + 60_000,
    grants: ["inboxRead", "inboxControl"],
  };
  const publisher = randomUUID(),
    intent: InboxIntent = {
      sequence: 1,
      expectedRevision: 0,
      action: "archive",
    };
  const values = new Map<string, string>(),
    order: string[] = [];
  let current = true,
    sends = 0,
    lookups = 0,
    recorded: InboxCommand | null = null;
  const storage = {
    getItem: async (key: string) => values.get(key) ?? null,
    setItem: async (key: string, value: string) => {
      order.push("persist");
      values.set(key, value);
    },
    removeItem: async (key: string) => {
      values.delete(key);
    },
  };
  const transport = {
    lookup: async () => {
      lookups++;
      return recorded;
    },
    enqueue: async (_p: GatewayPairing, input: InboxSubmission) => {
      sends++;
      order.push("send");
      recorded = {
        ...input,
        id: randomUUID(),
        deviceId: pairing.id,
        status: "queued",
        createdAt: Date.now(),
        updatedAt: Date.now(),
      };
      return recorded;
    },
  };
  const submit = (next = intent) =>
    submitGatewayInbox(
      pairing,
      storage,
      publisher,
      next,
      randomUUID,
      () => current,
      transport,
    );
  return {
    pairing,
    publisher,
    intent,
    values,
    order,
    storage,
    transport,
    submit,
    sends: () => sends,
    lookups: () => lookups,
    setCurrent: (v: boolean) => {
      current = v;
    },
    set: (v: InboxCommand | null) => {
      recorded = v;
    },
    recorded: () => recorded,
  };
}
test("Inbox request persists without credentials before send and concurrent taps retain one nonce", async () => {
  const f = fixture(),
    [a, b] = await Promise.all([f.submit(), f.submit()]);
  expect(f.order).toEqual(["persist", "send"]);
  expect(a.id).toBe(b.id);
  expect(f.sends()).toBe(1);
  expect([...f.values.values()][0]).not.toContain(f.pairing.token);
});
test("storage failure and changed focus during persistence fence the first Inbox write", async () => {
  const failed = fixture();
  failed.storage.setItem = async () => {
    throw new Error("Disk unavailable");
  };
  await expect(failed.submit()).rejects.toThrow("Disk unavailable");
  expect(failed.sends()).toBe(0);
  const changed = fixture(),
    save = changed.storage.setItem;
  changed.storage.setItem = async (k, v) => {
    await save(k, v);
    changed.setCurrent(false);
  };
  await expect(changed.submit()).rejects.toThrow("scope changed");
  expect(changed.sends()).toBe(0);
  expect(changed.values.size).toBe(1);
});
test("lost Inbox acknowledgement recovers the original receipt with no second send", async () => {
  const f = fixture(),
    enqueue = f.transport.enqueue;
  f.transport.enqueue = async (p, input) => {
    await enqueue(p, input);
    throw new Error("Connection lost");
  };
  const command = await f.submit();
  expect(command.id).toBe(f.recorded()!.id);
  expect(f.sends()).toBe(1);
  await f.submit();
  expect(f.sends()).toBe(1);
  expect(f.lookups()).toBe(2);
});
test("a changed saved action and failed status lookup retain the original request", async () => {
  const f = fixture();
  await f.submit();
  f.set(null);
  await expect(f.submit({ ...f.intent, action: "restore" })).rejects.toThrow(
    "differs",
  );
  expect(f.sends()).toBe(1);
  f.transport.lookup = async () => {
    throw new Error("Status unavailable");
  };
  await expect(f.submit()).rejects.toThrow("Status unavailable");
  expect(f.values.size).toBe(1);
  expect(f.sends()).toBe(1);
});
test("foreign receipt identity cannot clear saved intent; terminal receipt can", async () => {
  const f = fixture();
  await f.submit();
  const original = f.recorded()!;
  f.set({ ...original, deviceId: randomUUID(), status: "applied" });
  await expect(f.submit()).rejects.toThrow("identity");
  expect(f.values.size).toBe(1);
  f.set({ ...original, status: "applied" });
  expect((await f.submit()).status).toBe("applied");
  expect(f.values.size).toBe(0);
  expect(f.sends()).toBe(1);
});
test("confirmed absent expired request is cleared without replay or renewed deadline", async () => {
  const f = fixture();
  await f.submit();
  f.set(null);
  for (const [k, v] of f.values)
    f.values.set(
      k,
      JSON.stringify({ ...JSON.parse(v), expiresAt: Date.now() - 1 }),
    );
  await expect(f.submit()).rejects.toThrow("expired");
  expect(f.values.size).toBe(0);
  expect(f.sends()).toBe(1);
});
test("read-only, expired and wrong-scope pairings send no notification mutations", async () => {
  for (const grants of [
    ["inboxRead"],
    ["inboxControl"],
    ["automationRead", "automationControl"],
  ]) {
    const f = fixture();
    f.pairing.grants = grants;
    await expect(f.submit()).rejects.toThrow("Inbox read and control");
    expect(f.sends()).toBe(0);
    expect(f.values.size).toBe(0);
  }
  const f = fixture();
  f.pairing.expiresAt = Date.now() - 1;
  await expect(f.submit()).rejects.toThrow("Inbox read and control");
  expect(f.sends()).toBe(0);
});
test("gateway Inbox endpoints send only device headers, no-store and redirect refusal", async () => {
  const f = fixture();
  let calls = 0;
  globalThis.fetch = (async (url: any, init: any) => {
    calls++;
    expect(String(url)).toStartWith(
      f.pairing.baseUrl + "/v1/device/inbox/commands",
    );
    expect(init.cache).toBe("no-store");
    expect(init.redirect).toBe("error");
    expect(init.headers.Authorization).toBe("Bearer " + f.pairing.token);
    expect(init.headers["X-Waku-Device-Id"]).toBe(f.pairing.id);
    const command = f.recorded()!;
    return Response.json(init.method === "POST" ? command : { command });
  }) as typeof fetch;
  await f.submit();
  const input = JSON.parse([...f.values.values()][0]!);
  expect((await gatewayEnqueueInbox(f.pairing, input)).id).toBe(
    f.recorded()!.id,
  );
  expect((await gatewayInboxByKey(f.pairing, input.idempotencyKey))!.id).toBe(
    f.recorded()!.id,
  );
  expect(calls).toBe(2);
});
test("only confirmed gateway rejection and absence clear an unsent Inbox request", async () => {
  const f = fixture();
  f.transport.enqueue = async () => {
    throw new GatewayRequestError(409);
  };
  await expect(f.submit()).rejects.toThrow("409");
  expect(f.values.size).toBe(0);
  const unknown = fixture();
  unknown.transport.enqueue = async () => {
    throw new Error("Lost");
  };
  unknown.transport.lookup = async () => {
    throw new Error("Offline");
  };
  await expect(unknown.submit()).rejects.toThrow("Lost");
  expect(unknown.values.size).toBe(1);
});
