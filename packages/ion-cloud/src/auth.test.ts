import { beforeAll, describe, expect, test } from "bun:test";
import {
  createLocalJWKSet,
  exportJWK,
  generateKeyPair,
  SignJWT,
  type JWTPayload,
} from "jose";
import { createAuthenticator, CloudFailure } from "./auth";
import { ionAuth0, validateConfig } from "./config";

const config = { ...ionAuth0, publicOrigin: "https://cloud.ion.example" };
let keys: Awaited<ReturnType<typeof generateKeyPair>>;
let authenticate: ReturnType<typeof createAuthenticator>;
beforeAll(async () => {
  keys = await generateKeyPair("RS256");
  const key = await exportJWK(keys.publicKey);
  authenticate = createAuthenticator(
    config,
    createLocalJWKSet({ keys: [{ ...key, kid: "test" }] }),
  );
});
const request = (token: string) =>
  new Request("https://cloud.ion.example/v1/session", {
    headers: { Authorization: "Bearer " + token },
  });
async function sign(
  overrides: JWTPayload = {},
  header: Record<string, unknown> = {},
) {
  const now = Math.floor(Date.now() / 1000);
  return new SignJWT({
    iss: config.issuer,
    aud: config.audience,
    sub: "auth0|one",
    azp: config.desktopClientId,
    iat: now,
    exp: now + 900,
    scope: "openid ion:gateway",
    ...overrides,
  })
    .setProtectedHeader({ alg: "RS256", kid: "test", ...header })
    .sign(keys.privateKey);
}
async function status(token: string, scopes: string[] = []) {
  try {
    await authenticate(request(token), scopes);
    return 200;
  } catch (error) {
    return (error as CloudFailure).status;
  }
}

describe("Auth0 native user API tokens", () => {
  test("desktop and phone map the same issuer/subject to one workspace; email is irrelevant", async () => {
    const one = await authenticate(
      request(await sign({ email: "old@example.com" })),
      ["ion:gateway"],
    );
    const phone = await authenticate(
      request(
        await sign({ azp: config.mobileClientId, email: "new@example.com" }),
      ),
    );
    expect(one.workspaceId).toBe(phone.workspaceId);
    expect(one.workspaceId).toMatch(/^[a-f0-9]{64}$/);
    expect(
      (
        await authenticate(
          request(await sign({ sub: "auth0|two", email: "old@example.com" })),
        )
      ).workspaceId,
    ).not.toBe(one.workspaceId);
  });
  test("rejects wrong issuer, audience, expired/future claims and missing required claims", async () => {
    for (const override of [
      { iss: "https://untrusted.example/" },
      { aud: config.desktopClientId },
      { aud: "https://other-api" },
      { exp: 1 },
      { exp: undefined },
      { nbf: Math.floor(Date.now() / 1000) + 60 },
      { iat: Math.floor(Date.now() / 1000) + 60 },
      { iat: undefined },
      { sub: "" },
      { sub: undefined },
    ])
      expect(await status(await sign(override))).toBe(401);
  });
  test("rejects foreign clients and machine tokens, accepts RFC client_id profile", async () => {
    for (const override of [
      { azp: "foreign-client" },
      { azp: undefined },
      { sub: "machine@clients" },
      { gty: "client-credentials" },
      { scope: ["ion:gateway"] },
      { client_id: config.mobileClientId },
    ])
      expect(await status(await sign(override))).toBe(401);
    expect(
      await status(
        await sign({ azp: undefined, client_id: config.mobileClientId }),
      ),
    ).toBe(200);
  });
  test("enforces scopes independently from a valid signature", async () => {
    expect(await status(await sign({ scope: "openid" }), ["ion:gateway"])).toBe(
      403,
    );
    expect(await status(await sign(), ["ion:gateway"])).toBe(200);
  });
  test("rejects unsigned, symmetric, tampered and unknown-key tokens; ignores token JWKS hints", async () => {
    const valid = await sign();
    const segments = valid.split(".");
    const altered =
      segments[0] +
      "." +
      Buffer.from(JSON.stringify({ sub: "victim" })).toString("base64url") +
      "." +
      segments[2];
    expect(await status(altered)).toBe(401);
    expect(await status(valid.slice(0, valid.lastIndexOf(".") + 1))).toBe(401);
    expect(
      await status(
        await sign(
          {},
          { kid: "unknown", jku: "https://attacker.example/keys" },
        ),
      ),
    ).toBe(401);
    const symmetric = await new SignJWT({
      iss: config.issuer,
      aud: config.audience,
    })
      .setProtectedHeader({ alg: "HS256" })
      .sign(new Uint8Array(32));
    expect(await status(symmetric)).toBe(401);
    expect(await status("opaque-device-token")).toBe(401);
  });
  test("configuration rejects origin paths and client IDs used as audience", () => {
    expect(() => validateConfig(config)).not.toThrow();
    for (const changes of [
      { issuer: "http://issuer.example/" },
      { issuer: config.issuer + "path" },
      { publicOrigin: "https://cloud.ion.example/path" },
      { publicOrigin: "https://user:password@cloud.ion.example" },
      { audience: config.desktopClientId },
    ])
      expect(() => validateConfig({ ...config, ...changes })).toThrow();
  });
});
