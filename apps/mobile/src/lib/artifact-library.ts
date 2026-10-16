import type {
  ArtifactLibraryItem,
  ArtifactLibraryPage,
  ArtifactLibraryQuery,
  WakuClient,
} from "@waku/client";
import {
  isTextDocumentFormat,
  MAX_DOCUMENT_TEXT_BYTES,
  prepareDocumentText,
} from "./document-text";
const ID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const NIL = "00000000-0000-0000-0000-000000000000";
const HASH = /^[0-9a-f]{64}$/;
export const ARTIFACT_FORMATS = [
  "md",
  "txt",
  "json",
  "csv",
  "html",
  "htm",
  "pdf",
  "docx",
  "xlsx",
  "pptx",
  "png",
  "svg",
  "xml",
  "yaml",
  "yml",
] as const;
const FIELDS = "bytes,createdAt,format,id,name,origin,projectId,sha256,taskId";
const shape = (
  value: unknown,
  fields: string,
): value is Record<string, unknown> =>
  !!value &&
  typeof value === "object" &&
  !Array.isArray(value) &&
  Object.keys(value).sort().join(",") === fields;
const number = (value: unknown): value is number =>
  typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
function utf8Size(text: string): number {
  let size = 0;
  for (const character of text) {
    const point = character.codePointAt(0)!;
    size += point <= 0x7f ? 1 : point <= 0x7ff ? 2 : point <= 0xffff ? 3 : 4;
  }
  return size;
}
export const artifactId = (value: unknown): value is string =>
  typeof value === "string" && ID.test(value) && value !== NIL;
