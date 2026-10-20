import { requestJson, requestAccepted } from "./request";
import type { CloudSession, LoginConfig } from "./login";

export interface SessionVault {
  read(): Promise<string | null>;
  write(value: string): Promise<void>;
  remove(): Promise<void>;
}
export interface AccountStatus {
  signedIn: boolean;
  workspaceId?: string;
}
export class SessionFailure extends Error {
  constructor() {
    super("Sign in to Ion Cloud again to continue.");
  }
}
interface SavedSession extends CloudSession {
  version: 1;
  origin: string;
  issuer: string;
  audience: string;
  clientId: string;
  refreshing: boolean;
}

/** Serializes vault mutations and rotating refreshes. A durable in-flight
 * marker prevents refresh-token replay after a crash or ambiguous response. */
export function createAccountSession(
  config: LoginConfig,
  vault: SessionVault,
  send: typeof fetch = fetch,
  now: () => number = Date.now,
) {
  config = Object.freeze({ ...config });
  const origin = new URL(config.cloudOrigin);
  if (origin.protocol !== "https:" || config.cloudOrigin !== origin.origin)
    throw new SessionFailure();
  const issuer = new URL(config.issuer);
  if (
    issuer.protocol !== "https:" ||
    config.issuer !== issuer.origin + "/" ||
    !config.audience ||
    config.audience === config.clientId ||
    !/^[A-Za-z0-9_-]{16,128}$/.test(config.clientId)
  )
    throw new SessionFailure();
  let state: SavedSession | null | undefined;
  let tail: Promise<unknown> = Promise.resolve();
  let generation = 0;
  const listeners = new Set<() => void>();
  const notify = () => {
    for (const listener of listeners) {
      try {
        listener();
      } catch {
        /* Observers cannot change credential outcomes. */
      }
    }
  };
  const serial = <T>(work: () => Promise<T>): Promise<T> => {
    const pending = tail.then(work);
    tail = pending.catch(() => {});
    return pending;
  };
  function valid(value: unknown): value is SavedSession {
    if (!value || typeof value !== "object") return false;
    const v = value as SavedSession;
    return (
      v.version === 1 &&
      v.origin === config.cloudOrigin &&
      v.issuer === config.issuer &&
      v.audience === config.audience &&
      v.clientId === config.clientId &&
      /^[a-f0-9]{64}$/.test(v.workspaceId) &&
      typeof v.accessToken === "string" &&
      v.accessToken.length > 0 &&
      v.accessToken.length <= 16_384 &&
      Number.isSafeInteger(v.expiresAt) &&
      v.expiresAt > 0 &&
      typeof v.refreshing === "boolean" &&
      (v.refreshToken === undefined ||
        (typeof v.refreshToken === "string" &&
          v.refreshToken.length > 0 &&
          v.refreshToken.length <= 16_384))
    );
  }
  async function clear() {
    state = null;
    try {
      await vault.remove();
    } finally {
      notify();
    }
  }
  async function load() {
    if (state !== undefined) return state;
    const raw = await vault.read();
    if (raw === null) {
      state = null;
      return null;
    }
    let parsed: unknown;
    try {
      parsed = JSON.parse(raw);
    } catch {
      parsed = null;
    }
    if (!valid(parsed) || parsed.refreshing) {
      await clear();
      return null;
    }
    state = parsed;
    return state;
  }
  async function refresh(current: SavedSession, run: number) {
    if (!current.refreshToken) {
      await clear();
      throw new SessionFailure();
    }
    try {
      // Persist before sending. A restarted process must demand a new login.
      await vault.write(JSON.stringify({ ...current, refreshing: true }));
      if (generation !== run) throw new SessionFailure();
      const tokens = await requestJson<Record<string, unknown>>(
        send,
        new URL("oauth/token", config.issuer),
        {
          method: "POST",
          redirect: "error",
          headers: { "Content-Type": "application/x-www-form-urlencoded" },
          body: new URLSearchParams({
            grant_type: "refresh_token",
            client_id: config.clientId,
            refresh_token: current.refreshToken,
          }).toString(),
        },
      );
      if (
        typeof tokens.access_token !== "string" ||
        !tokens.access_token ||
        tokens.access_token.length > 16_384 ||
        tokens.token_type !== "Bearer" ||
        !Number.isSafeInteger(tokens.expires_in) ||
        (tokens.expires_in as number) <= 0 ||
        (tokens.expires_in as number) > 31_536_000 ||
        typeof tokens.refresh_token !== "string" ||
        !tokens.refresh_token ||
        tokens.refresh_token.length > 16_384
      ) {
        throw new SessionFailure();
      }
      const identity = await requestJson<{ workspaceId?: unknown }>(
        send,
        new URL("/v1/session", config.cloudOrigin),
        {
          redirect: "error",
          headers: { Authorization: "Bearer " + tokens.access_token },
        },
      );
      if (identity.workspaceId !== current.workspaceId || generation !== run)
        throw new SessionFailure();
      const next: SavedSession = {
        ...current,
        accessToken: tokens.access_token,
        refreshToken: tokens.refresh_token,
        expiresAt: now() + (tokens.expires_in as number) * 1000,
        refreshing: false,
      };
      await vault.write(JSON.stringify(next));
      if (generation !== run) throw new SessionFailure();
      state = next;
      notify();
      return next;
    } catch {
      await clear();
      throw new SessionFailure();
    }
  }
  return {
    subscribe(listener: () => void) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    status: (): Promise<AccountStatus> =>
      serial(async () => {
        const saved = await load();
        return saved
          ? { signedIn: true, workspaceId: saved.workspaceId }
          : { signedIn: false };
      }),
    save: (session: CloudSession): Promise<void> => {
      const run = generation;
      return serial(async () => {
        const saved: SavedSession = {
          ...session,
          version: 1,
          origin: config.cloudOrigin,
          issuer: config.issuer,
          audience: config.audience,
          clientId: config.clientId,
          refreshing: false,
        };
        if (!valid(saved) || run !== generation || saved.expiresAt <= now())
          throw new SessionFailure();
        await vault.write(JSON.stringify(saved));
        if (run !== generation) {
          await clear();
          throw new SessionFailure();
        }
        state = saved;
        notify();
      });
    },
    authorization: (
      expectedWorkspaceId?: string,
    ): Promise<{ token: string; workspaceId: string }> => {
      const run = generation;
      return serial(async () => {
        let saved = await load();
        if (
          !saved ||
          run !== generation ||
          (expectedWorkspaceId && saved.workspaceId !== expectedWorkspaceId)
        )
          throw new SessionFailure();
        if (saved.expiresAt <= now() + 30_000)
          saved = await refresh(saved, run);
        if (run !== generation) throw new SessionFailure();
        return { token: saved.accessToken, workspaceId: saved.workspaceId };
      });
    },
    signOut: (
      revokeDevice?: (
        auth: { token: string; workspaceId: string } | null,
      ) => Promise<boolean>,
    ): Promise<{ revoked: boolean }> => {
      generation++; // Invalidate active refresh/save before waiting for the queue.
      return serial(async () => {
        const saved = await load();
        await clear(); // Local sign-out completes even when the provider is offline.
        let deviceRevoked = true;
        if (revokeDevice) {
          try {
            deviceRevoked = await revokeDevice(
              saved
                ? { token: saved.accessToken, workspaceId: saved.workspaceId }
                : null,
            );
          } catch {
            deviceRevoked = false;
          }
        }
        if (!saved?.refreshToken) return { revoked: deviceRevoked };
        try {
          const accepted = await requestAccepted(
            send,
            new URL("oauth/revoke", config.issuer),
            {
              method: "POST",
              redirect: "error",
              headers: { "Content-Type": "application/x-www-form-urlencoded" },
              body: new URLSearchParams({
                client_id: config.clientId,
                token: saved.refreshToken,
                token_type_hint: "refresh_token",
              }).toString(),
            },
          );
          return { revoked: accepted && deviceRevoked };
        } catch {
          return { revoked: false };
        }
      });
    },
  };
}
