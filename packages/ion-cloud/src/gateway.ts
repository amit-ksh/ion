import { randomBytes } from "node:crypto";
import { mkdir, lstat, realpath, readdir } from "node:fs/promises";
import { join, resolve } from "node:path";
import { createGatewayRuntime } from "../../personal-gateway/src/server";
import { CloudFailure, type Account } from "./auth";

/** Pilot isolation covers every relay table and cursor with a database per
 * account. An authentication/user database is unnecessary. */
export class ManagedGateways {
  private readonly runtimes = new Map<
    string,
    Promise<{
      runtime: ReturnType<typeof createGatewayRuntime>;
      ownerToken: string;
    }>
  >();
  private readonly directory: string;
  private ready?: Promise<{ root: string; workspaces: Set<string> }>;

  constructor(
    directory: string,
    private readonly limit = 64,
  ) {
    this.directory = resolve(directory);
    if (!Number.isInteger(limit) || limit < 1 || limit > 256)
      throw new Error("Invalid workspace limit");
  }

  private async initialize() {
    await mkdir(this.directory, { recursive: true, mode: 0o700 });
    const info = await lstat(this.directory);
    if (
      !info.isDirectory() ||
      info.isSymbolicLink() ||
      (process.platform !== "win32" && (info.mode & 0o077) !== 0)
    ) {
      throw new Error(
        "Cloud state directory must be private and must not be a link",
      );
    }
    const root = await realpath(this.directory);
    const workspaces = new Set(
      (await readdir(root))
        .filter((name) => /^[a-f0-9]{64}\.sqlite$/.test(name))
        .map((name) => name.slice(0, -7)),
    );
    return { root, workspaces };
  }

  private async open(workspaceId: string) {
    this.ready ??= this.initialize();
    const { root, workspaces } = await this.ready;
    // Admission includes persisted databases, so a restart cannot reset the
    // pilot account cap. Reservations occur before awaiting any file operation.
    if (!workspaces.has(workspaceId)) {
      if (workspaces.size >= this.limit)
        throw new CloudFailure(503, "Workspace capacity reached");
      workspaces.add(workspaceId);
    }
    const databasePath = join(root, workspaceId + ".sqlite");
    try {
      const file = await lstat(databasePath);
      if (
        !file.isFile() ||
        file.isSymbolicLink() ||
        (process.platform !== "win32" && (file.mode & 0o077) !== 0)
      ) {
        throw new Error("Workspace state must be a private regular file");
      }
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
    }
    const ownerToken = randomBytes(32).toString("base64url");
    return {
      ownerToken,
      // No project-wide Composio credentials or effects are enabled here.
      runtime: createGatewayRuntime({
        databasePath,
        ownerToken,
        allowedTools: [],
      }),
    };
  }

  async handle(
    account: Account,
    request: Request,
    path: string,
  ): Promise<Response> {
    if (!/^[a-f0-9]{64}$/.test(account.workspaceId))
      throw new Error("Invalid trusted workspace");
    if (!/^\/v1\/(owner\/|device\/|pair$|health$)/.test(path))
      throw new CloudFailure(404, "Not found");
    let pending = this.runtimes.get(account.workspaceId);
    if (!pending) {
      if (this.runtimes.size >= this.limit)
        throw new CloudFailure(503, "Workspace capacity reached");
      pending = this.open(account.workspaceId);
      this.runtimes.set(account.workspaceId, pending);
      void pending.catch(() => this.runtimes.delete(account.workspaceId));
    }
    const { runtime, ownerToken } = await pending;
    const target = new URL(request.url);
    target.pathname = path;
    // Only the verified JWT selects a workspace. Device grants remain an
    // additional requirement for remote operations.
    const headers = new Headers(request.headers);
    headers.delete("authorization");
    const deviceToken = headers.get("x-ion-device-token");
    headers.delete("x-ion-device-token");
    if (path.startsWith("/v1/owner/")) {
      headers.set("authorization", "Bearer " + ownerToken);
    } else if (path.startsWith("/v1/device/") && deviceToken) {
      headers.set("authorization", "Bearer " + deviceToken);
    }
    return runtime.fetch(
      new Request(new Request(target.toString(), request), { headers }),
    );
  }

  async close(): Promise<void> {
    const entries = await Promise.allSettled(this.runtimes.values());
    this.runtimes.clear();
    await Promise.all(
      entries.map((entry) =>
        entry.status === "fulfilled" ? entry.value.runtime.close() : undefined,
      ),
    );
  }
}
