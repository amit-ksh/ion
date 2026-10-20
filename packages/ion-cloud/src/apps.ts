import { CloudFailure, type Account } from "./auth";
import { ServiceStore, readJson } from "./service-store";

const base = "https://backend.composio.dev/api/v3.1";
const identifier = (v: unknown): v is string => typeof v === "string" && /^[A-Za-z0-9_-]{1,160}$/.test(v);
const slug = (v: unknown): v is string => typeof v === "string" && /^[a-z][a-z0-9_]{0,99}$/.test(v);
const invalid = () => new CloudFailure(502, "Invalid app service response");
interface Session { id?: string; state: "starting" | "ready" }
interface Operation { signature: string; state: "uncertain" | "complete"; result?: unknown }
export function composioUser(account: Account): string {
  if (!/^[a-f0-9]{64}$/.test(account.workspaceId)) throw new CloudFailure(401, "Sign in required");
  // Same verified issuer/subject mapping as Ion's gateway; never caller-selected.
  return "ion_" + account.workspaceId;
}
function safeLink(value: unknown): string {
  if (typeof value !== "string" || value.length > 4096) throw invalid();
  const url = new URL(value);
  if (url.protocol !== "https:" || url.username || url.password ||
      !["composio.dev", "composio.app"].some(h => url.hostname === h || url.hostname.endsWith("." + h))) throw invalid();
  return value;
}
/** Backend-only connection management. Does not expose session/MCP capabilities,
 * project-wide accounts, credential state, proxy calls or tool execution. */
