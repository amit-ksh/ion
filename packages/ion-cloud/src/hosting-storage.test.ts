import { expect, test } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { Hosting, type Release, type Site } from "./hosting";
import { ServiceStore, hash } from "./service-store";
import type { BlobStore } from "./blobs";
import type { Account } from "./auth";
const account: Account = {
  subject: "judge",
  issuer: "https://issuer/",
  workspaceId: "a".repeat(64),
  scopes: new Set(["ion:gateway"]),
};
const req = (path: string, body: unknown, method = "POST") =>
  new Request("https://api.example/v1/hosting/" + path, {
    method,
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
test("publication and cleanup cannot race asynchronous object storage within an account", async () => {
  const directory = mkdtempSync(join(tmpdir(), "ion-hosting-")),
    store = new ServiceStore(directory);
  let release!: () => void, entered!: () => void;
  const waiting = new Promise<void>((resolve) => {
      release = resolve;
    }),
    writing = new Promise<void>((resolve) => {
      entered = resolve;
    });
  const objects: BlobStore = {
    async put() {
      entered();
      await waiting;
    },
    async get() {
      return Buffer.from("");
    },
    async delete() {},
  };
  const hosting = new Hosting(store, objects, {
    contentDomain: "content.example",
  });
  const bytes = Buffer.from("<h1>Fixture</h1>");
  const body = {
    title: "Fixture",
    kind: "artifact",
    sourceId: "fixture",
    visibility: "private",
    idempotencyKey: "fixture-publication-001",
    settings: {
      runtime: "document",
      entry: "index.html",
      build: [],
      start: [],
      outputDirectory: "dist",
      port: 3000,
      healthPath: "/",
    },
    files: [
      {
        path: "index.html",
        content: bytes.toString("base64"),
        sha256: hash(bytes),
        bytes: bytes.length,
      },
    ],
  };
  try {
    const pending = hosting.handle(account, req("releases", body));
    await writing;
    await expect(
      hosting.handle(
        account,
        req("releases", {
          ...body,
          sourceId: "other",
          idempotencyKey: "fixture-publication-002",
        }),
      ),
    ).rejects.toThrow("already in progress");
    release();
    expect((await pending).status).toBe(201);
    expect((await hosting.handle(account, req("releases", body))).status).toBe(
      200,
    );
  } finally {
    release();
    store.close();
    rmSync(directory, {
      recursive: true,
      force: true,
      maxRetries: 5,
      retryDelay: 100,
    });
  }
});

test("static-only policy blocks legacy runtime activation and worker claims without waking compute", async () => {
  const directory = mkdtempSync(join(tmpdir(), "ion-static-policy-"));
  const store = new ServiceStore(directory);
  const id = "b".repeat(32), releaseId = "c".repeat(32);
  const release: Release = { id: releaseId, siteId: id, operation: "legacy", status: "ready", created: 1,
    files: [], logs: "legacy", jobId: "legacy-job", settings: { runtime: "node", entry: "server.js",
      build: [], start: ["node", "server.js"], outputDirectory: "dist", port: 3000, healthPath: "/" } };
  const site: Site = { id, sourceId: "legacy", title: "Legacy", kind: "project", visibility: "public",
    active: releaseId, created: 1, stopped: true };
  store.put(account.workspaceId, "site", id, site);
  store.put(account.workspaceId, "release", releaseId, release);
  const token = "t".repeat(43);
  const hosting = new Hosting(store, directory, { contentDomain: "content.example", workerToken: token,
    workerOrigin: "https://worker.example" }, () => { throw new Error("Must not inspect compute"); },
    true, () => { throw new Error("Must not wake compute"); });
  try {
    await expect(hosting.handle(account, req(`sites/${id}/start`, {}))).rejects.toThrow("Only static");
    await expect(hosting.handle(account, req(`sites/${id}/rollback`, { releaseId }))).rejects.toThrow("Only static");
    store.put(account.workspaceId, "site", id, { ...site, stopped: false });
    expect((await hosting.content(new Request(`https://${id}.content.example/`)))?.status).toBe(403);
    const worker = (path: string) => hosting.worker(new Request("https://api.example/internal/hosting/" + path, {
      method: "POST", headers: { Authorization: "Bearer " + token }, body: "{}",
    }));
    expect(await (await worker("claim")).json()).toEqual({ job: null });
    expect(await (await worker("desired")).json()).toEqual({ jobs: [] });
    await expect(worker("complete")).rejects.toThrow("Only static");
    expect(await (await hosting.handle(account, req(`sites/${id}/logs`, {}))).json()).toEqual({ logs: "legacy" });
  } finally {
    store.close();
    rmSync(directory, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
  }
});
