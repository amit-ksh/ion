import {
  Parser,
  html as constants,
  defaultTreeAdapter as tree,
  type DefaultTreeAdapterMap,
} from "parse5";

export const MAX_RENDERED_HTML_BYTES = 1024 * 1024;
const CHUNK = 8192;
const CSP =
  "default-src 'none'; script-src 'none'; style-src 'unsafe-inline' data:; img-src data:; font-src data:; connect-src 'none'; frame-src 'none'; object-src 'none'; media-src 'none'; base-uri 'none'; form-action 'none'";
const HTML_NS = constants.NS.HTML;
const SVG_NS = constants.NS.SVG;
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
]);
const DATA_SRCSET =
  /^data:image\/(?:png|jpeg|gif|webp);base64,[a-z0-9+/]+={0,2}(?:\s+([\s\S]+))?$/i;
function embeddedSrcset(value: string): boolean {
  const candidates = value.trim().split(/\s*,\s*(?=data:image\/)/i);
  if (!candidates.length || candidates.length > 32) return false;
  return candidates.every((candidate) => {
    const match = DATA_SRCSET.exec(candidate);
    if (!match) return false;
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
    return (
      !(seen.has("x") && (seen.has("w") || seen.has("h"))) &&
      !(seen.has("h") && !seen.has("w"))
    );
  });
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
  let size = 0;
  // Count UTF-8 without a native TextEncoder requirement, yielding between batches.
  for (let i = 0; i < source.length;) {
    const end = Math.min(source.length, i + CHUNK);
    while (i < end) {
      const point = source.codePointAt(i)!;
      size += point <= 127 ? 1 : point <= 2047 ? 2 : point <= 65535 ? 3 : 4;
      i += point > 65535 ? 2 : 1;
      if (size > MAX_RENDERED_HTML_BYTES)
        throw new Error(
          "HTML rendering supports documents up to 1 MiB. Export the original to open a larger document.",
        );
    }
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
    allowed();
  }
  let created = 0;
  const complexityError = () =>
    new Error(
      "This HTML is too complex to render. Use Reading view or export the original.",
    );
  const adapter = {
    ...tree,
    createElement: (...args: Parameters<typeof tree.createElement>) => {
      if (++created > 50_000) throw complexityError();
      return tree.createElement(...args);
    },
    onItemPush: (node: DefaultTreeAdapterMap["parentNode"]) => {
      // Reject deep trees while parsing, before allocating the complete DOM.
      let depth = 0,
        parent: DefaultTreeAdapterMap["parentNode"] | null = node;
      while (parent && "parentNode" in parent) {
        if (++depth > 128) throw complexityError();
        parent = parent.parentNode;
      }
    },
  };
  const parser = new Parser<DefaultTreeAdapterMap>({
    scriptingEnabled: false,
    treeAdapter: adapter,
  });
  for (let i = 0; i < source.length; i += CHUNK) {
    allowed();
    parser.tokenizer.write(
      source.slice(i, i + CHUNK),
      i + CHUNK >= source.length,
    );
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
  }
  if (!source.length) parser.tokenizer.write("", true);
  allowed();
  const document = parser.document;
  const pending: { node: DefaultTreeAdapterMap["node"]; depth: number }[] = [
    { node: document, depth: 0 },
  ];
  let removed = false,
    visited = 0;
  while (pending.length) {
    const { node, depth } = pending.pop()!;
    if (++visited > 50_000 || depth > 128)
      throw new Error(
        "This HTML is too complex to render. Use Reading view or export the original.",
      );
    if (tree.isElementNode(node)) {
      const tag = node.tagName.toLowerCase();
      const pictureSource =
        tag === "source" &&
        node.parentNode &&
        tree.isElementNode(node.parentNode) &&
        node.parentNode.namespaceURI === HTML_NS &&
        node.parentNode.tagName === "picture";
      if (
        ![HTML_NS, SVG_NS].includes(node.namespaceURI) ||
        OMIT.has(tag) ||
        (tag === "source" && !pictureSource)
      ) {
        tree.detachNode(node);
        removed = true;
        continue;
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
          // Only raster data images and same-document SVG references are visual.
          const embedded =
            ((node.namespaceURI === HTML_NS &&
              tag === "img" &&
              name === "src") ||
              (node.namespaceURI === SVG_NS &&
                ["image", "feimage"].includes(tag) &&
                name === "href")) &&
            /^data:image\/(?:png|jpeg|gif|webp);base64,[a-z0-9+/=\s]+$/i.test(
              attr.value,
            );
          const fragment =
            node.namespaceURI === SVG_NS &&
            name === "href" &&
            /^#[a-z0-9_-]+$/i.test(attr.value);
          const responsive =
            name === "srcset" &&
            (tag === "img" || pictureSource) &&
            embeddedSrcset(attr.value);
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
      await new Promise<void>((resolve) => setTimeout(resolve, 0));
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
  const escape = (text: string, attribute = false) =>
    text
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(attribute ? /"/g : /$^/g, "&quot;");
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
    if (typeof node === "string") output.push(node);
    else if (tree.isTextNode(node))
      output.push(
        node.parentNode &&
          tree.isElementNode(node.parentNode) &&
          node.parentNode.tagName === "style" &&
          node.parentNode.namespaceURI === HTML_NS
          ? node.value
          : escape(node.value),
      );
    else if (tree.isElementNode(node)) {
      output.push(
        `<${node.tagName}${node.attrs.map((attr) => ` ${attr.prefix ? attr.prefix + ":" : ""}${attr.name}="${escape(attr.value, true)}"`).join("")}>`,
      );
      if (!(node.namespaceURI === HTML_NS && voids.has(node.tagName))) {
        stack.push({ node: `</${node.tagName}>` });
        for (let i = node.childNodes.length - 1; i >= 0; i--)
          stack.push({ node: node.childNodes[i]! });
      }
    } else if (node.nodeName === "#documentType")
      output.push("<!doctype html>");
    else if ("childNodes" in node)
      for (let i = node.childNodes.length - 1; i >= 0; i--)
        stack.push({ node: node.childNodes[i]! });
    if (++count % 512 === 0) {
      await new Promise<void>((resolve) => setTimeout(resolve, 0));
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
  const { prepareDocumentReading } = await import("./document-preview");
  const { previewUtf8Size } = await import("./html-reading");
  const sourceRows: import("./document-preview").DocumentPreviewRow[] = [];
  for (let i = 0; i < text.length;) {
    let end = Math.min(text.length, i + 1000);
    if (end < text.length && /[\uD800-\uDBFF]/.test(text[end - 1]!)) end--;
    sourceRows.push({
      id: String(i),
      text: text.slice(i, end),
      node: null,
    });
    i = end;
    if (sourceRows.length % 32 === 0) {
      await new Promise<void>((resolve) => setTimeout(resolve, 0));
      if (!current()) throw new Error("Document rendering scope changed");
    }
  }
  let readingPreview: import("./document-preview").DocumentReadingPreview;
  if (format === "svg")
    readingPreview = {
      rows: sourceRows,
      sourceFallback: true,
      note: "SVG Source · Choose Rendered to view the drawing.",
    };
  else if (previewUtf8Size(text) <= 65536)
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
