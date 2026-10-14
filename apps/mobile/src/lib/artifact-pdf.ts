import type { ArtifactLibraryItem, WakuClient } from "@waku/client";
import { decodeArtifactBytes, validateArtifactItem } from "./artifact-library";
export const PDF_ZOOMS = [800, 1200, 1600] as const;
export const PDF_ORIGINAL_LIMIT = 16 * 1024 * 1024;
const PNG_LIMIT = 12 * 1024 * 1024;
const HASH = /^[0-9a-f]{64}$/;
export type PdfPagePreview = {
  page: number;
  pages: number;
  requestedWidth: number;
  width: number;
  height: number;
  uri: string;
};
const object = (v: unknown, fields: string): v is Record<string, unknown> =>
  !!v &&
  typeof v === "object" &&
  !Array.isArray(v) &&
  Object.keys(v).sort().join(",") === fields;
const integer = (v: unknown): v is number =>
  typeof v === "number" && Number.isSafeInteger(v) && v >= 0;
function u32(bytes: Uint8Array, offset: number): number {
  return (
    bytes[offset]! * 16777216 +
    (bytes[offset + 1]! << 16) +
    (bytes[offset + 2]! << 8) +
    bytes[offset + 3]!
  );
}
/** One static native-generated RGBA page. No APNG, profiles, external references or extra chunks. */
export function validatePdfPng(
  bytes: Uint8Array,
  width: number,
  height: number,
): void {
  const signature = [137, 80, 78, 71, 13, 10, 26, 10];
  if (
    bytes.length < 57 ||
    bytes.length > PNG_LIMIT ||
    signature.some((v, i) => bytes[i] !== v)
  )
    throw new Error("Invalid PDF page image.");
  let offset = 8,
    chunks = 0,
    data = false,
    end = false;
  while (offset < bytes.length) {
    if (++chunks > 1024) throw new Error("PDF page has too many image chunks.");
    if (offset + 12 > bytes.length)
      throw new Error("Incomplete PDF page image.");
    const size = u32(bytes, offset),
      tag = String.fromCharCode(...bytes.subarray(offset + 4, offset + 8));
    if (size > bytes.length - offset - 12)
      throw new Error("Incomplete PDF page image.");
    if (offset === 8) {
      if (
        tag !== "IHDR" ||
        size !== 13 ||
        u32(bytes, offset + 8) !== width ||
        u32(bytes, offset + 12) !== height ||
        bytes[offset + 16] !== 8 ||
        bytes[offset + 17] !== 6 ||
        bytes[offset + 18] !== 0 ||
        bytes[offset + 19] !== 0 ||
        bytes[offset + 20] !== 0
      )
        throw new Error("PDF page dimensions or format changed.");
    } else if (tag === "IDAT" && !end && size > 0) data = true;
    else if (
      tag === "IEND" &&
      data &&
      size === 0 &&
      offset + 12 === bytes.length
    )
      end = true;
    else throw new Error("PDF page is not a supported static image.");
    offset += size + 12;
  }
  if (!data || !end) throw new Error("Incomplete PDF page image.");
}
export async function readArtifactPdfPage(
  client: Pick<WakuClient, "request">,
  expected: ArtifactLibraryItem,
  page: number,
  width: number,
  current: () => boolean,
  digest: (bytes: Uint8Array) => Promise<string>,
): Promise<PdfPagePreview> {
  validateArtifactItem(expected);
  if (
    expected.format !== "pdf" ||
    expected.bytes > PDF_ORIGINAL_LIMIT ||
    !integer(page) ||
    page >= 32 ||
    !(PDF_ZOOMS as readonly number[]).includes(width)
  )
    throw new Error(
      "This PDF has no supported page preview. Export its verified original.",
    );
  const allowed = () => {
    if (!current())
      throw new Error("PDF read scope changed. Reopen the current document.");
  };
  allowed();
  const reply = await client.request(
    { type: "renderArtifactPdfPage", query: { expected, page, width } },
    undefined,
    undefined,
    { timeoutMs: 30000 },
  );
  allowed();
  if (
    reply.type !== "artifactPdfPage" ||
    !object(
      reply.page,
      "document,height,page,pages,png,requestedWidth,sha256,width",
    )
  )
    throw new Error("Invalid PDF page response.");
  const result = reply.page;
  validateArtifactItem(result.document);
  if (
    Object.keys(expected).some(
      (k) =>
        expected[k as keyof ArtifactLibraryItem] !==
        result.document[k as keyof ArtifactLibraryItem],
    ) ||
    result.page !== page ||
    result.requestedWidth !== width ||
    !integer(result.pages) ||
    result.pages < 1 ||
    result.pages > 32 ||
    page >= result.pages ||
    !integer(result.width) ||
    result.width < 1 ||
    result.width > width ||
    !integer(result.height) ||
    result.height < 1 ||
    result.height > 2000 ||
    result.width * result.height > 3200000 ||
    typeof result.sha256 !== "string" ||
    !HASH.test(result.sha256) ||
    typeof result.png !== "string"
  )
    throw new Error("PDF response belongs to another document, page or zoom.");
  // Padding determines the exact output length; canonical decoding rechecks all words.
  const encoded = result.png,
    padding = encoded.endsWith("==") ? 2 : encoded.endsWith("=") ? 1 : 0;
  if (
    !encoded.length ||
    encoded.length % 4 !== 0 ||
    encoded.length > Math.ceil(PNG_LIMIT / 3) * 4
  )
    throw new Error("PDF page image exceeds its bound.");
  const count = (encoded.length / 4) * 3 - padding;
  const bytes = await decodeArtifactBytes(encoded, count, current);
  allowed();
  validatePdfPng(bytes, result.width, result.height);
  if ((await digest(bytes)) !== result.sha256)
    throw new Error("PDF page digest changed. Refresh the document.");
  allowed();
  return {
    page,
    pages: result.pages,
    requestedWidth: width,
    width: result.width,
    height: result.height,
    uri: "data:image/png;base64," + encoded,
  };
}
