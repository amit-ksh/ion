import { validateHtmlManifest, type VerifiedHtmlResources } from "@waku/client";
import { SaxesParser } from "saxes";
import { documentPreparation, prepareDocumentText } from "./document-text";
import { prepareRenderedHtml } from "./html-rendering";
import { svgImageValidator, SVG_FRAGMENT, RASTER_IMAGE } from "./svg-images";

export type HtmlResourceDocument = {
  entryPath: string;
  files: { path: string; mediaType: string; data: string }[];
};
export const RESOURCE_CSP =
  "default-src 'none'; script-src 'none'; style-src 'self' 'unsafe-inline' data:; img-src 'self' data:; font-src 'self' data:; connect-src 'none'; frame-src 'none'; child-src 'none'; worker-src 'none'; object-src 'none'; media-src 'none'; base-uri 'none'; form-action 'none'";

const ORIGIN = "https://ion-document.invalid";
const images = (mime: string) => mime.startsWith("image/");
/** Resolve authored URLs in their original document, without making host paths
 * or unpinned URLs part of the renderer's authority. CSS remains native CSS:
 * the isolated host and response CSP restrict its requests to this file set. */
export function resourceReferences(set: VerifiedHtmlResources, from: string) {
  const paths = new Map(set.manifest.sources.map((s) => [s.path, s.mediaType]));
  return (
    value: string,
    accept: (mime: string, fragment: string) => boolean,
  ) => {
    if (
      !value ||
      /[\\\x00-\x1f\x7f]/.test(value) ||
      value !== value.trim() ||
      /^[a-z][a-z0-9+.-]*:/i.test(value) ||
      value.startsWith("/") ||
      /%(?:2f|5c|00)/i.test(value)
    )
      return false;
    try {
      const prefix = "/publication/";
      const base =
        ORIGIN + prefix + from.split("/").map(encodeURIComponent).join("/");
      const url = new URL(value, base);
      if (
        url.origin !== ORIGIN ||
        url.search ||
        !url.pathname.startsWith(prefix)
      )
        return false;
      const path = url.pathname
        .slice(prefix.length)
        .split("/")
        .map(decodeURIComponent)
        .join("/");
      const mime = paths.get(path);
      return !!mime && accept(mime, url.hash);
    } catch {
      return false;
    }
  };
}
export async function resourceBase64(
  bytes: Uint8Array,
  current: () => boolean,
) {
  const alphabet =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  const parts: string[] = [];
  const pause = documentPreparation(current, "HTML resource preparation");
  for (let start = 0; start < bytes.length; start += 12_288) {
    const end = Math.min(start + 12_288, bytes.length);
    let part = "";
    for (let i = start; i < end; i += 3) {
      const a = bytes[i]!,
        b = bytes[i + 1],
        c = bytes[i + 2];
      part +=
        alphabet[a >> 2]! +
        alphabet[((a & 3) << 4) | ((b ?? 0) >> 4)]! +
        (b === undefined ? "=" : alphabet[((b & 15) << 2) | ((c ?? 0) >> 6)]!) +
        (c === undefined ? "=" : alphabet[c & 63]!);
    }
    parts.push(part);
    await pause();
  }
  if (!current()) throw new Error("HTML resource preparation scope changed");
  return parts.join("");
}
async function utf8(text: string, current: () => boolean) {
  const parts: Uint8Array[] = [];
  let size = 0;
  const pause = documentPreparation(current, "HTML resource preparation");
  for (let i = 0; i < text.length;) {
    const values: number[] = [],
      end = Math.min(i + 8192, text.length);
    while (i < end) {
      const n = text.codePointAt(i)!;
      i += n > 65535 ? 2 : 1;
      if (n < 128) values.push(n);
      else if (n < 2048) values.push(192 | (n >> 6), 128 | (n & 63));
      else if (n < 65536)
        values.push(224 | (n >> 12), 128 | ((n >> 6) & 63), 128 | (n & 63));
      else
        values.push(
          240 | (n >> 18),
          128 | ((n >> 12) & 63),
          128 | ((n >> 6) & 63),
          128 | (n & 63),
        );
    }
    const part = new Uint8Array(values);
    parts.push(part);
    size += part.length;
    await pause();
  }
  const result = new Uint8Array(size);
  let offset = 0;
  for (const part of parts) {
    result.set(part, offset);
    offset += part.length;
    await pause();
  }
  return result;
}
async function validateSvg(
  source: string,
  set: VerifiedHtmlResources,
  path: string,
  current: () => boolean,
) {
  const reference = resourceReferences(set, path),
    embedded: string[] = [];
  const parser = new SaxesParser({ xmlns: true });
  const reject = () => {
    throw new Error("Unsupported active content in retained SVG");
  };
  let depth = 0,
    nodes = 0,
    root = false;
  parser.on("error", reject);
  parser.on("doctype", reject);
  parser.on("processinginstruction", reject);
  parser.on("opentag", (tag) => {
    if (
      ++nodes > 50_000 ||
      ++depth > 128 ||
      tag.uri !== "http://www.w3.org/2000/svg" ||
      [
        "script",
        "foreignobject",
        "animate",
        "animatemotion",
        "animatetransform",
        "set",
        "discard",
        "handler",
      ].includes(tag.local.toLowerCase())
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
        (attr.uri &&
          ![
            "http://www.w3.org/1999/xlink",
            "http://www.w3.org/XML/1998/namespace",
          ].includes(attr.uri)) ||
        (attr.uri === "http://www.w3.org/XML/1998/namespace" &&
          !["lang", "space"].includes(attr.local))
      )
        reject();
      if (attr.local === "href") {
        if (SVG_FRAGMENT.test(attr.value)) continue;
        const image = ["image", "feImage"].includes(tag.local);
        if (image && RASTER_IMAGE.test(attr.value)) continue;
        if (image && /^data:image\/svg\+xml;base64,/i.test(attr.value)) {
          embedded.push(attr.value);
          continue;
        }
        if (
          !reference(attr.value, (mime, fragment) =>
            image ? images(mime) : mime === "image/svg+xml" && !!fragment,
          )
        )
          reject();
      }
    }
  });
  parser.on("closetag", () => {
    depth--;
  });
  const pause = documentPreparation(current, "SVG resource preparation");
  for (let i = 0; i < source.length; i += 8192) {
    parser.write(source.slice(i, i + 8192));
    await pause();
  }
  parser.close();
  if (!root || depth) reject();
  const validate = svgImageValidator(current);
  for (const value of embedded) if (!(await validate(value))) reject();
}
/** Called only after the publication commitment and every source digest passed.
 * Preserve original CSS/SVG identities and names; sanitize the entry independently.
 * No native file path, gateway token, script or application message bridge exists
 * in this transport object. It is disposable memory, never a disk cache. */
