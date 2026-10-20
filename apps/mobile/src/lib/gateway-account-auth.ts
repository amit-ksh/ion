/** Installed by the native account adapter. Isolated from Expo for request tests. */
export type GatewayAccountAuthorization = (
  origin: string,
  workspaceId: string,
) => Promise<string>;
let authorization: GatewayAccountAuthorization | undefined;
export function configureGatewayAccountAuthorization(
  provider?: GatewayAccountAuthorization,
): void {
  authorization = provider;
}
export async function gatewayAccountToken(
  origin: string,
  workspaceId: string,
): Promise<string> {
  if (!authorization || !/^[a-f0-9]{64}$/.test(workspaceId))
    throw new Error("Sign in to Ion Cloud to reconnect this phone.");
  return authorization(origin, workspaceId);
}
