import { Controls } from "./controls";
import { Admission } from "./admission";
import type { JWTVerifyGetKey } from "jose";
import type { Server } from "bun";
import { createAuthenticator, CloudFailure } from "./auth";
import { validateConfig, type CloudConfig } from "./config";
import { ManagedGateways } from "./gateway";
import { ServiceStore } from "./service-store";
import {
  ManagedSpeech,
  type SpeechOptions,
  type SpeechSocketData,
} from "./speech";
import { Hosting, type HostingOptions } from "./hosting";

export function createCloudHandler(options: {
  config: CloudConfig;
  stateDirectory: string;
  keys?: JWTVerifyGetKey;
  admission?: Admission;
  workspaceLimit?: number;
  speech?: SpeechOptions;
  hosting?: HostingOptions;
}) {
  const config = validateConfig(options.config),
    authenticate = createAuthenticator(config, options.keys);
  const gateways = new ManagedGateways(
    options.stateDirectory,
    options.workspaceLimit,
  );
  const store = new ServiceStore(options.stateDirectory),
    speech = new ManagedSpeech(store, config.publicOrigin, options.speech);
  const controls = options.admission ? new Controls(store, options.admission) : undefined;
  const hosting = new Hosting(store, options.stateDirectory, options.hosting, controls?.computeEnabled);
  if (
    options.hosting &&
    (new URL(config.publicOrigin).hostname === options.hosting.contentDomain ||
      new RegExp("^[a-f0-9]{32}\\." + options.hosting.contentDomain.replaceAll(".", "\\.") + "$").test(new URL(config.publicOrigin).hostname))
  )
    throw new Error("Content and account origins must be separate");
  const canonical = (request: Request) => {
    const url = new URL(request.url),
      apiHost = new URL(config.publicOrigin).host;
    // The listener is loopback-only behind a same-host TLS proxy. Match the
    // configured Host; never use forwarded identity or arbitrary origins.
    if (
      url.protocol === "http:" &&
      (url.host === apiHost ||
        (options.hosting &&
          url.hostname.endsWith("." + options.hosting.contentDomain)))
    ) {
      url.protocol = "https:";
      return new Request(url.toString(), request);
    }
    return request;
  };
  let closed = false;
  const json = (value: unknown, status = 200) =>
    Response.json(value, {
      status,
      headers: {
        "Cache-Control": "no-store",
        "X-Content-Type-Options": "nosniff",
        "Referrer-Policy": "no-referrer",
        ...(status === 401
          ? { "WWW-Authenticate": 'Bearer realm="ion-cloud"' }
          : {}),
      },
    });
  const failure = (error: unknown) =>
    error instanceof CloudFailure
      ? json({ error: error.message }, error.status)
      : json({ error: "Request could not be completed" }, 503);
  const active = new Set<Promise<Response>>();
  const handle = async (request: Request): Promise<Response> => {
    try {
      if (closed) throw new CloudFailure(503, "Service is stopping");
      request = canonical(request);
      const url = new URL(request.url);
      const content = await hosting.content(request);
      if (content) return content;
      if (url.origin !== config.publicOrigin)
        throw new CloudFailure(404, "Unknown service host");
      if (
        [...url.searchParams.keys()].some((key) =>
          /token|authorization|secret/i.test(key),
        )
      )
        throw new CloudFailure(
          400,
          "Credentials must be sent in request headers",
        );
      if (url.pathname === "/v1/health" && request.method === "GET")
        return json({ status: "ok" });
      if (url.pathname === "/v1/bootstrap" && request.method === "GET")
        return json({
          version: 1,
          auth: {
            issuer: config.issuer,
            audience: config.audience,
            desktopClientId: config.desktopClientId,
            mobileClientId: config.mobileClientId,
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
            gateway: config.publicOrigin + "/v1/gateway",
            ...(speech.enabled ? { speech: config.publicOrigin } : {}),
            ...(hosting.enabled
              ? { hosting: config.publicOrigin + "/v1/hosting" }
              : {}),
          },
          features: {
            gateway: true,
            speech: speech.enabled,
            htmlPublishing: hosting.enabled,
            appDeployment: hosting.enabled,
          },
          limits: {
            speechMaximumSeconds: 120,
            speechDailySeconds: options.speech?.dailySeconds ?? 3600,
            hostingBundleBytes: 20_000_000,
            hostingSites: 10,
          },
        });
      if (url.pathname.startsWith("/internal/hosting/"))
        return await hosting.worker(request);
      if (["/v1/models", "/v1/audio/transcriptions"].includes(url.pathname))
        return await speech.handle(request);
      if (
        !["/v1/session", "/v1/speech/sessions", "/v1/services/controls"].includes(url.pathname) &&
        !url.pathname.startsWith("/v1/gateway/") &&
        !url.pathname.startsWith("/v1/hosting/")
      )
        throw new CloudFailure(404, "Not found");
      const account = await authenticate(request, ["ion:gateway"]);
      options.admission?.admit(account);
      if (url.pathname === "/v1/services/controls") {
        if (!controls) throw new CloudFailure(503, "Owner controls require closed admission");
        return await controls.handle(account, request);
      }
      if (url.pathname === "/v1/session" && request.method === "GET")
        return json({ workspaceId: account.workspaceId });
      if (url.pathname.startsWith("/v1/gateway/"))
        return await gateways.handle(
          account,
          request,
          url.pathname.slice("/v1/gateway".length),
        );
      if (
        url.pathname === "/v1/speech/sessions" ||
        url.pathname.startsWith("/v1/hosting/")
      )
        store.admit(account.workspaceId);
      if (url.pathname === "/v1/speech/sessions")
        return await speech.handle(request, account);
      if (url.pathname.startsWith("/v1/hosting/"))
        return await hosting.handle(account, request);
      throw new CloudFailure(405, "Method not allowed");
    } catch (error) {
      return failure(error);
    }
  };
  const cleanup = setInterval(() => store.expire(), 60_000);
  cleanup.unref();
  return {
    websocket: speech.websocket,
    upgrade(
      request: Request,
      server: Server<SpeechSocketData>,
    ): Response | undefined {
      try {
        if (closed) throw new CloudFailure(503, "Service stopping");
        const url = new URL(canonical(request).url);
        if (
          url.origin !== config.publicOrigin ||
          url.pathname !== "/v1/audio/transcriptions/realtime" ||
          url.search
        )
          throw new CloudFailure(404, "Not found");
        return speech.upgrade(request, server)
          ? undefined
          : json({ error: "Could not open speech" }, 400);
      } catch (error) {
        return failure(error);
      }
    },
    close: async () => {
      closed = true;
      clearInterval(cleanup);
      speech.close();
      await Promise.allSettled([...active]);
      await gateways.close();
      store.close();
    },
    fetch: (request: Request): Promise<Response> => {
      if (closed)
        return Promise.resolve(json({ error: "Service is stopping" }, 503));
      const pending = handle(request);
      active.add(pending);
      return pending.finally(() => active.delete(pending));
    },
  };
}