export class ManagedApps {
  readonly available: boolean;
  private pending = new Map<string, Promise<string>>();
  constructor(private store: ServiceStore, private key?: string,
    private send: (input: string, init: RequestInit) => Promise<Response> = (input, init) => globalThis.fetch(input, init), private clock = Date.now) { this.available = !!key; }
  private rate(account: Account) {
    const window = Math.floor(this.clock() / 60_000);
    this.store.transaction(() => {
      for (const [scope, maximum] of [["_operator", 120], [account.workspaceId, 20]] as const) {
        const old = this.store.get<{ window: number; count: number }>(scope, "apps-rate", "minute");
        const count = old?.window === window ? old.count : 0;
        if (count >= maximum) throw new CloudFailure(429, "App connection request limit reached; retry later");
        this.store.put(scope, "apps-rate", "minute", { window, count: count + 1 });
      }
    });
  }
  private async call(path: string, method = "GET", body?: unknown): Promise<any> {
    let response: Response;
    try {
      response = await this.send(base + path, {
        method, redirect: "manual", signal: AbortSignal.timeout(15_000),
        headers: { "x-api-key": this.key!, accept: "application/json", ...(body === undefined ? {} : { "Content-Type": "application/json" }) },
        ...(body === undefined ? {} : { body: JSON.stringify(body) }),
      });
    } catch { throw new CloudFailure(503, "App service could not be reached; check connection status before trying again"); }
    if (!response.ok) {
      await response.body?.cancel();
      throw new CloudFailure(response.status === 429 ? 429 : 502, "App service request failed; check connection status before trying again");
    }
    try {
      return await readJson(new Request("https://response.invalid", { method: "POST", body: response.body, duplex: "half" } as RequestInit), 1_000_000);
    } catch { throw invalid(); }
  }
  private async session(account: Account): Promise<string> {
    const user = composioUser(account);
    const pending = this.pending.get(user);
    if (pending) return pending;
    const operation = this.openSession(account, user);
    this.pending.set(user, operation);
    try { return await operation; } finally { this.pending.delete(user); }
  }
  private async openSession(account: Account, user: string): Promise<string> {
    const saved = this.store.get<Session>(account.workspaceId, "apps", "session");
    if (saved) {
      if (saved.state !== "ready" || !identifier(saved.id))
        throw new CloudFailure(409, "App session creation has an uncertain outcome; operator reconciliation is required");
      const current = await this.call("/tool_router/session/" + saved.id);
      if (current.session_id !== saved.id || current.config?.user_id !== user || current.config?.instant !== false ||
          current.config?.workbench?.enable !== false || current.config?.proxy_execute?.enable !== false) throw invalid();
      return saved.id;
    }
    // A crash or ambiguous response cannot silently create another provider session.
    this.store.put(account.workspaceId, "apps", "session", { state: "starting" } satisfies Session);
    const created = await this.call("/tool_router/session", "POST", {
      user_id: user, instant: false, workbench: { enable: false }, proxy_execute: { enable: false },
      manage_connections: { enable: true },
    });
    if (!identifier(created.session_id) || created.config?.user_id !== user || created.config?.instant !== false ||
        created.config?.workbench?.enable !== false || created.config?.proxy_execute?.enable !== false) throw invalid();
    this.store.put(account.workspaceId, "apps", "session", { state: "ready", id: created.session_id } satisfies Session);
    return created.session_id;
  }
  private async owned(account: Account, id: string) {
    const data = await this.call("/connected_accounts/" + id);
    if (data.id !== id || data.user_id !== composioUser(account))
      throw new CloudFailure(404, "App connection is unavailable");
    if (!slug(data.toolkit?.slug) || typeof data.status !== "string" || data.status.length > 64) throw invalid();
    return { id, toolkit: data.toolkit.slug as string, status: data.status as string };
  }
  private async once(account: Account, body: any, signature: string, run: () => Promise<unknown>) {
    if (typeof body.operationId !== "string" || !/^[A-Za-z0-9_-]{32,80}$/.test(body.operationId))
      throw new CloudFailure(400, "An app operation ID is required");
    const saved = this.store.get<Operation>(account.workspaceId, "apps-operation", body.operationId);
    if (saved) {
      if (saved.signature !== signature) throw new CloudFailure(409, "App operation details changed");
      if (saved.state !== "complete") throw new CloudFailure(409, "App operation has an uncertain outcome; check connections before proceeding");
      return saved.result;
    }
    if (this.store.all(account.workspaceId, "apps-operation").length >= 256)
      throw new CloudFailure(429, "App operation history capacity reached; operator review is required");
    this.store.put(account.workspaceId, "apps-operation", body.operationId, { signature, state: "uncertain" } satisfies Operation);
    const result = await run();
    this.store.put(account.workspaceId, "apps-operation", body.operationId, { signature, state: "complete", result } satisfies Operation);
    return result;
  }
  async handle(account: Account, request: Request): Promise<Response> {
    if (!this.available) throw new CloudFailure(503, "Managed app connections are not configured yet");
    composioUser(account); this.store.admit(account.workspaceId); this.rate(account);
    const path = new URL(request.url).pathname;
    const body = request.method === "POST" ? await readJson(request, 4096) : {};
    if (!body || typeof body !== "object" || Array.isArray(body)) throw new CloudFailure(400, "Invalid app request");
    let result: unknown;
    if (path === "/v1/apps/catalog" && ["GET", "POST"].includes(request.method)) {
      if (Object.keys(body).some(k => !["search", "cursor", "connectedOnly"].includes(k)) ||
          (body.connectedOnly !== undefined && typeof body.connectedOnly !== "boolean") ||
          (body.search !== undefined && (typeof body.search !== "string" || body.search.length > 256)) ||
          (body.cursor !== undefined && (typeof body.cursor !== "string" || body.cursor.length > 1024))) throw new CloudFailure(400, "Invalid app filters");
      const id = await this.session(account);
      const query = new URLSearchParams({ limit: "50" });
      if (body.search) query.set("search", body.search);
      if (body.cursor) query.set("cursor", body.cursor);
      if (body.connectedOnly) query.set("is_connected", "true");
      const data = await this.call("/tool_router/session/" + id + "/toolkits?" + query);
      if (!Array.isArray(data.items) || data.items.length > 50 ||
          (data.next_cursor != null && (typeof data.next_cursor !== "string" || data.next_cursor.length > 1024))) throw invalid();
      const items = [];
      for (const row of data.items) {
        if (!slug(row.slug) || typeof row.name !== "string" || row.name.length > 256) throw invalid();
        // A shared project must never turn the operator's connections into user accounts.
        const connection = row.connected_account;
        const own = connection?.user_id === composioUser(account) && identifier(connection?.id)
          ? await this.owned(account, connection.id) : undefined;
        items.push({ toolkit: row.slug, name: row.name, description: String(row.meta?.description ?? "").slice(0, 512),
          connectable: row.enabled !== false && Array.isArray(row.composio_managed_auth_schemes) && row.composio_managed_auth_schemes.length > 0,
          ...(own ? { account: own } : {}) });
      }
      result = { managed: true, items, nextCursor: data.next_cursor || null };
    } else if (path === "/v1/apps/connect" && request.method === "POST") {
      if (!slug(body.toolkit) || Object.keys(body).some(k => !["toolkit", "operationId"].includes(k))) throw new CloudFailure(400, "Choose an app from the catalog");
      const id = await this.session(account);
      result = await this.once(account, body, "connect:" + body.toolkit, async () => {
        const linked = await this.call("/tool_router/session/" + id + "/link", "POST", { toolkit: body.toolkit });
        if (!identifier(linked.connected_account_id)) throw invalid();
        const own = await this.owned(account, linked.connected_account_id);
        if (own.toolkit !== body.toolkit) throw invalid();
        return { account: own, redirectUrl: safeLink(linked.redirect_url) };
      });
    } else if (path === "/v1/apps/disconnect" && request.method === "POST") {
      if (!identifier(body.accountId) || Object.keys(body).some(k => !["accountId", "operationId"].includes(k))) throw new CloudFailure(400, "Choose a connected app account");
      const own = await this.owned(account, body.accountId);
      result = await this.once(account, body, "disconnect:" + own.id, async () => {
        const data = await this.call("/connected_accounts/" + own.id + "/revoke", "POST");
        if (data.connected_account?.id !== own.id || data.connected_account?.status !== "REVOKED") throw invalid();
        return { account: { ...own, status: "REVOKED" } };
      });
    } else throw new CloudFailure(404, "Unknown app operation");
    return Response.json(result, { headers: { "Cache-Control": "no-store", "X-Content-Type-Options": "nosniff" } });
  }
}
