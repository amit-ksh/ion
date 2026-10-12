import {
  validateWorkflowDocument,
  workflowChunkBytes,
  type WorkflowDocument,
} from "./gateway-history";
export type GatewayPdfQuery = {
  kind: "workflowPdfPage";
  query: {
    runId: string;
    revision: number;
    expected: WorkflowDocument;
    page: number;
    width: number;
    transferId: string | null;
    offset: number;
    limit: number;
  };
};
export type GatewayPdfResult = {
  kind: "workflowPdfPage";
  page: {
    runId: string;
    revision: number;
    document: WorkflowDocument;
    transferId: string;
    page: number;
    pages: number;
    requestedWidth: number;
    width: number;
    height: number;
    bytes: number;
    sha256: string;
    expiresAt: number;
    offset: number;
    nextOffset: number | null;
    data: string;
  };
  pageTruncated: false;
  servedAt: number;
};
const shape = (v: unknown, fields: string): v is Record<string, any> =>
  !!v &&
  typeof v === "object" &&
  !Array.isArray(v) &&
  Object.keys(v).sort().join(",") === fields.split(",").sort().join(",");
const integer = (v: unknown): v is number =>
  typeof v === "number" && Number.isSafeInteger(v) && v >= 0;
const id = (v: unknown): v is string =>
  typeof v === "string" &&
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(v) &&
  v !== "00000000-0000-0000-0000-000000000000";
export function validateGatewayPdfQuery(
  value: unknown,
): asserts value is GatewayPdfQuery {
  if (
    !shape(value, "kind,query") ||
    value.kind !== "workflowPdfPage" ||
    !shape(
      value.query,
      "runId,revision,expected,page,width,transferId,offset,limit",
    )
  )
    throw new Error("Invalid PDF page query.");
  const q = value.query;
  validateWorkflowDocument(q.expected);
  if (
    !id(q.runId) ||
    !integer(q.revision) ||
    q.expected.format !== "pdf" ||
    q.expected.bytes > 65536 ||
    !["id", "projectId", "teamRunId", "assignmentId"].every((k) =>
      id(q.expected[k]),
    ) ||
    (q.expected.taskId !== null && !id(q.expected.taskId)) ||
    [...q.expected.name].length > 80 ||
    !integer(q.page) ||
    q.page >= 32 ||
    ![800, 1200, 1600].includes(q.width) ||
    !integer(q.offset) ||
    q.offset >= 12 * 1024 * 1024 ||
    q.offset % 32768 !== 0 ||
    q.limit !== 32768 ||
    (q.transferId === null
      ? q.offset !== 0
      : !id(q.transferId) || q.offset === 0)
  )
    throw new Error("Unsupported PDF source, page, zoom or chunk.");
}
export function validateGatewayPdfResult(
  value: unknown,
  query: GatewayPdfQuery,
): asserts value is GatewayPdfResult {
  validateGatewayPdfQuery(query);
  if (
    !shape(value, "kind,page,pageTruncated,servedAt") ||
    value.kind !== query.kind ||
    value.pageTruncated !== false ||
    !integer(value.servedAt) ||
    !shape(
      value.page,
      "runId,revision,document,transferId,page,pages,requestedWidth,width,height,bytes,sha256,expiresAt,offset,nextOffset,data",
    )
  )
    throw new Error("Invalid PDF image response.");
  const p = value.page,
    q = query.query;
  validateWorkflowDocument(p.document);
  if (
    p.runId !== q.runId ||
    p.revision !== q.revision ||
    Object.keys(q.expected).some(
      (k) => q.expected[k as keyof WorkflowDocument] !== p.document[k],
    ) ||
    !id(p.transferId) ||
    (q.transferId !== null && p.transferId !== q.transferId) ||
    p.page !== q.page ||
    p.requestedWidth !== q.width ||
    !integer(p.pages) ||
    p.pages < 1 ||
    p.pages > 32 ||
    p.page >= p.pages ||
    !integer(p.width) ||
    p.width < 1 ||
    p.width > q.width ||
    !integer(p.height) ||
    p.height < 1 ||
    p.height > 2000 ||
    p.width * p.height > 3200000 ||
    !integer(p.bytes) ||
    p.bytes < 57 ||
    p.bytes > 12 * 1024 * 1024 ||
    typeof p.sha256 !== "string" ||
    !/^[0-9a-f]{64}$/.test(p.sha256) ||
    !integer(p.expiresAt) ||
    p.expiresAt <= value.servedAt ||
    p.expiresAt > value.servedAt + 10 * 60 * 1000 ||
    p.offset !== q.offset ||
    typeof p.data !== "string"
  )
    throw new Error("PDF image crossed source or transfer scope.");
  const count = workflowChunkBytes(p.data, 32768).length,
    end = p.offset + count;
  if (
    count < 1 ||
    count > q.limit ||
    end > p.bytes ||
    count !== Math.min(q.limit, p.bytes - p.offset) ||
    p.nextOffset !== (end < p.bytes ? end : null)
  )
    throw new Error("PDF image chunk cursor changed.");
}
