import { cloudAccount } from "./cloud-account";
import * as Crypto from "expo-crypto";
import { fromByteArray } from "base64-js";
import type { ArtifactLibraryItem, WakuClient } from "@waku/client";
import { readArtifactHtmlResources } from "./html-resource-readers";
export interface HostedRelease {
  id: string;
  status: string;
  created: number;
  logs: string;
}
export interface HostedSite {
  /** Local account scope captured when this site was loaded. */
  workspaceId: string;
  id: string;
  sourceId: string;
  title: string;
  visibility: "private" | "public";
  active: string | null;
  stopped: boolean;
  url: string;
  releases: HostedRelease[];
}
export async function hostingSites(workspaceId: string): Promise<HostedSite[]> {
  const value = await cloudAccount.hostingRequest(
    workspaceId,
    "/v1/hosting/sites",
  );
  if (!Array.isArray(value.items)) throw new Error("Invalid deployment list");
  return value.items.map((site: Omit<HostedSite, "workspaceId">) => ({
    ...site,
    workspaceId,
  }));
}
export async function hostingAction(
  site: HostedSite,
  action: "view" | "stop" | "start" | "visibility" | "rollback" | "logs",
  body: unknown = {},
) {
  if (!/^[a-f0-9]{32}$/.test(site.id)) throw new Error("Invalid deployment");
  return cloudAccount.hostingRequest(
    site.workspaceId,
    "/v1/hosting/sites/" + site.id + "/" + action,
    "POST",
    body,
  );
}
const digest = async (bytes: Uint8Array) =>
  Array.from(
    new Uint8Array(
      await Crypto.digest(
        Crypto.CryptoDigestAlgorithm.SHA256,
        bytes as Uint8Array<ArrayBuffer>,
      ),
    ),
    (b) => b.toString(16).padStart(2, "0"),
  ).join("");
export async function prepareHtmlPublication(
  client: WakuClient,
  document: ArtifactLibraryItem,
  bytes: Uint8Array,
  current: () => boolean,
) {
  if (
    !current() ||
    document.format !== "html" ||
    (await digest(bytes)) !== document.sha256
  )
    throw new Error("Refresh the verified HTML document first");
  const resources = await readArtifactHtmlResources(
    client,
    document,
    bytes,
    current,
    digest,
  );
  const sources = resources
    ? resources.manifest.sources.map((source, index) => ({
        path: source.path,
        data: resources.sources[index]!,
        sha256: source.sha256,
      }))
    : [{ path: "index.html", data: bytes, sha256: document.sha256 }];
  return {
    sourceId: "artifact-" + document.id,
    title: document.name,
    kind: "artifact",
    idempotencyKey: Crypto.randomUUID(),
    settings: {
      runtime: "document",
      entry: resources?.manifest.entryPath ?? "index.html",
      build: [],
      start: [],
      outputDirectory: "dist",
      port: 3000,
      healthPath: "/",
    },
    files: sources.map((source) => ({
      path: source.path,
      bytes: source.data.length,
      sha256: source.sha256,
      content: fromByteArray(source.data),
    })),
  };
}

export async function removeHostedVersion(
  site: HostedSite,
  release: HostedRelease,
) {
  if (
    site.active === release.id ||
    !/^[a-f0-9]{32}$/.test(site.id) ||
    !/^[A-Za-z0-9_-]{32}$/.test(release.id)
  )
    throw new Error("Choose an inactive finished version");
  return cloudAccount.hostingRequest(
    site.workspaceId,
    "/v1/hosting/sites/" + site.id + "/versions/" + release.id,
    "DELETE",
  );
}
