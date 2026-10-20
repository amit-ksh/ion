import { extname } from "node:path";
import { LocalBlobs, type BlobStore } from "./blobs";
import { createCipheriv, createDecipheriv, randomBytes } from "node:crypto";
import { CloudFailure, type Account } from "./auth";
import { ServiceStore, hash, readJson, secret } from "./service-store";

export type Runtime = "document" | "static" | "node" | "bun" | "python";
export interface BundleFile {
  path: string;
  sha256: string;
  bytes: number;
  content: string;
}
export interface BuildSettings {
  runtime: Runtime;
  entry: string;
  build: string[];
  start: string[];
  outputDirectory: string;
  port: number;
  healthPath: string;
  spa?: boolean;
}
export interface Site {
  id: string;
  sourceId: string;
  title: string;
  kind: "artifact" | "project";
  visibility: "private" | "public";
  active: string | null;
  created: number;
  stopped: boolean;
  requested?: string;
  pendingCancelled?: boolean;
}
export interface Release {
  id: string;
  siteId: string;
  operation: string;
  status:
    "uploading" | "queued" | "building" | "ready" | "failed" | "uncertain";
  settings: BuildSettings;
  files: Omit<BundleFile, "content">[];
  sourceFiles?: Omit<BundleFile, "content">[];
  created: number;
  logs: string;
  jobId?: string;
}
export interface HostingOptions {
  contentDomain: string;
  workerToken?: string;
  workerOrigin?: string;
  secretsKey?: string;
  maxStorageBytes?: number;
}
export const safePath = (value: unknown): value is string =>
  typeof value === "string" &&
  value.length > 0 &&
  value.length <= 240 &&
  value
    .split("/")
    .every(
      (part) =>
        /^[A-Za-z0-9_@()[\]. +,-]+$/.test(part) &&
        part !== "." &&
        part !== ".." &&
        !part.startsWith("."),
    ) &&
  !/[\\:%?#\0]/.test(value);
const credentialPath = (path: string) =>
  /(^|\/)(\.|.*\.(pem|key|p12|pfx|keystore|sqlite|db)$|id_rsa|credentials|secrets?|service[-_]?account|token)(\/|$)|(^|\/)(package-lock|bun\.lockb)$/i.test(
    path,
  );
const mime: Record<string, string> = {
  ".html": "text/html; charset=utf-8",
  ".htm": "text/html; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".json": "application/json",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg",
  ".webp": "image/webp",
  ".gif": "image/gif",
  ".ico": "image/x-icon",
  ".woff": "font/woff",
  ".woff2": "font/woff2",
  ".ttf": "font/ttf",
  ".txt": "text/plain; charset=utf-8",
  ".pdf": "application/pdf",
  ".wasm": "application/wasm",
  ".webm": "video/webm",
  ".mp4": "video/mp4",
  ".avif": "image/avif",
  ".mp3": "audio/mpeg",
  ".wav": "audio/wav",
};
export class Hosting {
  readonly enabled: boolean;
  private readonly objects: BlobStore;
  private readonly mutations = new Set<string>();
  constructor(
    readonly store: ServiceStore,
    directory: string | BlobStore,
    private options?: HostingOptions,
    private readonly computeEnabled: () => boolean = () => true,
    recoverIncomplete = true,
    private readonly wake?: () => boolean,
  ) {
    this.enabled = !!options;
    this.objects =
      typeof directory === "string" ? new LocalBlobs(directory) : directory;
    if (
      options &&
      (!/^[a-z0-9](?:[a-z0-9.-]{1,220}[a-z0-9])$/.test(options.contentDomain) ||
        options.contentDomain.includes("..") ||
        options.contentDomain.split(".").length < 2)
    )
      throw new Error("Invalid separate content domain");
    if (
      options?.workerToken &&
      (!/^[A-Za-z0-9_-]{43,128}$/.test(options.workerToken) ||
        !options.workerOrigin ||
        new URL(options.workerOrigin).protocol !== "https:")
    )
      throw new Error("Hosting worker requires HTTPS and a private token");
    if (options?.secretsKey && !/^[a-f0-9]{64}$/.test(options.secretsKey))
      throw new Error("Hosting secrets require a 32-byte hex encryption key");
    // No side effect is retried automatically after a lost worker acknowledgment.
    if (recoverIncomplete)
      for (const row of store.db
        .query<{ workspace: string; id: string; value: string }, []>(
          "SELECT workspace,id,value FROM service_objects WHERE kind='release'",
        )
        .all()) {
        const release = JSON.parse(row.value) as Release;
        if (release.status === "building" || release.status === "uploading") {
          release.status = "uncertain";
          release.logs =
            "Service restarted during this operation. Inspect before retrying.";
          store.put(row.workspace, "release", row.id, release);
        }
      }
  }
  private require() {
    if (!this.options) throw new CloudFailure(503, "Hosting is not configured");
    return this.options;
  }
  site(workspace: string, id: string) {
    const site = this.store.get<Site>(workspace, "site", id);
    if (!site) throw new CloudFailure(404, "Deployment not found");
    return site;
  }
  release(workspace: string, id: string) {
    const release = this.store.get<Release>(workspace, "release", id);
    if (!release) throw new CloudFailure(404, "Version not found");
    return release;
  }
  private url(site: Site) {
    return "https://" + site.id + "." + this.require().contentDomain + "/";
  }
  private validate(input: any): {
    files: BundleFile[];
    settings: BuildSettings;
    bytes: number;
  } {
    if (
      !input ||
      !Array.isArray(input.files) ||
      !input.files.length ||
      input.files.length > 500
    )
      throw new CloudFailure(400, "Choose 1–500 source files");
    let bytes = 0;
    const names = new Set<string>();
    for (const file of input.files) {
      if (
        !safePath(file.path) ||
        credentialPath(file.path) ||
        names.has(file.path) ||
        typeof file.content !== "string" ||
        !/^[a-f0-9]{64}$/.test(file.sha256) ||
        !Number.isSafeInteger(file.bytes) ||
        file.bytes < 0 ||
        file.bytes > 8_000_000 ||
        file.content.length > 10_666_668 ||
        !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(
          file.content,
        )
      )
        throw new CloudFailure(400, "Invalid or excluded source file");
      const decoded = Buffer.from(file.content, "base64");
      if (decoded.length !== file.bytes || hash(decoded) !== file.sha256)
        throw new CloudFailure(400, "Source file hash changed");
      // These headers are never accepted as bundle configuration.
      if (
        /-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/.test(
          decoded.subarray(0, 8192).toString(),
        )
      )
        throw new CloudFailure(
          400,
          "A private key was found in the source export",
        );
      names.add(file.path);
      bytes += file.bytes;
    }
    if (bytes > 20_000_000) throw new CloudFailure(413, "Bundle exceeds 20 MB");
    const settings = input.settings as BuildSettings;
    if (
      !settings ||
      Object.keys(settings).some(
        (key) =>
          ![
            "build",
            "entry",
            "healthPath",
            "outputDirectory",
            "port",
            "runtime",
            "start",
            "spa",
          ].includes(key),
      ) ||
      (settings.spa !== undefined && typeof settings.spa !== "boolean") ||
      !["document", "static", "node", "bun", "python"].includes(
        settings.runtime,
      ) ||
      !safePath(settings.entry) ||
      ((!Array.isArray(settings.build) || !settings.build.length) &&
        !names.has(settings.entry)) ||
      !Array.isArray(settings.build) ||
      !Array.isArray(settings.start) ||
      settings.build.length > 32 ||
      settings.start.length > 32 ||
      ![...settings.build, ...settings.start].every(
        (v) =>
          typeof v === "string" &&
          v.length > 0 &&
          v.length <= 1000 &&
          !/[\0\r\n]/.test(v),
      ) ||
      !Number.isInteger(settings.port) ||
      settings.port < 1024 ||
      settings.port > 65535 ||
      typeof settings.healthPath !== "string" ||
      !/^\/[A-Za-z0-9_./-]{0,120}$/.test(settings.healthPath) ||
      !safePath(settings.outputDirectory)
    )
      throw new CloudFailure(400, "Review the build and runtime settings");
    if (
      settings.runtime === "document" &&
      (settings.build.length ||
        settings.start.length ||
        !settings.entry.endsWith(".html"))
    )
      throw new CloudFailure(
        400,
        "Document publishing requires an HTML entry without build commands",
      );
    if (
      ["node", "bun", "python"].includes(settings.runtime) &&
      !settings.start.length
    )
      throw new CloudFailure(400, "A start command is required");
    if (
      (settings.build.length ||
        ["node", "bun", "python"].includes(settings.runtime)) &&
      !this.options?.workerToken
    )
      throw new CloudFailure(
        503,
        "The isolated hosting worker is not configured",
      );
    return { files: input.files, settings, bytes };
  }
  private publicSite(workspace: string, site: Site) {
    return {
      ...site,
      url: this.url(site),
      releases: this.store
        .all<Release>(workspace, "release")
        .filter((v) => v.siteId === site.id)
        .map(({ files, sourceFiles, ...v }) => ({
          ...v,
          files: files.length,
          bytes: [
            ...new Map(
              [...files, ...(sourceFiles ?? [])].map((f) => [f.sha256, f]),
            ).values(),
          ].reduce((n, f) => n + f.bytes, 0),
        }))
        .sort((a, b) => b.created - a.created),
    };
  }
  private async mutate<T>(
    workspace: string,
    operation: () => Promise<T>,
  ): Promise<T> {
    if (this.mutations.has(workspace))
      throw new CloudFailure(
        409,
        "A publication or cleanup is already in progress. Refresh and retry.",
      );
    this.mutations.add(workspace);
    try {
      return await operation();
    } finally {
      this.mutations.delete(workspace);
    }
  }
  async handle(account: Account, request: Request): Promise<Response> {
    if (request.method === "GET") return this.handleAccount(account, request);
    return this.mutate(account.workspaceId, () =>
      this.handleAccount(account, request),
    );
  }
  private async handleAccount(
    account: Account,
    request: Request,
  ): Promise<Response> {
    this.require();
    const workspace = account.workspaceId,
      url = new URL(request.url),
      parts = url.pathname.split("/").filter(Boolean).slice(2);
    const json = (value: unknown, status = 200) =>
      Response.json(value, {
        status,
        headers: { "Cache-Control": "no-store" },
      });
    if (parts[0] === "sites" && parts.length === 1 && request.method === "GET")
      return json({
        items: this.store
          .all<Site>(workspace, "site")
          .map((site) => this.publicSite(workspace, site)),
      });
    if (
      parts[0] === "releases" &&
      parts.length === 1 &&
      request.method === "POST"
    ) {
      const input = await readJson(request);
      const { files, settings, bytes } = this.validate(input);
      if (
        (settings.build.length ||
          ["node", "bun", "python"].includes(settings.runtime)) &&
        !this.computeEnabled()
      )
        throw new CloudFailure(403, "Nebius Cloud is disabled by the owner");
      if (
        typeof input.sourceId !== "string" ||
        input.sourceId.length > 150 ||
        !input.sourceId ||
        typeof input.title !== "string" ||
        !input.title.trim() ||
        input.title.length > 150 ||
        !["artifact", "project"].includes(input.kind) ||
        !["private", "public"].includes(input.visibility) ||
        !/^[-a-zA-Z0-9_]{16,100}$/.test(input.idempotencyKey)
      )
        throw new CloudFailure(400, "Invalid publication review");
      const operation = hash(input.idempotencyKey),
        id = hash(workspace + "\0" + input.sourceId).slice(0, 32),
        fingerprint = hash(
          JSON.stringify({
            ...input,
            files: files.map(({ content, ...f }) => f),
            sourceFiles: files.map(({ content, ...f }) => f),
          }),
        );
      const prior = this.store.get<{ id: string; fingerprint: string }>(
        workspace,
        "operation",
        operation,
      );
      if (prior) {
        if (prior.fingerprint !== fingerprint)
          throw new CloudFailure(
            409,
            "Operation key was already used for different content",
          );
        return json(this.publicSite(workspace, this.site(workspace, id)));
      }
      const environment = input.environment ?? {};
      if (
        typeof environment !== "object" ||
        Array.isArray(environment) ||
        Object.keys(environment).length > 32 ||
        Object.entries(environment).some(
          ([key, value]) =>
            !/^[_A-Za-z][_A-Za-z0-9]{0,63}$/.test(key) ||
            [
              "PORT",
              "PATH",
              "HOME",
              "NODE_OPTIONS",
              "LD_PRELOAD",
              "LD_LIBRARY_PATH",
            ].includes(key) ||
            typeof value !== "string" ||
            value.length > 8000 ||
            /[\r\n\0]/.test(value),
        )
      )
        throw new CloudFailure(400, "Invalid runtime environment");
      if (Object.keys(environment).length && !this.options?.secretsKey)
        throw new CloudFailure(503, "Runtime secret storage is not configured");
      const release: Release = {
        id: secret().slice(0, 32),
        siteId: id,
        operation,
        status: "uploading",
        settings,
        files: files.map(({ content, ...f }) => f),
        sourceFiles: files.map(({ content, ...f }) => f),
        created: Date.now(),
        logs: "",
      };
      this.store.transaction(() => {
        const sites = this.store.all<Site>(workspace, "site");
        if (!sites.some((s) => s.id === id) && sites.length >= 10)
          throw new CloudFailure(429, "Deployment limit reached");
        const releases = this.store.all<Release>(workspace, "release");
        if (releases.filter((r) => r.siteId === id).length >= 20)
          throw new CloudFailure(
            429,
            "Clean an inactive version before publishing more",
          );
        const stored = releases.reduce(
          (sum, r) =>
            sum +
            [
              ...new Map(
                [...r.files, ...(r.sourceFiles ?? [])].map((f) => [
                  f.sha256,
                  f,
                ]),
              ).values(),
            ].reduce((n, f) => n + f.bytes, 0),
          0,
        );
        if (stored + bytes > (this.options?.maxStorageBytes ?? 200_000_000))
          throw new CloudFailure(429, "Hosting storage allowance reached");
        const current = this.store.get<Site>(workspace, "site", id);
        if (current && current.kind !== input.kind)
          throw new CloudFailure(409, "Source identity already exists");
        const site =
          current ??
          ({
            id,
            sourceId: input.sourceId,
            title: input.title,
            kind: input.kind,
            visibility: "private",
            active: null,
            created: Date.now(),
            stopped: false,
          } as Site);
        site.requested = release.id;
        site.pendingCancelled = false;
        this.store.put(workspace, "site", id, site);
        this.store.put(workspace, "release", release.id, release);
        this.store.put(workspace, "operation", operation, {
          id: release.id,
          fingerprint,
        });
        this.store.put(
          workspace,
          "requestedVisibility",
          release.id,
          input.visibility,
        );
        if (Object.keys(environment).length) {
          const nonce = randomBytes(12),
            cipher = createCipheriv(
              "aes-256-gcm",
              Buffer.from(this.options!.secretsKey!, "hex"),
              nonce,
            );
          cipher.setAAD(Buffer.from(workspace + ":" + release.id));
          const bytes = Buffer.concat([
            cipher.update(JSON.stringify(environment), "utf8"),
            cipher.final(),
          ]);
          this.store.put(workspace, "runtimeSecrets", release.id, {
            nonce: nonce.toString("base64"),
            tag: cipher.getAuthTag().toString("base64"),
            bytes: bytes.toString("base64"),
          });
        }
      });
      try {
        for (const file of files)
          await this.objects.put(
            workspace,
            file.sha256,
            Buffer.from(file.content, "base64"),
          );
        if (
          settings.build.length ||
          ["node", "bun", "python"].includes(settings.runtime)
        ) {
          release.status = "queued";
        } else {
          release.status = "ready";
          const site = this.site(workspace, id);
          site.active = release.id;
          site.visibility = input.visibility;
          site.stopped = false;
          this.store.put(workspace, "site", id, site);
        }
        this.store.put(workspace, "release", release.id, release);
      } catch {
        release.status = "failed";
        release.logs =
          "Source upload could not be committed. The previous version remains active.";
        this.store.put(workspace, "release", release.id, release);
        throw new CloudFailure(503, release.logs);
      }
      return json(this.publicSite(workspace, this.site(workspace, id)), 201);
    }
    if (parts[0] === "sites" && parts[1]) {
      const site = this.site(workspace, parts[1]);
      const action = parts[2];
      if (action === "logs" && request.method === "POST") {
        if (!site.active) throw new CloudFailure(409, "No active runtime");
        const release = this.release(workspace, site.active);
        if (!release.jobId || !this.options?.workerOrigin)
          return json({ logs: release.logs });
        const response = await fetch(
          this.options.workerOrigin + "/logs/" + release.jobId,
          {
            headers: { Authorization: "Bearer " + this.options.workerToken },
            redirect: "manual",
            signal: AbortSignal.timeout(10_000),
          },
        );
        if (!response.ok)
          throw new CloudFailure(503, "Runtime logs unavailable");
        const result = (await response.json()) as { logs: unknown };
        if (typeof result.logs !== "string" || result.logs.length > 20_000)
          throw new CloudFailure(503, "Invalid runtime log response");
        release.logs = result.logs;
        this.store.put(workspace, "release", release.id, release);
        return json({ logs: result.logs });
      }
      if (!action && request.method === "DELETE") {
        const releases = this.store
          .all<Release>(workspace, "release")
          .filter((r) => r.siteId === site.id);
        if (
          releases.some((r) =>
            ["uploading", "queued", "building"].includes(r.status),
          )
        )
          throw new CloudFailure(
            409,
            "Stop and wait for pending work before removing this deployment",
          );
        this.store.transaction(() => {
          for (const release of releases) {
            this.store.delete(workspace, "release", release.id);
            this.store.delete(workspace, "runtimeSecrets", release.id);
            this.store.delete(workspace, "requestedVisibility", release.id);
          }
          this.store.delete(workspace, "site", site.id);
        });
        const kept = new Set(
          this.store
            .all<Release>(workspace, "release")
            .flatMap((r) =>
              [...r.files, ...(r.sourceFiles ?? [])].map((f) => f.sha256),
            ),
        );
        for (const release of releases)
          for (const file of [...release.files, ...(release.sourceFiles ?? [])])
            if (!kept.has(file.sha256))
              await this.objects.delete(workspace, file.sha256);
        return json({ removed: true });
      }
      if (!action && request.method === "GET")
        return json(this.publicSite(workspace, site));
      if (request.method === "POST" && action === "view") {
        if (!site.active || site.stopped)
          throw new CloudFailure(409, "Deployment is stopped");
        const ticket = this.store.capability(
          workspace,
          "content-exchange",
          { siteId: site.id, releaseId: site.active },
          Date.now() + 30_000,
        );
        return json({ url: this.url(site) + "__ion_access#" + ticket });
      }
      if (
        request.method === "POST" &&
        ["stop", "visibility", "rollback", "start"].includes(action ?? "")
      ) {
        const input = await readJson(request, 2000);
        if (action === "stop") {
          site.stopped = true;
          site.pendingCancelled = true;
          for (const release of this.store
            .all<Release>(workspace, "release")
            .filter(
              (r) =>
                r.siteId === site.id &&
                ["uploading", "queued"].includes(r.status),
            )) {
            release.status = "failed";
            release.logs = "Cancelled before build.";
            this.store.put(workspace, "release", release.id, release);
          }
        }
        if (action === "start") {
          if (!site.active) throw new CloudFailure(409, "No ready version");
          if (
            ["node", "bun", "python"].includes(
              this.release(workspace, site.active).settings.runtime,
            ) &&
            !this.computeEnabled()
          )
            throw new CloudFailure(
              403,
              "Nebius Cloud is disabled by the owner",
            );
          site.stopped = false;
        }
        if (action === "visibility") {
          if (!["private", "public"].includes(input.visibility))
            throw new CloudFailure(400, "Choose visibility");
          site.visibility = input.visibility;
        }
        if (action === "rollback") {
          const release = this.release(workspace, input.releaseId);
          if (release.siteId !== site.id || release.status !== "ready")
            throw new CloudFailure(409, "Choose a ready version");
          if (
            ["node", "bun", "python"].includes(release.settings.runtime) &&
            !this.computeEnabled()
          )
            throw new CloudFailure(
              403,
              "Nebius Cloud is disabled by the owner",
            );
          site.active = release.id;
          site.requested = release.id;
          site.stopped = false;
        }
        this.store.put(workspace, "site", site.id, site);
        return json(this.publicSite(workspace, site));
      }
      if (action === "versions" && parts[3] && request.method === "DELETE") {
        const release = this.release(workspace, parts[3]);
        if (
          release.siteId !== site.id ||
          site.active === release.id ||
          ["queued", "building", "uploading"].includes(release.status)
        )
          throw new CloudFailure(
            409,
            "Only inactive finished versions can be removed",
          );
        this.store.delete(workspace, "release", release.id);
        this.store.delete(workspace, "runtimeSecrets", release.id);
        this.store.delete(workspace, "requestedVisibility", release.id);
        const kept = new Set(
          this.store
            .all<Release>(workspace, "release")
            .flatMap((r) =>
              [...r.files, ...(r.sourceFiles ?? [])].map((f) => f.sha256),
            ),
        );
        for (const file of [...release.files, ...(release.sourceFiles ?? [])])
          if (!kept.has(file.sha256))
            await this.objects.delete(workspace, file.sha256);
        return json({ removed: true });
      }
    }
    throw new CloudFailure(404, "Hosting route not found");
  }
  // Trusted worker requests are separate from user JWT routes.
  async worker(request: Request): Promise<Response> {
    const options = this.require();
    if (
      !options.workerToken ||
      hash(request.headers.get("authorization") ?? "") !==
        hash("Bearer " + options.workerToken)
    )
      throw new CloudFailure(401, "Worker authorization required");
    const input = await readJson(request, 40_000_000),
      path = new URL(request.url).pathname;
    if (path === "/internal/hosting/claim") {
      if (!this.computeEnabled()) return Response.json({ job: null });
      const result = this.store.transaction(() => {
        const rows = this.store.db
          .query<{ workspace: string; id: string; value: string }, []>(
            "SELECT workspace,id,value FROM service_objects WHERE kind='release'",
          )
          .all();
        for (const row of rows) {
          const release = JSON.parse(row.value) as Release;
          if (release.status !== "queued") continue;
          release.status = "building";
          release.jobId = release.id;
          this.store.put(row.workspace, "release", release.id, release);
          return { workspace: row.workspace, release };
        }
        return null;
      });
      if (!result) return Response.json({ job: null });
      const files: BundleFile[] = [];
      for (const file of result.release.files)
        files.push({
          ...file,
          content: (
            await this.objects.get(result.workspace, file.sha256)
          ).toString("base64"),
        });
      const sealed = this.store.get<{
        nonce: string;
        tag: string;
        bytes: string;
      }>(result.workspace, "runtimeSecrets", result.release.id);
      let environment = {};
      if (sealed) {
        if (!this.options?.secretsKey)
          throw new CloudFailure(503, "Secret key unavailable");
        const cipher = createDecipheriv(
          "aes-256-gcm",
          Buffer.from(this.options.secretsKey, "hex"),
          Buffer.from(sealed.nonce, "base64"),
        );
        cipher.setAAD(Buffer.from(result.workspace + ":" + result.release.id));
        cipher.setAuthTag(Buffer.from(sealed.tag, "base64"));
        environment = JSON.parse(
          Buffer.concat([
            cipher.update(Buffer.from(sealed.bytes, "base64")),
            cipher.final(),
          ]).toString("utf8"),
        );
      }
      return Response.json({ job: { ...result, files, environment } });
    }
    if (path === "/internal/hosting/complete") {
      if (
        !/^[a-f0-9]{64}$/.test(input.workspace) ||
        typeof input.releaseId !== "string"
      )
        throw new CloudFailure(400, "Invalid worker receipt");
      return this.mutate(input.workspace, async () => {
        const release = this.release(input.workspace, input.releaseId);
        if (release.status === "ready" && input.uncertain === true && ["node", "bun", "python"].includes(release.settings.runtime)) {
          const site = this.site(input.workspace, release.siteId);
          site.stopped = true;
          this.store.put(input.workspace, "site", site.id, site);
          release.status = "uncertain";
          release.logs = "Runtime could not be recovered after host restart. Review and publish a new version.";
          this.store.put(input.workspace, "release", release.id, release);
          return Response.json({ accepted: true });
        }
        if (release.status !== "building" && release.status !== "uncertain")
          return Response.json({ accepted: release.status === "ready" });
        if (input.uncertain === true) {
          release.status = "uncertain";
          release.logs =
            "Worker restarted during execution. Inspect this version before replacing it.";
          this.store.put(input.workspace, "release", release.id, release);
          return Response.json({ accepted: true });
        }
        if (input.success !== true) {
          release.status = "failed";
          release.logs =
            "Isolated build/runtime failed. Review the worker's private diagnostic log.";
          this.store.put(input.workspace, "release", release.id, release);
          return Response.json({ accepted: true });
        }
        if (release.settings.runtime === "static") {
          const validated = this.validate({
            files: input.files,
            settings: {
              ...release.settings,
              build: [],
              entry: release.settings.entry,
            },
          });
          const retained = this.store
            .all<Release>(input.workspace, "release")
            .filter((r) => r.id !== release.id)
            .reduce(
              (n, r) =>
                n +
                [...r.files, ...(r.sourceFiles ?? [])].reduce(
                  (sum, f) => sum + f.bytes,
                  0,
                ),
              0,
            );
          const thisVersion = [
            ...new Map(
              [
                ...validated.files,
                ...(release.sourceFiles ?? release.files),
              ].map((f) => [f.sha256, f]),
            ).values(),
          ].reduce((sum, f) => sum + f.bytes, 0);
          if (
            retained + thisVersion >
            (this.options?.maxStorageBytes ?? 200_000_000)
          )
            throw new CloudFailure(
              429,
              "Built output exceeds the retained storage allowance",
            );
          release.files = validated.files.map(({ content, ...f }) => f);
          this.store.put(input.workspace, "release", release.id, release); // Reserve retained output before filesystem work.
          for (const file of validated.files)
            await this.objects.put(
              input.workspace,
              file.sha256,
              Buffer.from(file.content, "base64"),
            );
        }
        release.status = "ready";
        release.logs = "Build and readiness completed.";
        this.store.put(input.workspace, "release", release.id, release);
        const site = this.site(input.workspace, release.siteId);
        if (site.requested !== release.id || site.pendingCancelled)
          return Response.json({ accepted: true });
        site.active = release.id;
        site.stopped = false;
        site.visibility =
          this.store.get<"private" | "public">(
            input.workspace,
            "requestedVisibility",
            release.id,
          ) ?? "private";
        this.store.put(input.workspace, "site", site.id, site);
        return Response.json({ accepted: true });
      });
    }
    if (path === "/internal/hosting/desired") {
      const rows = this.store.db
        .query<{ workspace: string; value: string }, []>(
          "SELECT workspace,value FROM service_objects WHERE kind='release'",
        )
        .all();
      return Response.json({
        jobs: rows.flatMap((row) => {
          const r = JSON.parse(row.value) as Release;
          if (!r.jobId) return [];
          const site = this.site(row.workspace, r.siteId);
          return [
            {
              id: r.jobId,
              running:
                this.computeEnabled() &&
                r.status === "ready" &&
                site.active === r.id &&
                !site.stopped,
              building:
                this.computeEnabled() &&
                r.status === "building" &&
                site.requested === r.id &&
                !site.pendingCancelled,
            },
          ];
        }),
      });
    }
    throw new CloudFailure(404, "Worker route not found");
  }
  async content(request: Request): Promise<Response | null> {
    if (!this.options) return null;
    const url = new URL(request.url),
      suffix = "." + this.options.contentDomain;
    if (!url.hostname.endsWith(suffix)) return null;
    const id = url.hostname.slice(0, -suffix.length);
    if (!/^[a-f0-9]{32}$/.test(id)) return null;
    const row = this.store.db
      .query<{ workspace: string; value: string }, [string]>(
        "SELECT workspace,value FROM service_objects WHERE kind='site' AND id=?",
      )
      .get(id);
    if (!row) return new Response("Not found", { status: 404 });
    const site = JSON.parse(row.value) as Site;
    const deny = () =>
      new Response("This deployment is private, stopped or unavailable.", {
        status: 403,
        headers: { "Cache-Control": "no-store" },
      });
    if (url.pathname === "/__ion_access" && request.method === "GET") {
      return new Response(
        '<!doctype html><meta name="referrer" content="no-referrer"><title>Opening deployment</title><script>const ticket=location.hash.slice(1);history.replaceState(null,"","/__ion_access");fetch("/__ion_access",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({ticket})}).then(r=>{if(!r.ok)throw 0;location.replace("/")}).catch(()=>document.body.textContent="Access expired. Open this deployment again from Ion.")</script>',
        {
          headers: {
            "Content-Type": "text/html; charset=utf-8",
            "Cache-Control": "no-store",
            "Content-Security-Policy":
              "default-src 'none'; script-src 'unsafe-inline'; connect-src 'self'; base-uri 'none'; frame-ancestors 'none'",
            "Referrer-Policy": "no-referrer",
          },
        },
      );
    }
    if (url.pathname === "/__ion_access" && request.method === "POST") {
      if (request.headers.get("origin") !== url.origin) return deny();
      try {
        const input = await readJson(request, 2000),
          cap = this.store.inspect(input.ticket, "content-exchange", true);
        if (
          cap.workspace !== row.workspace ||
          cap.value.siteId !== id ||
          site.stopped ||
          site.active !== cap.value.releaseId
        )
          return deny();
        const ticket = this.store.capability(
          row.workspace,
          "content-session",
          cap.value,
          Date.now() + 900_000,
        );
        return new Response(null, {
          status: 204,
          headers: {
            "Set-Cookie":
              "__Host-ion-content=" +
              ticket +
              "; Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age=900",
            "Cache-Control": "no-store",
          },
        });
      } catch {
        return deny();
      }
    }
    if (!site.active || site.stopped) return deny();
    if (site.visibility !== "public") {
      const ticket =
        /(?:^|; *)__Host-ion-content=([A-Za-z0-9_-]{43})(?:;|$)/.exec(
          request.headers.get("cookie") ?? "",
        )?.[1] ?? "";
      try {
        const cap = this.store.inspect(ticket, "content-session");
        if (
          cap.workspace !== row.workspace ||
          cap.value.siteId !== id ||
          cap.value.releaseId !== site.active
        )
          return deny();
      } catch {
        return deny();
      }
    }
    const release = this.release(row.workspace, site.active);
    if (["node", "bun", "python"].includes(release.settings.runtime)) {
      if (!this.computeEnabled())
        return new Response("Application compute is disabled by the owner", {
          status: 503,
          headers: { "Cache-Control": "no-store" },
        });
      if (this.wake && !this.wake()) {
        const html = request.method === "GET" && (request.headers.get("accept") ?? "").includes("text/html");
        return new Response(html ? '<!doctype html><meta http-equiv="refresh" content="10"><title>Starting app</title><p>Starting this application. Retrying in 10 seconds.</p>' : "Application is starting. Retry later.", {
          status: 503, headers: { "Retry-After": "10", "Cache-Control": "no-store", "Content-Type": html ? "text/html; charset=utf-8" : "text/plain", "Content-Security-Policy": "default-src 'none'; frame-ancestors 'none'" },
        });
      }
      if (
        !this.options.workerOrigin ||
        !this.options.workerToken ||
        !release.jobId
      )
        return new Response("Runtime unavailable", { status: 503 });
      const headers = new Headers();
      for (const key of [
        "content-type",
        "accept",
        "accept-language",
        "if-none-match",
        "if-modified-since",
        "range",
        "user-agent",
      ]) {
        const value = request.headers.get(key);
        if (value) headers.set(key, value);
      }
      headers.set("authorization", "Bearer " + this.options.workerToken);
      const target =
        this.options.workerOrigin +
        "/runtime/" +
        release.jobId +
        url.pathname +
        url.search;
      try {
        const response = await fetch(target, {
          method: request.method,
          headers,
          body: request.body,
          redirect: "manual",
          signal: AbortSignal.timeout(30_000),
          duplex: "half",
        } as RequestInit);
        const output = new Headers(response.headers);
        output.delete("set-cookie");
        output.delete("server");
        output.set("Cache-Control", "no-store");
        output.set("Referrer-Policy", "no-referrer");
        output.set("X-Content-Type-Options", "nosniff");
        output.set(
          "Content-Security-Policy",
          "default-src 'self' data: blob:; script-src 'self' 'unsafe-inline' 'unsafe-eval'; style-src 'self' 'unsafe-inline'; connect-src 'self'; base-uri 'none'; frame-ancestors 'none'",
        );
        const location = output.get("location");
        if (
          location &&
          (!location.startsWith("/") || location.startsWith("//"))
        )
          output.delete("location");
        return new Response(response.body, {
          status: response.status,
          headers: output,
        });
      } catch {
        return new Response("Runtime unavailable", {
          status: 503,
          headers: { "Cache-Control": "no-store" },
        });
      }
    }
    if (!["GET", "HEAD"].includes(request.method))
      return new Response("Method not allowed", { status: 405 });
    let path: string;
    try {
      path =
        decodeURIComponent(url.pathname).replace(/^\/+/, "") ||
        release.settings.entry;
    } catch {
      return new Response("Invalid path", { status: 400 });
    }
    const file =
      release.files.find((file) => file.path === path) ??
      (release.settings.runtime === "static" &&
      release.settings.spa === true &&
      !extname(path)
        ? release.files.find((file) => file.path === release.settings.entry)
        : undefined);
    if (!file) return new Response("Not found", { status: 404 });
    const body = await this.objects.get(row.workspace, file.sha256);
    if (hash(body) !== file.sha256)
      return new Response("Object unavailable", { status: 503 });
    const headers: Record<string, string> = {
      "Content-Type":
        mime[extname(file.path).toLowerCase()] ?? "application/octet-stream",
      "X-Content-Type-Options": "nosniff",
      "Referrer-Policy": "no-referrer",
      "Cache-Control": "no-store",
      "Origin-Agent-Cluster": "?1",
      "Permissions-Policy":
        "camera=(), microphone=(), geolocation=(), document-domain=()",
      "X-Frame-Options": "DENY",
    };
    if (release.settings.runtime === "document")
      headers["Content-Security-Policy"] =
        "default-src 'none'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; font-src 'self' data:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'; sandbox allow-same-origin";
    else
      headers["Content-Security-Policy"] =
        "default-src 'self' data: blob:; script-src 'self' 'unsafe-inline' 'unsafe-eval'; style-src 'self' 'unsafe-inline'; connect-src 'self'; base-uri 'none'; frame-ancestors 'none'";
    if (headers["Content-Type"] === "application/octet-stream")
      headers["Content-Disposition"] = "attachment";
    return new Response(request.method === "HEAD" ? null : body, { headers });
  }
}
