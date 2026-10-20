import { expect, test } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { Account } from "./auth";
import { ManagedApps, composioUser } from "./apps";
import { ServiceStore } from "./service-store";
const account = (letter = "a"): Account => ({ workspaceId: letter.repeat(64), subject: letter, issuer: "https://issuer.example/", scopes: new Set(["ion:gateway"]) });
const operationId = "00000000-0000-4000-8000-000000000001";
const request = (path = "catalog", body?: unknown) => new Request("https://api.example/v1/apps/" + path, {
  method: body === undefined ? "GET" : "POST", ...(body === undefined ? {} : { body: JSON.stringify(body) }),
});
async function fixture(run: (store: ServiceStore) => Promise<void>) {
  const directory = mkdtempSync(join(tmpdir(), "ion-apps-")), store = new ServiceStore(directory);
  try { await run(store); } finally { store.close(); rmSync(directory, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 }); }
}
function provider() {
  const calls: { path: string; method: string; body: any }[] = [];
  const sessions = new Map<string, any>();
  let foreignAccount = false, failLink = false, linkPosts = 0, revokes = 0;
  const send = (async (input: any, init: any) => {
    expect(init.headers["x-api-key"]).toBe("server-project-key"); expect(init.redirect).toBe("manual");
    const url = new URL(String(input)), path = url.pathname.replace("/api/v3.1", "");
    const body = init.body ? JSON.parse(init.body) : undefined;
    calls.push({ path, method: init.method, body });
    if (path === "/tool_router/session") {
      expect(body.instant).toBe(false); expect(body.workbench.enable).toBe(false); expect(body.proxy_execute.enable).toBe(false);
      const id = "trs_" + body.user_id; const result = { session_id: id, config: body, mcp: { url: "private-capability" } };
      sessions.set(id, result); return Response.json(result);
    }
    if (path.endsWith("/toolkits")) {
      const id = path.split("/")[3]!, user = sessions.get(id).config.user_id;
      return Response.json({ items: [
        { name: "Own app", slug: "own_app", enabled: true, composio_managed_auth_schemes: ["OAUTH2"], connected_account: { id: "ca_own", user_id: user }, auth_state: "never-forward" },
        { name: "Shared app", slug: "shared_app", enabled: true, composio_managed_auth_schemes: ["OAUTH2"], connected_account: { id: "ca_operator", user_id: "operator-personal-user" } },
      ], next_cursor: "page2" });
    }
    if (path === "/connected_accounts/ca_own") return Response.json({ id: "ca_own", user_id: foreignAccount ? "operator-personal-user" : composioUser(account()), toolkit: { slug: "own_app" }, status: "ACTIVE", state: { access_token: "private-provider-token" } });
    if (path.endsWith("/link")) {
      linkPosts++;
      if (failLink) throw new Error("key must not escape");
      return Response.json({ redirect_url: "https://app.composio.dev/link/opaque", connected_account_id: "ca_own", link_token: "never-forward" });
    }
    if (path.endsWith("/revoke")) { revokes++; return Response.json({ connected_account: { id: "ca_own", status: "REVOKED" }, revoked_tokens: ["access_token"] }); }
    const session = sessions.get(path.split("/")[3]!);
    if (session) return Response.json(session);
    return new Response(null, { status: 404 });
  });
  return { send, calls, sessions, setForeign: () => { foreignAccount = true; }, failLink: () => { failLink = true; }, get linkPosts() { return linkPosts; }, get revokes() { return revokes; } };
}
test("managed catalog uses stable authenticated users, redacts capabilities and hides operator accounts", () => fixture(async store => {
  const upstream = provider(), apps = new ManagedApps(store, "server-project-key", upstream.send);
  const a = await (await apps.handle(account(), request())).json() as { items: { account?: { id: string } }[] };
  expect(a.items[0]!.account!.id).toBe("ca_own"); expect(a.items[1]!.account).toBeUndefined();
  const text = JSON.stringify(a);
  for (const secret of ["private-provider-token", "private-capability", "server-project-key", "never-forward", "ca_operator", "operator-personal-user"]) expect(text).not.toContain(secret);
  const second = new ManagedApps(store, "server-project-key", upstream.send);
  await second.handle(account(), request());
  expect(upstream.calls.filter(c => c.path === "/tool_router/session")).toHaveLength(1);
  upstream.setForeign();
  await expect(second.handle(account(), request())).rejects.toThrow("unavailable");
}));
test("simultaneous session initialization is coalesced; a second account gets another identity", () => fixture(async store => {
  const upstream = provider(), apps = new ManagedApps(store, "server-project-key", upstream.send);
  await Promise.all([apps.handle(account(), request()), apps.handle(account(), request())]);
  expect(upstream.calls.filter(c => c.path === "/tool_router/session")).toHaveLength(1);
  await expect(apps.handle(account("b"), request())).rejects.toThrow("unavailable"); // provider fixture deliberately returns a's connection
  expect(upstream.calls.filter(c => c.path === "/tool_router/session").map(c => c.body.user_id)).toEqual([composioUser(account()), composioUser(account("b"))]);
}));
test("restarted uncertain session initialization never silently creates another session", () => fixture(async store => {
  let calls = 0;
  const apps = new ManagedApps(store, "key", (async () => { calls++; throw new Error("private-provider-error"); }));
  await expect(apps.handle(account(), request())).rejects.toThrow("could not be reached");
  await expect(new ManagedApps(store, "key", (async () => { calls++; return Response.json({}); })).handle(account(), request())).rejects.toThrow("reconciliation");
  expect(calls).toBe(1);
}));
test("connection operations bind exact payloads, survive restart and do not repeat effects", () => fixture(async store => {
  const upstream = provider(), apps = new ManagedApps(store, "server-project-key", upstream.send);
  const input = { toolkit: "own_app", operationId };
  const first = await (await apps.handle(account(), request("connect", input))).json() as { redirectUrl: string };
  expect(first.redirectUrl).toBe("https://app.composio.dev/link/opaque");
  expect(JSON.stringify(first)).not.toContain("link_token");
  await new ManagedApps(store, "server-project-key", upstream.send).handle(account(), request("connect", input));
  expect(upstream.linkPosts).toBe(1);
  await expect(apps.handle(account(), request("connect", { ...input, toolkit: "shared_app" }))).rejects.toThrow("details changed");
  await apps.handle(account(), request("disconnect", { accountId: "ca_own", operationId: operationId.replace(/1$/, "2") }));
  expect(upstream.revokes).toBe(1);
  upstream.setForeign();
  await expect(apps.handle(account("b"), request("disconnect", { accountId: "ca_own", operationId }))).rejects.toThrow("unavailable");
  expect(upstream.revokes).toBe(1);
}));
test("uncertain link is never retried and provider credentials/errors are not returned", () => fixture(async store => {
  const upstream = provider(); upstream.failLink();
  const apps = new ManagedApps(store, "server-project-key", upstream.send), input = { toolkit: "own_app", operationId };
  await expect(apps.handle(account(), request("connect", input))).rejects.toThrow("could not be reached");
  await expect(new ManagedApps(store, "server-project-key", upstream.send).handle(account(), request("connect", input))).rejects.toThrow("uncertain outcome");
  expect(upstream.linkPosts).toBe(1);
}));
test("limits survive recreation and invalid configuration/identity overrides fail closed", () => fixture(async store => {
  const upstream = provider(), apps = new ManagedApps(store, "server-project-key", upstream.send, () => 1000);
  await expect(apps.handle(account(), request("connect", { toolkit: "own_app", user_id: "operator", operationId }))).rejects.toThrow("catalog");
  await expect(apps.handle(account(), request("catalog", { session_id: "foreign" }))).rejects.toThrow("filters");
  for (let i = 0; i < 18; i++) await expect(apps.handle(account(), request("execute", {}))).rejects.toThrow("Unknown");
  await expect(new ManagedApps(store, "server-project-key", upstream.send, () => 1000).handle(account(), request())).rejects.toThrow("limit");
  expect(upstream.calls).toHaveLength(0);
  await expect(new ManagedApps(store).handle(account(), request())).rejects.toThrow("not configured");
}));
