import { createHash } from "node:crypto";
import { createRemoteJWKSet, jwtVerify, type JWTVerifyGetKey } from "jose";
import type { CloudConfig } from "./config";

export class CloudFailure extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
  }
}

export interface Account {
  readonly workspaceId: string;
  readonly subject: string;
  readonly issuer: string;
  readonly scopes: ReadonlySet<string>;
}

export function createAuthenticator(
  config: CloudConfig,
  keys?: JWTVerifyGetKey,
) {
  const keySet =
    keys ??
    createRemoteJWKSet(new URL(".well-known/jwks.json", config.issuer), {
      timeoutDuration: 5000,
      cooldownDuration: 30_000,
      cacheMaxAge: 600_000,
    });
  const clients = new Set([config.desktopClientId, config.mobileClientId]);
  return async (
    request: Request,
    requiredScopes: string[] = [],
  ): Promise<Account> => {
    const match =
      /^Bearer ([A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+)$/i.exec(
        request.headers.get("authorization") ?? "",
      );
    if (!match || match[1]!.length > 16_384)
      throw new CloudFailure(401, "Sign in required");
    let account: Account;
    try {
      const { payload } = await jwtVerify(match[1]!, keySet, {
        issuer: config.issuer,
        audience: config.audience,
        algorithms: ["RS256"],
        requiredClaims: ["iss", "sub", "aud", "exp", "iat"],
        clockTolerance: 5,
      });
      // Auth0's regular profile uses azp; its RFC 9068 profile uses client_id.
      const client = payload.azp ?? payload.client_id;
      if (
        typeof client !== "string" ||
        !clients.has(client) ||
        (payload.azp !== undefined &&
          payload.client_id !== undefined &&
          payload.azp !== payload.client_id) ||
        typeof payload.sub !== "string" ||
        !payload.sub ||
        payload.sub.length > 512 ||
        payload.sub.endsWith("@clients") ||
        payload.gty === "client-credentials" ||
        typeof payload.iat !== "number" ||
        payload.iat > Date.now() / 1000 + 5 ||
        (payload.scope !== undefined && typeof payload.scope !== "string")
      ) {
        throw new Error("Not an Ion native user access token");
      }
      const workspaceId = createHash("sha256")
        .update(JSON.stringify([payload.iss, payload.sub]))
        .digest("hex");
      account = Object.freeze({
        workspaceId,
        subject: payload.sub,
        issuer: config.issuer,
        scopes: new Set(
          (payload.scope as string | undefined)?.split(/\s+/).filter(Boolean) ??
            [],
        ),
      });
    } catch {
      // Never return JWTs, provider errors or claim contents.
      throw new CloudFailure(401, "Sign in required");
    }
    if (requiredScopes.some((scope) => !account.scopes.has(scope))) {
      throw new CloudFailure(403, "Required account permission is missing");
    }
    return account;
  };
}
