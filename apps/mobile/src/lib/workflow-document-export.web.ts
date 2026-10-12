import type { WorkflowDocument } from "@waku/client";
import {
  workflowExportName,
  workflowDocumentMime,
} from "./gateway-workflow-document";
export async function exportWorkflowDocument(
  record: Pick<WorkflowDocument, "name" | "format" | "id">,
  bytes: Uint8Array,
  current: () => boolean,
): Promise<void> {
  if (!current()) throw new Error("Document export scope changed");
  const mime = workflowDocumentMime(record.format);
  const url = URL.createObjectURL(
    new Blob([bytes.slice().buffer], { type: mime }),
  );
  try {
    if (!current()) throw new Error("Document export scope changed");
    const link = document.createElement("a");
    link.href = url;
    link.download = workflowExportName(record, Date.now().toString());
    document.body.appendChild(link);
    link.click();
    link.remove();
  } finally {
    setTimeout(() => URL.revokeObjectURL(url), 1_000);
  }
}
