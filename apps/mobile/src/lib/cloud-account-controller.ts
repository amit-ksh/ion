import {
  requestJson,
  requestAccepted,
} from "../../../../packages/ion-cloud-client/src/request";
import { createNativeLogin } from "../../../../packages/ion-cloud-client/src/login";
import { createAccountSession } from "../../../../packages/ion-cloud-client/src/session";
import { ionAuth0 } from "../../../../packages/ion-cloud-client/src/public-config";
import type { CloudAccountActions } from "./cloud-account.types";
import type { GatewayPairing } from "./gateway-client";

export function createCloudAccount(options: {
  origin: string;
  vault: import("../../../../packages/ion-cloud-client/src/session").SessionVault;
  crypto: import("../../../../packages/ion-cloud-client/src/login").LoginCrypto;
  openBrowser(
    url: string,
    callback: string,
  ): Promise<{ type: string; url?: string }>;
  readPairing(): Promise<GatewayPairing | null>;
  writePairing(pairing: GatewayPairing): Promise<void>;
  deletePairing(): Promise<void>;
  clearTasks(): Promise<void>;
  send?: typeof fetch;
  cloudServicesEnabled?: boolean;
}) {
  const origin = options.origin;
  const send = options.send ?? fetch;
  const {
    readPairing: readGatewayPairing,
    writePairing: writeGatewayPairing,
    deletePairing: deleteGatewayPairing,
    clearTasks: clearCachedGatewayTasks,
  } = options;
  const redirectUri = "ion://auth/callback";
  const listeners = new Set<() => void>();
  const signedOut = new Set<() => void>();
  const emit = () => {
    for (const listener of listeners) {
      try {
        listener();
      } catch {}
    }
  };
  let busy = false;

  function configuredOrigin(): string {
    const url = new URL(origin);
    if (
      url.protocol !== "https:" ||
      url.origin !== origin ||
      url.username ||
      url.password
    )
      throw new Error("Ion Cloud is not configured in this build.");
    return origin;
  }
  const config = {
    ...ionAuth0,
    clientId: ionAuth0.mobileClientId,
    redirectUri,
    cloudOrigin: origin,
  };
  let session: ReturnType<typeof createAccountSession> | undefined;
  function account() {
    configuredOrigin();
    if (!session) {
      session = createAccountSession(config, options.vault, send);
      session.subscribe(emit);
    }
    return session;
  }
  async function authorize(target: string, workspaceId: string) {
    if (target !== configuredOrigin())
      throw new Error("The saved gateway belongs to another cloud service.");
    return (await account().authorization(workspaceId)).token;
  }
  async function ownerCall(path: string, body?: unknown) {
    const auth = await account().authorization();
    return requestJson<Record<string, unknown>>(
      send,
      origin + "/v1/gateway" + path,
      {
        method: body === undefined ? "GET" : "POST",
        redirect: "error",
        headers: {
          Authorization: "Bearer " + auth.token,
          "Content-Type": "application/json",
        },
        body: body === undefined ? undefined : JSON.stringify(body),
      },
    );
  }
  const actions: CloudAccountActions = {
    async request(path, method = "GET", body) {
      if (options.cloudServicesEnabled === false && (path.startsWith('/v1/services/') || path.startsWith('/v1/hosting/') || path === '/v1/speech/sessions'))
        throw new Error('Cloud services are disabled in this app.');
      if (
        !(
          (path === "/v1/bootstrap" && method === "GET") ||
          (path === "/v1/speech/sessions" && method === "POST")
        ) ||
        /[?#\\]/.test(path) ||
        path.split("/").some((v) => v === "." || v === "..")
      )
        throw new Error("Invalid managed service request");
      const auth = await account().authorization();
      return requestJson<any>(send, configuredOrigin() + path, {
        method,
        redirect: "error",
        headers: {
          Authorization: "Bearer " + auth.token,
          "Content-Type": "application/json",
        },
        body: body === undefined ? undefined : JSON.stringify(body),
      });
    },
    async serviceRequest(workspaceId, path, method = "GET", body) {
      if (
        !/^[a-f0-9]{64}$/.test(workspaceId) ||
        !(
          (path === "/v1/services/controls" &&
            ["GET", "PUT"].includes(method)) ||
          (path === "/v1/services/usage" && method === "GET") ||
          (["/v1/apps/catalog", "/v1/apps/connect", "/v1/apps/disconnect"].includes(path) && method === "POST")
        )
      )
        throw new Error("Invalid account-bound service operation");
      const auth = await account().authorization(workspaceId);
      return requestJson<any>(send, configuredOrigin() + path, {
        method,
        redirect: "error",
        headers: {
          Authorization: "Bearer " + auth.token,
          "Content-Type": "application/json",
        },
        body: body === undefined ? undefined : JSON.stringify(body),
      });
    },
    async hostingRequest(workspaceId, path, method = "GET", body) {
      if (
        !/^[a-f0-9]{64}$/.test(workspaceId) ||
        !["GET", "POST", "DELETE"].includes(method) ||
        !/^\/v1\/hosting\/(?:[A-Za-z0-9_-]+\/)*[A-Za-z0-9_-]+$/.test(path) ||
        path.length > 240
      )
        throw new Error("Invalid account-bound publishing operation");
      const auth = await account().authorization(workspaceId);
      return requestJson<any>(send, configuredOrigin() + path, {
        method,
        redirect: "error",
        headers: {
          Authorization: "Bearer " + auth.token,
          "Content-Type": "application/json",
        },
        body: body === undefined ? undefined : JSON.stringify(body),
      });
    },
    async verifySession() {
      const current = await actions.status();
      if (!current.available || !current.signedIn) return current;
      const auth = await account().authorization();
      const identity = await requestJson<{ workspaceId?: unknown }>(
        send,
        configuredOrigin() + "/v1/session",
        {
          redirect: "error",
          headers: { Authorization: "Bearer " + auth.token },
        },
      );
      if (identity.workspaceId !== auth.workspaceId)
        throw new Error("Account verification failed. Sign in again.");
      return { ...current, signedIn: true, workspaceId: auth.workspaceId };
    },
    async status() {
      if (!origin) return { available: false, signedIn: false };
      try {
        configuredOrigin();
      } catch {
        return { available: false, signedIn: false };
      }
      try {
        const state = await account().status();
        const pairing = await readGatewayPairing();
        return {
          available: true,
          ...state,
          controlsEnabled:
            !!pairing?.cloudWorkspaceId &&
            pairing.grants.includes("taskControl"),
          phoneConnected:
            !!state.workspaceId &&
            pairing?.cloudWorkspaceId === state.workspaceId &&
            pairing.baseUrl === origin &&
            pairing.expiresAt > Date.now(),
        };
      } catch {
        return {
          available: true,
          signedIn: false,
          problem:
            "Unlock this phone and try again to read your saved account.",
        };
      }
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    onSignOut(listener) {
      signedOut.add(listener);
      return () => {
        signedOut.delete(listener);
      };
    },
    async signIn() {
      if (busy) return false;
      configuredOrigin();
      const login = createNativeLogin(config, options.crypto, send);
      busy = true;
      try {
        const previous = await account().status();
        configuredOrigin();
        const meta = await requestJson<any>(send, origin + "/v1/bootstrap");
        if (
          meta?.auth?.issuer !== ionAuth0.issuer ||
          meta?.auth?.audience !== ionAuth0.audience ||
          meta?.auth?.mobileClientId !== ionAuth0.mobileClientId ||
          meta?.auth?.pkce !== "S256" ||
          meta?.services?.gateway !== origin + "/v1/gateway"
        )
          throw new Error();
        const authorize = await login.begin();
        const result = await options.openBrowser(authorize, redirectUri);
        if (result.type !== "success") {
          login.cancel();
          return false;
        }
        const next = await login.finish(result.url ?? "");
        const pairing = await readGatewayPairing();
        const foreignPairing =
          pairing?.cloudWorkspaceId &&
          pairing.cloudWorkspaceId !== next.workspaceId;
        if (foreignPairing) await deleteGatewayPairing();
        if (
          foreignPairing ||
          (previous.workspaceId && previous.workspaceId !== next.workspaceId)
        ) {
          await clearCachedGatewayTasks();
          for (const listener of signedOut) {
            try {
              listener();
            } catch {}
          }
        }
        await account().save(next);
        return true;
      } catch {
        throw new Error(
          "Could not sign in. Check your connection and try again.",
        );
      } finally {
        login.cancel();
        busy = false;
      }
    },
    async connectPhone(controls) {
      if (busy)
        throw new Error("Wait for the current account action to finish.");
      busy = true;
      try {
        const existing = await readGatewayPairing();
        const state = await account().status();
        if (!state.workspaceId) throw new Error("Sign in first.");
        if (existing && !existing.cloudWorkspaceId)
          throw new Error(
            "Remove the personal gateway pairing before connecting Ion Cloud.",
          );
        if (
          existing &&
          existing.baseUrl === origin &&
          existing.cloudWorkspaceId === state.workspaceId
        ) {
          // Replace only after the old device has been explicitly revoked. A lost
          // response requires owner review; pairing is never silently replayed.
          await ownerCall("/v1/owner/devices/" + existing.id + "/revoke", {});
        }
        const grants = [
          "inspect",
          "taskRead",
          "automationRead",
          "inboxRead",
          "usageRead",
          "evidenceRead",
          ...(controls ? ["review", "taskControl", "automationControl"] : []),
        ];
        const invitation = await ownerCall("/v1/owner/invites", { grants });
        const paired = await ownerCall("/v1/pair", {
          invite: invitation.invite,
          name: "ion mobile",
        });
        if (
          typeof paired.id !== "string" ||
          !/^[0-9a-f-]{36}$/.test(paired.id) ||
          typeof paired.token !== "string" ||
          !/^[A-Za-z0-9_-]{20,256}$/.test(paired.token) ||
          typeof paired.expiresAt !== "number" ||
          paired.expiresAt <= Date.now() ||
          !Array.isArray(paired.grants) ||
          paired.grants.some(
            (g: unknown) => typeof g !== "string" || !grants.includes(g),
          )
        )
          throw new Error("Cloud returned an invalid device grant.");
        await clearCachedGatewayTasks();
        await writeGatewayPairing({
          ...paired,
          baseUrl: origin,
          cloudWorkspaceId: state.workspaceId,
        } as unknown as GatewayPairing);
        emit();
      } finally {
        busy = false;
      }
    },
    async signOut() {
      if (busy)
        throw new Error("Wait for the current account action to finish.");
      busy = true;
      try {
        let pairing: GatewayPairing | null = null;
        try {
          pairing = await readGatewayPairing();
        } catch {}
        const reset = async () => {
          if (pairing?.cloudWorkspaceId) {
            await deleteGatewayPairing();
            await clearCachedGatewayTasks();
          }
          for (const listener of signedOut) {
            try {
              listener();
            } catch {}
          }
          emit();
        };
        if (!origin) {
          await options.vault.remove();
          await reset();
          return { revoked: false };
        }
        return await account().signOut(async (auth) => {
          await reset();
          if (!pairing?.cloudWorkspaceId) return true;
          if (
            !auth ||
            pairing.baseUrl !== origin ||
            pairing.cloudWorkspaceId !== auth.workspaceId
          )
            return false;
          return await requestAccepted(
            send,
            origin + "/v1/gateway/v1/owner/devices/" + pairing.id + "/revoke",
            {
              method: "POST",
              redirect: "error",
              headers: {
                Authorization: "Bearer " + auth.token,
                "Content-Type": "application/json",
              },
              body: "{}",
            },
          );
        });
      } finally {
        busy = false;
      }
    },
  };

  return { ...actions, authorize };
}
