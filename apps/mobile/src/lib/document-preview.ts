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
  if (new TextEncoder().encode(text).byteLength > 65_536)
    throw new Error("Document preview exceeds its text limit");
  if (!reading) return chunks(text);
  if (format === "json") {
    try {
      return chunks(JSON.stringify(JSON.parse(text), null, 2));
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
