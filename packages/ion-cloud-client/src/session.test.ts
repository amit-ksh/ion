import { describe, expect, test } from "bun:test";
import { createAccountSession, type SessionVault } from "./session";
const config = {
  issuer: "https://tenant.auth0.com/",
  audience: "https://ion-cloud-api",
  clientId: "mobile-client-1234567890",
  redirectUri: "ion://auth/callback",
  cloudOrigin: "https://cloud.example",
};
function fixture() {
  let raw: string | null = null,
    time = 1000,
    exchanges = 0,
    mode = "ok";
  const writes: string[] = [];
  const vault: SessionVault = {
    read: async () => raw,
    write: async (value) => {
      raw = value;
      writes.push(value);
    },
    remove: async () => {
      raw = null;
    },
  };
  const send = (async (input: Parameters<typeof fetch>[0]) => {
    const url = String(input);
    if (url.endsWith("/oauth/revoke")) return Response.json({});
    if (url.endsWith("/oauth/token")) {
      exchanges++;
      expect(JSON.parse(raw!).refreshing).toBe(true);
      await Promise.resolve();
      if (mode === "offline") throw new Error("private upstream detail");
      if (mode === "denied") return Response.json({}, { status: 400 });
      return Response.json({
        access_token: "new.access.token",
        refresh_token: "rotated-refresh",
        token_type: "Bearer",
        expires_in: 900,
      });
    }
    return Response.json({
      workspaceId: (mode === "foreign" ? "b" : "a").repeat(64),
    });
  }) as typeof fetch;
  const session = createAccountSession(config, vault, send, () => time);
  return {
    session,
    vault,
    writes,
    send,
    raw: () => raw,
    exchanges: () => exchanges,
    setRaw: (value: string) => {
      raw = value;
    },
    setMode: (value: string) => {
      mode = value;
    },
    advance: () => {
      time += 70_000;
    },
    now: () => time,
  };
}
const saved = {
  accessToken: "old.access.token",
  refreshToken: "old-refresh",
  expiresAt: 61_000,
  workspaceId: "a".repeat(64),
};
describe("native account session", () => {
  test("stores tokens only through the vault and concurrent requests share one rotating refresh", async () => {
    const f = fixture();
    await f.session.save(saved);
    expect(f.raw()).toContain("old-refresh");
    f.advance();
    const results = await Promise.all(
      Array.from({ length: 8 }, () => f.session.authorization("a".repeat(64))),
    );
    expect(results.every((r) => r.token === "new.access.token")).toBe(true);
    expect(f.exchanges()).toBe(1);
    expect(JSON.parse(f.raw()!).refreshToken).toBe("rotated-refresh");
    expect(JSON.parse(f.raw()!).refreshing).toBe(false);
  });
  test("a restarted in-flight refresh is discarded without replay", async () => {
    const f = fixture();
    await f.session.save(saved);
    f.setRaw(JSON.stringify({ ...JSON.parse(f.raw()!), refreshing: true }));
    const restarted = createAccountSession(config, f.vault, f.send, f.now);
    expect(await restarted.status()).toEqual({ signedIn: false });
    await expect(restarted.authorization()).rejects.toThrow("Sign in");
    expect(f.exchanges()).toBe(0);
    expect(f.raw()).toBeNull();
  });
  test("network ambiguity, rejected refresh and changed account all require new sign-in without retries", async () => {
    for (const mode of ["offline", "denied", "foreign"]) {
      const f = fixture();
      await f.session.save(saved);
      f.advance();
      f.setMode(mode);
      await expect(f.session.authorization()).rejects.toThrow("Sign in");
      await expect(f.session.authorization()).rejects.toThrow("Sign in");
      expect(f.exchanges()).toBe(1);
      expect(f.raw()).toBeNull();
    }
  });
  test("a saved device cannot borrow a different account's token", async () => {
    const f = fixture();
    await f.session.save(saved);
    await expect(f.session.authorization("b".repeat(64))).rejects.toThrow();
    expect(f.exchanges()).toBe(0);
    expect((await f.session.authorization("a".repeat(64))).token).toBe(
      "old.access.token",
    );
  });
  test("corrupt and foreign-configuration vault entries are cleared", async () => {
    for (const raw of [
      "invalid-json",
      JSON.stringify({
        ...saved,
        version: 1,
        origin: "https://foreign.example",
        issuer: config.issuer,
        clientId: config.clientId,
        refreshing: false,
      }),
    ]) {
      const f = fixture();
      f.setRaw(raw);
      expect(await f.session.status()).toEqual({ signedIn: false });
      expect(f.raw()).toBeNull();
    }
  });
  test("sign-out removes local credentials and denies queued requests", async () => {
    const f = fixture();
    await f.session.save(saved);
    const logout = f.session.signOut();
    const queued = f.session.authorization();
    expect(await logout).toEqual({ revoked: true });
    await expect(queued).rejects.toThrow();
    expect(f.raw()).toBeNull();
    expect(await f.session.status()).toEqual({ signedIn: false });
  });
});

test("saved account refresh remains pinned when caller configuration is mutated", async () => {
  const f = fixture();
  const changed = { ...config };
  const urls: string[] = [];
  const send = (async (
    input: Parameters<typeof fetch>[0],
    init?: RequestInit,
  ) => {
    urls.push(String(input));
    return f.send(input, init);
  }) as typeof fetch;
  const session = createAccountSession(changed, f.vault, send, f.now);
  await session.save(saved);
  f.advance();
  changed.cloudOrigin = "https://foreign.example";
  changed.issuer = "https://foreign-issuer.example/";
  await session.authorization();
  expect(urls).toEqual([
    "https://tenant.auth0.com/oauth/token",
    "https://cloud.example/v1/session",
  ]);
});

test("sign-out during refresh invalidates its result and clears credentials", async () => {
  const f = fixture();
  let release!: (response: Response) => void;
  let started!: () => void;
  const entering = new Promise<void>((resolve) => {
    started = resolve;
  });
  const send = (async (
    input: Parameters<typeof fetch>[0],
    init?: RequestInit,
  ) => {
    if (String(input).endsWith("/oauth/token")) {
      started();
      return new Promise<Response>((resolve) => {
        release = resolve;
      });
    }
    return f.send(input, init);
  }) as typeof fetch;
  const session = createAccountSession(config, f.vault, send, f.now);
  await session.save(saved);
  f.advance();
  const authorization = session.authorization().then(
    () => null,
    (error) => error,
  );
  await entering;
  const logout = session.signOut(async () => false);
  release(
    Response.json({
      access_token: "new.access.token",
      refresh_token: "rotated",
      token_type: "Bearer",
      expires_in: 900,
    }),
  );
  expect(await authorization).toBeInstanceOf(Error);
  expect(await logout).toEqual({ revoked: false });
  expect(f.raw()).toBeNull();
  expect(await session.status()).toEqual({ signedIn: false });
});
