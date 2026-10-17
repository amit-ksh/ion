import {
  readHtmlResourceSet,
  validatePortfolioResult,
  type ArtifactLibraryItem,
  type HtmlSourceQuery,
  type PortfolioQuery,
  type PortfolioResult,
  type WakuClient,
  type WorkflowDocument,
} from "@waku/client";
import { validateArtifactItem } from "./artifact-library";
import type { GatewayPairing } from "./gateway-client";
import { loadGatewayPortfolio } from "./gateway-portfolio";
const shape = (v: unknown, fields: string): v is Record<string, any> =>
  !!v &&
  typeof v === "object" &&
  !Array.isArray(v) &&
  Object.keys(v).sort().join(",") === fields.split(",").sort().join(",");
const same = (a: object, b: Record<string, unknown>) =>
  Object.keys(a).every((k) => (a as Record<string, unknown>)[k] === b[k]);
export async function readArtifactHtmlResources(
  client: Pick<WakuClient, "request">,
  expected: ArtifactLibraryItem,
  original: Uint8Array,
  current: () => boolean,
  digest: (bytes: Uint8Array) => Promise<string>,
) {
  validateArtifactItem(expected);
  if (expected.format !== "html") return null;
  const read = async (source?: HtmlSourceQuery) => {
    if (!current()) throw new Error("HTML resource read scope changed");
    const reply = await client.request(
      source
        ? { type: "readArtifactHtmlSource", query: { expected, source } }
        : { type: "readArtifactHtmlManifest", query: { expected } },
      undefined,
      undefined,
      { timeoutMs: 30_000 },
    );
    if (!current()) throw new Error("HTML resource read scope changed");
    if (
      (reply.type !== "artifactHtmlManifest" &&
        reply.type !== "artifactHtmlSource") ||
      reply.type !== (source ? "artifactHtmlSource" : "artifactHtmlManifest") ||
      !shape(reply.result, source ? "document,chunk" : "document,manifest")
    )
      throw new Error("Invalid HTML resource response");
    validateArtifactItem(reply.result.document);
    if (!same(expected, reply.result.document))
      throw new Error("HTML resource publication changed");
    return source
      ? (reply.result as any).chunk
      : (reply.result as any).manifest;
  };
  return readHtmlResourceSet(expected, original, {
    manifest: () => read(),
    chunk: (q) => read(q),
    digest,
    current,
  });
}
export async function readGatewayHtmlResources(
  pairing: GatewayPairing,
  publisherId: string,
  runId: string,
  document: WorkflowDocument,
  original: Uint8Array,
  current: () => boolean,
  nonce: () => string,
  digest: (bytes: Uint8Array) => Promise<string>,
  reader?: (query: PortfolioQuery) => Promise<PortfolioResult>,
) {
  const allowed = () =>
    current() &&
    pairing.expiresAt > Date.now() &&
    pairing.grants.includes("evidenceRead");
  if (!allowed())
    throw new Error("Pair with Read workflow documents to read HTML resources");
  if (document.format !== "html") return null;
  const read = async (source?: HtmlSourceQuery) => {
    if (!allowed()) throw new Error("HTML resource read scope changed");
    const selection = {
      runId,
      artifactId: document.id,
      sha256: document.sha256,
    };
    const query: PortfolioQuery = source
      ? { kind: "workflowHtmlSource", query: { ...selection, source } }
      : { kind: "workflowHtmlManifest", query: selection };
    const result = await (reader
      ? reader(query)
      : loadGatewayPortfolio(pairing, publisherId, query, nonce(), allowed));
    if (!allowed()) throw new Error("HTML resource read scope changed");
    validatePortfolioResult(result, query);
    if (
      (result.kind !== "workflowHtmlManifest" &&
        result.kind !== "workflowHtmlSource") ||
      !same(
        document,
        result.page.document as unknown as Record<string, unknown>,
      )
    )
      throw new Error("HTML resource belongs to different workflow evidence");
    return result.kind === "workflowHtmlSource"
      ? result.page.chunk
      : result.page.manifest;
  };
  return readHtmlResourceSet(
    { id: document.id, sha256: document.sha256 },
    original,
    { manifest: () => read(), chunk: (q) => read(q), digest, current: allowed },
  );
}
