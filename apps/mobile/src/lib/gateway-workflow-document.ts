import {
  workflowChunkBytes,
  validatePortfolioResult,
  type PortfolioResult,
  type PortfolioQuery,
  type WorkflowDocument,
} from "@waku/client";
import { loadGatewayPortfolio } from "./gateway-portfolio";
import type { GatewayPairing } from "./gateway-client";
interface DocumentReader {
  read(query: PortfolioQuery): Promise<PortfolioResult>;
  digest(bytes: Uint8Array): Promise<string>;
}
/** Nothing is previewed or exported until every chunk and the full SHA-256
 * match the selected immutable document. Reads never evaluate HTML/scripts. */
export async function readGatewayWorkflowDocument(
  pairing: GatewayPairing,
  publisherId: string,
  runId: string,
  document: WorkflowDocument,
  current: () => boolean,
  nonce: () => string,
  digest: DocumentReader["digest"],
  reader?: DocumentReader["read"],
): Promise<Uint8Array> {
  if (
    !pairing.grants.includes("evidenceRead") ||
    pairing.expiresAt <= Date.now()
  )
    throw new Error("Pair with Read workflow documents to read this evidence");
  if (
    !Number.isSafeInteger(document.bytes) ||
    document.bytes < 1 ||
    document.bytes > 16 * 1024 * 1024
  )
    throw new Error("Document exceeds its transfer bound");
  const read =
    reader ??
    ((query: PortfolioQuery) =>
      loadGatewayPortfolio(pairing, publisherId, query, nonce(), current));
  const bytes = new Uint8Array(document.bytes);
  let offset = 0;
  const allowed = () => {
    if (!current() || pairing.expiresAt <= Date.now())
      throw new Error("Document read scope changed");
  };
  while (offset < bytes.length) {
    allowed();
    const query: PortfolioQuery = {
      kind: "workflowDocument",
      query: {
        runId,
        artifactId: document.id,
        sha256: document.sha256,
        offset,
        limit: 16_384,
      },
    };
    const result = await read(query);
    allowed();
    validatePortfolioResult(result, query);
    // The transport's full validator checks shape/cursors. Repeat the exact
    // selected metadata and chunk bounds here to protect injected/native readers.
    if (
      result.kind !== "workflowDocument" ||
      result.page.runId !== runId ||
      result.page.offset !== offset ||
      Object.keys(document).some(
        (key) =>
          result.page.document[key as keyof WorkflowDocument] !==
          document[key as keyof WorkflowDocument],
      )
    )
      throw new Error("Document response belongs to different evidence");
    const chunk = workflowChunkBytes(result.page.data),
      end = offset + chunk.length;
    if (
      !chunk.length ||
      chunk.length > 16_384 ||
      end > bytes.length ||
      result.page.nextOffset !== (end < bytes.length ? end : null)
    )
      throw new Error("Document chunk cursor changed");
    bytes.set(chunk, offset);
    offset = end;
  }
  allowed();
  const actual = await digest(bytes);
  allowed();
  if (actual !== document.sha256)
    throw new Error(
      "Document digest changed; refresh its evidence before reading again",
    );
  return bytes;
}
/** Native UTF-8 decoding without a required TextDecoder/Buffer polyfill. */
export function workflowDocumentText(bytes: Uint8Array): string {
  const text: string[] = [];
  for (let i = 0; i < bytes.length;) {
    const first = bytes[i++]!;
    let point: number, count: number, min: number;
    if (first < 128) {
      point = first;
      count = 0;
      min = 0;
    } else if (first >= 194 && first <= 223) {
      point = first & 31;
      count = 1;
      min = 128;
    } else if (first >= 224 && first <= 239) {
      point = first & 15;
      count = 2;
      min = 2048;
    } else if (first >= 240 && first <= 244) {
      point = first & 7;
      count = 3;
      min = 65536;
    } else throw new Error("Document is not valid UTF-8 text");
    for (let j = 0; j < count; j++) {
      const next = bytes[i++];
      if (next === undefined || (next & 192) !== 128)
        throw new Error("Document is not valid UTF-8 text");
      point = (point << 6) | (next & 63);
    }
    if (point < min || point > 0x10ffff || (point >= 0xd800 && point <= 0xdfff))
      throw new Error("Document is not valid UTF-8 text");
    text.push(String.fromCodePoint(point));
  }
  return text.join("");
}
export function workflowExportName(
  document: Pick<WorkflowDocument, "name" | "format" | "id">,
  suffix: string,
): string {
  const format = /^[a-z0-9]{1,12}$/.test(document.format.toLowerCase())
    ? document.format.toLowerCase()
    : "bin";
  const stem =
    document.name
      .replace(/[^a-zA-Z0-9 _.-]/g, "_")
      .replace(/^[. ]+|[. ]+$/g, "")
      .slice(0, 60) || "document";
  return `${stem}-${document.id.slice(0, 8)}-${suffix.replace(/[^a-zA-Z0-9]/g, "").slice(0, 32)}.${format}`;
}

export function workflowDocumentMime(format: string): string {
  if (["html", "htm"].includes(format)) return "text/html";
  return (
    {
      pdf: "application/pdf",
      docx: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
      json: "application/json",
      xlsx: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
      png: "image/png",
      svg: "image/svg+xml",
      xml: "application/xml",
      yaml: "application/yaml",
      yml: "application/yaml",
      csv: "text/csv",
    }[format] ?? "text/plain"
  );
}
