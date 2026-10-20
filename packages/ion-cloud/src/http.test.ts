import { afterEach, beforeAll, describe, expect, test } from "bun:test";
import { mkdtempSync, rmSync, readdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createLocalJWKSet, exportJWK, generateKeyPair, SignJWT } from "jose";
import { ionAuth0 } from "./config";
import { createCloudHandler } from "./http";

const config = { ...ionAuth0, publicOrigin: "https://cloud.ion.example" };
let pair: Awaited<ReturnType<typeof generateKeyPair>>;
let keys: ReturnType<typeof createLocalJWKSet>;
const directories: string[] = [];
const handlers: ReturnType<typeof createCloudHandler>[] = [];
beforeAll(async () => {
  pair = await generateKeyPair("RS256");
  keys = createLocalJWKSet({
    keys: [{ ...(await exportJWK(pair.publicKey)), kid: "local" }],
  });
});
afterEach(async () => {
  for (const handler of handlers.splice(0)) await handler.close();
  for (const directory of directories.splice(0))
    rmSync(directory, { recursive: true, force: true });
});
function setup(workspaceLimit?: number) {
  const directory = mkdtempSync(join(tmpdir(), "ion-cloud-"));
  directories.push(directory);
  const handler = createCloudHandler({
    config,
    keys,
    stateDirectory: directory,
    workspaceLimit,
  });
  handlers.push(handler);
  return { handler, directory };
}
async function token(subject: string, client: string = config.desktopClientId) {
  return new SignJWT({ azp: client, scope: "ion:gateway" })
    .setProtectedHeader({ alg: "RS256", kid: "local" })
    .setSubject(subject)
    .setIssuer(config.issuer)
    .setAudience(config.audience)
    .setIssuedAt()
    .setExpirationTime("15m")
    .sign(pair.privateKey);
}
function call(
  handler: ReturnType<typeof createCloudHandler>,
  path: string,
  jwt?: string,
  data?: unknown,
  device?: { id: string; token: string },
) {
  return handler.fetch(
    new Request(config.publicOrigin + path, {
      method: data === undefined ? "GET" : "POST",
      headers: {
        ...(jwt ? { Authorization: "Bearer " + jwt } : {}),
        ...(data !== undefined ? { "Content-Type": "application/json" } : {}),
        ...(device
          ? {
              "X-Waku-Device-Id": device.id,
              "X-Ion-Device-Token": device.token,
            }
          : {}),
      },
      body: data === undefined ? undefined : JSON.stringify(data),
    }),
  );
}
const root = "/v1/gateway";
async function invite(
  handler: ReturnType<typeof createCloudHandler>,
  jwt: string,
  grants = ["inspect", "taskRead"],
) {
  const response = await call(handler, root + "/v1/owner/invites", jwt, {
    grants,
  });
  expect(response.status).toBe(201);
  return ((await response.json()) as { invite: string }).invite;
}

