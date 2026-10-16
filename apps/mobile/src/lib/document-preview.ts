import { MAX_DOCUMENT_TEXT_BYTES, documentPreparation } from "./document-text";
import type { RootContent } from "mdast";
import { parseBlocks } from "../md/parse";
import {
  htmlNodeText,
  htmlReadingSteps,
  previewUtf8Size,
  readHtml,
  type HtmlReading,
} from "./html-reading";
export type DocumentPreviewRow = {
  id: string;
  text: string;
  node: RootContent | null;
};
function chunks(text: string): DocumentPreviewRow[] {
  return (text.match(/[\s\S]{1,1000}/gu) ?? []).map((text, index) => ({
    id: String(index),
    text,
    node: null,
  }));
}
// Estimate whitespace growth before JSON.stringify: deep nesting must not turn
// a bounded input into a giant allocation. Source remains complete on fallback.
function canFormatJson(text: string, bytes: number): boolean {
  let depth = 0,
    quoted = false,
    escaped = false,
    bound = bytes;
  for (const character of text) {
    if (quoted) {
      if (escaped) escaped = false;
      else if (character === "\\") escaped = true;
      else if (character === '"') quoted = false;
      continue;
    }
    if (character === '"') quoted = true;
    else if (character === "{" || character === "[") {
      depth++;
      bound += 1 + 2 * depth;
    } else if (character === "}" || character === "]") {
      depth--;
      bound += 1 + 2 * Math.max(0, depth);
    } else if (character === ",") bound += 1 + 2 * depth;
    else if (character === ":") bound++;
    if (depth > 64 || bound > 65_536) return false;
  }
  return true;
}
/** Reading never creates external image requests, navigation or app commands. */
function inert(
  node: Record<string, unknown>,
  root = false,
): Record<string, unknown> {
  if (["image", "imageReference"].includes(String(node.type)))
    return {
      type: "text",
      value: `Image: ${typeof node.alt === "string" ? node.alt : ""}`,
    };
  if (["link", "linkReference"].includes(String(node.type)))
    return { type: "text", value: visible(node) };
  if (node.type === "html") {
    const text = { type: "text", value: String(node.value ?? "") };
    return root ? { type: "paragraph", children: [text] } : text;
  }
  return {
    ...node,
    ...(Array.isArray(node.children)
      ? {
          children: node.children.map((child) =>
            inert(child as Record<string, unknown>),
          ),
        }
      : {}),
  };
}
function visible(node: Record<string, unknown>): string {
  if (typeof node.value === "string") return node.value;
  if (typeof node.alt === "string") return node.alt;
  return Array.isArray(node.children)
    ? node.children
        .map((child) => visible(child as Record<string, unknown>))
        .join("")
    : "";
}
export function documentPreviewRows(
  format: string,
  text: string,
  reading: boolean,
): DocumentPreviewRow[] {
  const byteLength = previewUtf8Size(text);
  if (byteLength > 65_536)
    throw new Error("Document preview exceeds its text limit");
  if (!reading) return chunks(text);
  if (format === "json") {
    try {
      if (!canFormatJson(text, byteLength)) return chunks(text);
      const formatted = JSON.stringify(JSON.parse(text), null, 2);
      return chunks(previewUtf8Size(formatted) <= 65_536 ? formatted : text);
    } catch {
      return chunks(text);
    } // Preserve inspectable original source even if the agent wrote invalid JSON.
  }
  if (["html", "htm"].includes(format))
    return documentReadingPreview(format, text).rows;
  if (format !== "md") return chunks(text);
  return parseBlocks(text).flatMap((block, index) => {
    // A single giant block must not defeat list virtualization.
    if (block.source.length > 4096)
      return chunks(block.source).map((row) => ({
        ...row,
        id: `${index}-${row.id}`,
      }));
    return [
      {
        id: String(index),
        text: block.source,
        node: inert(
          block.node as unknown as Record<string, unknown>,
          true,
        ) as unknown as RootContent,
      },
    ];
  });
}