export function validateArtifactItem(
  value: unknown,
): asserts value is ArtifactLibraryItem {
  if (
    !shape(value, FIELDS) ||
    !artifactId(value.id) ||
    !artifactId(value.projectId) ||
    !artifactId(value.taskId) ||
    typeof value.name !== "string" ||
    !value.name.trim() ||
    utf8Size(value.name) > 160 ||
    /[\x00-\x1f\x7f-\x9f/\\:]/.test(value.name) ||
    [".", ".."].includes(value.name) ||
    typeof value.format !== "string" ||
    !(ARTIFACT_FORMATS as readonly string[]).includes(value.format) ||
    !number(value.createdAt) ||
    !value.createdAt ||
    !number(value.bytes) ||
    value.bytes < 1 ||
    value.bytes > 16 * 1024 * 1024 ||
    typeof value.sha256 !== "string" ||
    !HASH.test(value.sha256) ||
    !["agentGenerated", "agentEdit", "teamAssignment"].includes(
      String(value.origin),
    )
  )
    throw new Error(
      "Invalid published document metadata. Refresh the artifact library.",
    );
}
function cursor(value: unknown): value is string {
  return (
    typeof value === "string" &&
    value.length <= 4096 &&
    value.length > 0 &&
    value.length % 2 === 0 &&
    /^[0-9a-f]+$/.test(value)
  );
}
export function validateArtifactQuery(query: ArtifactLibraryQuery): void {
  if (
    !shape(query, "after,format,limit,projectId,taskId") ||
    (query.projectId !== null && !artifactId(query.projectId)) ||
    (query.taskId !== null && !artifactId(query.taskId)) ||
    (query.format !== null &&
      !(ARTIFACT_FORMATS as readonly unknown[]).includes(query.format)) ||
    (query.after !== null && !cursor(query.after)) ||
    !number(query.limit) ||
    query.limit < 1 ||
    query.limit > 32
  )
    throw new Error(
      "Invalid artifact filters. Reset the library filters and refresh.",
    );
}
export async function loadArtifactLibrary(
  client: Pick<WakuClient, "request">,
  query: ArtifactLibraryQuery,
): Promise<ArtifactLibraryPage> {
  validateArtifactQuery(query);
  const reply = await client.request({ type: "listArtifactLibrary", query });
  if (
    reply.type !== "artifactLibrary" ||
    !shape(reply.page, "items,nextCursor,total") ||
    !Array.isArray(reply.page.items) ||
    reply.page.items.length > query.limit ||
    !number(reply.page.total) ||
    reply.page.total < reply.page.items.length ||
    reply.page.total > 1000 ||
    (reply.page.nextCursor !== null && !cursor(reply.page.nextCursor))
  )
    throw new Error("Invalid artifact library response. Refresh to retry.");
  const seen = new Set<string>();
  for (const item of reply.page.items) {
    validateArtifactItem(item);
    if (
      seen.has(item.id) ||
      (query.projectId && item.projectId !== query.projectId) ||
      (query.taskId && item.taskId !== query.taskId) ||
      (query.format && item.format !== query.format)
    )
      throw new Error(
        "Document response belongs to different artifact filters.",
      );
    seen.add(item.id);
  }
  for (let i = 1; i < reply.page.items.length; i++) {
    const earlier = reply.page.items[i - 1]!,
      later = reply.page.items[i]!;
    if (
      earlier.createdAt < later.createdAt ||
      (earlier.createdAt === later.createdAt && earlier.id <= later.id)
    )
      throw new Error("Artifact ordering changed. Refresh the library.");
  }
  if (
    reply.page.nextCursor &&
    (reply.page.items.length !== query.limit ||
      reply.page.nextCursor === query.after)
  )
    throw new Error("Artifact cursor did not advance safely.");
  return reply.page;
}
export function artifactRoute(params: {
  owner?: unknown;
  document?: unknown;
  address?: unknown;
}): { owner: string; document: ArtifactLibraryItem; address: string } {
  if (
    !artifactId(params.owner) ||
    typeof params.address !== "string" ||
    !params.address ||
    params.address.length > 512 ||
    /[\x00-\x1f]/.test(params.address) ||
    typeof params.document !== "string" ||
    params.document.length > 2048
  )
    throw new Error("Open a document from the paired artifact library.");
  let value: unknown;
  try {
    value = JSON.parse(params.document);
  } catch {
    throw new Error("Open a document from the paired artifact library.");
  }
  validateArtifactItem(value);
  return {
    owner: params.owner,
    document: value,
    address: params.address as string,
  };
}
function same(
  expected: ArtifactLibraryItem,
  actual: ArtifactLibraryItem,
): boolean {
  return Object.keys(expected).every(
    (key) =>
      expected[key as keyof ArtifactLibraryItem] ===
      actual[key as keyof ArtifactLibraryItem],
  );
}
/** Fixed destination, exact metadata and complete bytes. Nothing is returned before SHA matches. */
export async function readArtifactDocument(
  client: Pick<WakuClient, "request">,
  expected: ArtifactLibraryItem,
  current: () => boolean,
  digest: (bytes: Uint8Array) => Promise<string>,
): Promise<Uint8Array> {
  validateArtifactItem(expected);
  const allowed = () => {
    if (!current())
      throw new Error(
        "Document read scope changed. Reopen it from the current artifact library.",
      );
  };
  allowed();
  const reply = await client.request(
    { type: "readArtifactDocument", query: { expected } },
    undefined,
    undefined,
    { timeoutMs: 30_000 },
  );
  allowed();
  if (
    reply.type !== "artifactDocument" ||
    !shape(reply.document, "bytes,document")
  )
    throw new Error("Invalid artifact document response.");
  validateArtifactItem(reply.document.document);
  if (!same(expected, reply.document.document))
    throw new Error("Document metadata changed. Refresh its artifact library.");
  const encoded = reply.document.bytes;
  if (
    typeof encoded !== "string" ||
    encoded.length !== Math.ceil(expected.bytes / 3) * 4
  )
    throw new Error("Document byte count changed.");
  const bytes = await decodeArtifactBytes(encoded, expected.bytes, current);
  const actual = await digest(bytes);
  allowed();
  if (actual !== expected.sha256)
    throw new Error(
      "Document digest changed. Refresh before reading or exporting.",
    );
  return bytes;
}
/** Complete canonical bytes, bounded allocation and yielding owner-scope checks. */
export async function decodeArtifactBytes(
  encoded: string,
  count: number,
  current: () => boolean,
): Promise<Uint8Array> {
  if (
    typeof encoded !== "string" ||
    !Number.isSafeInteger(count) ||
    count < 1 ||
    count > 16 * 1024 * 1024 ||
    encoded.length !== Math.ceil(count / 3) * 4
  )
    throw new Error("Document byte count changed.");
  const allowed = () => {
    if (!current())
      throw new Error(
        "Document read scope changed. Reopen it from the current artifact library.",
      );
  };
  allowed();
  const bytes = new Uint8Array(count);
  const alphabet =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  let written = 0;
  for (let i = 0; i < encoded.length; i += 4) {
    const a = alphabet.indexOf(encoded[i]!),
      b = alphabet.indexOf(encoded[i + 1]!),
      last = i + 4 === encoded.length;
    const c = encoded[i + 2] === "=" ? -1 : alphabet.indexOf(encoded[i + 2]!),
      d = encoded[i + 3] === "=" ? -1 : alphabet.indexOf(encoded[i + 3]!);
    if (
      a < 0 ||
      b < 0 ||
      (!last && (c < 0 || d < 0)) ||
      (c < 0 && encoded[i + 2] !== "=") ||
      (d < 0 && encoded[i + 3] !== "=") ||
      (c < 0 && d >= 0) ||
      (c < 0 && (b & 15) !== 0) ||
      (d < 0 && c >= 0 && (c & 3) !== 0)
    )
      throw new Error("Invalid document byte encoding.");
    const word = (a << 18) | (b << 12) | (Math.max(c, 0) << 6) | Math.max(d, 0);
    if (written < bytes.length) bytes[written++] = (word >> 16) & 255;
    if (written < bytes.length) bytes[written++] = (word >> 8) & 255;
    if (written < bytes.length) bytes[written++] = word & 255;
    if (i > 0 && i % 65_536 === 0) {
      allowed();
      await new Promise<void>((resolve) => setTimeout(resolve, 0));
      allowed();
    }
    if (last) {
      const padding = c < 0 ? 2 : d < 0 ? 1 : 0;
      if ((encoded.length / 4) * 3 - padding !== bytes.length)
        throw new Error("Invalid document byte encoding.");
    }
  }
  allowed();
  return bytes;
}
export async function artifactText(
  document: ArtifactLibraryItem,
  bytes: Uint8Array,
  current: () => boolean,
): Promise<string | null> {
  if (!current()) throw new Error("Document reading scope changed");
  if (
    !isTextDocumentFormat(document.format) ||
    bytes.length > MAX_DOCUMENT_TEXT_BYTES
  )
    return null;
  return prepareDocumentText(bytes, current);
}
