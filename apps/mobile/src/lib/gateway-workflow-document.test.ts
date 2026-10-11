import { expect, test } from "bun:test";
import { createHash, randomUUID } from "node:crypto";
import {
  portfolioReadGrant,
  validatePortfolioQuery,
  validatePortfolioResult,
  workflowChunkBytes,
  type PortfolioQuery,
  type PortfolioResult,
  type WorkflowDocument,
} from "@waku/client";
import {
  readGatewayWorkflowDocument,
  workflowDocumentText,
  workflowExportName,
} from "./gateway-workflow-document";
import type { GatewayPairing } from "./gateway-client";
const pairing: GatewayPairing = {
  id: randomUUID(),
  baseUrl: "https://gateway.example",
  token: "private-evidence-token",
  grants: ["evidenceRead"],
  expiresAt: Date.now() + 120_000,
};
const publisher = randomUUID(),
  run = randomUUID();
const digest = async (bytes: Uint8Array) =>
  createHash("sha256").update(bytes).digest("hex");
function fixture() {
  const content = Buffer.from(
    "Verified 😀 <script>alert(1)</script>\n".repeat(2000),
  );
  const document: WorkflowDocument = {
    id: randomUUID(),
    name: "report.html",
    format: "html",
    projectId: randomUUID(),
    taskId: randomUUID(),
    stage: 0,
    teamRunId: randomUUID(),
    assignmentId: randomUUID(),
    bytes: content.length,
    sha256: createHash("sha256").update(content).digest("hex"),
    createdAt: 100,
  };
  const offsets: number[] = [];
  let current = true;
  const reader = async (query: PortfolioQuery): Promise<PortfolioResult> => {
    if (query.kind !== "workflowDocument") throw new Error("Unexpected query");
    offsets.push(query.query.offset);
    expect(query.query.artifactId).toBe(document.id);
    expect(query.query.sha256).toBe(document.sha256);
    const end = Math.min(
      content.length,
      query.query.offset + query.query.limit,
    );
    return {
      kind: "workflowDocument",
      page: {
        runId: run,
        document: { ...document },
        offset: query.query.offset,
        nextOffset: end < content.length ? end : null,
        data: content.subarray(query.query.offset, end).toString("base64"),
      },
      pageTruncated: false,
      servedAt: Date.now(),
    };
  };
  return {
    content,
    document,
    reader,
    offsets,
    current: () => current,
    invalidate: () => {
      current = false;
    },
  };
}
test("complete immutable document reads assemble chunks and verify SHA-256 before returning bytes", async () => {
  const f = fixture();
  const bytes = await readGatewayWorkflowDocument(
    pairing,
    publisher,
    run,
    f.document,
    f.current,
    randomUUID,
    digest,
    f.reader,
  );
  expect(Buffer.from(bytes)).toEqual(f.content);
  expect(f.offsets).toEqual([0, 16384, 32768, 49152, 65536]);
  expect(workflowDocumentText(bytes)).toBe(f.content.toString("utf8"));
});
test("changed bytes fail full digest verification and changed document identities fail before preview/export", async () => {
  const f = fixture();
  const reader = async (q: PortfolioQuery) => {
    const result = await f.reader(q);
    if (result.kind === "workflowDocument" && result.page.offset === 0)
      result.page.data = Buffer.from("x".repeat(16384)).toString("base64");
    return result;
  };
  await expect(
    readGatewayWorkflowDocument(
      pairing,
      publisher,
      run,
      f.document,
      f.current,
      randomUUID,
      digest,
      reader,
    ),
  ).rejects.toThrow("digest changed");
  for (const change of [
    { name: "changed" },
    { id: randomUUID() },
    { sha256: "a".repeat(64) },
    { bytes: 1 },
    { teamRunId: randomUUID() },
  ]) {
    const reader = async (q: PortfolioQuery) => {
      const result = await f.reader(q);
      if (result.kind === "workflowDocument")
        result.page.document = { ...result.page.document, ...change };
      return result;
    };
    await expect(
      readGatewayWorkflowDocument(
        pairing,
        publisher,
        run,
        f.document,
        f.current,
        randomUUID,
        digest,
        reader,
      ),
    ).rejects.toThrow();
  }
});
test("document reading fences focus, credentials and owner changes before each chunk and after hashing", async () => {
  const f = fixture();
  f.invalidate();
  await expect(
    readGatewayWorkflowDocument(
      pairing,
      publisher,
      run,
      f.document,
      f.current,
      randomUUID,
      digest,
      f.reader,
    ),
  ).rejects.toThrow("scope");
  expect(f.offsets).toEqual([]);
  const next = fixture();
  const reader = async (q: PortfolioQuery) => {
    const result = await next.reader(q);
    next.invalidate();
    return result;
  };
  await expect(
    readGatewayWorkflowDocument(
      pairing,
      publisher,
      run,
      next.document,
      next.current,
      randomUUID,
      digest,
      reader,
    ),
  ).rejects.toThrow("scope");
  expect(next.offsets).toHaveLength(1);
  const hash = fixture();
  await expect(
    readGatewayWorkflowDocument(
      pairing,
      publisher,
      run,
      hash.document,
      hash.current,
      randomUUID,
      async (bytes) => {
        hash.invalidate();
        return digest(bytes);
      },
      hash.reader,
    ),
  ).rejects.toThrow("scope");
});
test("workflow-only, expired and invalid document grants cannot fetch bytes", async () => {
  const f = fixture();
  for (const credential of [
    { ...pairing, grants: ["automationRead"] },
    { ...pairing, grants: ["taskRead", "inboxRead"] },
    { ...pairing, expiresAt: Date.now() - 1 },
  ])
    await expect(
      readGatewayWorkflowDocument(
        credential,
        publisher,
        run,
        f.document,
        f.current,
        randomUUID,
        digest,
        f.reader,
      ),
    ).rejects.toThrow("Read workflow documents");
  expect(f.offsets).toEqual([]);
});
test("base64 decoding is exact without Buffer or atob and rejects noncanonical or oversized chunks", () => {
  for (const size of [1, 2, 3, 16_384]) {
    const bytes = Buffer.from(Array.from({ length: size }, (_, i) => i % 256));
    expect(Buffer.from(workflowChunkBytes(bytes.toString("base64")))).toEqual(
      bytes,
    );
  }
  for (const value of [
    "YQ=",
    "YR==",
    "YWJ=",
    "_AAA",
    "YQ==\n",
    "A".repeat(21_852),
  ])
    expect(() => workflowChunkBytes(value)).toThrow();
});
test("native UTF-8 decoding preserves supplementary Unicode and rejects invalid text without evaluating markup", () => {
  const value = "é文😀 <script>throw Error()</script>\n";
  expect(workflowDocumentText(Buffer.from(value))).toBe(value);
  for (const bytes of [
    [0xc0, 0x80],
    [0xed, 0xa0, 0x80],
    [0xf4, 0x90, 0x80, 0x80],
    [0xe2, 0x82],
    [0x80],
  ])
    expect(() => workflowDocumentText(Uint8Array.from(bytes))).toThrow("UTF-8");
});
test("history pages validate creation-time ordering, cursor, scope and extra private fields", () => {
  const query: PortfolioQuery = {
    kind: "workflowHistory",
    query: {
      projectId: null,
      definitionId: null,
      pendingOnly: false,
      after: null,
      limit: 32,
    },
  };
  const make = (id: string, createdAt: number) => ({
    id,
    name: "Release",
    projectId: randomUUID(),
    definitionId: randomUUID(),
    revision: 1,
    createdAt,
    scheduledAt: null,
    deadline: 1000,
    status: "completed",
    currentStage: 1,
    stageCount: 1,
  });
  const newest = make("00000000-0000-0000-0000-000000000001", 200),
    oldest = make("ffffffff-ffff-ffff-ffff-ffffffffffff", 100);
  const page = {
    kind: "workflowHistory",
    page: { runs: [newest, oldest], nextCursor: oldest.id, pendingCount: 50 },
    pageTruncated: false,
    servedAt: Date.now(),
  };
  expect(() => validatePortfolioResult(page, query)).not.toThrow();
  expect(() =>
    validatePortfolioResult(
      { ...page, page: { ...page.page, runs: [oldest, newest] } },
      query,
    ),
  ).toThrow("scope");
  expect(() =>
    validatePortfolioResult(
      { ...page, page: { ...page.page, nextCursor: newest.id } },
      query,
    ),
  ).toThrow("cursor");
  expect(() =>
    validatePortfolioResult(page, {
      ...query,
      query: { ...query.query, pendingOnly: true },
    }),
  ).toThrow("scope");
  expect(() =>
    validatePortfolioResult(
      {
        ...page,
        page: { ...page.page, runs: [{ ...newest, host: "/private" }, oldest] },
      },
      query,
    ),
  ).toThrow();
});
test("history read schemas reject effect fields and map to independent grants", () => {
  const queries: PortfolioQuery[] = [
    { kind: "workflowRun", query: { runId: run } },
    {
      kind: "workflowHistory",
      query: {
        projectId: null,
        definitionId: null,
        pendingOnly: false,
        after: null,
        limit: 32,
      },
    },
    { kind: "workflowEvidence", query: { runId: run, after: null, limit: 32 } },
    {
      kind: "workflowDocument",
      query: {
        runId: run,
        artifactId: randomUUID(),
        sha256: "a".repeat(64),
        offset: 0,
        limit: 16384,
      },
    },
  ];
  expect(queries.map(portfolioReadGrant)).toEqual([
    "automationRead",
    "automationRead",
    "evidenceRead",
    "evidenceRead",
  ]);
  for (const query of queries) {
    expect(() => validatePortfolioQuery(query)).not.toThrow();
    expect(() =>
      validatePortfolioQuery({
        ...query,
        query: { ...query.query, approved: true },
      }),
    ).toThrow();
  }
});
test("export names cannot traverse folders or substitute executable extensions", () => {
  const f = fixture();
  const name = workflowExportName(
    { ...f.document, name: "../../evil\\script", format: "pdf/../../exe" },
    "../token",
  );
  expect(name).not.toContain("/");
  expect(name).not.toContain("\\");
  expect(name).toEndWith(".bin");
});