export type DocumentReadingPreview = {
  rows: DocumentPreviewRow[];
  note: string | null;
  sourceFallback: boolean;
};
function htmlRows(value: HtmlReading): DocumentReadingPreview {
  return {
    rows: value.nodes.map((node, index) => ({
      id: String(index),
      text: htmlNodeText(node),
      node,
    })),
    note: "Structured HTML Reading · Page styling is not reproduced. Scripts, links and media remain inactive.",
    sourceFallback: false,
  };
}
function htmlFallback(text: string): DocumentReadingPreview {
  return {
    rows: chunks(text),
    note: "HTML Reading is unavailable for this document’s structure or preview limits. Showing complete Source. Export keeps the original document.",
    sourceFallback: true,
  };
}
export function documentReadingPreview(
  format: string,
  text: string,
): DocumentReadingPreview {
  if (previewUtf8Size(text) > 65_536)
    throw new Error("Document preview exceeds its text limit");
  if (["html", "htm"].includes(format)) {
    try {
      return htmlRows(readHtml(text));
    } catch {
      return htmlFallback(text);
    }
  }
  return {
    rows: documentPreviewRows(format, text, true),
    note: null,
    sourceFallback: false,
  };
}
/** Prepare cached layout after complete digest verification, outside rendering.
 * Batches yield to the native event loop and the owner/foreground generation
 * fence runs before each batch and before any prepared result can be stored. */
export async function prepareDocumentReading(
  format: string,
  text: string,
  current: () => boolean,
): Promise<DocumentReadingPreview> {
  const allowed = () => {
    if (!current()) throw new Error("Document reading scope changed");
  };
  allowed();
  if (previewUtf8Size(text) > 65_536)
    throw new Error("Document preview exceeds its text limit");
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
  allowed();
  if (!["html", "htm"].includes(format)) {
    const result = documentReadingPreview(format, text);
    allowed();
    return result;
  }
  const steps = htmlReadingSteps(text);
  while (true) {
    allowed();
    let result: IteratorResult<void, HtmlReading>;
    try {
      result = steps.next();
    } catch {
      allowed();
      return htmlFallback(text);
    }
    if (result.done) {
      allowed();
      return htmlRows(result.value);
    }
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
  }
}

/** Complete virtualized source for every supported text publication. The
 * synchronous semantic parsers retain their small-input bounds. */
export async function prepareDocumentSource(
  text: string,
  current: () => boolean,
) {
  const rows: DocumentPreviewRow[] = [];
  let bytes = 0;
  const pause = documentPreparation(current);
  if (!current()) throw new Error("Document reading scope changed");
  for (let i = 0; i < text.length;) {
    let end = Math.min(text.length, i + 1000);
    if (end < text.length && /[\uD800-\uDBFF]/.test(text[end - 1]!)) end--;
    const piece = text.slice(i, end);
    for (const character of piece) {
      const point = character.codePointAt(0)!;
      bytes += point <= 127 ? 1 : point <= 2047 ? 2 : point <= 65535 ? 3 : 4;
    }
    if (bytes > MAX_DOCUMENT_TEXT_BYTES)
      throw new Error("Document text exceeds its 16 MiB limit");
    rows.push({ id: String(i), text: piece, node: null });
    i = end;
    await pause(piece.length);
  }
  if (!current()) throw new Error("Document reading scope changed");
  return { rows, bytes };
}
export async function prepareTextDocumentViews(
  format: string,
  text: string,
  current: () => boolean,
) {
  const source = await prepareDocumentSource(text, current);
  const readingPreview: DocumentReadingPreview =
    source.bytes <= 65_536
      ? await prepareDocumentReading(format, text, current)
      : {
          rows: source.rows,
          sourceFallback: true,
          note: "Showing complete Source for this larger document. Export keeps the original.",
        };
  if (!current()) throw new Error("Document reading scope changed");
  return { sourceRows: source.rows, readingPreview };
}
