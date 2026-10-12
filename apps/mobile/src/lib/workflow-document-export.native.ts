import { Directory, FileMode, type FileHandle } from "expo-file-system";
import type { WorkflowDocument } from "@waku/client";
import {
  workflowExportName,
  workflowDocumentMime,
} from "./gateway-workflow-document";

/** Bounded bridge writes leave native input and cancellation runnable between chunks. */
export async function exportWorkflowDocument(
  document: Pick<WorkflowDocument, "name" | "format" | "id">,
  bytes: Uint8Array,
  current: () => boolean,
): Promise<void> {
  const allowed = () => {
    if (!current()) throw new Error("Document export scope changed");
  };
  allowed();
  if (bytes.length < 1 || bytes.length > 16 * 1024 * 1024)
    throw new Error("Document export exceeds its byte limit");
  const directory = await Directory.pickDirectoryAsync();
  allowed();
  const file = directory.createFile(
    workflowExportName(document, Date.now().toString()),
    workflowDocumentMime(document.format),
  );
  let handle: FileHandle | undefined;
  let complete = false;
  try {
    allowed();
    // Android SAF directory-picker files default to read-only handles.
    handle = file.open(FileMode.WriteOnly);
    for (let offset = 0; offset < bytes.length; offset += 65_536) {
      allowed();
      handle.writeBytes(bytes.subarray(offset, offset + 65_536));
      await new Promise<void>((resolve) => setTimeout(resolve, 0));
      allowed();
    }
    if (handle.offset !== bytes.length || handle.size !== bytes.length)
      throw new Error("Document export did not write the complete original");
    handle.close();
    handle = undefined;
    allowed();
    complete = true;
  } finally {
    if (handle) {
      try {
        handle.close();
      } catch {}
    }
    if (!complete) {
      try {
        file.delete();
      } catch {}
    }
  }
}
