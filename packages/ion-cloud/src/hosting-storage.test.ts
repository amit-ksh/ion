import { expect, test } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { Hosting } from "./hosting";
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
