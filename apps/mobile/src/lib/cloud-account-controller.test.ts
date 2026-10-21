import { afterEach, beforeAll, describe, expect, test } from "bun:test";
import { createHash, randomBytes } from "node:crypto";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createLocalJWKSet, exportJWK, generateKeyPair, SignJWT } from "jose";
import { createCloudHandler } from "../../../../packages/ion-cloud/src/http";
import { ionAuth0 } from "../../../../packages/ion-cloud-client/src/public-config";
import { createCloudAccount } from "./cloud-account-controller";
import type { GatewayPairing } from "./gateway-client";

let signing: Awaited<ReturnType<typeof generateKeyPair>>;
let keys: ReturnType<typeof createLocalJWKSet>;
const cleanup: (() => Promise<void>)[] = [];
beforeAll(async () => {
  signing = await generateKeyPair("RS256");
  keys = createLocalJWKSet({
    keys: [{ ...(await exportJWK(signing.publicKey)), kid: "phone" }],
  });
});
afterEach(async () => {
  for (const close of cleanup.splice(0)) await close();
});
function fixture(origin = "https://cloud.example") {
  const directory = mkdtempSync(join(tmpdir(), "ion-mobile-account-"));
  const server = createCloudHandler({
    config: { ...ionAuth0, publicOrigin: "https://cloud.example" },
    keys,
    stateDirectory: directory,
  });
  cleanup.push(async () => {
    await server.close();
    rmSync(directory, { recursive: true, force: true });
  });
  let raw: string | null = null,
    pairing: GatewayPairing | null = null;
  let subject = "auth0|one",
    cancel = false,
    offline = false,
    wrongBootstrap = false;
  let browserCalls = 0,
    clears = 0,
    resets = 0;
  const network: string[] = [];
  const send = (async (
    input: Parameters<typeof fetch>[0],
    init?: RequestInit,
  ) => {
    const url = String(input);
    network.push(url);
    if (offline) throw new Error("offline");
    if (url.endsWith("oauth/revoke")) {
      expect(raw).toBeNull(); // Clear the vault before remote revocation.
      return Response.json({});
    }
    if (url.endsWith("oauth/token")) {
      const params = new URLSearchParams(String(init?.body));
      expect(params.has("client_secret")).toBe(false);
      const token = await new SignJWT({
        azp: ionAuth0.mobileClientId,
        scope: "ion:gateway",
      })
        .setProtectedHeader({ alg: "RS256", kid: "phone" })
        .setIssuer(ionAuth0.issuer)
        .setAudience(ionAuth0.audience)
        .setSubject(subject)
        .setIssuedAt()
        .setExpirationTime("15m")
        .sign(signing.privateKey);
      return Response.json({
        access_token: token,
        refresh_token: "rotating-refresh",
        token_type: "Bearer",
        expires_in: 900,
      });
    }
    if (wrongBootstrap && url.endsWith("/bootstrap"))
      return Response.json({ auth: { issuer: "https://attacker.example/" } });
    if (/\/v1\/apps\/(catalog|connect|disconnect)$/.test(url)) {
      expect(new Headers(init?.headers).get('Authorization')?.startsWith('Bearer ')).toBe(true);
      expect(init?.redirect).toBe('error');
      return Response.json({ path: new URL(url).pathname, method: init?.method });
    }
    return server.fetch(new Request(url, init));
  }) as typeof fetch;
  const controller = createCloudAccount({
    origin,
    send,
    vault: {
      read: async () => raw,
      write: async (value) => {
        raw = value;
      },
      remove: async () => {
        raw = null;
      },
    },
    crypto: {
      randomBytes: async (size) => randomBytes(size),
      sha256: async (value) => createHash("sha256").update(value).digest(),
    },
    openBrowser: async (url, callback) => {
      browserCalls++;
      expect(callback).toBe("ion://auth/callback");
      const authorize = new URL(url);
      expect(authorize.searchParams.get("prompt")).toBe("login");
      return cancel
        ? { type: "cancel" }
        : {
            type: "success",
            url:
              callback +
              "?" +
              new URLSearchParams({
                state: authorize.searchParams.get("state")!,
                code: "one-use",
              }),
          };
    },
    readPairing: async () => pairing,
    writePairing: async (value) => {
      pairing = value;
    },
    deletePairing: async () => {
      pairing = null;
    },
    clearTasks: async () => {
      clears++;
    },
  });
  controller.onSignOut(() => {
    resets++;
  });
  return {
    controller,
    server,
    send,
    raw: () => raw,
    pairing: () => pairing,
    clears: () => clears,
    resets: () => resets,
    network,
    browserCalls: () => browserCalls,
    setSubject: (value: string) => {
      subject = value;
    },
    setCancel: () => {
      cancel = true;
    },
    setOffline: () => {
      offline = true;
    },
    setBadBootstrap: () => {
      wrongBootstrap = true;
    },
    setPairing: (value: GatewayPairing) => {
      pairing = value;
    },
  };
}
describe("native phone managed-account workflow", () => {
  test('permits exactly the managed app connection routes while refusing unrelated app capabilities', async () => {
    const f = fixture();
    expect(await f.controller.signIn()).toBe(true);
    for (const route of ['catalog', 'connect', 'disconnect']) {
      const path = `/v1/apps/${route}`;
      expect(await f.controller.request(path, 'POST', {})).toEqual({ path, method: 'POST' });
    }
    const before = f.network.length;
    for (const path of ['/v1/apps/actions/execute', '/v1/apps/catalog?extra=true', '/v1/apps/../bootstrap', '/v1/apps/catalog/extra'])
      await expect(f.controller.request(path, 'POST', {})).rejects.toThrow('Invalid managed service');
    expect(f.network.length).toBe(before);
  });
  test("unconfigured build and cancelled sign-in keep local mode and store no account", async () => {
    const unavailable = fixture("");
    expect(await unavailable.controller.status()).toEqual({
      available: false,
      signedIn: false,
    });
    expect(unavailable.network).toEqual([]);
    const f = fixture();
    f.setCancel();
    expect(await f.controller.signIn()).toBe(false);
    expect(f.raw()).toBeNull();
    expect(f.pairing()).toBeNull();
  });
  test("sign-in, explicit phone access, control consent and logout use the real isolated gateway", async () => {
    const f = fixture();
    expect(await f.controller.signIn()).toBe(true);
    await f.controller.connectPhone(false);
    const readOnly = f.pairing()!;
    const state = await f.controller.status();
    expect(state.phoneConnected).toBe(true);
    expect(readOnly.grants).toContain("taskRead");
    expect(readOnly.grants).not.toContain("taskControl");
    const token = await f.controller.authorize(
      "https://cloud.example",
      readOnly.cloudWorkspaceId!,
    );
    function request(device: GatewayPairing) {
      return new Request("https://cloud.example/v1/gateway/v1/device/session", {
        headers: {
          Authorization: "Bearer " + token,
          "X-Ion-Device-Token": device.token,
          "X-Waku-Device-Id": device.id,
        },
      });
    }
    expect((await f.server.fetch(request(readOnly))).status).toBe(200);
    await f.controller.connectPhone(true);
    const controls = f.pairing()!;
    expect(controls.id).not.toBe(readOnly.id);
    expect(controls.grants).toContain("taskControl");
    expect(controls.grants).toContain("review");
    expect((await f.server.fetch(request(readOnly))).status).toBe(401);
    expect((await f.server.fetch(request(controls))).status).toBe(200);
    expect(await f.controller.signOut()).toEqual({ revoked: true });
    expect(f.raw()).toBeNull();
    expect(f.pairing()).toBeNull();
    expect(f.resets()).toBe(1);
    expect((await f.server.fetch(request(controls))).status).toBe(401);
    await expect(
      f.controller.authorize(
        "https://cloud.example",
        controls.cloudWorkspaceId!,
      ),
    ).rejects.toThrow();
  });
  test("switching account clears old phone binding/cache and never lends the new token to old work", async () => {
    const f = fixture();
    await f.controller.signIn();
    await f.controller.connectPhone(false);
    const old = f.pairing()!;
    f.setSubject("auth0|two");
    await f.controller.signIn();
    expect(f.pairing()).toBeNull();
    expect(f.clears()).toBe(2);
    expect(f.resets()).toBe(1);
    await expect(
      f.controller.authorize("https://cloud.example", old.cloudWorkspaceId!),
    ).rejects.toThrow();
    await f.controller.connectPhone(false);
    expect(f.pairing()!.cloudWorkspaceId).not.toBe(old.cloudWorkspaceId);
    await expect(
      f.controller.authorize(
        "https://foreign.example",
        f.pairing()!.cloudWorkspaceId!,
      ),
    ).rejects.toThrow("another cloud");
  });
  test("self-hosted pairing is preserved and cannot be silently replaced", async () => {
    const f = fixture();
    const local = {
      baseUrl: "https://personal.example",
      id: "11111111-1111-4111-8111-111111111111",
      token: "personal-token",
      expiresAt: Date.now() + 60_000,
      grants: ["taskRead"],
    };
    f.setPairing(local);
    await f.controller.signIn();
    await expect(f.controller.connectPhone(false)).rejects.toThrow(
      "Remove the personal gateway",
    );
    await f.controller.signOut();
    expect(f.pairing()).toEqual(local);
    expect(f.clears()).toBe(0);
  });
  test("offline logout still clears this device and reports unconfirmed remote revocation", async () => {
    const f = fixture();
    await f.controller.signIn();
    await f.controller.connectPhone(false);
    f.setOffline();
    expect(await f.controller.signOut()).toEqual({ revoked: false });
    expect(f.raw()).toBeNull();
    expect(f.pairing()).toBeNull();
    expect(f.resets()).toBe(1);
  });
  test("a substituted bootstrap is rejected before opening login or storing anything", async () => {
    const f = fixture();
    f.setBadBootstrap();
    await expect(f.controller.signIn()).rejects.toThrow("Could not sign in");
    expect(f.browserCalls()).toBe(0);
    expect(f.raw()).toBeNull();
  });
});
