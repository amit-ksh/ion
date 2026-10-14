/** Prepare only digest-verified static PNGs, outside render, with scope fences. */
export type ArtifactImage = { uri: string; width: number; height: number };
const LIMIT = 16 * 1024 * 1024;
const SIGNATURE = [137, 80, 78, 71, 13, 10, 26, 10];
const CRC = Uint32Array.from({ length: 256 }, (_, value) => {
  let crc = value;
  for (let bit = 0; bit < 8; bit++)
    crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0);
  return crc >>> 0;
});
const u32 = (b: Uint8Array, i: number) =>
  b[i]! * 16777216 + b[i + 1]! * 65536 + b[i + 2]! * 256 + b[i + 3]!;
export async function prepareArtifactImage(
  bytes: Uint8Array,
  current: () => boolean,
): Promise<ArtifactImage> {
  const allowed = () => {
    if (!current()) throw new Error("Document image scope changed.");
  };
  const pause = async () => {
    allowed();
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
    allowed();
  };
  allowed();
  if (
    bytes.length < 57 ||
    bytes.length > LIMIT ||
    SIGNATURE.some((value, i) => bytes[i] !== value)
  )
    throw new Error(
      "This document has no supported PNG preview. Export the original.",
    );
  let offset = 8,
    chunks = 0,
    width = 0,
    height = 0,
    color = -1,
    palette = false,
    transparency = false,
    data = false,
    endedData = false,
    end = false;
  while (offset < bytes.length) {
    if (++chunks > 4096 || offset + 12 > bytes.length)
      throw new Error("PNG structure exceeds the preview limit.");
    const size = u32(bytes, offset),
      tag = String.fromCharCode(...bytes.subarray(offset + 4, offset + 8));
    if (
      size > bytes.length - offset - 12 ||
      !/^[A-Za-z]{4}$/.test(tag) ||
      tag[2] !== tag[2]!.toUpperCase()
    )
      throw new Error("Invalid PNG chunk.");
    const next = offset + 12 + size;
    if (offset === 8) {
      width = u32(bytes, offset + 8);
      height = u32(bytes, offset + 12);
      color = bytes[offset + 17]!;
      const depths: Record<number, number[]> = {
        0: [1, 2, 4, 8, 16],
        2: [8, 16],
        3: [1, 2, 4, 8],
        4: [8, 16],
        6: [8, 16],
      };
      if (
        tag !== "IHDR" ||
        size !== 13 ||
        width < 1 ||
        height < 1 ||
        width > 8192 ||
        height > 8192 ||
        width * height > 8 * 1024 * 1024 ||
        !depths[color]?.includes(bytes[offset + 16]!) ||
        bytes[offset + 18] !== 0 ||
        bytes[offset + 19] !== 0 ||
        bytes[offset + 20]! > 1
      )
        throw new Error(
          "PNG dimensions or color format exceed the preview limit.",
        );
    } else if (
      tag === "IHDR" ||
      tag === "acTL" ||
      tag === "fcTL" ||
      tag === "fdAT"
    )
      throw new Error("Only a single static PNG is supported.");
    else if (tag === "PLTE") {
      if (
        palette ||
        data ||
        size < 3 ||
        size > 768 ||
        size % 3 ||
        color === 0 ||
        color === 4
      )
        throw new Error("Invalid PNG palette.");
      palette = true;
    } else if (tag === "tRNS") {
      if (
        transparency ||
        data ||
        color === 4 ||
        color === 6 ||
        (color === 3 && (!palette || size > 256)) ||
        (color === 0 && size !== 2) ||
        (color === 2 && size !== 6)
      )
        throw new Error("Invalid PNG transparency.");
      transparency = true;
    } else if (tag === "IDAT") {
      if (endedData || (color === 3 && !palette))
        throw new Error("Invalid PNG image data order.");
      data = true;
    } else if (tag === "IEND") {
      if (!data || size !== 0 || next !== bytes.length)
        throw new Error("Incomplete PNG image.");
      end = true;
    } else {
      if (tag[0] === tag[0]!.toUpperCase())
        throw new Error("Unsupported critical PNG chunk.");
      if (data) endedData = true;
    }
    let crc = 0xffffffff;
    for (let i = offset + 4; i < next - 4; i++) {
      crc = CRC[(crc ^ bytes[i]!) & 255]! ^ (crc >>> 8);
      if (i % 32768 === 0) await pause();
    }
    if ((crc ^ 0xffffffff) >>> 0 !== u32(bytes, next - 4))
      throw new Error("PNG chunk checksum changed.");
    offset = next;
    await pause();
  }
  if (!end) throw new Error("Incomplete PNG image.");
  const alphabet =
      "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/",
    parts: string[] = [];
  for (let start = 0; start < bytes.length; start += 12288) {
    let part = "";
    for (let i = start; i < Math.min(bytes.length, start + 12288); i += 3) {
      const a = bytes[i]!,
        b = bytes[i + 1],
        c = bytes[i + 2];
      part +=
        alphabet[a >>> 2]! +
        alphabet[((a & 3) << 4) | ((b ?? 0) >>> 4)]! +
        (b === undefined
          ? "="
          : alphabet[((b & 15) << 2) | ((c ?? 0) >>> 6)]!) +
        (c === undefined ? "=" : alphabet[c & 63]!);
    }
    parts.push(part);
    await pause();
  }
  allowed();
  return { uri: "data:image/png;base64," + parts.join(""), width, height };
}
