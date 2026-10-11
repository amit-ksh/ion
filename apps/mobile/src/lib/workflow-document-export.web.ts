import type { WorkflowDocument } from "@waku/client";
import { workflowExportName } from "./gateway-workflow-document";
export async function exportWorkflowDocument(
  record: WorkflowDocument,
  bytes: Uint8Array,
  current: () => boolean,
): Promise<void> {
  if (!current()) throw new Error("Document export scope changed");
  const mime =
    record.format === "pdf"
      ? "application/pdf"
      : record.format === "html"
        ? "text/html"
        : record.format === "json"
          ? "application/json"
          : "text/plain";
  const url = URL.createObjectURL(
    new Blob([bytes.slice().buffer], { type: mime }),
  );
  try {
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
