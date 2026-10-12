import { expect, test } from "bun:test";
import { createHash, randomUUID } from "node:crypto";
import {
  portfolioReadGrant,
  validatePortfolioQuery,
  validatePortfolioResult,
  type GatewayPdfQuery,
  type GatewayPdfResult,
  type PortfolioQuery,
  type PortfolioResult,
  type WorkflowDocument,
} from "@waku/client";
import {
  GATEWAY_PDF_CHUNK,
  GATEWAY_PDF_WINDOW,
  readGatewayPdfPage,
} from "./gateway-pdf";
import type { GatewayPairing } from "./gateway-client";
const hash = async (bytes: Uint8Array) =>
  createHash("sha256").update(bytes).digest("hex");
function png(size = 300000) {
  const bytes = Buffer.alloc(size);
  Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]).copy(bytes);
  bytes.writeUInt32BE(13, 8);
  bytes.write("IHDR", 12);
  bytes.writeUInt32BE(800, 16);
  bytes.writeUInt32BE(1000, 20);
  bytes[24] = 8;
  bytes[25] = 6;
  bytes.writeUInt32BE(size - 57, 33);
  bytes.write("IDAT", 37);
  bytes.writeUInt32BE(0, size - 12);
  bytes.write("IEND", size - 8);
  return bytes;
}
function fixture(size = 300000) {
  const bytes = png(size),
    run = randomUUID(),
    publisher = randomUUID(),
    transfer = randomUUID(),
    expiry = Date.now() + 600000;
  const pairing: GatewayPairing = {
    id: randomUUID(),
    baseUrl: "https://gateway.example",
    token: "private",
    grants: ["evidenceRead"],
    expiresAt: Date.now() + 1000000,
  };
  const document: WorkflowDocument = {
    id: randomUUID(),
    name: "Report.pdf",
    format: "pdf",
    projectId: randomUUID(),
    taskId: null,
    stage: 0,
    teamRunId: randomUUID(),
    assignmentId: randomUUID(),
    bytes: 100,
    sha256: "a".repeat(64),
    createdAt: 100,
  };
  const imageHash = createHash("sha256").update(bytes).digest("hex");
  let mounted = true,
    inflight = 0,
    maximum = 0;
  const offsets: number[] = [];
  const batches: number[][] = [];
  const read = async (q: PortfolioQuery): Promise<PortfolioResult> => {
    if (q.kind !== "workflowPdfPage") throw Error("Unexpected query");
    offsets.push(q.query.offset);
    inflight++;
    maximum = Math.max(maximum, inflight);
    await new Promise((resolve) => setTimeout(resolve, 0));
    inflight--;
    const end = Math.min(bytes.length, q.query.offset + 32768);
    return {
      kind: "workflowPdfPage",
      page: {
        runId: run,
        revision: 7,
        document: { ...document },
        transferId: transfer,
        page: 0,
        pages: 1,
        requestedWidth: 800,
        width: 800,
        height: 1000,
        bytes: bytes.length,
        sha256: imageHash,
        expiresAt: expiry,
        offset: q.query.offset,
        nextOffset: end < bytes.length ? end : null,
        data: bytes.subarray(q.query.offset, end).toString("base64"),
      },
      pageTruncated: false,
      servedAt: Date.now(),
    };
  };
  return {
    bytes,
    run,
    publisher,
    pairing,
    document,
    read,
    offsets,
    current: () => mounted,
    invalidate: () => {
      mounted = false;
    },
    maximum: () => maximum,
    transfer,
    batches,
  };
}
const load = (f: ReturnType<typeof fixture>, read = f.read, digest = hash) =>
  readGatewayPdfPage(
    f.pairing,
    f.publisher,
    f.run,
    7,
    f.document,
    0,
    800,
    f.current,
    randomUUID,
    digest,
    read,
  );
