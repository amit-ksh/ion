import { DurableObject } from "cloudflare:workers";
import { createGatewayRuntime } from "../../personal-gateway/src/server";
import { durableDatabase } from "./database";
import { secret } from "../../ion-cloud/src/service-store";
import type { Env } from "./env";
/** One SQL database per verified account; no user-supplied database selector. */
export class AccountRelay extends DurableObject<Env> {
  private readonly runtime: ReturnType<typeof createGatewayRuntime>;
  private readonly ownerToken = secret();
  constructor(ctx: DurableObjectState, env: Env) {
    super(ctx, env);
    this.runtime = createGatewayRuntime({
      databasePath: durableDatabase(ctx.storage),
      ownerToken: this.ownerToken,
      allowedTools: [],
      periodicPush: false,
    });
  }
  override async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url),
      path = url.pathname;
    if (!/^\/v1\/(owner\/|device\/|pair$|health$)/.test(path))
      return new Response("Not found", { status: 404 });
    const headers = new Headers(request.headers);
    headers.delete("authorization");
    const token = headers.get("x-ion-device-token");
    headers.delete("x-ion-device-token");
    if (path.startsWith("/v1/owner/"))
      headers.set("authorization", "Bearer " + this.ownerToken);
    else if (path.startsWith("/v1/device/") && token)
      headers.set("authorization", "Bearer " + token);
    const response = await this.runtime.fetch(
      new Request(request, { headers }),
    );
    // Background push work exists only while a request or short alarm needs it.
    this.ctx.waitUntil(this.flushAndSchedule());
    return response;
  }
  private async flushAndSchedule() {
    await this.runtime.flush();
    if (this.runtime.hasPendingPush())
      await this.ctx.storage.setAlarm(Date.now() + 30_000);
  }
  override async alarm() {
    await this.flushAndSchedule();
  }
}
