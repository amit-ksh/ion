import { Directory } from "expo-file-system";
import type { WorkflowDocument } from "@waku/client";
import { workflowExportName } from "./gateway-workflow-document";
export async function exportWorkflowDocument(
  document: WorkflowDocument,
  bytes: Uint8Array,
  current: () => boolean,
): Promise<void> {
  if (!current()) throw new Error("Document export scope changed");
  const directory = await Directory.pickDirectoryAsync();
  if (!current()) throw new Error("Document export scope changed");
  const mime =
    document.format === "pdf"
      ? "application/pdf"
      : document.format === "html"
        ? "text/html"
        : document.format === "json"
          ? "application/json"
          : "text/plain";
  const file = directory.createFile(
    workflowExportName(document, Date.now().toString()),
    mime,
  );
  try {
    file.write(bytes);
  } catch (error) {
    try {
      file.delete();
    } catch {}
    throw error;
  }
}
