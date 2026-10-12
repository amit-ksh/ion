import { portfolioId, type GatewayRemoteExecution } from "@waku/client";
import type { GatewayPairing } from "./gateway-client";
export function remoteTaskDestination(
  row: GatewayRemoteExecution,
  owner: string,
  pairing: GatewayPairing,
) {
  if (
    !portfolioId(owner) ||
    !portfolioId(row.rootTaskId) ||
    !portfolioId(row.projectId) ||
    pairing.expiresAt <= Date.now() ||
    !pairing.grants.includes("usageRead") ||
    !pairing.grants.includes("taskRead")
  )
    throw new Error("Pair with Read tasks to open the original team task");
  return {
    pathname: "/gateway-task/[id]" as const,
    params: { id: row.rootTaskId, owner, project: row.projectId },
  };
}
/** Capture the entire pairing identity; the same phone ID after replacement is a new scope. */
export function gatewayPairingIdentity(pairing: GatewayPairing | null): string {
  return pairing
    ? JSON.stringify([
        pairing.baseUrl,
        pairing.id,
        pairing.token,
        pairing.expiresAt,
        [...pairing.grants].sort(),
      ])
    : "";
}