test("gateway PDF chunks use exact evidence grant and immutable source/page/zoom", async () => {
  const f = fixture();
  const result = await load(f);
  expect(result.page).toBe(0);
  expect(result.pages).toBe(1);
  expect(Buffer.from(result.uri.split(",")[1]!, "base64")).toEqual(f.bytes);
  expect(f.maximum()).toBe(8);
  expect(f.offsets).toEqual(
    Array.from(
      { length: Math.ceil(f.bytes.length / 32768) },
      (_, i) => i * 32768,
    ),
  );
  const query: GatewayPdfQuery = {
    kind: "workflowPdfPage",
    query: {
      runId: f.run,
      revision: 7,
      expected: f.document,
      page: 0,
      width: 800,
      transferId: null,
      offset: 0,
      limit: 32768,
    },
  };
  expect(portfolioReadGrant(query)).toBe("evidenceRead");
  expect(() =>
    validatePortfolioQuery({
      ...query,
      query: { ...query.query, offset: 32768 },
    }),
  ).toThrow();
  expect(() =>
    validatePortfolioQuery({
      ...query,
      query: { ...query.query, limit: 65536 },
    }),
  ).toThrow();
});
test("largest allowed page uses bounded parallel windows and a subminute active cadence bound", async () => {
  const f = fixture(12 * 1024 * 1024);
  let renderCount = 0,
    batch = 0,
    active = 0;
  const windows: number[] = [];
  const read = async (q: PortfolioQuery) => {
    if (q.kind !== "workflowPdfPage") throw Error("Wrong query");
    if (q.query.transferId === null) renderCount++;
    active++;
    windows[batch] = (windows[batch] ?? 0) + 1;
    const result = await f.read(q);
    if (--active === 0) batch++;
    return result;
  };
  const started = performance.now();
  const result = await load(f, read);
  expect(result.uri.length).toBe(
    "data:image/png;base64,".length + Math.ceil(f.bytes.length / 3) * 4,
  );
  expect(renderCount).toBe(1);
  expect(f.maximum()).toBeLessThanOrEqual(GATEWAY_PDF_WINDOW);
  expect(windows.length).toBe(49);
  expect(windows[0]).toBe(1);
  expect(windows.slice(1).every((n) => n <= 8)).toBe(true);
  // Deterministic scheduling bound excludes real network/host render latency:
  // first old idle tick(5s), native deadline(10s), 48 active windows(500ms)+device poll(500ms/window).
  expect(5000 + 10000 + (windows.length - 1) * 1000).toBeLessThan(65000);
  expect(windows.length).toBeLessThan(
    Math.ceil(f.bytes.length / GATEWAY_PDF_CHUNK),
  );
  expect(performance.now() - started).toBeLessThan(20000);
}, 30000);
test("mixed transfer/hash/dimensions/revision/source and incorrect advancement never return an image", async () => {
  for (const key of [
    "transferId",
    "sha256",
    "width",
    "revision",
    "bytes",
    "expiresAt",
    "nextOffset",
    "document",
  ]) {
    const f = fixture();
    const read = async (q: PortfolioQuery) => {
      const result = (await f.read(q)) as GatewayPdfResult;
      if (result.page.offset > 0) {
        const row = result.page as any;
        row[key] =
          key === "document"
            ? { ...row.document, assignmentId: randomUUID() }
            : key === "transferId"
              ? randomUUID()
              : key === "sha256"
                ? "b".repeat(64)
                : row[key] + 1;
      }
      return result;
    };
    await expect(load(f, read)).rejects.toThrow();
  }
});
test("background/grant expiry/revocation and late digest discard current page bytes", async () => {
  const f = fixture();
  const read = async (q: PortfolioQuery) => {
    const r = await f.read(q);
    f.invalidate();
    return r;
  };
  await expect(load(f, read)).rejects.toThrow(/scope/);
  const g = fixture();
  await expect(
    load(g, g.read, async (bytes) => {
      g.invalidate();
      return hash(bytes);
    }),
  ).rejects.toThrow(/scope/);
  const h = fixture();
  const revoked = async (q: PortfolioQuery) => {
    const r = await h.read(q);
    h.pairing.grants = [];
    return r;
  };
  await expect(load(h, revoked)).rejects.toThrow(/grant/);
  const i = fixture();
  i.pairing.expiresAt = Date.now() - 1;
  await expect(load(i)).rejects.toThrow();
  expect(i.offsets).toHaveLength(0);
});
test("digest mismatch and unsupported PNG preserve only the separately verified original", async () => {
  const f = fixture();
  await expect(load(f, f.read, async () => "c".repeat(64))).rejects.toThrow(
    /digest/,
  );
  const g = fixture();
  g.bytes.write("acTL", 37);
  await expect(load(g)).rejects.toThrow(/supported/);
});
test("strict gateway validator rejects private fields, overlarge chunks and malformed canonical base64", async () => {
  const f = fixture();
  const query: GatewayPdfQuery = {
    kind: "workflowPdfPage",
    query: {
      runId: f.run,
      revision: 7,
      expected: f.document,
      page: 0,
      width: 800,
      transferId: null,
      offset: 0,
      limit: 32768,
    },
  };
  const result = (await f.read(query)) as GatewayPdfResult;
  expect(() => validatePortfolioResult(result, query)).not.toThrow();
  for (const change of [
    () => ({ ...result, page: { ...result.page, path: "/private/source" } }),
    () => ({ ...result, page: { ...result.page, data: "AB==" } }),
    () => ({
      ...result,
      page: { ...result.page, data: Buffer.alloc(32769).toString("base64") },
    }),
  ])
    expect(() => validatePortfolioResult(change(), query)).toThrow();
});