describe("managed gateway account boundary", () => {
  test("authenticated desktop publishers can discover relay capabilities without weakening account admission", async () => {
    const { handler, directory } = setup();
    expect((await call(handler, root + "/v1/health")).status).toBe(401);
    expect(
      readdirSync(directory).filter(
        (name) => !name.startsWith("services.sqlite"),
      ),
    ).toEqual([]);
    const jwt = await token("auth0|desktop-owner");
    const response = await call(handler, root + "/v1/health", jwt);
    expect(response.status).toBe(200);
    expect(await response.json()).toEqual({
      status: "ok",
      features: {
        automationRelay: 1,
        portfolioRelay: 1,
        inboxControl: 1,
        inboxNotifications: 1,
      },
    });
    expect((await call(handler, root + "/owner", jwt)).status).toBe(404);
  });

  test("bootstrap is public, reports only available services, and never creates account state", async () => {
    const { handler, directory } = setup();
    const response = await call(handler, "/v1/bootstrap");
    const bootstrap = (await response.json()) as {
      auth: unknown;
      features: unknown;
    };
    expect(bootstrap.features).toEqual({
      gateway: true,
      speech: false,
      htmlPublishing: false,
      appDeployment: false,
    });
    expect(response.headers.get("cache-control")).toBe("no-store");
    expect((await call(handler, "/v1/session")).status).toBe(401);
    expect((await call(handler, root + "/v1/owner/devices")).status).toBe(401);
    expect((await call(handler, "/v1/owner/devices")).status).toBe(404);
    expect(
      (await call(handler, "/v1/session?access_token=secret")).status,
    ).toBe(400);
    expect(
      readdirSync(directory).filter(
        (name) => !name.startsWith("services.sqlite"),
      ),
    ).toEqual([]);
  });
  test("isolates invites, devices, owner reads and device access across two signed-in users; persists restart", async () => {
    const { handler, directory } = setup();
    const a = await token("auth0|a"),
      b = await token("auth0|b");
    const aPhone = await token("auth0|a", config.mobileClientId);
    const secret = await invite(handler, a);
    expect(
      (
        await call(handler, root + "/v1/pair", b, {
          invite: secret,
          name: "foreign",
        })
      ).status,
    ).toBe(403);
    const paired = await call(handler, root + "/v1/pair", aPhone, {
      invite: secret,
      name: "My phone",
    });
    expect(paired.status).toBe(201);
    const device = (await paired.json()) as { id: string; token: string };
    expect(
      (
        await call(handler, root + "/v1/pair", aPhone, {
          invite: secret,
          name: "replay",
        })
      ).status,
    ).toBe(403);
    const devices = (await (
      await call(handler, root + "/v1/owner/devices", a)
    ).json()) as { devices: { id: string }[] };
    expect(devices.devices.map((d) => d.id)).toEqual([device.id]);
    expect(
      await (await call(handler, root + "/v1/owner/devices", b)).json(),
    ).toEqual({ devices: [] });
    expect(
      (await call(handler, root + "/v1/device/session", a, undefined, device))
        .status,
    ).toBe(200);
    expect(
      (await call(handler, root + "/v1/device/session", b, undefined, device))
        .status,
    ).toBe(401);
    expect((await call(handler, root + "/v1/device/session", a)).status).toBe(
      401,
    );
    expect(
      (
        await call(
          handler,
          root + "/v1/device/session",
          device.token,
          undefined,
          device,
        )
      ).status,
    ).toBe(401);
    expect((await call(handler, root + "/owner", a)).status).toBe(404);
    // Disabled connected-app execution cannot inherit another user's operator credentials.
    expect(
      (await call(handler, root + "/v1/owner/connected-accounts", a)).status,
    ).toBe(503);
    await handler.close();
    const restarted = createCloudHandler({
      config,
      keys,
      stateDirectory: directory,
    });
    handlers.push(restarted);
    expect(
      (
        await call(
          restarted,
          root + "/v1/device/session",
          aPhone,
          undefined,
          device,
        )
      ).status,
    ).toBe(200);
    expect(
      (
        await call(
          restarted,
          root + "/v1/owner/devices/" + device.id + "/revoke",
          b,
          {},
        )
      ).status,
    ).toBe(400);
    expect(
      (await call(restarted, root + "/v1/device/session", a, undefined, device))
        .status,
    ).toBe(200);
    expect(
      (
        await call(
          restarted,
          root + "/v1/owner/devices/" + device.id + "/revoke",
          a,
          {},
        )
      ).status,
    ).toBe(200);
    expect(
      (await call(restarted, root + "/v1/device/session", a, undefined, device))
        .status,
    ).toBe(401);
  });
  test("bounded workspace admission fails closed and preserves existing workspaces", async () => {
    const { handler, directory } = setup(1);
    const a = await token("auth0|a"),
      b = await token("auth0|b");
    expect((await call(handler, root + "/v1/owner/devices", a)).status).toBe(
      200,
    );
    expect((await call(handler, root + "/v1/owner/devices", b)).status).toBe(
      503,
    );
    await handler.close();
    const restarted = createCloudHandler({
      config,
      keys,
      stateDirectory: directory,
      workspaceLimit: 1,
    });
    handlers.push(restarted);
    expect((await call(restarted, root + "/v1/owner/devices", b)).status).toBe(
      503,
    );
    expect((await call(restarted, root + "/v1/owner/devices", a)).status).toBe(
      200,
    );
    expect((await call(handler, "/v1/bootstrap")).status).toBe(503);
    expect((await call(restarted, root + "/v1/owner/devices", a)).status).toBe(
      200,
    );
  });
});

test("two accounts with identical task IDs receive only their own task feeds", async () => {
  const { handler } = setup();
  const a = await token("auth0|a"),
    b = await token("auth0|b");
  async function pairDevice(jwt: string) {
    const secret = await invite(handler, jwt, ["publish", "taskRead"]);
    const response = await call(handler, root + "/v1/pair", jwt, {
      invite: secret,
      name: "Desktop",
    });
    return (await response.json()) as { id: string; token: string };
  }
  const aDevice = await pairDevice(a),
    bDevice = await pairDevice(b);
  const taskId = "11111111-1111-4111-8111-111111111111";
  async function publish(
    jwt: string,
    device: { id: string; token: string },
    title: string,
  ) {
    return handler.fetch(
      new Request(config.publicOrigin + root + "/v1/device/tasks/" + taskId, {
        method: "PUT",
        headers: {
          Authorization: "Bearer " + jwt,
          "X-Waku-Device-Id": device.id,
          "X-Ion-Device-Token": device.token,
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          task: {
            id: taskId,
            projectId: "22222222-2222-4222-8222-222222222222",
            projectName: "Private project",
            title,
            provider: "codex",
            status: "running",
            runtimeId: "33333333-3333-4333-8333-333333333333",
            updatedAt: Date.now(),
          },
        }),
      }),
    );
  }
  expect((await publish(a, aDevice, "Account A")).status).toBe(200);
  expect((await publish(b, bDevice, "Account B")).status).toBe(200);
  for (const [jwt, device, expected] of [
    [a, aDevice, "Account A"],
    [b, bDevice, "Account B"],
  ] as const) {
    const response = await call(
      handler,
      root + "/v1/device/tasks",
      jwt,
      undefined,
      device,
    );
    expect(response.status).toBe(200);
    const body = (await response.json()) as { tasks: { title: string }[] };
    expect(body.tasks.map((task) => task.title)).toEqual([expected]);
  }
});
