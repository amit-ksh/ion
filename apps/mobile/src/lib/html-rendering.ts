import { documentHtmlParser, HTML_PARSE_CHUNK } from "./html-parser";
import {
  html as constants,
  defaultTreeAdapter as tree,
  type DefaultTreeAdapterMap,
} from "parse5";

import { SVG_FRAGMENT, svgImageValidator } from "./svg-images";

import { MAX_DOCUMENT_TEXT_BYTES, documentPreparation } from "./document-text";

export const MAX_RENDERED_HTML_BYTES = MAX_DOCUMENT_TEXT_BYTES;
const CHUNK = HTML_PARSE_CHUNK;
export const MAX_PREPARED_HTML_BYTES = 32 * 1024 * 1024;
const CSP =
  "default-src 'none'; script-src 'none'; style-src 'unsafe-inline' data:; img-src data:; font-src data:; connect-src 'none'; frame-src 'none'; object-src 'none'; media-src 'none'; base-uri 'none'; form-action 'none'";
const HTML_NS = constants.NS.HTML;
const SVG_NS = constants.NS.SVG;
const MATH_NS = constants.NS.MATHML;
// Presentation MathML stays in its native namespace. No script-based equation
// renderer is required. Unknown/legacy external-glyph elements stay excluded.
const MATH_ELEMENTS = new Set([
  "math",
  "merror",
  "mfrac",
  "mi",
  "mmultiscripts",
  "mn",
  "mo",
  "mover",
  "mpadded",
  "mphantom",
  "mprescripts",
  "mroot",
  "mrow",
  "ms",
  "mspace",
  "msqrt",
  "mstyle",
  "msub",
  "msubsup",
  "msup",
  "mtable",
  "mtd",
  "mtext",
  "mtr",
  "munder",
  "munderover",
  "none",
  "semantics",
  "maction",
  "annotation",
  "annotation-xml",
]);
const MATH_TEXT_ENCODINGS = new Set([
  "text/plain",
  "application/x-tex",
  "application/x-latex",
]);
const MATH_XML_ENCODINGS = new Set([
  "text/html",
  "application/xhtml+xml",
  "application/mathml+xml",
  "application/mathml-presentation+xml",
]);
function keepMathElement(node: DefaultTreeAdapterMap["element"]): boolean {
  if (!MATH_ELEMENTS.has(node.tagName)) return false;
  if (!["annotation", "annotation-xml"].includes(node.tagName)) return true;
  // External annotations are never retrieved, even when their encoding is known.
  if (node.attrs.some((attr) => attr.name.toLowerCase() === "href"))
    return false;
  const encoding = node.attrs
    .find((attr) => attr.name === "encoding")
    ?.value.trim()
    .toLowerCase();
  if (!encoding) return false;
  if (node.tagName === "annotation")
    return (
      MATH_TEXT_ENCODINGS.has(encoding) &&
      !node.childNodes.some(tree.isElementNode)
    );
  // HTML integration points are parsed by HTML5 and pass through the same
  // namespace, active-content and attribute checks as the surrounding page.
  return MATH_XML_ENCODINGS.has(encoding);
}
const OMIT = new Set([
  "xmp",
  "noembed",
  "noframes",
  "plaintext",
  "script",
  "iframe",
  "frame",
  "frameset",
  "object",
  "embed",
  "applet",
  "base",
  "meta",
  "link",
  "template",
  "foreignobject",
  // HTML table repair can foster-parent these names into a MathML text
  // integration point. A browser reparse would then change their namespace.
  "mglyph",
  "malignmark",
  "animate",
  "animatemotion",
  "animatetransform",
  "set",
  "audio",
  "video",
  "track",
]);
const URL_ATTRS = new Set([
  "href",
  "src",
  "srcset",
  "action",
  "formaction",
  "poster",
  "data",
  "background",
  "ping",
  "cite",
  "longdesc",
  "manifest",
  "codebase",
  "archive",
  "usemap",
  "profile",
  "altimg",
  "definitionurl",
  "cdgroup",
]);
const DATA_SRCSET =
  /^data:image\/(?:png|jpeg|gif|webp|svg\+xml);base64,[a-z0-9+/]+={0,2}(?:#[^\s<>"'\\#\x00-\x1f\x7f]+)?(?:\s+([\s\S]+))?$/i;
async function embeddedSrcset(
  value: string,
  svg: (value: string) => Promise<boolean>,
): Promise<boolean> {
  const candidates = value.trim().split(/\s*,\s*(?=data:image\/)/i);
  if (!candidates.length || candidates.length > 32) return false;
  for (const candidate of candidates) {
    const match = DATA_SRCSET.exec(candidate);
    if (
      !match ||
      (/^data:image\/svg\+xml;/i.test(candidate) &&
        !(await svg(candidate.split(/\s/)[0]!)))
    )
      return false;
    const seen = new Set<string>();
    for (const descriptor of (match[1] ?? "")
      .trim()
      .split(/\s+/)
      .filter(Boolean)) {
      const kind = descriptor.slice(-1),
        count = Number(descriptor.slice(0, -1));
      if (
        !/^(?:[0-9]+[wh]|(?:[0-9]+(?:\.[0-9]+)?|\.[0-9]+)x)$/.test(
          descriptor,
        ) ||
        !Number.isFinite(count) ||
        count <= 0 ||
        seen.has(kind)
      )
        return false;
      seen.add(kind);
    }
    if (
      (seen.has("x") && (seen.has("w") || seen.has("h"))) ||
      (seen.has("h") && !seen.has("w"))
    )
      return false;
  }
  return true;
}
/** Validate large raster data attributes in bounded pieces too. Browser image
 * decoding remains separate; the document CSP still refuses remote fetches. */
async function embeddedRaster(value: string, current: () => boolean) {
  const pause = documentPreparation(current, "Document rendering");
  const header = /^data:image\/(?:png|jpeg|gif|webp);base64,/i.exec(value);
  if (!header || value.length === header[0].length) return false;
  for (let i = header[0].length; i < value.length; i += CHUNK) {
    if (!/^[a-z0-9+/=\s]+$/i.test(value.slice(i, i + CHUNK))) return false;
    await pause();
  }
  return true;
}
export type RenderedHtml = { html: string; removed: boolean };
export function allowDocumentNavigation(url: string): boolean {
  return url === "about:blank" || /^about:blank#[^\s]*$/.test(url);
}
/** Incremental HTML5 parsing after digest verification. Keep authored CSS/layout;
 * discard active content and URLs before a script-disabled, isolated native view.
 * CSP is first in the reconstructed head and also blocks CSS fetches/@imports.
 * No daemon/gateway credential, base URL or message bridge reaches the document. */
export async function prepareRenderedHtml(
  source: string,
  current: () => boolean,
): Promise<RenderedHtml> {
  const allowed = () => {
    if (!current()) throw new Error("Document rendering scope changed");
  };
  allowed();
  const pause = documentPreparation(current, "Document rendering");
  const byteError = () =>
    new Error(
      "HTML rendering supports documents up to 16 MiB. Export the original to open a larger document.",
    );
  if (source.length > MAX_RENDERED_HTML_BYTES) throw byteError();
  let size = 0;
  // Count UTF-8 without a native TextEncoder requirement, yielding between batches.
  for (let i = 0; i < source.length;) {
    const end = Math.min(source.length, i + CHUNK);
    while (i < end) {
      const point = source.codePointAt(i)!;
      size += point <= 127 ? 1 : point <= 2047 ? 2 : point <= 65535 ? 3 : 4;
      i += point > 65535 ? 2 : 1;
      if (size > MAX_RENDERED_HTML_BYTES) throw byteError();
    }
    await pause();
    allowed();
  }
  const parser = documentHtmlParser();
  for (let i = 0; i < source.length; i += CHUNK) {
    allowed();
    parser.write(source.slice(i, i + CHUNK), i + CHUNK >= source.length);
    await pause();
  }
  if (!source.length) parser.write("", true);
  allowed();
  const document = parser.document;
  const pending: { node: DefaultTreeAdapterMap["node"]; depth: number }[] = [
    { node: document, depth: 0 },
  ];
  const svgImage = svgImageValidator(current);
  let removed = false,
    visited = 0;
  while (pending.length) {
    const { node, depth } = pending.pop()!;
    if (++visited > 50_000 || depth > 128)
      throw new Error(
        "This HTML is too complex to render. Use Reading view or export the original.",
      );
    if (tree.isElementNode(node)) {
      let tag = node.tagName.toLowerCase();
      const pictureSource =
        tag === "source" &&
        node.parentNode &&
        tree.isElementNode(node.parentNode) &&
        node.parentNode.namespaceURI === HTML_NS &&
        node.parentNode.tagName === "picture";
      if (
        ![HTML_NS, SVG_NS, MATH_NS].includes(node.namespaceURI) ||
        (node.namespaceURI === MATH_NS && !keepMathElement(node)) ||
        OMIT.has(tag) ||
        (tag === "source" && !pictureSource)
      ) {
        tree.detachNode(node);
        removed = true;
        continue;
      }
      if (node.namespaceURI === MATH_NS && tag === "maction") {
        // Preserve static children without introducing interactive tooltips or
        // selection-dependent content into the script-disabled document view.
        tag = node.tagName = "mrow";
        node.attrs = node.attrs.filter(
          (attr) => !["actiontype", "selection"].includes(attr.name),
        );
        removed = true;
      }
      const imageContext =
        (node.namespaceURI === HTML_NS && tag === "img") ||
        (node.namespaceURI === SVG_NS && ["image", "feimage"].includes(tag));
      const embeddedImages = new Set<(typeof node.attrs)[number]>(),
        responsiveImages = new Set<(typeof node.attrs)[number]>();
      for (const attribute of node.attrs) {
        if (
          imageContext &&
          ["src", "href"].includes(attribute.name) &&
          ((/^data:image\/svg\+xml;/i.test(attribute.value) &&
            (await svgImage(attribute.value))) ||
            (await embeddedRaster(attribute.value, current)))
        )
          embeddedImages.add(attribute);
        if (
          attribute.name === "srcset" &&
          (tag === "img" || pictureSource) &&
          (await embeddedSrcset(attribute.value, svgImage))
        )
          responsiveImages.add(attribute);
      }
      node.attrs = node.attrs.filter((attr) => {
        const name = attr.name.toLowerCase();
        if (
          !/^[a-z_:][a-z0-9_.:-]*$/i.test(name) ||
          name.startsWith("on") ||
          [
            "srcdoc",
            "autofocus",
            "contenteditable",
            "download",
            "target",
            "http-equiv",
            "nonce",
            "is",
          ].includes(name)
        ) {
          removed = true;
          return false;
        }
        if (URL_ATTRS.has(name)) {
          // Only bounded data images and same-document SVG references are visual.
          const embedded =
            ((node.namespaceURI === HTML_NS &&
              tag === "img" &&
              name === "src") ||
              (node.namespaceURI === SVG_NS &&
                ["image", "feimage"].includes(tag) &&
                name === "href")) &&
            embeddedImages.has(attr);
          const fragment =
            node.namespaceURI === SVG_NS &&
            name === "href" &&
            SVG_FRAGMENT.test(attr.value);
          const responsive =
            name === "srcset" &&
            (tag === "img" || pictureSource) &&
            responsiveImages.has(attr);
          if (!embedded && !fragment && !responsive) {
            removed = true;
            return false;
          }
        }
        return true;
      });
      if (["input", "button", "select", "textarea", "fieldset"].includes(tag))
        node.attrs.push({ name: "disabled", value: "" });
    }
    if ("childNodes" in node)
      for (let i = node.childNodes.length - 1; i >= 0; i--)
        pending.push({ node: node.childNodes[i]!, depth: depth + 1 });
    if (visited % 512 === 0) {
      await pause();
      allowed();
    }
  }
  const html = document.childNodes.find(
    (node) => tree.isElementNode(node) && node.tagName === "html",
  ) as DefaultTreeAdapterMap["element"];
  const head = html.childNodes.find(
    (node) => tree.isElementNode(node) && node.tagName === "head",
  ) as DefaultTreeAdapterMap["element"];
  const policies = [
    tree.createElement("meta", HTML_NS, [{ name: "charset", value: "utf-8" }]),
    tree.createElement("meta", HTML_NS, [
      { name: "http-equiv", value: "Content-Security-Policy" },
      { name: "content", value: CSP },
    ]),
    tree.createElement("meta", HTML_NS, [
      { name: "name", value: "viewport" },
      { name: "content", value: "width=device-width, initial-scale=1" },
    ]),
  ];
  for (const policy of policies.reverse()) {
    if (head.childNodes.length)
      tree.insertBefore(head, policy, head.childNodes[0]!);
    else tree.appendChild(head, policy);
  }
  allowed();
  // Serialize in batches too: a large DOM must not monopolize the JS thread.
  const output: string[] = [],
    stack: { node: DefaultTreeAdapterMap["node"] | string }[] = [
      { node: document },
    ];
  let preparedBytes = 0,
    work = 0;
  const emit = async (text: string, attribute = false, raw = false) => {
    for (let i = 0; i < text.length;) {
      let end = Math.min(text.length, i + CHUNK);
      if (end < text.length && /[\uD800-\uDBFF]/.test(text[end - 1]!)) end--;
      let piece = text.slice(i, end);
      if (!raw)
        piece = piece
          .replace(/&/g, "&amp;")
          .replace(/</g, "&lt;")
          .replace(/>/g, "&gt;")
          .replace(attribute ? /"/g : /$^/g, "&quot;");
      for (const character of piece) {
        const point = character.codePointAt(0)!;
        preparedBytes +=
          point <= 127 ? 1 : point <= 2047 ? 2 : point <= 65535 ? 3 : 4;
      }
      if (preparedBytes > MAX_PREPARED_HTML_BYTES)
        throw new Error(
          "Prepared HTML exceeds its 32 MiB limit. Use Source or export the original.",
        );
      output.push(piece);
      work += end - i;
      i = end;
      if (work >= CHUNK) {
        work = 0;
        await pause();
        allowed();
      }
    }
  };
  const voids = new Set([
    "area",
    "base",
    "br",
    "col",
    "embed",
    "hr",
    "img",
    "input",
    "link",
    "meta",
    "param",
    "track",
    "wbr",
  ]);
  let count = 0;
  while (stack.length) {
    const { node } = stack.pop()!;
    if (typeof node === "string") await emit(node, false, true);
    else if (tree.isTextNode(node))
      await emit(
        node.value,
        false,
        !!(
          node.parentNode &&
          tree.isElementNode(node.parentNode) &&
          node.parentNode.tagName === "style" &&
          node.parentNode.namespaceURI === HTML_NS
        ),
      );
    else if (tree.isElementNode(node)) {
      await emit(`<${node.tagName}`, false, true);
      for (const attribute of node.attrs) {
        await emit(
          ` ${attribute.prefix ? attribute.prefix + ":" : ""}${attribute.name}="`,
          false,
          true,
        );
        await emit(attribute.value, true);
        await emit('"', false, true);
      }
      await emit(">", false, true);
      if (!(node.namespaceURI === HTML_NS && voids.has(node.tagName))) {
        stack.push({ node: `</${node.tagName}>` });
        for (let i = node.childNodes.length - 1; i >= 0; i--)
          stack.push({ node: node.childNodes[i]! });
      }
    } else if (node.nodeName === "#documentType")
      await emit("<!doctype html>", false, true);
    else if ("childNodes" in node)
      for (let i = node.childNodes.length - 1; i >= 0; i--)
        stack.push({ node: node.childNodes[i]! });
    if (++count % 512 === 0) {
      await pause();
      allowed();
    }
  }
  const result = output.join("");
  allowed();
  return { html: result, removed };
}

/** Cache complete source rows and a bounded accessible reading alternative. */
export async function prepareHtmlViews(
  text: string,
  current: () => boolean,
  format = "html",
) {
  const { prepareDocumentReading, prepareDocumentSource } =
    await import("./document-preview");
  const source = await prepareDocumentSource(text, current);
  const sourceRows = source.rows;
  let readingPreview: import("./document-preview").DocumentReadingPreview;
  if (format === "svg")
    readingPreview = {
      rows: sourceRows,
      sourceFallback: true,
      note: "SVG Source · Choose Rendered to view the drawing.",
    };
  else if (source.bytes <= 65536)
    readingPreview = await prepareDocumentReading("html", text, current);
  else
    readingPreview = {
      rows: sourceRows,
      sourceFallback: true,
      note: "Use Rendered for page styling. Source shows the complete document.",
    };
  let rendered: RenderedHtml | null = null,
    renderError: string | null = null;
  try {
    rendered = await prepareRenderedHtml(text, current);
  } catch (error) {
    if (!current()) throw error;
    renderError =
      error instanceof Error
        ? error.message
        : "Rendered view unavailable. Use Source or export the original.";
  }
  if (!current()) throw new Error("Document rendering scope changed");
  return { sourceRows, readingPreview, rendered, renderError };
}
