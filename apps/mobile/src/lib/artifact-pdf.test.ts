import { describe, expect, test } from "bun:test";
import { createHash, randomBytes } from "node:crypto";
import { deflateSync } from "node:zlib";
import type { ArtifactLibraryItem, WakuClient } from "@waku/client";
import { readArtifactPdfPage, validatePdfPng } from "./artifact-pdf";
const expected: ArtifactLibraryItem = {
  id: "11111111-1111-4111-8111-111111111111",
  name: "page.pdf",
  format: "pdf",
  projectId: "22222222-2222-4222-8222-222222222222",
  taskId: "33333333-3333-4333-8333-333333333333",
  createdAt: 1,
  bytes: 100,
  sha256: "1".repeat(64),
  origin: "agentEdit",
};
const digest = async (b: Uint8Array) =>
  createHash("sha256").update(b).digest("hex");
function chunk(tag: string, data: Buffer): Buffer {
  const body = Buffer.concat([Buffer.from(tag), data]),
    crc = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let i = 0; i < 8; i++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    crc[n] = c >>> 0;
  }
  let c = 0xffffffff;
  for (const b of body) c = crc[(c ^ b) & 255]! ^ (c >>> 8);
  const head = Buffer.alloc(4),
    tail = Buffer.alloc(4);
  head.writeUInt32BE(data.length);
  tail.writeUInt32BE((c ^ 0xffffffff) >>> 0);
  return Buffer.concat([head, body, tail]);
}
function png(width = 2, height = 3, random = false): Buffer {
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;
  const pixels = random
    ? randomBytes((width * 4 + 1) * height)
    : Buffer.alloc((width * 4 + 1) * height, 255);
  for (let i = 0; i < height; i++) pixels[i * (width * 4 + 1)] = 0;
  return Buffer.concat([
    Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
    chunk("IHDR", ihdr),
    chunk("IDAT", deflateSync(pixels)),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}
const original = png(),
  hash = createHash("sha256").update(original).digest("hex");
const result = {
  document: expected,
  page: 0,
  pages: 2,
  requestedWidth: 800,
  width: 2,
  height: 3,
  sha256: hash,
  png: original.toString("base64"),
};
const client = (page: unknown, request: (value: unknown) => void = () => {}) =>
  ({
    request: async (value: unknown) => {
      request(value);
      return { type: "artifactPdfPage", page };
    },
  }) as Pick<WakuClient, "request">;
describe("verified owner PDF pages", () => {
  test("one static page pins original, page and zoom and yields an inert image URI", async () => {
    let sent: unknown;
    const page = await readArtifactPdfPage(
      client(result, (v) => (sent = v)),
      expected,
      0,
      800,
      () => true,
      digest,
    );
    expect(sent).toEqual({
      type: "renderArtifactPdfPage",
      query: { expected, page: 0, width: 800 },
    });
    expect(page).toEqual({
      page: 0,
      pages: 2,
      requestedWidth: 800,
      width: 2,
      height: 3,
      uri: "data:image/png;base64," + result.png,
    });
  });
  test("invalid requests and stale scope cannot contact owner", async () => {
    let requests = 0;
    for (const [doc, index, width] of [
      [{ ...expected, format: "html" }, 0, 800],
      [{ ...expected, bytes: 16 * 1024 * 1024 + 1 }, 0, 800],
      [expected, 32, 800],
      [expected, 0, 500],
      [expected, -1, 800],
    ] as const)
      await expect(
        readArtifactPdfPage(
          client(result, () => requests++),
          doc,
          index,
          width,
          () => true,
          digest,
        ),
      ).rejects.toThrow();
    await expect(
      readArtifactPdfPage(
        client(result, () => requests++),
        expected,
        0,
        800,
        () => false,
        digest,
      ),
    ).rejects.toThrow();
    expect(requests).toBe(0);
  });
  test("foreign metadata/page/zoom and private or malformed response fields fail", async () => {
    for (const invalid of [
      {
        ...result,
        document: {
          ...expected,
          projectId: "44444444-4444-4444-8444-444444444444",
        },
      },
      { ...result, page: 1 },
      { ...result, requestedWidth: 1200 },
      { ...result, pages: 33 },
      { ...result, width: 801 },
      { ...result, height: 2001 },
      { ...result, width: 1600, height: 2001 },
      { ...result, sourcePath: "private" },
      { ...result, png: "https://example.com/page.png" },
      { ...result, sha256: "bad" },
    ])
      await expect(
        readArtifactPdfPage(
          client(invalid),
          expected,
          0,
          800,
          () => true,
          digest,
        ),
      ).rejects.toThrow();
  });
  test("wrong digest, truncated and noncanonical encodings never yield an image", async () => {
    for (const invalid of [
      { ...result, sha256: "0".repeat(64) },
      { ...result, png: result.png.slice(0, -4) },
      { ...result, png: "AAAA=" },
      { ...result, png: "!!!!" },
    ])
      await expect(
        readArtifactPdfPage(
          client(invalid),
          expected,
          0,
          800,
          () => true,
          digest,
        ),
      ).rejects.toThrow();
  });
  test("PNG dimensions, active/unknown chunks, APNG and trailing bytes are refused", () => {
    expect(() => validatePdfPng(original, 3, 3)).toThrow();
    expect(() =>
      validatePdfPng(Buffer.concat([original, Buffer.from("extra")]), 2, 3),
    ).toThrow();
    for (const tag of ["acTL", "fcTL", "fdAT", "iCCP", "IHDR"]) {
      const invalid = Buffer.concat([
        original.subarray(0, 33),
        chunk(tag, Buffer.alloc(8)),
        original.subarray(33),
      ]);
      expect(() => validatePdfPng(invalid, 2, 3)).toThrow();
    }
    const tooMany = Buffer.concat([
      original.subarray(0, 33),
      ...Array.from({ length: 1025 }, () => chunk("IDAT", Buffer.from([0]))),
      chunk("IEND", Buffer.alloc(0)),
    ]);
    expect(() => validatePdfPng(tooMany, 2, 3)).toThrow();
    const excessive = Buffer.alloc(12 * 1024 * 1024 + 1);
    expect(() => validatePdfPng(excessive, 2, 3)).toThrow();
    expect(() =>
      validatePdfPng(original.subarray(0, original.length - 1), 2, 3),
    ).toThrow();
  });
  test("scope changes after owner response or hash suppress derived content", async () => {
    let active = true;
    await expect(
      readArtifactPdfPage(
        client(result, () => (active = false)),
        expected,
        0,
        800,
        () => active,
        digest,
      ),
    ).rejects.toThrow();
    active = true;
    await expect(
      readArtifactPdfPage(
        client(result),
        expected,
        0,
        800,
        () => active,
        async (bytes) => {
          active = false;
          return digest(bytes);
        },
      ),
    ).rejects.toThrow();
  });
  test("large canonical PNG decode yields and cancels before hash/display", async () => {
    const image = png(200, 300, true),
      page = {
        ...result,
        width: 200,
        height: 300,
        png: image.toString("base64"),
        sha256: await digest(image),
      };
    let active = true,
      hashed = false;
    setTimeout(() => (active = false), 0);
    await expect(
      readArtifactPdfPage(
        client(page),
        expected,
        0,
        800,
        () => active,
        async (bytes) => {
          hashed = true;
          return digest(bytes);
        },
      ),
    ).rejects.toThrow();
    expect(hashed).toBe(false);
  });
});
