import { afterEach, test, expect } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { startGateway } from "./server";
const dirs: string[] = [];
afterEach(() => {
  for (const dir of dirs.splice(0))
    rmSync(dir, { recursive: true, force: true });
});
const ownerToken = "owner_" + "x".repeat(48),
  token = "ExpoPushToken[" + "x".repeat(32) + "]";
function fixture() {
  const dir = mkdtempSync(join(tmpdir(), "ion-inbox-push-http-"));
  dirs.push(dir);
  const sent: any[] = [];
  const gateway = startGateway({
    databasePath: join(dir, "state.sqlite"),
    ownerToken,
    allowedTools: [],
    pushFetch: (async (_url, init) => {
      const messages = JSON.parse(String(init?.body));
      sent.push(messages);
      return Response.json({
        data: messages.map(() => ({ status: "ok", id: "ticket" })),
      });
    }) as typeof fetch,
  });
  const owner = { id: "", token: ownerToken };
  async function call(
    path: string,
    method = "GET",
    device?: { id: string; token: string },
    body?: unknown,
  ) {
    return fetch(new URL(path, gateway.server.url), {
      method,
      headers: {
        ...(device
          ? {
              Authorization: `Bearer ${device.token}`,
              "X-Waku-Device-Id": device.id,
            }
          : {}),
        ...(body !== undefined ? { "Content-Type": "application/json" } : {}),
      },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
  }
  async function pair(grants: string[]) {
    const invite = (await (
      await call("/v1/owner/invites", "POST", owner, { grants })
    ).json()) as { invite: string };
    return (await (
      await call("/v1/pair", "POST", undefined, {
        invite: invite.invite,
        name: "device",
      })
    ).json()) as { id: string; token: string };
  }
  return { gateway, sent, owner, call, pair };
}
test("Inbox subscriptions need independent read grant, explicit opt-in and exact topic", async () => {
  const f = fixture();
  try {
    const phone = await f.pair(["inboxRead"]),
      task = await f.pair(["taskRead"]),
      publisher = await f.pair(["publish"]);
    expect((await f.call("/v1/device/inbox/push", "GET", phone)).status).toBe(
      200,
    );
    expect(
      await (await f.call("/v1/device/inbox/push", "GET", phone)).json(),
    ).toEqual({ enabled: false });
    expect(
      (await f.call("/v1/device/inbox/push", "PUT", task, { token })).status,
    ).toBe(401);
    expect(
      (await f.call("/v1/device/push", "PUT", phone, { token })).status,
    ).toBe(401);
    expect(
      (await f.call("/v1/device/push?topic=all", "PUT", phone, { token }))
        .status,
    ).toBe(400);
    expect(
      (await f.call("/v1/device/inbox/push", "PUT", phone, { token })).status,
    ).toBe(200);
    expect(
      (await f.call("/v1/device/inbox/publication", "GET", phone)).status,
    ).toBe(401);
    expect(
      await (
        await f.call("/v1/device/inbox/publication", "GET", publisher)
      ).json(),
    ).toEqual({ after: null });
    expect(
      (await f.call("/v1/device/inbox/push", "DELETE", phone)).status,
    ).toBe(200);
    expect(f.sent).toHaveLength(0);
  } finally {
    await f.gateway.close();
  }
});
test("owner cursor and generic alerts commit once, while malformed content or foreign publisher IDs reject", async () => {
  const f = fixture();
  try {
    const owner = await f.pair(["publish"]),
      other = await f.pair(["publish"]),
      phone = await f.pair(["inboxRead"]);
    await f.call("/v1/device/inbox/push", "PUT", phone, { token });
    const baseline = { after: null, next: 10, policy: "all", events: [] },
      batch = {
        after: 10,
        next: 11,
        policy: "all",
        events: [{ sequence: 11, needsAttention: true }],
      };
    expect(
      (
        await f.call("/v1/device/inbox/publication", "PUT", owner, {
          batch: baseline,
        })
      ).status,
    ).toBe(200);
    expect(
      (
        await f.call("/v1/device/inbox/publication", "PUT", owner, {
          publisherId: other.id,
          batch,
        })
      ).status,
    ).toBe(400);
    expect(
      (
        await f.call("/v1/device/inbox/publication", "PUT", owner, {
          batch: {
            ...batch,
            events: [{ ...batch.events[0], title: "private" }],
          },
        })
      ).status,
    ).toBe(400);
    expect(
      (await f.call("/v1/device/inbox/publication", "PUT", owner, { batch }))
        .status,
    ).toBe(200);
    expect(
      (await f.call("/v1/device/inbox/publication", "PUT", owner, { batch }))
        .status,
    ).toBe(200);
    await new Promise((r) => setTimeout(r, 10));
    expect(f.sent).toHaveLength(1);
    expect(f.sent[0][0].data).toEqual({ version: 1, scope: "inbox" });
    expect(
      await (await f.call("/v1/device/inbox/publication", "GET", owner)).json(),
    ).toEqual({ after: 11 });
    expect(
      await (await f.call("/v1/device/inbox/publication", "GET", other)).json(),
    ).toEqual({ after: null });
    await f.call(`/v1/owner/devices/${owner.id}/revoke`, "POST", f.owner);
    expect(
      (await f.call("/v1/device/inbox/publication", "PUT", owner, { batch }))
        .status,
    ).toBe(401);
  } finally {
    await f.gateway.close();
  }
});