test("real gateway HTTP carries two 12 MiB page transfers at the active 500ms owner cadence", async () => {
  const { startGateway } =
    await import("../../../../packages/personal-gateway/src/server");
  const { mkdtempSync, rmSync } = await import("node:fs");
  const { tmpdir } = await import("node:os");
  const { join } = await import("node:path");
  const dir = mkdtempSync(join(tmpdir(), "ion-pdf-transfer-")),
    ownerToken = "owner_" + "q".repeat(48),
    gateway = startGateway({
      databasePath: join(dir, "gateway.sqlite"),
      ownerToken,
      allowedTools: [],
    });
  const stopOwner = new AbortController();
  const call = async (
    path: string,
    method: string,
    identity: { id: string; token: string },
    body?: unknown,
  ) => {
    const deadline = setTimeout(() => stopOwner.abort(), 10000);
    try {
      const response = await fetch(new URL(path, gateway.server.url), {
        method,
        signal: stopOwner.signal,
        headers: {
          Authorization: `Bearer ${identity.token}`,
          "X-Waku-Device-Id": identity.id,
          ...(body === undefined ? {} : { "Content-Type": "application/json" }),
        },
        body: body === undefined ? undefined : JSON.stringify(body),
      });
      if (!response.ok)
        throw Error(
          "Gateway PDF fixture request failed with " + response.status,
        );
      return (await response.json()) as any;
    } finally {
      clearTimeout(deadline);
    }
  };
  const pair = async (grants: string[]) => {
    const invite = await call(
      "/v1/owner/invites",
      "POST",
      { id: "", token: ownerToken },
      { grants },
    );
    return call(
      "/v1/pair",
      "POST",
      { id: "", token: "" },
      { invite: invite.invite, name: "PDF acceptance" },
    );
  };
  let stopped = false,
    ownerTask: Promise<void> | null = null;
  try {
    const publisher = await pair(["publish"]),
      phone = await pair(["evidenceRead"]),
      f = fixture(12 * 1024 * 1024);
    const pairing: GatewayPairing = {
      id: phone.id,
      token: phone.token,
      baseUrl: String(gateway.server.url),
      grants: ["evidenceRead"],
      expiresAt: Date.now() + 300000,
    };
    await call("/v1/device/portfolio", "PUT", publisher, { version: 1 });
    let chunks = 0,
      renders = 0,
      maximum = 0,
      lastPublish = performance.now();
    const fixedExpiry = Date.now() + 599000;
    const pageIds = new Map<number, string>();
    const digest = await hash(f.bytes);
    ownerTask = (async () => {
      while (!stopped) {
        const tick = performance.now();
        if (tick - lastPublish > 5000) {
          await call("/v1/device/portfolio", "PUT", publisher, { version: 1 });
          lastPublish = tick;
        }
        const queue = await call(
          "/v1/device/portfolio/queue",
          "GET",
          publisher,
        );
        maximum = Math.max(maximum, queue.jobs.length);
        for (const job of queue.jobs) {
          const q = job.query as GatewayPdfQuery;
          validatePortfolioQuery(q);
          expect(q.kind).toBe("workflowPdfPage");
          expect(q.query.expected).toEqual(f.document);
          expect(q.query.runId).toBe(f.run);
          if (q.query.transferId === null) {
            pageIds.set(q.query.page, randomUUID());
            renders++;
          } else expect(q.query.transferId).toBe(pageIds.get(q.query.page)!);
          const offset = q.query.offset,
            end = Math.min(f.bytes.length, offset + 32768),
            result: GatewayPdfResult = {
              kind: "workflowPdfPage",
              page: {
                runId: f.run,
                revision: 7,
                document: f.document,
                transferId: pageIds.get(q.query.page)!,
                page: q.query.page,
                pages: 2,
                requestedWidth: 800,
                width: 800,
                height: 1000,
                bytes: f.bytes.length,
                sha256: digest,
                expiresAt: Date.now() + 599000,
                offset,
                nextOffset: end < f.bytes.length ? end : null,
                data: f.bytes.subarray(offset, end).toString("base64"),
              },
              pageTruncated: false,
              servedAt: Date.now(),
            };
          // A transfer expiry is immutable across all chunks, just like native cache bytes.
          result.page.expiresAt = fixedExpiry;
          expect(JSON.stringify(result).length).toBeLessThan(60000);
          await call(
            `/v1/device/portfolio/queries/${job.id}/complete`,
            "POST",
            publisher,
            { result },
          );
          chunks++;
        }
        await new Promise((resolve) =>
          setTimeout(resolve, Math.max(1, 500 - (performance.now() - tick))),
        );
      }
    })();
    const start = performance.now();
    for (const page of [0, 1]) {
      const preview = await readGatewayPdfPage(
        pairing,
        publisher.id,
        f.run,
        7,
        f.document,
        page,
        800,
        () => true,
        randomUUID,
        hash,
      );
      expect(
        await hash(Buffer.from(preview.uri.split(",")[1]!, "base64")),
      ).toBe(digest);
      expect(preview.page).toBe(page);
    }
    expect(chunks).toBe(768);
    expect(renders).toBe(2);
    expect(maximum).toBeLessThanOrEqual(8);
    const elapsed = performance.now() - start;
    expect(elapsed).toBeLessThan(120000);
    expect(elapsed).toBeLessThan((768 * 5000) / 10); // More than 10x faster than old serial five-second scheduling.
    console.log(
      "Two maximum PDF HTTP transfers:",
      Math.round(elapsed),
      "ms,",
      chunks,
      "chunks, max",
      maximum,
      "pending.",
    );
  } finally {
    stopped = true;

    stopOwner.abort();
    if (ownerTask)
      await ownerTask.catch((error) => {
        if (!(
          stopped &&
          stopOwner.signal.aborted &&
          (error === null || error?.name === "AbortError")
        ))
          throw error;
      });

    await gateway.server.stop(true);
    await gateway.close();

    rmSync(dir, { recursive: true, force: true });
  }
}, 150000);
