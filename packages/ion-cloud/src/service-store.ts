import { openDatabase, type DatabaseSource, type SqlDatabase } from "../../personal-gateway/src/database";
import { mkdirSync, lstatSync } from "node:fs";
import { join } from "node:path";
import { createHash, randomBytes } from "node:crypto";
import { CloudFailure } from "./auth";

export const secret = () => randomBytes(32).toString("base64url");
export const hash = (value: string | Uint8Array) =>
  createHash("sha256").update(value).digest("hex");
export function requireValue(
  value: unknown,
  condition: boolean,
  message: string,
): asserts condition {
  if (!condition) throw new CloudFailure(400, message);
}
export async function readJson(
  request: Request,
  limit = 40_000_000,
): Promise<any> {
  const reader = request.body?.getReader();
  if (!reader) throw new CloudFailure(400, "Request body is required");
  const chunks: Uint8Array[] = [];
  let size = 0;
  try {
    while (true) {
      const next = await reader.read();
      if (next.done) break;
      size += next.value.byteLength;
      if (size > limit) {
        await reader.cancel();
        throw new CloudFailure(413, "Request exceeds its limit");
      }
      chunks.push(next.value);
    }
    return JSON.parse(Buffer.concat(chunks).toString("utf8"));
  } catch (error) {
    if (error instanceof CloudFailure) throw error;
    throw new CloudFailure(400, "Invalid request JSON");
  }
}
export class ServiceStore {
  private transactionDepth = 0;
  readonly db: SqlDatabase;
  constructor(directory: DatabaseSource) {
    if (typeof directory !== "string") { this.db = directory; this.initialize(); return; }
    mkdirSync(directory, { recursive: true, mode: 0o700 });
    const info = lstatSync(directory);
    if (
      !info.isDirectory() ||
      info.isSymbolicLink() ||
      (process.platform !== "win32" && info.mode & 0o077)
    )
      throw new Error("Cloud service state must be a private directory");
    const path = join(directory, "services.sqlite");
    try {
      const file = lstatSync(path);
      if (!file.isFile() || file.isSymbolicLink())
        throw new Error("Invalid service store");
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
    }
    this.db = openDatabase(path);
    this.db.query("PRAGMA busy_timeout=5000").get();
    this.db.query("PRAGMA journal_mode=WAL").get();
    this.db.run("PRAGMA synchronous=FULL");
    this.initialize();
  }
  private initialize() {
    this.db.run(
      "CREATE TABLE IF NOT EXISTS service_workspaces (workspace TEXT PRIMARY KEY)",
    );
    this.db.run(
      "CREATE TABLE IF NOT EXISTS service_objects (workspace TEXT NOT NULL, kind TEXT NOT NULL, id TEXT NOT NULL, value TEXT NOT NULL, PRIMARY KEY(workspace,kind,id))",
    );
    this.db.run(
      "CREATE TABLE IF NOT EXISTS service_capabilities (digest TEXT PRIMARY KEY, workspace TEXT NOT NULL, purpose TEXT NOT NULL, value TEXT NOT NULL, expires INTEGER NOT NULL)",
    );
    this.db.run(
      "CREATE INDEX IF NOT EXISTS service_capability_owner ON service_capabilities(workspace,purpose)",
    );
  }
  admit(workspace: string) {
    this.transaction(() => {
      if (
        this.db
          .query("SELECT 1 FROM service_workspaces WHERE workspace=?")
          .get(workspace)
      )
        return;
      const count = this.db
        .query<{ n: number }, []>(
          "SELECT count(*) AS n FROM service_workspaces",
        )
        .get()!.n;
      if (count >= 64)
        throw new CloudFailure(
          503,
          "Managed service workspace capacity reached",
        );
      this.db.run("INSERT INTO service_workspaces VALUES(?)", [workspace]);
    });
  }
  get<T>(workspace: string, kind: string, id: string): T | null {
    const row = this.db
      .query<{ value: string }, [string, string, string]>(
        "SELECT value FROM service_objects WHERE workspace=? AND kind=? AND id=?",
      )
      .get(workspace, kind, id);
    return row ? (JSON.parse(row.value) as T) : null;
  }
  all<T>(workspace: string, kind: string): T[] {
    return this.db
      .query<{ value: string }, [string, string]>(
        "SELECT value FROM service_objects WHERE workspace=? AND kind=? ORDER BY id",
      )
      .all(workspace, kind)
      .map((row) => JSON.parse(row.value) as T);
  }
  put(workspace: string, kind: string, id: string, value: unknown) {
    this.db.run(
      "INSERT INTO service_objects VALUES(?,?,?,?) ON CONFLICT(workspace,kind,id) DO UPDATE SET value=excluded.value",
      [workspace, kind, id, JSON.stringify(value)],
    );
  }
  delete(workspace: string, kind: string, id: string) {
    this.db.run(
      "DELETE FROM service_objects WHERE workspace=? AND kind=? AND id=?",
      [workspace, kind, id],
    );
  }
  transaction<T>(operation: () => T): T {
    // Budget settlement and lifecycle state changes share one SQL commit.
    // Inner operations join the current transaction and never issue a nested BEGIN.
    if (this.transactionDepth) return operation();
    return this.db.atomic(() => {
      this.transactionDepth++;
      try { return operation(); }
      finally { this.transactionDepth--; }
    });
  }
  capability(
    workspace: string,
    purpose: string,
    value: unknown,
    expires: number,
  ): string {
    const active = this.db
      .query<{ n: number }, [string, string, number]>(
        "SELECT count(*) AS n FROM service_capabilities WHERE workspace=? AND purpose=? AND expires>?",
      )
      .get(workspace, purpose, Date.now())!.n;
    if (active >= 128)
      throw new CloudFailure(429, "Too many open access sessions");
    const token = secret();
    this.db.run("INSERT INTO service_capabilities VALUES(?,?,?,?,?)", [
      hash(token),
      workspace,
      purpose,
      JSON.stringify(value),
      expires,
    ]);
    return token;
  }
  inspect(
    token: string,
    purpose: string,
    consume = false,
  ): { workspace: string; value: any; expires: number } {
    if (!/^[A-Za-z0-9_-]{43}$/.test(token))
      throw new CloudFailure(401, "Access expired");
    return this.transaction(() => {
      const row = this.db
        .query<
          { workspace: string; value: string; expires: number },
          [string, string]
        >(
          "SELECT workspace,value,expires FROM service_capabilities WHERE digest=? AND purpose=?",
        )
        .get(hash(token), purpose);
      if (!row || row.expires <= Date.now())
        throw new CloudFailure(401, "Access expired");
      if (consume)
        this.db.run("DELETE FROM service_capabilities WHERE digest=?", [
          hash(token),
        ]);
      return { ...row, value: JSON.parse(row.value) };
    });
  }
  expire() {
    this.db.run("DELETE FROM service_capabilities WHERE expires<?", [
      Date.now(),
    ]);
    // Operational counters only; source artifacts/releases are retained until
    // explicit cleanup. Daily speech accounting needs at most the recent days.
    this.db.run(
      "DELETE FROM service_objects WHERE kind='speech' AND json_extract(value,'$.expires')<?",
      [Date.now() - 172_800_000],
    );
  }
  close() {
    this.db.close(true);
  }
}
