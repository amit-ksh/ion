import { Admission } from "./admission";
import { readFileSync } from "node:fs";
import { ionAuth0 } from "./config";
import { createCloudHandler } from "./http";
import type { SpeechSocketData } from "./speech";
if (process.platform !== "win32") process.umask(0o077);
const publicOrigin = process.env.ION_CLOUD_PUBLIC_ORIGIN,
  stateDirectory = process.env.ION_CLOUD_STATE_DIRECTORY;
if (!publicOrigin || !stateDirectory)
  throw new Error("Set ION_CLOUD_PUBLIC_ORIGIN and ION_CLOUD_STATE_DIRECTORY");
const port = Number(process.env.ION_CLOUD_PORT ?? 8790);
if (!Number.isInteger(port) || port < 1 || port > 65535)
  throw new Error("Invalid cloud port");
const privateValue = (name: string) => {
  const path = process.env[name + "_FILE"];
  return path ? readFileSync(path, "utf8").trim() : process.env[name];
};
const workerToken = privateValue("ION_HOSTING_WORKER_TOKEN");
const memberFile = process.env.ION_CLOUD_MEMBERS_FILE;
if (!memberFile) throw new Error("Set ION_CLOUD_MEMBERS_FILE to the approved owner/judge list");
const admission = new Admission(JSON.parse(readFileSync(memberFile, "utf8")));
const handler = createCloudHandler({
  config: { ...ionAuth0, publicOrigin },
  stateDirectory,
  admission,
  workspaceLimit: Number(process.env.ION_CLOUD_WORKSPACE_LIMIT ?? 64),
  ...(process.env.ION_CONTENT_DOMAIN
    ? {
        hosting: {
          contentDomain: process.env.ION_CONTENT_DOMAIN,
          workerToken,
          workerOrigin: process.env.ION_HOSTING_WORKER_ORIGIN,
          secretsKey: privateValue("ION_HOSTING_SECRETS_KEY"),
        },
      }
    : {}),
});
const server = Bun.serve<SpeechSocketData>({
  hostname: "127.0.0.1",
  port,
  maxRequestBodySize: 40_000_000,
  fetch: (request, server) =>
    request.headers.get("upgrade")?.toLowerCase() === "websocket"
      ? handler.upgrade(request, server)
      : handler.fetch(request),
  websocket: handler.websocket,
  error: () => Response.json({ error: "Request rejected" }, { status: 500 }),
});
console.log("Ion Cloud listening on " + server.url.origin);
let stopping = false;
for (const signal of ["SIGINT", "SIGTERM"] as const)
  process.on(signal, () => {
    if (stopping) return;
    stopping = true;
    void (async () => {
      await handler.close();
      await server.stop();
    })();
  });
