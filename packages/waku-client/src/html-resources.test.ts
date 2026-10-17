import { expect, test } from "bun:test";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import {
  htmlManifestBytes,
  readHtmlResourceSet,
  validateHtmlManifest,
  validateHtmlSourceQuery,
} from "./html-resources";
import {
  portfolioReadGrant,
  validatePortfolioQuery,
  validatePortfolioResult,
} from "./gateway-portfolio";
import type {
  HtmlResourceManifest,
  HtmlSourceChunk,
  HtmlSourceQuery,
} from "./generated";
const sha = (b: Uint8Array | string) =>
  createHash("sha256").update(b).digest("hex");
const digest = async (b: Uint8Array) => sha(b);
const publicationId = "11111111-1111-4111-8111-111111111111";
function fixture(repetitions = 5000) {
  const bodies = [
    Buffer.from("<math><mi>x</mi></math>"),
    Buffer.from("/*" + "研究🚀".repeat(repetitions) + "*/"),
    Buffer.alloc(0),
    Buffer.from([0, 255, 128]),
  ];
  const paths = ["index.html", "styles/研究🚀.css", "empty.css", "font.woff2"],
    media = ["text/html", "text/css", "text/css", "font/woff2"];
  const sources = bodies.map((b, index) => ({
    index,
    path: paths[index]!,
    mediaType: media[index]!,
    bytes: b.length,
    sha256: sha(b),
  }));
  const totalBytes = bodies.reduce((sum, b) => sum + b.length, 0);
  // Independent canonical encoding; deliberately not htmlManifestBytes().
  const root = sha(
    JSON.stringify([
      1,
      "index.html",
      totalBytes,
      sources.map((s) => [s.index, s.path, s.mediaType, s.bytes, s.sha256]),
    ]),
  );
  const original = Buffer.from(
    `<!--ion-html-sources-v1:${root}-->\n<p>Fallback</p>`,
  );
  const expected = { id: publicationId, sha256: sha(original) };
  const manifest: HtmlResourceManifest = {
    version: 1,
    artifactId: expected.id,
    artifactSha256: expected.sha256,
    entryPath: "index.html",
    totalBytes,
    sha256: root,
    sources,
  };
  const chunk = (q: HtmlSourceQuery): HtmlSourceChunk => {
    const s = sources[q.sourceIndex]!,
      b = bodies[q.sourceIndex]!,
      end = Math.min(b.length, q.offset + q.limit);
    return {
      artifactId: expected.id,
      artifactSha256: expected.sha256,
      manifestSha256: root,
      sourceIndex: s.index,
      sourceSha256: s.sha256,
      offset: q.offset,
      nextOffset: end < b.length ? end : null,
      data: b.subarray(q.offset, end).toString("base64"),
    };
  };
  const reader = {
    manifest: async () => manifest,
    chunk: async (q: HtmlSourceQuery) => chunk(q),
    digest,
    current: () => true,
  };
  return { bodies, original, expected, manifest, chunk, reader };
}
test("publication-bound sources retain Unicode, multiple chunks, empty CSS and binary bytes", async () => {
  const f = fixture();
  expect(sha(htmlManifestBytes(f.manifest))).toBe(f.manifest.sha256);
  const result = await readHtmlResourceSet(f.expected, f.original, f.reader);
  expect(result?.sources.map((b) => Buffer.from(b))).toEqual(f.bodies);
  expect(result?.manifest.entryPath).toBe("index.html");
});
test("bounded batches keep at most eight source chunks in flight", async () => {
  const f = fixture(20000);
  let active = 0,
    peak = 0;
  const result = await readHtmlResourceSet(f.expected, f.original, {
    ...f.reader,
    chunk: async (q) => {
      active++;
      peak = Math.max(peak, active);
      await new Promise((resolve) => setTimeout(resolve, 1));
      try {
        return f.chunk(q);
      } finally {
        active--;
      }
    },
  });
  expect(peak).toBe(8);
  expect(active).toBe(0);
  expect(result?.sources.map((b) => Buffer.from(b))).toEqual(f.bodies);
});
test("resource verification needs no native Buffer or text codec globals", async () => {
  const f = fixture(),
    globals = globalThis as any;
  const saved = {
    Buffer: globals.Buffer,
    TextEncoder: globals.TextEncoder,
    TextDecoder: globals.TextDecoder,
  };
  try {
    globals.Buffer = undefined;
    globals.TextEncoder = undefined;
    globals.TextDecoder = undefined;
    const result = await readHtmlResourceSet(f.expected, f.original, f.reader);
    expect(result?.sources.length).toBe(4);
  } finally {
    Object.assign(globals, saved);
  }
});
test("legacy snapshots need no resource request and original hash checks precede reads", async () => {
  let reads = 0;
  const f = fixture(),
    original = Buffer.from("<p>Legacy</p>");
  const reader = {
    ...f.reader,
    manifest: async () => {
      reads++;
      return f.manifest;
    },
  };
  expect(
    await readHtmlResourceSet(
      { ...f.expected, sha256: sha(original) },
      original,
      reader,
    ),
  ).toBeNull();
  expect(reads).toBe(0);
  await expect(
    readHtmlResourceSet(f.expected, original, reader),
  ).rejects.toThrow("original digest");
  expect(reads).toBe(0);
  const malformed = Buffer.from("<!--ion-html-sources-v1:bad-->");
  await expect(
    readHtmlResourceSet(
      { ...f.expected, sha256: sha(malformed) },
      malformed,
      reader,
    ),
  ).rejects.toThrow("commitment");
});
test("a relay cannot replace source descriptors and recompute a different valid manifest", async () => {
  const f = fixture(),
    manifest = structuredClone(f.manifest);
  manifest.sources[1]!.sha256 = sha("forged");
  manifest.sha256 = sha(htmlManifestBytes(manifest));
  let chunks = 0;
  await expect(
    readHtmlResourceSet(f.expected, f.original, {
      ...f.reader,
      manifest: async () => manifest,
      chunk: async (q) => {
        chunks++;
        return f.chunk(q);
      },
    }),
  ).rejects.toThrow("not committed");
  expect(chunks).toBe(0);
});
test("manifest validation refuses private fields, traversal, duplicated paths and aggregate expansion", () => {
  const f = fixture();
  for (const mutate of [
    (v: any) => (v.endpoint = "file:///private"),
    (v: any) => (v.sources[1].path = "../outside.css"),
    (v: any) => (v.sources[1].path = v.sources[0].path),
    (v: any) => (v.sources[1].index = 0),
    (v: any) => (v.sources[1].bytes = 3 * 1024 * 1024),
    (v: any) => v.totalBytes++,
    (v: any) => (v.sources[1].mediaType = "text/html"),
    (v: any) => (v.sources[1].path = "bad\ud800.css"),
  ]) {
    const v = structuredClone(f.manifest);
    mutate(v);
    expect(() => validateHtmlManifest(v, f.expected)).toThrow();
  }
  const query: HtmlSourceQuery = {
    manifestSha256: f.manifest.sha256,
    sourceIndex: 0,
    sourceSha256: f.manifest.sources[0]!.sha256,
    offset: 0,
    limit: 16384,
  };
  expect(() => validateHtmlSourceQuery(query)).not.toThrow();
  for (const extra of [
    { offset: 1 },
    { limit: 1 },
    { sourceIndex: 33 },
    { manifestSha256: "BAD" },
    { path: "secret" },
  ])
    expect(() => validateHtmlSourceQuery({ ...query, ...extra })).toThrow();
});
test("chunk metadata, order, byte counts and terminal cursors cannot cross source scope", async () => {
  for (const mutate of [
    (c: any) => (c.artifactId = "22222222-2222-4222-8222-222222222222"),
    (c: any) => (c.artifactSha256 = "0".repeat(64)),
    (c: any) => c.sourceIndex++,
    (c: any) => (c.sourceSha256 = "0".repeat(64)),
    (c: any) => (c.manifestSha256 = "0".repeat(64)),
    (c: any) => c.offset++,
    (c: any) => (c.nextOffset = 1),
    (c: any) => (c.data = ""),
    (c: any) => (c.data = Buffer.alloc(16385).toString("base64")),
    (c: any) => (c.privateKey = "secret"),
  ]) {
    const f = fixture();
    await expect(
      readHtmlResourceSet(f.expected, f.original, {
        ...f.reader,
        chunk: async (q) => {
          const c = f.chunk(q);
          mutate(c);
          return c;
        },
      }),
    ).rejects.toThrow();
  }
  const f = fixture();
  await expect(
    readHtmlResourceSet(f.expected, f.original, {
      ...f.reader,
      chunk: async (q) => {
        const c = f.chunk(q);
        if (q.sourceIndex === 1) c.nextOffset = null;
        return c;
      },
    }),
  ).rejects.toThrow("final cursor");
});
test("same-size body substitution is refused by the full source hash before results return", async () => {
  const f = fixture();
  await expect(
    readHtmlResourceSet(f.expected, f.original, {
      ...f.reader,
      chunk: async (q) => {
        const c = f.chunk(q);
        const b = Buffer.from(c.data, "base64");
        b[0] = b[0]! ^ 1;
        c.data = b.toString("base64");
        return c;
      },
    }),
  ).rejects.toThrow("resource digest");
});
test("revocation at metadata, chunk, hashing and final return never exposes a resource set", async () => {
  for (const stage of ["manifest", "chunk", "digest"]) {
    const f = fixture();
    let live = true,
      count = 0;
    await expect(
      readHtmlResourceSet(f.expected, f.original, {
        ...f.reader,
        current: () => live,
        manifest: async () => {
          if (stage === "manifest") live = false;
          return f.manifest;
        },
        chunk: async (q) => {
          if (stage === "chunk") live = false;
          return f.chunk(q);
        },
        digest: async (b) => {
          if (stage === "digest" && ++count === 3) live = false;
          return sha(b);
        },
      }),
    ).rejects.toThrow("scope changed");
  }
});
test("gateway HTML uses only evidenceRead and strict source-index queries", () => {
  const f = fixture(),
    base = {
      runId: publicationId,
      artifactId: f.expected.id,
      sha256: f.expected.sha256,
    };
  for (const query of [
    { kind: "workflowHtmlManifest", query: base },
    {
      kind: "workflowHtmlSource",
      query: {
        ...base,
        source: {
          manifestSha256: f.manifest.sha256,
          sourceIndex: 0,
          sourceSha256: f.manifest.sources[0]!.sha256,
          offset: 0,
          limit: 16384,
        },
      },
    },
  ])
    expect(portfolioReadGrant(query as any)).toBe("evidenceRead");
  expect(() =>
    validatePortfolioQuery({
      kind: "workflowHtmlManifest",
      query: { ...base, url: "file:///secret" },
    }),
  ).toThrow();
  const query = { kind: "workflowHtmlManifest", query: base } as const;
  const document = {
    id: f.expected.id,
    sha256: f.expected.sha256,
    name: "Report.html",
    format: "html",
    projectId: publicationId,
    taskId: null,
    stage: 0,
    teamRunId: publicationId,
    assignmentId: publicationId,
    bytes: f.original.length,
    createdAt: 100,
  };
  const result = {
    kind: query.kind,
    page: { runId: base.runId, document, manifest: f.manifest },
    pageTruncated: false,
    servedAt: 100,
  };
  validatePortfolioResult(result, query);
  expect(() =>
    validatePortfolioResult(
      {
        ...result,
        page: { ...result.page, runId: "22222222-2222-4222-8222-222222222222" },
      },
      query,
    ),
  ).toThrow();
});
const native = process.env.ION_HTML_RESOURCES_FIXTURE;
test.skipIf(!native)(
  "actual native publication/RPC chunks verify against the independent phone manifest commitment",
  async () => {
    const f = JSON.parse(readFileSync(native!, "utf8"));
    const original = Buffer.from(f.original, "base64");
    const result = await readHtmlResourceSet(f.expected, original, {
      manifest: async () => f.manifest,
      chunk: async (q) =>
        f.chunks.find(
          (c: any) => c.sourceIndex === q.sourceIndex && c.offset === q.offset,
        ),
      digest,
      current: () => true,
    });
    expect(result?.sources.length).toBe(5);
    expect(result?.manifest.entryPath).toBe("経緯🚀.html");
    expect(result?.sources[2]!.length).toBeGreaterThan(50_000);
    expect(result?.sources[4]!.length).toBe(0);
  },
);
