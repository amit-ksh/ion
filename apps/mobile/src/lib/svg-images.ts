import { workflowChunkBytes } from "@waku/client";
import { SaxesParser } from "saxes";
import { workflowDocumentText } from "./gateway-workflow-document";

export const RASTER_IMAGE =
  /^data:image\/(?:png|jpeg|gif|webp);base64,[a-z0-9+/=\s]+$/i;
export const SVG_FRAGMENT = /^#[^\s<>"'\\\x00-\x1f\x7f]+$/u;
const SVG_IMAGE =
  /^data:image\/svg\+xml;base64,([a-z0-9+/]+={0,2})(?:#[^\s<>"'\\#\x00-\x1f\x7f]+)?$/i;
const NS = "http://www.w3.org/2000/svg";
const XLINK = "http://www.w3.org/1999/xlink";
const XML = "http://www.w3.org/XML/1998/namespace";
const DENY = new Set([
  "script",
  "foreignobject",
  "animate",
  "animatemotion",
  "animatetransform",
  "set",
  "discard",
  "handler",
]);

/** SVG is accepted only in image contexts, never as an iframe/object/link/use.
 * Strict, yielded XML validation bounds nested embedded drawings and rejects
 * active/foreign content. Browser image mode also keeps CSS/links inert and
 * external resources unavailable; document CSP/native navigation stays separate. */
export function svgImageValidator(current: () => boolean) {
  const cache = new Map<string, { valid: boolean; depth: number }>();
  let total = 0,
    images = 0,
    elements = 0;
  const allowed = () => {
    if (!current()) throw new Error("Document rendering scope changed");
  };
  const pause = async () => {
    allowed();
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
    allowed();
  };
  const validate = async (value: string, nested = 0): Promise<boolean> => {
    allowed();
    const match = SVG_IMAGE.exec(value);
    if (!match || value.length > 1024 * 1024 || nested > 8) return false;
    // Fragments choose a view of the same bytes; do not decode that file again.
    const key = match[1]!,
      cached = cache.get(key);
    if (cached) return cached.valid && nested + cached.depth <= 8;
    if (++images > 32) return false;
    // Mark in progress invalid, so duplicate/cyclic nested payloads cannot recurse.
    cache.set(key, { valid: false, depth: 0 });
    try {
      const base64 = match[1]!,
        raw = new Uint8Array(Math.floor(base64.length / 4) * 3);
      let size = 0;
      for (let offset = 0; offset < base64.length; offset += 8192) {
        const bytes = workflowChunkBytes(base64.slice(offset, offset + 8192));
        total += bytes.length;
        if (total > 1024 * 1024) return false;
        raw.set(bytes, size);
        size += bytes.length;
        await pause();
      }
      const parts: string[] = [];
      for (let offset = 0; offset < size;) {
        let end = Math.min(offset + 8192, size);
        while (end < size && (raw[end]! & 0xc0) === 0x80) end--;
        parts.push(workflowDocumentText(raw.subarray(offset, end)));
        offset = end;
        await pause();
      }
      const source = parts.join(""),
        children: string[] = [];
      let depth = 0,
        root = false;
      const parser = new SaxesParser({ xmlns: true });
      const reject = () => {
        throw new Error("Unsupported SVG image content");
      };
      parser.on("error", reject);
      parser.on("doctype", reject);
      parser.on("processinginstruction", reject);
      parser.on("opentag", (tag) => {
        if (
          ++elements > 50_000 ||
          ++depth > 128 ||
          Object.keys(tag.attributes).length > 64 ||
          tag.uri !== NS ||
          DENY.has(tag.local.toLowerCase())
        )
          reject();
        if (!root) {
          if (tag.local !== "svg") reject();
          root = true;
        }
        for (const attr of Object.values(tag.attributes)) {
          if (attr.uri === "http://www.w3.org/2000/xmlns/") continue;
          if (
            attr.local.toLowerCase().startsWith("on") ||
            (attr.uri && attr.uri !== XLINK && attr.uri !== XML) ||
            (attr.uri === XML && !["lang", "space"].includes(attr.local))
          )
            reject();
          if (attr.local === "href") {
            if (SVG_FRAGMENT.test(attr.value)) continue;
            if (!["image", "feImage"].includes(tag.local)) reject();
            if (RASTER_IMAGE.test(attr.value)) continue;
            if (!SVG_IMAGE.test(attr.value)) reject();
            children.push(attr.value);
          }
        }
      });
      parser.on("closetag", () => {
        depth--;
      });
      for (let offset = 0; offset < source.length; offset += 8192) {
        parser.write(source.slice(offset, offset + 8192));
        await pause();
      }
      parser.close();
      if (!root || depth !== 0) return false;
      let height = 0;
      for (const child of children) {
        if (!(await validate(child, nested + 1))) return false;
        height = Math.max(
          height,
          1 + cache.get(SVG_IMAGE.exec(child)![1]!)!.depth,
        );
      }
      cache.set(key, { valid: true, depth: height });
      allowed();
      return true;
    } catch {
      allowed();
      return false;
    }
  };
  return validate;
}
