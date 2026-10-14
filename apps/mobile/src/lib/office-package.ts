import { Inflate } from "fflate/browser";
import { SaxesParser, type SaxesTagNS } from "saxes";
import { workflowDocumentText } from "./gateway-workflow-document";

export type OfficeTag = SaxesTagNS;
export type OfficeScope = () => boolean;
const MAX_PART = 2 * 1024 * 1024;
const MAX_XML = 8 * 1024 * 1024;
const crcTable = new Uint32Array(256);
for (let i = 0; i < 256; i++) {
  let c = i;
  for (let bit = 0; bit < 8; bit++) c = (c >>> 1) ^ (c & 1 ? 0xedb88320 : 0);
  crcTable[i] = c;
}
export function officeAllowed(current: OfficeScope) {
  if (!current())
    throw new Error("Document access changed. Reopen the document.");
}
export async function officeYield(current: OfficeScope) {
  officeAllowed(current);
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
  officeAllowed(current);
}
type Entry = {
  name: string;
  flags: number;
  method: number;
  crc: number;
  size: number;
  compressed: number;
  start: number;
  end: number;
};
/** Index a bounded OPC ZIP without inflating unrelated media. No filesystem/network. */
export async function openOfficePackage(
  bytes: Uint8Array,
  current: OfficeScope,
) {
  officeAllowed(current);
  if (bytes.length < 22 || bytes.length > 16 * 1024 * 1024)
    throw new Error("Office preview accepts originals up to 16 MiB.");
  const data = new DataView(bytes.buffer, bytes.byteOffset, bytes.length);
  const u16 = (p: number) => data.getUint16(p, true);
  const u32 = (p: number) => data.getUint32(p, true);
  let end = -1;
  for (let p = bytes.length - 22; p >= Math.max(0, bytes.length - 65557); p--) {
    if (u32(p) === 0x06054b50 && p + 22 + u16(p + 20) === bytes.length) {
      end = p;
      break;
    }
  }
  if (end < 0 || u16(end + 4) || u16(end + 6) || u16(end + 8) !== u16(end + 10))
    throw new Error("Unsupported or damaged Office ZIP directory.");
  const count = u16(end + 10),
    length = u32(end + 12),
    directory = u32(end + 16);
  if (!count || count > 2048 || directory + length !== end)
    throw new Error("Office ZIP directory exceeds preview limits.");
  const entries = new Map<string, Entry>(),
    folded = new Set<string>(),
    ranges: [number, number][] = [];
  let p = directory,
    total = 0;
  for (let i = 0; i < count; i++) {
    if (i % 32 === 0) await officeYield(current);
    if (p + 46 > end || u32(p) !== 0x02014b50)
      throw new Error("Damaged Office ZIP entry.");
    const flags = u16(p + 8),
      method = u16(p + 10),
      crc = u32(p + 16),
      compressed = u32(p + 20),
      size = u32(p + 24),
      n = u16(p + 28),
      extra = u16(p + 30),
      comment = u16(p + 32),
      local = u32(p + 42);
    if (
      !n ||
      n > 1024 ||
      p + 46 + n + extra + comment > end ||
      u16(p + 34) ||
      flags & ~0x080e ||
      ![0, 8].includes(method) ||
      (method === 0 && flags & 6) ||
      size === 0xffffffff ||
      compressed === 0xffffffff ||
      local === 0xffffffff
    )
      throw new Error("Unsupported Office ZIP entry.");
    const nameBytes = bytes.subarray(p + 46, p + 46 + n),
      name = workflowDocumentText(nameBytes);
    const parts = name.replace(/\/$/, "").split("/");
    if (
      name.startsWith("/") ||
      /[\\\x00-\x1f\x7f:#?%]/.test(name) ||
      parts.some((part) => !part || part === "." || part === "..") ||
      folded.has(name.toLowerCase()) ||
      ((u32(p + 38) >>> 16) & 0xf000) === 0xa000
    )
      throw new Error("Ambiguous or unsafe Office part name.");
    folded.add(name.toLowerCase());
    if (
      local + 30 > directory ||
      u32(local) !== 0x04034b50 ||
      u16(local + 6) !== flags ||
      u16(local + 8) !== method ||
      u16(local + 26) !== n
    )
      throw new Error("Office ZIP local entry disagrees with directory.");
    const start = local + 30 + n + u16(local + 28),
      payloadEnd = start + compressed;
    if (
      payloadEnd > directory ||
      nameBytes.some((value, index) => value !== bytes[local + 30 + index])
    )
      throw new Error("Damaged Office ZIP payload.");
    let rangeEnd = payloadEnd;
    if (flags & 8) {
      const descriptor =
        payloadEnd +
        (payloadEnd + 4 <= directory && u32(payloadEnd) === 0x08074b50 ? 4 : 0);
      if (
        descriptor + 12 > directory ||
        u32(descriptor) !== crc ||
        u32(descriptor + 4) !== compressed ||
        u32(descriptor + 8) !== size
      )
        throw new Error("Office ZIP descriptor disagrees with directory.");
      rangeEnd = descriptor + 12;
    } else if (
      u32(local + 14) !== crc ||
      u32(local + 18) !== compressed ||
      u32(local + 22) !== size
    )
      throw new Error("Office ZIP local sizes or checksum changed.");
    total += size;
    if (total > 32 * 1024 * 1024 || (method === 0 && size !== compressed))
      throw new Error("Expanded Office package exceeds preview limits.");
    ranges.push([local, rangeEnd]);
    entries.set(name, {
      name,
      flags,
      method,
      crc,
      compressed,
      size,
      start,
      end: payloadEnd,
    });
    p += 46 + n + extra + comment;
  }
  if (p !== end) throw new Error("Office ZIP directory has trailing entries.");
  ranges.sort((a, b) => a[0] - b[0]);
  for (let i = 1; i < ranges.length; i++)
    if (ranges[i]![0] < ranges[i - 1]![1])
      throw new Error("Office ZIP entries overlap.");
  let readBytes = 0;
  const seen = new Set<string>();
  return {
    has: (name: string) => entries.has(name),
    async read(name: string): Promise<string> {
      officeAllowed(current);
      const entry = entries.get(name);
      if (!entry || name.endsWith("/"))
        throw new Error(`Office part is missing: ${name}`);
      if (!seen.has(name)) {
        readBytes += entry.size;
        seen.add(name);
      }
      if (entry.size > MAX_PART || readBytes > MAX_XML)
        throw new Error(
          "Office XML exceeds reading limits. Export the original.",
        );
      const chunks: Uint8Array[] = [];
      let size = 0,
        crc = 0xffffffff;
      const receive = (chunk: Uint8Array) => {
        officeAllowed(current);
        size += chunk.length;
        if (size > entry.size || size > MAX_PART)
          throw new Error("Office part expands beyond its declared size.");
        for (const byte of chunk)
          crc = (crc >>> 8) ^ crcTable[(crc ^ byte) & 255]!;
        if (chunk.length) chunks.push(chunk);
      };
      if (entry.method === 8) {
        const inflate = new Inflate(receive);
        // Small compressed chunks bound expansion/CPU before each callback/yield.
        for (let offset = entry.start; offset < entry.end; offset += 256) {
          inflate.push(
            bytes.subarray(offset, Math.min(offset + 256, entry.end)),
            offset + 256 >= entry.end,
          );
          await officeYield(current);
        }
        if (entry.start === entry.end)
          throw new Error("Empty compressed Office stream.");
      } else {
        for (let offset = entry.start; offset < entry.end; offset += 8192) {
          receive(bytes.subarray(offset, Math.min(offset + 8192, entry.end)));
          await officeYield(current);
        }
      }
      if (size !== entry.size || (crc ^ 0xffffffff) >>> 0 !== entry.crc)
        throw new Error("Office part size/checksum changed.");
      const raw = new Uint8Array(size);
      let offset = 0;
      for (const chunk of chunks) {
        raw.set(chunk, offset);
        offset += chunk.length;
      }
      const text: string[] = [];
      for (let offset = 0; offset < raw.length;) {
        let end = Math.min(offset + 8192, raw.length);
        while (end < raw.length && (raw[end]! & 0xc0) === 0x80) end--;
        text.push(workflowDocumentText(raw.subarray(offset, end)));
        offset = end;
        await officeYield(current);
      }
      return text.join("");
    },
  };
}

export function officeAttribute(
  tag: OfficeTag,
  name: string,
  uri = "",
): string | undefined {
  return Object.values(tag.attributes).find(
    (attribute) => attribute.local === name && attribute.uri === uri,
  )?.value;
}
/** Strict namespace-aware streaming XML. DTD/entities never reach a renderer. */
export async function readOfficeXml(
  text: string,
  current: OfficeScope,
  handlers: {
    open?: (tag: OfficeTag, ancestors: OfficeTag[]) => void;
    close?: (tag: OfficeTag, ancestors: OfficeTag[]) => void;
    text?: (text: string, ancestors: OfficeTag[]) => void;
  },
) {
  officeAllowed(current);
  const parser = new SaxesParser({ xmlns: true }),
    stack: OfficeTag[] = [];
  let events = 0;
  const event = () => {
    officeAllowed(current);
    if (++events > 100_000)
      throw new Error("Office XML has too many elements.");
  };
  parser.on("doctype", () => {
    throw new Error("Office XML document types are not supported.");
  });
  parser.on("error", (error) => {
    throw new Error(`Invalid Office XML: ${error.message}`);
  });
  parser.on("opentag", (tag) => {
    event();
    if (stack.length >= 128 || Object.keys(tag.attributes).length > 64)
      throw new Error("Office XML structure exceeds reading limits.");
    handlers.open?.(tag, stack);
    stack.push(tag);
  });
  parser.on("closetag", (tag) => {
    event();
    stack.pop();
    handlers.close?.(tag, stack);
  });
  const characters = (value: string) => {
    event();
    handlers.text?.(value, stack);
  };
  parser.on("text", characters);
  parser.on("cdata", characters);
  for (let offset = 0; offset < text.length; offset += 8192) {
    parser.write(text.slice(offset, offset + 8192));
    await officeYield(current);
  }
  parser.close();
  officeAllowed(current);
}

/** Resolve a package relationship within its archive; external URIs are rejected. */
export function officePart(base: string, target: string): string {
  if (
    !target ||
    /[\\\x00-\x20\x7f:#?]/.test(target) ||
    /%(?:2f|5c)/i.test(target)
  )
    throw new Error("Unsupported Office part relationship.");
  let decoded: string;
  try {
    decoded = decodeURIComponent(target);
  } catch {
    throw new Error("Invalid Office part URI.");
  }
  if (/[\\\x00-\x1f\x7f:#?%]/.test(decoded))
    throw new Error("Unsupported Office part URI.");
  const parts = target.startsWith("/") ? [] : base.split("/").slice(0, -1);
  for (const part of decoded.replace(/^\//, "").split("/")) {
    if (part === ".") continue;
    if (part === "..") {
      if (!parts.length)
        throw new Error("Office relationship escapes package.");
      parts.pop();
    } else if (!part) throw new Error("Empty Office part path.");
    else parts.push(part);
  }
  return parts.join("/");
}
