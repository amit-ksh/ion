import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { createHash, randomUUID } from "node:crypto";
import type { PortfolioQuery, PortfolioResult, WakuClient } from "@waku/client";
import type { GatewayPairing } from "./gateway-client";
import {
  readArtifactHtmlResources,
  readGatewayHtmlResources,
} from "./html-resource-readers";
const file = process.env.ION_HTML_RESOURCES_FIXTURE;
const check = test.skipIf(!file);
const digest = async (b: Uint8Array) =>
  createHash("sha256").update(b).digest("hex");
function fixture() {
  const f = JSON.parse(readFileSync(file!, "utf8")),
    original = Buffer.from(f.original, "base64"),
    runId = randomUUID();
  const pairing: GatewayPairing = {
    id: randomUUID(),
    baseUrl: "https://gateway.example",
    token: "synthetic-read-token",
    grants: ["evidenceRead"],
    expiresAt: Date.now() + 120000,
  };
  const document = {
    id: f.expected.id,
    name: f.expected.name,
    format: "html",
    sha256: f.expected.sha256,
    bytes: f.expected.bytes,
    projectId: f.expected.projectId,
    taskId: f.expected.taskId,
    createdAt: f.expected.createdAt,
    stage: 0,
    teamRunId: randomUUID(),
    assignmentId: randomUUID(),
  };
  const nativeChunk = (q: any) =>
    f.chunks.find(
      (c: any) => c.sourceIndex === q.sourceIndex && c.offset === q.offset,
    );
  const client = {
    request: async (q: any) =>
      q.type === "readArtifactHtmlManifest"
        ? {
            type: "artifactHtmlManifest",
            result: { document: f.expected, manifest: f.manifest },
          }
        : {
            type: "artifactHtmlSource",
            result: {
              document: f.expected,
              chunk: nativeChunk(q.query.source),
            },
          },
  } as unknown as Pick<WakuClient, "request">;
  const reader = async (q: PortfolioQuery): Promise<PortfolioResult> => {
    if (q.kind !== "workflowHtmlManifest" && q.kind !== "workflowHtmlSource")
      throw new Error("Unexpected request");
    return {
      kind: q.kind,
      page: {
        runId,
        document,
        ...(q.kind === "workflowHtmlManifest"
          ? { manifest: f.manifest }
          : { chunk: nativeChunk(q.query.source) }),
      },
      pageTruncated: false,
      servedAt: Date.now(),
    } as PortfolioResult;
  };
  return { f, original, runId, pairing, document, client, reader };
}
check(
  "direct phone source reader consumes actual native RPC data with its original commitment",
  async () => {
    const { f, original, client } = fixture();
    const result = await readArtifactHtmlResources(
      client,
      f.expected,
      original,
      () => true,
      digest,
    );
    expect(result?.sources.length).toBe(5);
    expect(result?.manifest.entryPath).toBe("経緯🚀.html");
    expect(result?.sources[2]!.length).toBeGreaterThan(50000);
  },
);
check(
  "direct source reading refuses changed selection and owner generations",
  async () => {
    const { f, original, client } = fixture();
    const bad = {
      request: async (q: any) => {
        const result: any = await client.request(q);
        return {
          ...result,
          result: {
            ...result.result,
            document: { ...result.result.document, taskId: randomUUID() },
          },
        };
      },
    } as unknown as Pick<WakuClient, "request">;
    await expect(
      readArtifactHtmlResources(bad, f.expected, original, () => true, digest),
    ).rejects.toThrow("publication changed");
    let live = true;
    const changed = {
      request: async (q: any) => {
        const result = await client.request(q);
        live = false;
        return result;
      },
    } as unknown as Pick<WakuClient, "request">;
    await expect(
      readArtifactHtmlResources(
        changed,
        f.expected,
        original,
        () => live,
        digest,
      ),
    ).rejects.toThrow("scope changed");
  },
);
check(
  "gateway phone source reading pins all workflow evidence and grant data",
  async () => {
    const f = fixture();
    const result = await readGatewayHtmlResources(
      f.pairing,
      randomUUID(),
      f.runId,
      f.document,
      f.original,
      () => true,
      randomUUID,
      digest,
      f.reader,
    );
    expect(result?.sources.length).toBe(5);
    expect(result?.manifest.artifactSha256).toBe(f.document.sha256);
    const reader = async (q: PortfolioQuery) => {
      const result: any = await f.reader(q);
      result.page.document = {
        ...result.page.document,
        assignmentId: randomUUID(),
      };
      return result;
    };
    await expect(
      readGatewayHtmlResources(
        f.pairing,
        randomUUID(),
        f.runId,
        f.document,
        f.original,
        () => true,
        randomUUID,
        digest,
        reader,
      ),
    ).rejects.toThrow("different workflow");
  },
);
check(
  "gateway grant loss, expiry and foreground changes revoke pending source reads",
  async () => {
    for (const revoke of ["grant", "expiry", "foreground"]) {
      const f = fixture();
      let live = true;
      const reader = async (q: PortfolioQuery) => {
        const result = await f.reader(q);
        if (revoke === "grant") f.pairing.grants = [];
        if (revoke === "expiry") f.pairing.expiresAt = 0;
        if (revoke === "foreground") live = false;
        return result;
      };
      await expect(
        readGatewayHtmlResources(
          f.pairing,
          randomUUID(),
          f.runId,
          f.document,
          f.original,
          () => live,
          randomUUID,
          digest,
          reader,
        ),
      ).rejects.toThrow("scope changed");
    }
  },
);
