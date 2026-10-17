import type {
  HtmlResourceManifest,
  HtmlSourceChunk,
  WorkflowHtmlManifestQuery,
  WorkflowHtmlSourceQuery,
} from "./generated";
import {
  validateHtmlManifest,
  validateHtmlChunk,
  validateHtmlSourceQuery,
} from "./html-resources";
import {
  validateWorkflowDocument,
  type WorkflowDocument,
} from "./gateway-history";
export type GatewayHtmlQuery =
  | { kind: "workflowHtmlManifest"; query: WorkflowHtmlManifestQuery }
  | { kind: "workflowHtmlSource"; query: WorkflowHtmlSourceQuery };
export type GatewayHtmlResult =
  | {
      kind: "workflowHtmlManifest";
      page: {
        runId: string;
        document: WorkflowDocument;
        manifest: HtmlResourceManifest | null;
      };
      pageTruncated: false;
      servedAt: number;
    }
  | {
      kind: "workflowHtmlSource";
      page: {
        runId: string;
        document: WorkflowDocument;
        chunk: HtmlSourceChunk;
      };
      pageTruncated: false;
      servedAt: number;
    };
const shape = (v: unknown, fields: string): v is Record<string, any> =>
  !!v &&
  typeof v === "object" &&
  !Array.isArray(v) &&
  Object.keys(v).sort().join(",") === fields.split(",").sort().join(",");
const id = (v: unknown) =>
  typeof v === "string" &&
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(v) &&
  v !== "00000000-0000-0000-0000-000000000000";
export const isGatewayHtml = (
  kind: unknown,
): kind is GatewayHtmlQuery["kind"] =>
  kind === "workflowHtmlManifest" || kind === "workflowHtmlSource";
export function validateGatewayHtmlQuery(
  v: unknown,
): asserts v is GatewayHtmlQuery {
  if (
    !shape(v, "kind,query") ||
    !isGatewayHtml(v.kind) ||
    !shape(
      v.query,
      v.kind === "workflowHtmlManifest"
        ? "runId,artifactId,sha256"
        : "runId,artifactId,sha256,source",
    ) ||
    !id(v.query.runId) ||
    !id(v.query.artifactId) ||
    typeof v.query.sha256 !== "string" ||
    !/^[0-9a-f]{64}$/.test(v.query.sha256)
  )
    throw new Error("Invalid HTML workflow query");
  if (v.kind === "workflowHtmlSource") validateHtmlSourceQuery(v.query.source);
}
export function validateGatewayHtmlResult(
  v: unknown,
  q: GatewayHtmlQuery,
): asserts v is GatewayHtmlResult {
  validateGatewayHtmlQuery(q);
  if (
    !shape(v, "kind,page,pageTruncated,servedAt") ||
    v.kind !== q.kind ||
    v.pageTruncated !== false ||
    !Number.isSafeInteger(v.servedAt) ||
    v.servedAt < 0 ||
    !shape(
      v.page,
      q.kind === "workflowHtmlManifest"
        ? "runId,document,manifest"
        : "runId,document,chunk",
    ) ||
    v.page.runId !== q.query.runId
  )
    throw new Error("Invalid HTML workflow result");
  validateWorkflowDocument(v.page.document);
  if (
    v.page.document.id !== q.query.artifactId ||
    v.page.document.sha256 !== q.query.sha256 ||
    v.page.document.format !== "html"
  )
    throw new Error("HTML workflow publication changed");
  const expected = { id: q.query.artifactId, sha256: q.query.sha256 };
  if (q.kind === "workflowHtmlManifest") {
    if (v.page.manifest !== null)
      validateHtmlManifest(v.page.manifest, expected);
  } else validateHtmlChunk(v.page.chunk, q.query.source, expected);
}
