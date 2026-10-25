import { expect, test } from "bun:test";
import { createHash, randomUUID } from "node:crypto";
import type {
  ArtifactLibraryItem,
  WakuClient,
  WorkflowDocument,
} from "@waku/client";
import { artifactText, readArtifactDocument } from "./artifact-library";
import {
  prepareTextDocumentViews,
  prepareDocumentSource,
} from "./document-preview";
import {
  prepareDocumentText,
  documentTextSteps,
  MAX_DOCUMENT_TEXT_BYTES,
  TEXT_DOCUMENT_FORMATS,
} from "./document-text";
import { prepareHtmlViews, prepareRenderedHtml } from "./html-rendering";
import { readGatewayWorkflowDocument } from "./gateway-workflow-document";
const digest = async (bytes: Uint8Array) =>
  createHash("sha256").update(bytes).digest("hex");

function publication(bytes: Uint8Array, format = "txt"): ArtifactLibraryItem {
  return {
    id: randomUUID(),
    projectId: randomUUID(),
    taskId: randomUUID(),
    format,
    name: `report.${format}`,
    origin: "teamAssignment",
    createdAt: 100,
    bytes: bytes.length,
    sha256: createHash("sha256").update(bytes).digest("hex"),
  };
}
function direct(item: ArtifactLibraryItem, bytes: Uint8Array) {
  return {
    request: async () => ({
      type: "artifactDocument",
      document: {
        document: { ...item },
        bytes: Buffer.from(bytes).toString("base64"),
      },
    }),
  } as Pick<WakuClient, "request">;
}

test("batched UTF8 preserves exact scalars across byte budgets without browser codecs", async () => {
  const original = globalThis.TextDecoder;
  try {
    Object.assign(globalThis, { TextDecoder: undefined });
    const text =
      "\uFEFF" + "a".repeat(8188) + "🚀" + "語".repeat(4096) + "\0\r\nend";
    expect(await prepareDocumentText(Buffer.from(text), () => true)).toBe(text);
    const steps = documentTextSteps(Buffer.from(text));
    let yields = 0;
    for (;;) {
      const step = steps.next();
      if (step.done) {
        expect(step.value).toBe(text);
        break;
      }
      yields++;
    }
    expect(yields).toBeGreaterThan(1);
  } finally {
    Object.assign(globalThis, { TextDecoder: original });
  }
});

test("malformed UTF8 refuses after a valid large prefix and never returns partial text", async () => {
  for (const suffix of [
    [0xc0, 0x80],
    [0xe0, 0x80, 0x80],
    [0xed, 0xa0, 0x80],
    [0xf4, 0x90, 0x80, 0x80],
    [0xf5, 0x80, 0x80, 0x80],
    [0xe2, 0x82],
    [0xff],
  ]) {
    await expect(
      prepareDocumentText(
        Buffer.concat([Buffer.alloc(8191, 65), Buffer.from(suffix)]),
        () => true,
      ),
    ).rejects.toThrow("UTF-8");
  }
});

test("text decoding, source batching and final results honor scope cancellation", async () => {
  const bytes = Buffer.alloc(128000, 65);
  await expect(prepareDocumentText(bytes, () => false)).rejects.toThrow(
    "scope changed",
  );
  let current = true;
  const decoded = prepareDocumentText(bytes, () => current);
  setTimeout(() => {
    current = false;
  }, 0);
  await expect(decoded).rejects.toThrow("scope changed");
  current = true;
  const rows = prepareDocumentSource("a".repeat(128000), () => current);
  setTimeout(() => {
    current = false;
  }, 0);
  await expect(rows).rejects.toThrow("scope changed");
  await expect(prepareDocumentSource("tiny", () => false)).rejects.toThrow(
    "scope changed",
  );
  let checks = 0;
  await expect(
    prepareDocumentText(Buffer.from("tiny"), () => ++checks < 2),
  ).rejects.toThrow("scope changed");
});

test("large source retains every code point and small semantic reading keeps its behavior", async () => {
  const text = "a".repeat(999) + "🚀" + "語\r\n".repeat(40000);
  const result = await prepareTextDocumentViews("md", text, () => true);
  expect(result.sourceRows.map((row) => row.text).join("")).toBe(text);
  expect(result.readingPreview.rows).toBe(result.sourceRows);
  expect(result.readingPreview.sourceFallback).toBe(true);
  for (const row of result.sourceRows) {
    expect(row.text.length).toBeLessThanOrEqual(1000);
    expect(/[\uD800-\uDBFF]$/.test(row.text)).toBe(false);
    expect(row.node).toBeNull();
  }
  const small = await prepareTextDocumentViews(
    "md",
    "# Heading\n\n[link](ion://run)",
    () => true,
  );
  expect(small.readingPreview.rows[0]?.node?.type).toBe("heading");
  expect(
    JSON.stringify(small.readingPreview.rows.map((row) => row.node)),
  ).not.toContain("ion://run");
  expect(small.sourceRows.map((row) => row.text).join("")).toContain(
    "ion://run",
  );
});

test("all supported text formats accept complete larger verified originals", async () => {
  const bytes = Buffer.from("report 語 🚀\n".repeat(7000));
  for (const format of TEXT_DOCUMENT_FORMATS) {
    expect(
      await artifactText(publication(bytes, format), bytes, () => true),
    ).toBe(bytes.toString());
  }
  expect(
    await artifactText(publication(bytes, "docx"), bytes, () => true),
  ).toBeNull();
  await expect(
    artifactText(publication(bytes), bytes, () => false),
  ).rejects.toThrow("scope changed");
}, 30000);

