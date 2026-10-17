import type {
  HtmlResourceManifest,
  HtmlSourceChunk,
  HtmlSourceQuery,
} from "./generated";
import { workflowChunkBytes } from "./gateway-history";
export type HtmlPublication = { id: string; sha256: string };
export type VerifiedHtmlResources = {
  manifest: HtmlResourceManifest;
  sources: Uint8Array[];
};
const PREFIX = "<!--ion-html-sources-v1:";
const HASH = /^[0-9a-f]{64}$/;
const id = (v: unknown): v is string =>
  typeof v === "string" &&
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(v) &&
  v !== "00000000-0000-0000-0000-000000000000";
const shape = (v: unknown, fields: string): v is Record<string, any> =>
  !!v &&
  typeof v === "object" &&
  !Array.isArray(v) &&
  Object.keys(v).sort().join(",") === fields.split(",").sort().join(",");
const integer = (v: unknown): v is number =>
  typeof v === "number" && Number.isSafeInteger(v) && v >= 0;
const hash = (v: unknown): v is string => typeof v === "string" && HASH.test(v);
/** Small canonical manifest encoding; no native TextEncoder/Buffer dependency. */
export function htmlManifestBytes(manifest: HtmlResourceManifest): Uint8Array {
  const text = JSON.stringify([
    1,
    manifest.entryPath,
    manifest.totalBytes,
    manifest.sources.map((s) => [
      s.index,
      s.path,
      s.mediaType,
      s.bytes,
      s.sha256,
    ]),
  ]);
  const bytes: number[] = [];
  for (const char of text) {
    const n = char.codePointAt(0)!;
    if (n < 128) bytes.push(n);
    else if (n < 2048) bytes.push(192 | (n >> 6), 128 | (n & 63));
    else if (n < 65536)
      bytes.push(224 | (n >> 12), 128 | ((n >> 6) & 63), 128 | (n & 63));
    else
      bytes.push(
        240 | (n >> 18),
        128 | ((n >> 12) & 63),
        128 | ((n >> 6) & 63),
        128 | (n & 63),
      );
  }
  return new Uint8Array(bytes);
}
function path(value: unknown): value is string {
  if (
    typeof value !== "string" ||
    !value.length ||
    value.length > 512 ||
    /[\\:?#\x00-\x1f\x7f-\x9f]/.test(value) ||
    /[\uD800-\uDFFF]/u.test(value) ||
    value.split("/").some((s) => !s || s === "." || s === "..")
  )
    return false;
  let size = 0;
  for (const char of value) {
    const n = char.codePointAt(0)!;
    size += n < 128 ? 1 : n < 2048 ? 2 : n < 65536 ? 3 : 4;
  }
  return size <= 512;
}
export function validateHtmlManifest(
  v: unknown,
  expected: HtmlPublication,
): asserts v is HtmlResourceManifest {
  if (
    !shape(
      v,
      "version,artifactId,artifactSha256,entryPath,totalBytes,sha256,sources",
    ) ||
    v.version !== 1 ||
    !id(v.artifactId) ||
    v.artifactId !== expected.id ||
    v.artifactSha256 !== expected.sha256 ||
    !hash(v.artifactSha256) ||
    !hash(v.sha256) ||
    !path(v.entryPath) ||
    !integer(v.totalBytes) ||
    v.totalBytes > 9 * 1024 * 1024 ||
    !Array.isArray(v.sources) ||
    !v.sources.length ||
    v.sources.length > 33
  )
    throw new Error("Invalid HTML resource manifest");
  const paths = new Set<string>();
  let total = 0;
  for (let i = 0; i < v.sources.length; i++) {
    const s = v.sources[i];
    if (
      !shape(s, "index,path,mediaType,bytes,sha256") ||
      s.index !== i ||
      !path(s.path) ||
      paths.has(s.path) ||
      !hash(s.sha256) ||
      !integer(s.bytes) ||
      s.bytes > (i === 0 ? 1024 * 1024 : 2 * 1024 * 1024) ||
      (i === 0
        ? s.path !== v.entryPath || s.mediaType !== "text/html" || !s.bytes
        : ![
            "text/css",
            "image/svg+xml",
            "image/png",
            "image/jpeg",
            "image/gif",
            "image/webp",
            "font/woff",
            "font/woff2",
            "font/ttf",
            "font/otf",
          ].includes(s.mediaType))
    )
      throw new Error("Invalid HTML resource descriptor");
    paths.add(s.path);
    total += s.bytes;
  }
  if (total !== v.totalBytes || total - v.sources[0].bytes > 8 * 1024 * 1024)
    throw new Error("HTML resource total changed");
}
export function validateHtmlSourceQuery(
  v: unknown,
): asserts v is HtmlSourceQuery {
  if (
    !shape(v, "manifestSha256,sourceIndex,sourceSha256,offset,limit") ||
    !hash(v.manifestSha256) ||
    !hash(v.sourceSha256) ||
    !integer(v.sourceIndex) ||
    v.sourceIndex > 32 ||
    !integer(v.offset) ||
    v.offset >= 2 * 1024 * 1024 ||
    v.offset % 16384 !== 0 ||
    v.limit !== 16384
  )
    throw new Error("Invalid HTML resource chunk query");
}
export function validateHtmlChunk(
  v: unknown,
  q: HtmlSourceQuery,
  expected: HtmlPublication,
): asserts v is HtmlSourceChunk {
  validateHtmlSourceQuery(q);
  if (
    !shape(
      v,
      "artifactId,artifactSha256,manifestSha256,sourceIndex,sourceSha256,offset,nextOffset,data",
    ) ||
    v.artifactId !== expected.id ||
    v.artifactSha256 !== expected.sha256 ||
    v.manifestSha256 !== q.manifestSha256 ||
    v.sourceIndex !== q.sourceIndex ||
    v.sourceSha256 !== q.sourceSha256 ||
    v.offset !== q.offset ||
    typeof v.data !== "string"
  )
    throw new Error("HTML resource chunk selection changed");
  const bytes = workflowChunkBytes(v.data),
    end = q.offset + bytes.length;
  if (
    !bytes.length ||
    bytes.length > 16384 ||
    !(v.nextOffset === null || v.nextOffset === end)
  )
    throw new Error("HTML resource chunk cursor changed");
}
function sourceCommitment(bytes: Uint8Array): string | null {
  for (let i = 0; i < PREFIX.length; i++)
    if (bytes[i] !== PREFIX.charCodeAt(i)) return null;
  let result = "";
  for (let i = PREFIX.length; i < PREFIX.length + 64; i++)
    result += String.fromCharCode(bytes[i] ?? 0);
  const end = PREFIX.length + 64;
  if (
    !hash(result) ||
    bytes[end] !== 45 ||
    bytes[end + 1] !== 45 ||
    bytes[end + 2] !== 62 ||
    bytes[end + 3] !== 10
  )
    throw new Error("Invalid HTML resource publication commitment");
  return result;
}
/** No source is exposed before the original, manifest and all resource hashes
 * agree. Continuation checks fence every request, digest and return. */
export async function readHtmlResourceSet(
  expected: HtmlPublication,
  original: Uint8Array,
  reader: {
    manifest(): Promise<unknown>;
    chunk(query: HtmlSourceQuery): Promise<unknown>;
    digest(bytes: Uint8Array): Promise<string>;
    current(): boolean;
  },
): Promise<VerifiedHtmlResources | null> {
  const allowed = () => {
    if (!reader.current()) throw new Error("HTML resource read scope changed");
  };
  allowed();
  if (
    !id(expected.id) ||
    !hash(expected.sha256) ||
    !original.length ||
    original.length > 16 * 1024 * 1024
  )
    throw new Error("Invalid HTML publication");
  const originalHash = await reader.digest(original);
  allowed();
  if (originalHash !== expected.sha256)
    throw new Error("HTML original digest changed");
  const commitment = sourceCommitment(original);
  if (commitment === null) return null;
  const manifest = await reader.manifest();
  allowed();
  validateHtmlManifest(manifest, expected);
  const actual = await reader.digest(htmlManifestBytes(manifest));
  allowed();
  if (actual !== commitment || manifest.sha256 !== commitment)
    throw new Error(
      "HTML resource manifest is not committed by this publication",
    );
  const sources: Uint8Array[] = [];
  for (const source of manifest.sources) {
    allowed();
    const bytes = new Uint8Array(source.bytes);
    let offset = 0;
    while (offset < bytes.length) {
      allowed();
      const requests: Promise<{ offset: number; data: Uint8Array }>[] = [];
      for (let i = 0; i < 8 && offset < bytes.length; i++, offset += 16384) {
        const at = offset;
        requests.push(
          (async () => {
            allowed();
            const query: HtmlSourceQuery = {
              manifestSha256: manifest.sha256,
              sourceIndex: source.index,
              sourceSha256: source.sha256,
              offset: at,
              limit: 16384,
            };
            const result = await reader.chunk(query);
            allowed();
            validateHtmlChunk(result, query, expected);
            const data = workflowChunkBytes(result.data),
              end = at + data.length;
            if (
              data.length !== Math.min(16384, bytes.length - at) ||
              result.nextOffset !== (end < bytes.length ? end : null)
            )
              throw new Error("HTML resource size or final cursor changed");
            return { offset: at, data };
          })(),
        );
      }
      const chunks = await Promise.all(requests);
      allowed();
      for (const chunk of chunks) bytes.set(chunk.data, chunk.offset);
    }
    const actual = await reader.digest(bytes);
    allowed();
    if (actual !== source.sha256)
      throw new Error("HTML resource digest changed");
    sources.push(bytes);
  }
  allowed();
  return { manifest, sources };
}