export async function prepareHtmlResourceDocument(
  set: VerifiedHtmlResources,
  current: () => boolean,
): Promise<HtmlResourceDocument> {
  validateHtmlManifest(set.manifest, {
    id: set.manifest.artifactId,
    sha256: set.manifest.artifactSha256,
  });
  if (
    set.sources.length !== set.manifest.sources.length ||
    set.sources.some((b, i) => b.length !== set.manifest.sources[i]!.bytes)
  )
    throw new Error("HTML resource bytes changed");
  if (!current()) throw new Error("HTML resource preparation scope changed");
  const entry = set.manifest.entryPath,
    reference = resourceReferences(set, entry);
  const source = await prepareDocumentText(set.sources[0]!, current);
  const rendered = await prepareRenderedHtml(source, current, {
    csp: RESOURCE_CSP,
    reference: (tag, ns, name, value) => {
      if (tag === "link" && name === "href")
        return reference(value, (mime) => mime === "text/css");
      if (tag === "a" && name === "href") return SVG_FRAGMENT.test(value);
      if (name === "srcset" && ["img", "source"].includes(tag)) {
        return value.split(",").every((item) => {
          const match =
            /^(\S+)(?:\s+(?:[1-9][0-9]*w|(?:[0-9]+(?:\.[0-9]+)?|\.[0-9]+)x))?$/.exec(
              item.trim(),
            );
          return !!match && reference(match[1]!, images);
        });
      }
      if (
        (tag === "img" && name === "src") ||
        (ns === "http://www.w3.org/2000/svg" &&
          ["image", "feimage"].includes(tag) &&
          name === "href")
      )
        return reference(value, images);
      return (
        ns === "http://www.w3.org/2000/svg" &&
        name === "href" &&
        reference(
          value,
          (mime, fragment) => mime === "image/svg+xml" && !!fragment,
        )
      );
    },
  });
  const files: HtmlResourceDocument["files"] = [];
  for (const descriptor of set.manifest.sources) {
    let bytes = set.sources[descriptor.index]!;
    if (!descriptor.index) bytes = await utf8(rendered.html, current);
    if (descriptor.mediaType === "image/svg+xml")
      await validateSvg(
        await prepareDocumentText(bytes, current),
        set,
        descriptor.path,
        current,
      );
    files.push({
      path: descriptor.path,
      mediaType: descriptor.mediaType,
      data: await resourceBase64(bytes, current),
    });
  }
  if (!current()) throw new Error("HTML resource preparation scope changed");
  return { entryPath: entry, files };
}