test("exact 16 MiB original survives direct immutable read, yielded decode and complete source", async () => {
  const bytes = Buffer.alloc(MAX_DOCUMENT_TEXT_BYTES, 65);
  bytes.set(Buffer.from("🚀"), 8190);
  const item = publication(bytes);
  const received = await readArtifactDocument(
    direct(item, bytes),
    item,
    () => true,
    digest,
  );
  expect(await digest(received)).toBe(item.sha256);
  let ticks = 0;
  const heartbeat = setInterval(() => ticks++, 1);
  try {
    const text = await artifactText(item, received, () => true);
    const views = await prepareTextDocumentViews(
      item.format,
      text!,
      () => true,
    );
    const restored = views.sourceRows.map((row) => row.text).join("");
    expect(Buffer.byteLength(restored)).toBe(MAX_DOCUMENT_TEXT_BYTES);
    expect(createHash("sha256").update(restored).digest("hex")).toBe(
      item.sha256,
    );
    expect(views.readingPreview.rows).toBe(views.sourceRows);
    expect(ticks).toBeGreaterThan(100);
  } finally {
    clearInterval(heartbeat);
  }
}, 60000);

test("gateway reads and prepares the same complete larger text with immutable scope", async () => {
  const bytes = Buffer.from("research 語 🚀\n".repeat(70000));
  const item = publication(bytes, "md"),
    run = randomUUID(),
    owner = randomUUID();
  const { origin: _origin, ...metadata } = item;
  const record: WorkflowDocument = {
    ...metadata,
    stage: 0,
    teamRunId: randomUUID(),
    assignmentId: randomUUID(),
  };
  const received = await readGatewayWorkflowDocument(
    {
      id: randomUUID(),
      baseUrl: "https://gateway.example",
      token: "fixture-only",
      grants: ["evidenceRead"],
      expiresAt: Date.now() + 120000,
    },
    owner,
    run,
    record,
    () => true,
    randomUUID,
    digest,
    async (query) => {
      if (query.kind !== "workflowDocument")
        throw new Error("Unexpected query");
      const offset = query.query.offset,
        end = Math.min(bytes.length, offset + query.query.limit);
      return {
        kind: "workflowDocument",
        page: {
          runId: run,
          document: { ...record },
          offset,
          nextOffset: end < bytes.length ? end : null,
          data: bytes.subarray(offset, end).toString("base64"),
        },
        pageTruncated: false,
        servedAt: Date.now(),
      };
    },
  );
  const text = await prepareDocumentText(received, () => true);
  const views = await prepareTextDocumentViews(record.format, text, () => true);
  expect(views.sourceRows.map((row) => row.text).join("")).toBe(
    bytes.toString(),
  );
  expect(await digest(received)).toBe(record.sha256);
}, 30000);

test("byte ceilings refuse oversized text rather than truncating Unicode/source", async () => {
  await expect(
    prepareDocumentText(
      new Uint8Array(MAX_DOCUMENT_TEXT_BYTES + 1),
      () => true,
    ),
  ).rejects.toThrow("16 MiB");
  await expect(
    prepareDocumentSource(
      "語".repeat(Math.floor(MAX_DOCUMENT_TEXT_BYTES / 3) + 1),
      () => true,
    ),
  ).rejects.toThrow("16 MiB");
}, 30000);

test("full publication-sized HTML retains authored styling and complete source", async () => {
  const prefix =
    "<style>@media(max-width:600px){main{display:grid}}main{background:linear-gradient(red,blue)}</style><main><p>";
  const tail = "</p><h2>Publication end</h2></main>";
  const text =
    prefix +
    "x".repeat(MAX_DOCUMENT_TEXT_BYTES - prefix.length - tail.length) +
    tail;
  const result = await prepareHtmlViews(text, () => true);
  expect(result.renderError).toBeNull();
  expect(result.rendered?.html).toContain("@media(max-width:600px)");
  expect(result.rendered?.html).toContain("linear-gradient(red,blue)");
  expect(result.rendered?.html).toContain("<h2>Publication end</h2>");
  expect(result.rendered?.html).toContain("script-src 'none'");
  expect(result.sourceRows.map((row) => row.text).join("")).toBe(text);
  // Preparation yields with setTimeout(0) every 4ms. Windows rounds each yield
  // up to its ~15ms timer tick, so 16 MiB takes 40-60s there; the work is ~12s.
}, 180000);

test("entity expansion remains bounded while complete original source survives", async () => {
  const text = "<p>" + "&".repeat(7 * 1024 * 1024) + "</p>";
  const result = await prepareHtmlViews(text, () => true);
  expect(result.rendered === null).toBe(true);
  expect(result.renderError).toContain("32 MiB");
  expect(result.sourceRows.map((row) => row.text).join("")).toBe(text);
}, 60000);

test("large retained attributes are serialized in cancellable batches", async () => {
  const value = "x".repeat(2 * 1024 * 1024);
  const source = `<p title="${value}">End</p>`;
  const result = await prepareRenderedHtml(source, () => true);
  expect(result.html).toContain(`title="${value}"`);
  let current = true;
  const pending = prepareRenderedHtml(source, () => current);
  setTimeout(() => {
    current = false;
  }, 0);
  await expect(pending).rejects.toThrow("scope changed");
}, 30000);
