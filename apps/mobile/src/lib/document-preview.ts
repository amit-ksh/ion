import type { RootContent } from "mdast";
import { parseBlocks } from "../md/parse";
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
function utf8Size(text: string): number {
  let size = 0;
  for (const character of text) {
    const point = character.codePointAt(0)!;
    size += point <= 0x7f ? 1 : point <= 0x7ff ? 2 : point <= 0xffff ? 3 : 4;
  }
  return size;
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
  const byteLength = utf8Size(text);
  if (byteLength > 65_536)
    throw new Error("Document preview exceeds its text limit");
  if (!reading) return chunks(text);
  if (format === "json") {
    try {
      if (!canFormatJson(text, byteLength)) return chunks(text);
      const formatted = JSON.stringify(JSON.parse(text), null, 2);
      return chunks(utf8Size(formatted) <= 65_536 ? formatted : text);
    } catch {
      return chunks(text);
    } // Preserve inspectable original source even if the agent wrote invalid JSON.
  }
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
