import { expect, test } from "bun:test";
import { deflateSync } from "node:zlib";
import { prepareArtifactImage } from "./artifact-image";
function chunk(tag: string, data: Uint8Array) {
  const b = Buffer.alloc(data.length + 12);
  b.writeUInt32BE(data.length);
  b.write(tag, 4);
  b.set(data, 8);
  let crc = 0xffffffff;
  for (let i = 4; i < b.length - 4; i++) {
    crc ^= b[i]!;
    for (let bit = 0; bit < 8; bit++)
      crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0);
  }
  b.writeUInt32BE((crc ^ 0xffffffff) >>> 0, b.length - 4);
  return b;
}
function png(width = 1, height = 1, extra: Uint8Array[] = []) {
  const h = Buffer.alloc(13);
  h.writeUInt32BE(width);
  h.writeUInt32BE(height, 4);
  h[8] = 8;
  h[9] = 6;
  return Buffer.concat([
    Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
    chunk("IHDR", h),
    ...extra,
    chunk("IDAT", deflateSync(Buffer.from([0, 1, 2, 3, 255]))),
    chunk("IEND", new Uint8Array()),
  ]);
}
test("verified static PNG preview preserves the exact original bytes", async () => {
  const bytes = png();
  const view = await prepareArtifactImage(bytes, () => true);
  expect(view.width).toBe(1);
  expect(view.height).toBe(1);
  expect(Buffer.from(view.uri.split(",")[1]!, "base64")).toEqual(bytes);
});
test("PNG preview rejects oversized dimensions before native decoding", async () => {
  for (const bytes of [png(0), png(8193), png(4096, 4096)])
    await expect(prepareArtifactImage(bytes, () => true)).rejects.toThrow(
      "dimensions",
    );
});
test("PNG preview rejects animated and unknown critical chunks", async () => {
  for (const tag of ["acTL", "fcTL", "fdAT", "ABCD"])
    await expect(
      prepareArtifactImage(
        png(1, 1, [chunk(tag, new Uint8Array(8))]),
        () => true,
      ),
    ).rejects.toThrow();
});
test("PNG preview refuses checksum damage, trailing bytes and truncation", async () => {
  const bytes = png(),
    bad = Buffer.from(bytes);
  bad[bad.length - 1] ^= 1;
  for (const source of [
    bad,
    bytes.subarray(0, bytes.length - 1),
    Buffer.concat([bytes, Buffer.from([0])]),
  ])
    await expect(prepareArtifactImage(source, () => true)).rejects.toThrow();
});
test("PNG preview rejects unsupported format and allocation sizes", async () => {
  await expect(
    prepareArtifactImage(new Uint8Array(16 * 1024 * 1024 + 1), () => true),
  ).rejects.toThrow();
  await expect(
    prepareArtifactImage(Buffer.from("not an image"), () => true),
  ).rejects.toThrow();
});
test("PNG preview cancels during yielding checksum and encoding work", async () => {
  let calls = 0;
  await expect(prepareArtifactImage(png(), () => ++calls < 3)).rejects.toThrow(
    "scope changed",
  );
  let current = true;
  const pending = prepareArtifactImage(png(), () => current);
  current = false;
  await expect(pending).rejects.toThrow("scope changed");
});
