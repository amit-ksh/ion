import {
  validatePortfolioResult,
  validateGatewayPdfQuery,
  workflowChunkBytes,
  type GatewayPdfQuery,
  type GatewayPdfResult,
  type PortfolioQuery,
  type PortfolioResult,
  type WorkflowDocument,
} from "@waku/client";
import { loadGatewayPortfolio } from "./gateway-portfolio";
import {
  readGatewayPortfolio,
  submitGatewayPortfolio,
  type GatewayPairing,
} from "./gateway-client";
import { validatePdfPng, type PdfPagePreview } from "./artifact-pdf";
export const GATEWAY_PDF_CHUNK = 32768;
export const GATEWAY_PDF_WINDOW = 8;
interface Reader {
  read(query: PortfolioQuery): Promise<PortfolioResult>;
  digest(bytes: Uint8Array): Promise<string>;
}
/** Encoding stays bounded/yielded; native runtimes need neither Buffer nor btoa. */
async function imageUri(
  bytes: Uint8Array,
  current: () => boolean,
): Promise<string> {
  const alphabet =
      "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/",
    parts: string[] = [];
  for (let start = 0; start < bytes.length; start += 49152) {
    if (!current()) throw new Error("PDF image read scope changed.");
    const end = Math.min(bytes.length, start + 49152),
      words: string[] = [];
    for (let i = start; i < end; i += 3) {
      const a = bytes[i]!,
        b = bytes[i + 1],
        c = bytes[i + 2],
        word = (a << 16) | ((b ?? 0) << 8) | (c ?? 0);
      words.push(
        alphabet[(word >>> 18) & 63]! +
          alphabet[(word >>> 12) & 63]! +
          (b === undefined ? "=" : alphabet[(word >>> 6) & 63]!) +
          (c === undefined ? "=" : alphabet[word & 63]!),
      );
    }
    parts.push(words.join(""));
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
  }
  if (!current()) throw new Error("PDF image read scope changed.");
  return "data:image/png;base64," + parts.join("");
}
/** One owner render, then bounded parallel immutable chunks. Nothing reaches
 * Image until all source/transfer identities, offsets, SHA and PNG checks pass. */
export async function readGatewayPdfPage(
  pairing: GatewayPairing,
  publisherId: string,
  runId: string,
  revision: number,
  document: WorkflowDocument,
  page: number,
  width: number,
  current: () => boolean,
  nonce: () => string,
  digest: Reader["digest"],
  reader?: Reader["read"],
): Promise<PdfPagePreview> {
  let failed = false;
  let expires = Number.MAX_SAFE_INTEGER;
  const active = () =>
    !failed &&
    current() &&
    pairing.grants.includes("evidenceRead") &&
    pairing.expiresAt > Date.now() &&
    expires > Date.now();
  const allowed = () => {
    if (!active())
      throw new Error("PDF read scope or grant changed. Reopen its evidence.");
  };
  const read =
    reader ??
    ((query: PortfolioQuery) =>
      loadGatewayPortfolio(pairing, publisherId, query, nonce(), active, {
        maxPolls: 240,
        submit: submitGatewayPortfolio,
        read: readGatewayPortfolio,
        // The active owner services PDF-only batches at 500ms; regular reads keep their existing cadence.
        wait: () => new Promise((resolve) => setTimeout(resolve, 250)),
      }));
  const query: GatewayPdfQuery = {
    kind: "workflowPdfPage",
    query: {
      runId,
      revision,
      expected: document,
      page,
      width,
      transferId: null,
      offset: 0,
      limit: GATEWAY_PDF_CHUNK,
    },
  };
  validateGatewayPdfQuery(query);
  allowed();
  const first = await read(query);
  allowed();
  validatePortfolioResult(first, query);
  if (first.kind !== "workflowPdfPage")
    throw new Error("Invalid PDF page response.");
  const original = first.page;
  expires = original.expiresAt;
  allowed();
  const bytes = new Uint8Array(original.bytes);
  const identity = (p: GatewayPdfResult["page"]) =>
    JSON.stringify([
      p.runId,
      p.revision,
      p.document,
      p.transferId,
      p.page,
      p.pages,
      p.requestedWidth,
      p.width,
      p.height,
      p.bytes,
      p.sha256,
      p.expiresAt,
    ]);
  const seal = identity(original);
  bytes.set(workflowChunkBytes(original.data, GATEWAY_PDF_CHUNK), 0);
  for (
    let start = GATEWAY_PDF_CHUNK;
    start < bytes.length;
    start += GATEWAY_PDF_CHUNK * GATEWAY_PDF_WINDOW
  ) {
    allowed();
    const offsets = Array.from(
      {
        length: Math.min(
          GATEWAY_PDF_WINDOW,
          Math.ceil((bytes.length - start) / GATEWAY_PDF_CHUNK),
        ),
      },
      (_, i) => start + i * GATEWAY_PDF_CHUNK,
    );
    const results = await Promise.allSettled(
      offsets.map(async (offset) => {
        allowed();
        const q: GatewayPdfQuery = {
          kind: "workflowPdfPage",
          query: { ...query.query, transferId: original.transferId, offset },
        };
        try {
          const result = await read(q);
          allowed();
          validatePortfolioResult(result, q);
          if (
            result.kind !== "workflowPdfPage" ||
            identity(result.page) !== seal
          )
            throw new Error("PDF chunks belong to different rendered pages.");
          const chunk = workflowChunkBytes(result.page.data, GATEWAY_PDF_CHUNK);
          allowed();
          bytes.set(chunk, offset);
        } catch (error) {
          failed = true;
          throw error;
        }
      }),
    );
    const rejected = results.find(
      (r): r is PromiseRejectedResult => r.status === "rejected",
    );
    if (rejected) throw rejected.reason;
  }
  allowed();
  validatePdfPng(bytes, original.width, original.height);
  const actual = await digest(bytes);
  allowed();
  if (actual !== original.sha256)
    throw new Error("PDF image digest changed. Refresh its evidence.");
  const uri = await imageUri(bytes, active);
  allowed();
  return {
    page,
    pages: original.pages,
    requestedWidth: width,
    width: original.width,
    height: original.height,
    uri,
  };
}
