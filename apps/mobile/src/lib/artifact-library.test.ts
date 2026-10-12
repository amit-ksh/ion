import { describe, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import type {
  ArtifactLibraryItem,
  ArtifactLibraryQuery,
  WakuClient,
} from "@waku/client";
import {
  artifactRoute,
  artifactText,
  loadArtifactLibrary,
  readArtifactDocument,
  validateArtifactItem,
} from "./artifact-library";
const ID = "11111111-1111-4111-8111-111111111111",
  PROJECT = "22222222-2222-4222-8222-222222222222",
  TASK = "33333333-3333-4333-8333-333333333333";
const bytes = Buffer.from(
    "# Generated document\n\n[Example](https://example.com)\n",
  ),
  hash = createHash("sha256").update(bytes).digest("hex");
const item: ArtifactLibraryItem = {
  id: ID,
  name: "document.md",
  format: "md",
  projectId: PROJECT,
  taskId: TASK,
  createdAt: 100,
  bytes: bytes.length,
  sha256: hash,
  origin: "agentEdit",
};
const query: ArtifactLibraryQuery = {
  projectId: null,
  taskId: null,
  format: null,
  after: null,
  limit: 32,
};
const digest = async (value: Uint8Array) =>
  createHash("sha256").update(value).digest("hex");
function client(reply: unknown, onRequest = () => {}) {
  return {
    request: async () => {
      onRequest();
      return reply;
    },
  } as Pick<WakuClient, "request">;
}
const document = (selected = item, payload = bytes) => ({
  type: "artifactDocument",
  document: { document: selected, bytes: payload.toString("base64") },
});
describe("direct phone artifact library", () => {
  test("validated page retains scoped immutable publication metadata", async () => {
    const result = await loadArtifactLibrary(
      client({
        type: "artifactLibrary",
        page: { items: [item], total: 1, nextCursor: null },
      }),
      query,
    );
    expect(result.items[0]).toEqual(item);
  });
  test("invalid query is refused before any owner request", async () => {
    let requests = 0;
    for (const invalid of [
      { ...query, limit: 33 },
      { ...query, projectId: "foreign" },
      { ...query, format: "exe" },
      { ...query, after: "not-opaque" },
    ]) {
      await expect(
        loadArtifactLibrary(
          client({}, () => requests++),
          invalid,
        ),
      ).rejects.toThrow();
    }
    expect(requests).toBe(0);
  });
  test("private fields, missing evidence fields, unknown formats and unsafe names fail closed", () => {
    for (const invalid of [
      { ...item, sourcePath: "private" },
      { ...item, origin: "userImported" },
      { ...item, name: "../outside.md" },
      { ...item, name: "bad\u0085.md" },
      { ...item, name: "文".repeat(60) },
      { ...item, sha256: "bad" },
      { ...item, bytes: 0 },
      { ...item, projectId: "00000000-0000-0000-0000-000000000000" },
    ])
      expect(() => validateArtifactItem(invalid)).toThrow();
  });
  test("foreign project/task/format rows cannot satisfy the selected filters", async () => {
    for (const filtered of [
      { ...query, projectId: "44444444-4444-4444-8444-444444444444" },
      { ...query, taskId: PROJECT },
      { ...query, format: "json" },
    ])
      await expect(
        loadArtifactLibrary(
          client({
            type: "artifactLibrary",
            page: { items: [item], total: 1, nextCursor: null },
          }),
          filtered,
        ),
      ).rejects.toThrow("different artifact filters");
  });
  test("duplicate, wrongly ordered, oversized and nonadvancing pages are refused", async () => {
    const duplicate = { items: [item, item], total: 2, nextCursor: null };
    const reversed = {
      items: [item, { ...item, id: PROJECT, createdAt: 101 }],
      total: 2,
      nextCursor: null,
    };
    for (const page of [
      duplicate,
      reversed,
      { items: Array(33).fill(item), total: 33, nextCursor: null },
      { items: [item], total: 1, nextCursor: "00" },
    ])
      await expect(
        loadArtifactLibrary(client({ type: "artifactLibrary", page }), query),
      ).rejects.toThrow();
  });
  test("saved routes pin the owner and every expected document field", () => {
    expect(
      artifactRoute({
        owner: PROJECT,
        address: "wss://owner.example.com",
        document: JSON.stringify(item),
      }),
    ).toEqual({
      owner: PROJECT,
      address: "wss://owner.example.com",
      document: item,
    });
    for (const route of [
      { owner: [PROJECT], document: JSON.stringify(item) },
      { owner: PROJECT, document: "{" },
      { owner: PROJECT, document: JSON.stringify({ ...item, sha256: "bad" }) },
      { owner: PROJECT, document: "x".repeat(2049) },
    ])
      expect(() => artifactRoute(route)).toThrow();
  });
  test("exact original bytes are returned only after full SHA verification", async () => {
    const result = await readArtifactDocument(
      client(document()),
      item,
      () => true,
      digest,
    );
    expect(Buffer.from(result)).toEqual(bytes);
    expect(artifactText(item, result)).toBe(bytes.toString());
  });
  test("metadata substitution and byte/hash mismatch are refused", async () => {
    for (const changed of [
      { ...item, name: "wrong.md" },
      { ...item, projectId: ID },
      { ...item, format: "html" },
      { ...item, sha256: "0".repeat(64) },
    ])
      await expect(
        readArtifactDocument(
          client(document(changed)),
          item,
          () => true,
          digest,
        ),
      ).rejects.toThrow("metadata changed");
    await expect(
      readArtifactDocument(
        client(document(item, Buffer.alloc(bytes.length))),
        item,
        () => true,
        digest,
      ),
    ).rejects.toThrow("digest changed");
  });
  test("noncanonical base64 padding, invalid alphabet and exact count changes are refused", async () => {
    for (const encoded of [
      "invalid!",
      bytes.toString("base64").slice(4),
      "!!!!".repeat(Math.ceil(bytes.length / 3)),
      Buffer.from([255]).toString("base64").replace("/w==", "/x=="),
    ]) {
      const expected = encoded.length === 4 ? { ...item, bytes: 1 } : item;
      await expect(
        readArtifactDocument(
          client({
            type: "artifactDocument",
            document: { document: expected, bytes: encoded },
          }),
          expected,
          () => true,
          digest,
        ),
      ).rejects.toThrow();
    }
  });
  test("pairing, route and foreground changes fence requests and digest completion", async () => {
    let requests = 0;
    await expect(
      readArtifactDocument(
        client(document(), () => requests++),
        item,
        () => false,
        digest,
      ),
    ).rejects.toThrow("scope changed");
    expect(requests).toBe(0);
    let current = true;
    await expect(
      readArtifactDocument(
        client(document(), () => {
          current = false;
        }),
        item,
        () => current,
        digest,
      ),
    ).rejects.toThrow("scope changed");
    current = true;
    await expect(
      readArtifactDocument(
        client(document()),
        item,
        () => current,
        async (value) => {
          current = false;
          return digest(value);
        },
      ),
    ).rejects.toThrow("scope changed");
  });
  test("binary and oversized textual documents keep verified export without truncated previews", () => {
    expect(artifactText({ ...item, format: "pdf" }, bytes)).toBe(null);
    expect(artifactText({ ...item, format: "docx" }, bytes)).toBe(null);
    expect(artifactText(item, new Uint8Array(65537))).toBe(null);
    expect(() => artifactText(item, new Uint8Array([255]))).toThrow("UTF-8");
  });
  test("large original transfer is complete and cancellation is checked while decoding", async () => {
    const payload = Buffer.alloc(100_000, 65),
      expected = {
        ...item,
        bytes: payload.length,
        sha256: createHash("sha256").update(payload).digest("hex"),
      };
    expect(
      (
        await readArtifactDocument(
          client(document(expected, payload)),
          expected,
          () => true,
          digest,
        )
      ).length,
    ).toBe(payload.length);
    let allowed = true;
    const request = readArtifactDocument(
      client(document(expected, payload)),
      expected,
      () => allowed,
      digest,
    );
    setTimeout(() => {
      allowed = false;
    }, 0);
    await expect(request).rejects.toThrow("scope changed");
  });
});
