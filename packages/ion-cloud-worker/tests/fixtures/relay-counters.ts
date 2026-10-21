import { DurableObject } from "cloudflare:workers";
import { createGatewayRuntime } from "../../../personal-gateway/src/server";
import { durableDatabase } from "../../src/database";

// Local test worker only. Never exported by the production entry point.
export class CounterRelay extends DurableObject {
  private now = 1_800_000_000_000;
  private counters = { requests: 0, statements: 0, rowsRead: 0, rowsWritten: 0 };
  private database: ReturnType<typeof durableDatabase>;
  private runtime: ReturnType<typeof createGatewayRuntime>;
  constructor(ctx: DurableObjectState, env: unknown) {
    super(ctx, env);
    Date.now = () => this.now;
    const storage = {
      sql: {
        exec: (sql: string, ...bindings: any[]) => {
          const cursor = ctx.storage.sql.exec(sql, ...bindings);
          const rows = cursor.toArray();
          this.counters.statements++;
          this.counters.rowsRead += cursor.rowsRead;
          this.counters.rowsWritten += cursor.rowsWritten;
          return { toArray: () => rows };
        },
      },
      transactionSync: <T>(operation: () => T) => ctx.storage.transactionSync(operation),
    };
    this.database = durableDatabase(storage as DurableObjectStorage);
    this.runtime = this.open();
  }
  private open() {
    return createGatewayRuntime({
      databasePath: this.database,
      ownerToken: "local_counter_owner_" + "x".repeat(48),
      allowedTools: [], periodicPush: false,
      pushFetch: async () => { throw new Error("Unexpected external request in local SQL test"); },
    });
  }
  override async fetch(request: Request) {
    const path = new URL(request.url).pathname;
    if (path === "/__reopen-legacy-schema" && request.method === "POST") {
      // Disposable local fixture only: remove the newly added receipt table to
      // represent a pre-upgrade DB. Keep tasks, grants and commands.
      // Production AccountRelay has no such route or schema reset operation.
      await this.runtime.close();
      this.ctx.storage.sql.exec("DROP TABLE relay_task_snapshots");
      this.runtime = this.open();
      return Response.json({ reopened: true });
    }
    if (path === "/__counters") {
      if (request.method === "DELETE")
        this.counters = { requests: 0, statements: 0, rowsRead: 0, rowsWritten: 0 };
      return Response.json(this.counters);
    }
    const now = request.headers.get("x-test-now");
    if (now) this.now = Number(now);
    this.counters.requests++;
    const response = await this.runtime.fetch(request);
    // Same flush/pending SQL as AccountRelay, awaited for exact attribution to
    // the current local cycle. No alarm, real phone or provider is involved.
    await this.runtime.flush();
    this.runtime.hasPendingPush();
    return response;
  }
}
export default {
  fetch(request: Request, env: { COUNTERS: DurableObjectNamespace }) {
    return env.COUNTERS.get(env.COUNTERS.idFromName("isolated-counters")).fetch(request);
  },
};
