import { afterEach, expect, test } from "bun:test";
import { gatewayTasks } from "./gateway-client";
import { configureGatewayAccountAuthorization } from "./gateway-account-auth";
const original = globalThis.fetch;
afterEach(() => {
  globalThis.fetch = original;
  configureGatewayAccountAuthorization();
});
const pairing = {
  baseUrl: "https://cloud.example",
  id: "11111111-1111-4111-8111-111111111111",
  token: "device-token-only",
  expiresAt: Date.now() + 60_000,
  grants: ["taskRead"],
  cloudWorkspaceId: "a".repeat(64),
};
test("managed requests keep account and device grants separate and obtain renewable tokens before dispatch", async () => {
  let calls = 0,
    authorizations = 0;
  configureGatewayAccountAuthorization(async (origin, workspace) => {
    expect(origin).toBe(pairing.baseUrl);
    expect(workspace).toBe(pairing.cloudWorkspaceId);
    return "account-token-" + ++authorizations;
  });
  globalThis.fetch = (async (url, init) => {
    calls++;
    expect(String(url)).toBe(
      "https://cloud.example/v1/gateway/v1/device/tasks",
    );
    const headers = new Headers(init!.headers);
    expect(headers.get("authorization")).toBe("Bearer account-token-" + calls);
    expect(headers.get("x-ion-device-token")).toBe("device-token-only");
    expect(headers.get("x-waku-device-id")).toBe(pairing.id);
    expect(init!.redirect).toBe("error");
    return Response.json({ tasks: [], publishedAt: null });
  }) as typeof fetch;
  await gatewayTasks(pairing);
  await gatewayTasks(pairing);
  expect(calls).toBe(2);
});
test("missing login and a foreign account/origin reject before any device request", async () => {
  let calls = 0;
  globalThis.fetch = (async () => {
    calls++;
    return Response.json({});
  }) as unknown as typeof fetch;
  await expect(gatewayTasks(pairing)).rejects.toThrow("Sign in");
  configureGatewayAccountAuthorization(async () => {
    throw new Error("Different account");
  });
  await expect(gatewayTasks(pairing)).rejects.toThrow("Different account");
  expect(calls).toBe(0);
});
test("failed managed writes/reads are not automatically replayed", async () => {
  let calls = 0;
  configureGatewayAccountAuthorization(async () => "account-token");
  globalThis.fetch = (async () => {
    calls++;
    throw new Error("lost response");
  }) as unknown as typeof fetch;
  await expect(gatewayTasks(pairing)).rejects.toThrow("lost response");
  expect(calls).toBe(1);
});
