import { requestJson } from "./request";
/** Native transport foundation. Open authorizeUrl in the system browser.
 * The platform caller must keep returned tokens in its OS vault/SecureStore.
 * No client secret, embedded browser, localStorage, cookies or password API. */
export interface LoginConfig {
  issuer: string;
  audience: string;
  clientId: string;
  redirectUri: string;
  cloudOrigin: string;
}
export interface LoginCrypto {
  randomBytes(size: number): Promise<Uint8Array>;
  sha256(value: string): Promise<Uint8Array>;
}
export interface CloudSession {
  accessToken: string;
  refreshToken?: string;
  expiresAt: number;
  workspaceId: string;
}
export class LoginFailure extends Error {
  constructor() {
    super("Sign-in could not be completed. Please try again.");
  }
}
function encode(value: Uint8Array): string {
  const alphabet =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
  let result = "";
  for (let index = 0; index < value.length; index += 3) {
    const a = value[index]!,
      b = value[index + 1],
      c = value[index + 2];
    result += alphabet[a >> 2]! + alphabet[((a & 3) << 4) | ((b ?? 0) >> 4)]!;
    if (b !== undefined) result += alphabet[((b & 15) << 2) | ((c ?? 0) >> 6)]!;
    if (c !== undefined) result += alphabet[c & 63]!;
  }
  return result;
}
function httpsOrigin(value: string): URL {
  const url = new URL(value);
  if (
    url.protocol !== "https:" ||
    url.username ||
    url.password ||
    value !== url.origin
  )
    throw new LoginFailure();
  return url;
}
export function createNativeLogin(
  config: LoginConfig,
  crypto: LoginCrypto,
  send: typeof fetch = fetch,
  now: () => number = Date.now,
) {
  httpsOrigin(config.cloudOrigin);
  const issuer = httpsOrigin(config.issuer.replace(/\/$/, ""));
  if (
    config.issuer !== issuer.origin + "/" ||
    !config.audience ||
    config.audience === config.clientId ||
    !/^[A-Za-z0-9_-]{16,128}$/.test(config.clientId)
  ) {
    throw new LoginFailure();
  }
  const redirect = new URL(config.redirectUri);
  const loopback =
    redirect.protocol === "http:" &&
    ["127.0.0.1", "[::1]"].includes(redirect.hostname);
  if (
    redirect.search ||
    redirect.hash ||
    redirect.username ||
    redirect.password ||
    (!loopback && !["https:", "ion:", "waku:"].includes(redirect.protocol))
  )
    throw new LoginFailure();
  let pending: { state: string; verifier: string; at: number } | undefined;
  let generation = 0;
  let exchanging = false;
  const callbackBase = (url: URL) =>
    url.protocol + "//" + url.host + url.pathname;
  return {
    cancel: () => {
      generation++;
      pending = undefined;
    },
    begin: async (): Promise<string> => {
      const run = ++generation;
      pending = undefined;
      const [stateBytes, verifierBytes] = await Promise.all([
        crypto.randomBytes(32),
        crypto.randomBytes(32),
      ]);
      if (stateBytes.length !== 32 || verifierBytes.length !== 32)
        throw new LoginFailure();
      const state = encode(stateBytes),
        verifier = encode(verifierBytes);
      const challengeBytes = await crypto.sha256(verifier);
      if (challengeBytes.length !== 32 || generation !== run)
        throw new LoginFailure();
      pending = { state, verifier, at: now() };
      const url = new URL("authorize", config.issuer);
      url.search = new URLSearchParams({
        response_type: "code",
        prompt: "login",
        client_id: config.clientId,
        redirect_uri: config.redirectUri,
        audience: config.audience,
        scope: "openid profile email offline_access ion:gateway",
        code_challenge_method: "S256",
        code_challenge: encode(challengeBytes),
        state,
      }).toString();
      return url.toString();
    },
    finish: async (callback: string): Promise<CloudSession> => {
      const run = generation,
        login = pending;
      let url: URL;
      try {
        url = new URL(callback);
      } catch {
        throw new LoginFailure();
      }
      if (
        !login ||
        exchanging ||
        callbackBase(url) !== callbackBase(redirect) ||
        url.hash ||
        url.username ||
        url.password ||
        url.searchParams.getAll("state").length !== 1 ||
        url.searchParams.get("state") !== login.state
      ) {
        throw new LoginFailure();
      }
      pending = undefined; // Consume before any network operation: no replay or automatic retry.
      const code = url.searchParams.get("code");
      if (
        now() < login.at ||
        now() - login.at > 600_000 ||
        url.searchParams.has("error") ||
        url.searchParams.getAll("code").length !== 1 ||
        !code ||
        code.length > 4096
      ) {
        throw new LoginFailure();
      }
      exchanging = true;
      try {
        const tokens = await requestJson<Record<string, unknown>>(
          send,
          new URL("oauth/token", config.issuer),
          {
            method: "POST",
            redirect: "error",
            headers: { "Content-Type": "application/x-www-form-urlencoded" },
            body: new URLSearchParams({
              grant_type: "authorization_code",
              client_id: config.clientId,
              code_verifier: login.verifier,
              code,
              redirect_uri: config.redirectUri,
            }).toString(),
          },
        );
        if (
          typeof tokens.access_token !== "string" ||
          !tokens.access_token ||
          tokens.access_token.length > 16_384 ||
          tokens.token_type !== "Bearer" ||
          typeof tokens.expires_in !== "number" ||
          !Number.isSafeInteger(tokens.expires_in) ||
          tokens.expires_in <= 0 ||
          tokens.expires_in > 31_536_000 ||
          (tokens.refresh_token !== undefined &&
            (typeof tokens.refresh_token !== "string" ||
              !tokens.refresh_token ||
              tokens.refresh_token.length > 16_384))
        )
          throw new LoginFailure();
        // Identity comes from the API's validated access token, never an unverified ID token.
        const session = await requestJson<{ workspaceId?: unknown }>(
          send,
          new URL("/v1/session", config.cloudOrigin),
          {
            redirect: "error",
            headers: { Authorization: "Bearer " + tokens.access_token },
          },
        );
        if (
          typeof session.workspaceId !== "string" ||
          !/^[a-f0-9]{64}$/.test(session.workspaceId) ||
          run !== generation
        )
          throw new LoginFailure();
        return {
          accessToken: tokens.access_token,
          refreshToken: tokens.refresh_token as string | undefined,
          expiresAt: now() + tokens.expires_in * 1000,
          workspaceId: session.workspaceId,
        };
      } catch {
        throw new LoginFailure();
      } finally {
        exchanging = false;
      }
    },
  };
}
