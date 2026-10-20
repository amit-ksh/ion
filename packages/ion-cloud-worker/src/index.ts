import model from "../../../resources/speech/parakeet-110m.json";
import { Admission } from "../../ion-cloud/src/admission";
import { createAuthenticator, CloudFailure } from "../../ion-cloud/src/auth";
import { ionAuth0, validateConfig } from "../../ion-cloud/src/config";
import type { Env } from "./env";
export { AccountRelay } from "./account";
export { Platform } from "./platform";
function json(value: unknown, status = 200) {
  return Response.json(value, {
    status,
    headers: {
      "Cache-Control": "no-store",
      "X-Content-Type-Options": "nosniff",
      "Referrer-Policy": "no-referrer",
      ...(status === 401 ? { "WWW-Authenticate": "Bearer" } : {}),
    },
  });
}
// Cached per-isolate verifier; trust settings are validated before reuse.
let cached:
  | {
      signature: string;
      authenticate: ReturnType<typeof createAuthenticator>;
      admission: Admission;
    }
  | undefined;
function settings(env: Env) {
  const signature = JSON.stringify([env.PUBLIC_ORIGIN, env.MEMBERS_JSON]);
  if (cached?.signature !== signature)
    cached = {
      signature,
      authenticate: createAuthenticator(
        validateConfig({ ...ionAuth0, publicOrigin: env.PUBLIC_ORIGIN }),
      ),
      admission: new Admission(JSON.parse(env.MEMBERS_JSON ?? "[]")),
    };
  return cached;
}
export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    try {
      const { authenticate, admission } = settings(env),
        url = new URL(request.url);
      const contentSuffix = "." + env.CONTENT_DOMAIN;
      const contentId = url.hostname.endsWith(contentSuffix)
        ? url.hostname.slice(0, -contentSuffix.length)
        : "";
      if (url.protocol === "https:" && /^[a-f0-9]{32}$/.test(contentId))
        return env.PLATFORM.get(env.PLATFORM.idFromName("platform-v1")).fetch(
          request,
        );
      if (url.origin !== env.PUBLIC_ORIGIN)
        throw new CloudFailure(404, "Unknown service host");
      if (
        [...url.searchParams.keys()].some((k) =>
          /token|authorization|secret/i.test(k),
        )
      )
        throw new CloudFailure(
          400,
          "Credentials must be sent in request headers",
        );
      const modelManifestPath =
        "/models/" + model.filename.replace(/\.gguf$/, ".json");
      if (url.pathname === modelManifestPath) {
        if (!["GET", "HEAD"].includes(request.method))
          throw new CloudFailure(405, "Method not allowed");
        const body = JSON.stringify(model);
        return new Response(request.method === "HEAD" ? null : body, {
          headers: {
            "Content-Type": "application/json",
            "Content-Length": String(new TextEncoder().encode(body).length),
            "Cache-Control": "public, max-age=31536000, immutable",
            "X-Content-Type-Options": "nosniff",
          },
        });
      }
      if (url.pathname === "/models/" + model.filename) {
        if (!["GET", "HEAD"].includes(request.method))
          throw new CloudFailure(405, "Method not allowed");
        let object: R2Object | null;
        let body: ReadableStream | null = null;
        if (request.method === "HEAD")
          object = await env.OBJECTS.head("models/" + model.filename);
        else {
          const downloaded = await env.OBJECTS.get("models/" + model.filename);
          object = downloaded;
          body = downloaded?.body ?? null;
        }
        if (
          !object ||
          object.size !== model.bytes ||
          (object.customMetadata?.sha256 !== undefined &&
            object.customMetadata.sha256 !== model.sha256)
        )
          throw new CloudFailure(
            503,
            "The verified speech model has not been published yet",
          );
        return new Response(body, {
          headers: {
            "Content-Type": "application/octet-stream",
            "Content-Length": String(model.bytes),
            "Cache-Control": "public, max-age=31536000, immutable",
            "X-Content-Type-Options": "nosniff",
            "X-Expected-SHA256": model.sha256,
            Link: `<${env.PUBLIC_ORIGIN}${modelManifestPath}>; rel="describedby"; type="application/json", <${model.licenseUrl}>; rel="license"`,
          },
        });
      }

      if (url.pathname === "/v1/health" && request.method === "GET")
        return json({ status: "ok" });
      if (url.pathname === "/v1/bootstrap" && request.method === "GET") {
        const capabilities = await env.PLATFORM.get(env.PLATFORM.idFromName("platform-v1")).fetch(
          new Request(env.PUBLIC_ORIGIN + "/internal/platform/capabilities"));
        const ready = await capabilities.json() as { appDeployment: boolean; sharedInference: boolean };
        return json({
          version: 1,
          auth: {
            ...ionAuth0,
            scopes: [
              "openid",
              "profile",
              "email",
              "offline_access",
              "ion:gateway",
            ],
            flow: "authorization_code",
            pkce: "S256",
          },
          services: {
            gateway: env.PUBLIC_ORIGIN + "/v1/gateway",
            hosting: env.PUBLIC_ORIGIN + "/v1/hosting",
          },
          features: {
            gateway: true,
            speech: false,
            htmlPublishing: true,
            appDeployment: ready.appDeployment,
            sharedInference: ready.sharedInference,
          },
          limits: { hostingBundleBytes: 20_000_000, hostingSites: 10 },
        });
      }
      if (url.pathname === "/internal/compute/tick" || url.pathname.startsWith("/internal/hosting/"))
        return env.PLATFORM.get(env.PLATFORM.idFromName("platform-v1")).fetch(
          request,
        );
      const account = await authenticate(request, ["ion:gateway"]);
      admission.admit(account);
      if (url.pathname === "/v1/session" && request.method === "GET")
        return json({ workspaceId: account.workspaceId });
      // Strip caller identity hints; only the verified token can select durable state.
      const headers = new Headers(request.headers);
      headers.delete("x-ion-account-subject");
      headers.delete("x-ion-workspace");
      if (url.pathname.startsWith("/v1/gateway/")) {
        url.pathname = url.pathname.slice("/v1/gateway".length);
        return env.ACCOUNTS.get(
          env.ACCOUNTS.idFromName(account.workspaceId),
        ).fetch(new Request(new Request(url.toString(), request), { headers }));
      }
      if (
        ["/v1/services/controls", "/v1/services/usage"].includes(url.pathname) ||
        url.pathname.startsWith("/v1/inference/") ||
        url.pathname.startsWith("/v1/hosting/")
      ) {
        headers.set("x-ion-account-subject", account.subject);
        headers.set("x-ion-workspace", account.workspaceId);
        headers.delete("authorization");
        return env.PLATFORM.get(env.PLATFORM.idFromName("platform-v1")).fetch(
          new Request(request, { headers }),
        );
      }
      throw new CloudFailure(404, "Not found");
    } catch (error) {
      return json(
        {
          error:
            error instanceof CloudFailure
              ? error.message
              : "Request could not be completed",
        },
        error instanceof CloudFailure ? error.status : 503,
      );
    }
  },
} satisfies ExportedHandler<Env>;
