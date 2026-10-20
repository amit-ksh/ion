import { describe, expect, test } from "bun:test";
import { createHash, randomBytes } from "node:crypto";
import { createNativeLogin, type LoginCrypto } from "./login";
const config = {
  issuer: "https://dev-amit-ksh.au.auth0.com/",
  audience: "https://ion-cloud-api",
  clientId: "M7iCZjpo5bzOVO2E5nrvKzDJ00p3ZWy0",
  redirectUri: "ion://auth/callback",
  cloudOrigin: "https://cloud.ion.example",
};
const crypto: LoginCrypto = {
  randomBytes: async (size) => randomBytes(size),
  sha256: async (value) => createHash("sha256").update(value).digest(),
};
function setup() {
  const calls: Array<{ url: string; init?: RequestInit }> = [];
  const send = (async (
    url: Parameters<typeof fetch>[0],
    init?: RequestInit,
  ) => {
    calls.push({ url: String(url), init });
    return String(url).includes("oauth/token")
      ? Response.json({
          access_token: "signed.access.token",
          token_type: "Bearer",
          expires_in: 900,
          refresh_token: "rotating-secret",
          id_token: "deliberately-unused",
        })
      : Response.json({ workspaceId: "a".repeat(64) });
  }) as typeof fetch;
  let time = 1000;
  const login = createNativeLogin(config, crypto, send, () => time);
  return {
    login,
    calls,
    advance: (ms: number) => {
      time += ms;
    },
  };
}
const callback = (authorize: string, changes?: Record<string, string>) => {
  const params = new URLSearchParams({
    state: new URL(authorize).searchParams.get("state")!,
    code: "one-use-code",
    ...changes,
  });
  return config.redirectUri + "?" + params.toString();
};
describe("native PKCE transport", () => {
  test("uses S256 and exact audience/callback; exchanges once without a client secret and verifies API identity", async () => {
    const { login, calls } = setup();
    const authorize = await login.begin();
    const url = new URL(authorize);
    expect(url.origin).toBe("https://dev-amit-ksh.au.auth0.com");
    expect(url.searchParams.get("response_type")).toBe("code");
    expect(url.searchParams.get("code_challenge_method")).toBe("S256");
    expect(url.searchParams.get("audience")).toBe(config.audience);
    const session = await login.finish(callback(authorize));
    expect(session.workspaceId).toBe("a".repeat(64));
    const exchange = new URLSearchParams(String(calls[0]!.init!.body));
    expect(exchange.has("client_secret")).toBe(false);
    expect(
      createHash("sha256")
        .update(exchange.get("code_verifier") ?? "")
        .digest("base64url"),
    ).toBe(url.searchParams.get("code_challenge") ?? "");
    expect(exchange.get("redirect_uri")).toBe(config.redirectUri);
    expect(new Headers(calls[1]!.init!.headers).get("authorization")).toBe(
      "Bearer signed.access.token",
    );
    await expect(login.finish(callback(authorize))).rejects.toThrow();
    expect(calls.length).toBe(2);
  });
  test("wrong state, duplicate parameters and wrong callback cause no exchange", async () => {
    const { login, calls } = setup();
    const authorize = await login.begin();
    for (const bad of [
      callback(authorize, { state: "foreign" }),
      callback(authorize).replace(
        "ion://auth/callback",
        "ion://foreign/callback",
      ),
      callback(authorize) + "&state=duplicate",
      callback(authorize) + "#access_token=bad",
    ])
      await expect(login.finish(bad)).rejects.toThrow();
    expect(calls.length).toBe(0);
    expect((await login.finish(callback(authorize))).workspaceId).toBe(
      "a".repeat(64),
    );
  });
  test("expiry, cancellation and superseded logins cannot exchange", async () => {
    const { login, calls, advance } = setup();
    const first = await login.begin(),
      second = await login.begin();
    await expect(login.finish(callback(first))).rejects.toThrow();
    advance(600_001);
    await expect(login.finish(callback(second))).rejects.toThrow();
    const third = await login.begin();
    login.cancel();
    await expect(login.finish(callback(third))).rejects.toThrow();
    expect(calls.length).toBe(0);
  });
  test("provider denial consumes the pending login without reflecting error detail", async () => {
    const { login, calls } = setup();
    const authorize = await login.begin();
    await expect(
      login.finish(
        callback(authorize, {
          error: "access_denied",
          error_description: "secret detail",
        }),
      ),
    ).rejects.toThrow("Sign-in could not be completed");
    await expect(login.finish(callback(authorize))).rejects.toThrow();
    expect(calls.length).toBe(0);
  });
  test("network failure is not replayed and untrusted redirects are rejected", async () => {
    let calls = 0;
    const send = (async () => {
      calls++;
      throw new Error("private upstream details");
    }) as unknown as typeof fetch;
    const login = createNativeLogin(config, crypto, send);
    const authorize = await login.begin();
    await expect(login.finish(callback(authorize))).rejects.toThrow(
      "Sign-in could not be completed",
    );
    await expect(login.finish(callback(authorize))).rejects.toThrow();
    expect(calls).toBe(1);
    expect(() =>
      createNativeLogin(
        { ...config, redirectUri: "http://attacker.example/callback" },
        crypto,
      ),
    ).toThrow();
  });
});

test("PKCE encoding works without browser btoa or TextEncoder globals", async () => {
  const previous = globalThis.btoa,
    encoder = globalThis.TextEncoder;
  try {
    Object.assign(globalThis, { btoa: undefined, TextEncoder: undefined });
    const { login } = setup();
    const url = new URL(await login.begin());
    expect(url.searchParams.get("state")).toMatch(/^[A-Za-z0-9_-]{43}$/);
    expect(url.searchParams.get("code_challenge")).toMatch(
      /^[A-Za-z0-9_-]{43}$/,
    );
  } finally {
    Object.assign(globalThis, { btoa: previous, TextEncoder: encoder });
  }
});
