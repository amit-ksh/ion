export interface CloudConfig {
  issuer: string;
  audience: string;
  desktopClientId: string;
  mobileClientId: string;
  publicOrigin: string;
}

export { ionAuth0 } from "../../ion-cloud-client/src/public-config";

export function validateConfig(config: CloudConfig): CloudConfig {
  const issuer = new URL(config.issuer);
  if (
    issuer.protocol !== "https:" ||
    issuer.pathname !== "/" ||
    issuer.search ||
    issuer.hash ||
    issuer.username ||
    issuer.password ||
    issuer.port ||
    config.issuer !== issuer.origin + "/"
  ) {
    throw new Error(
      "Auth issuer must be a canonical HTTPS origin with a trailing slash",
    );
  }
  const origin = new URL(config.publicOrigin);
  if (
    origin.protocol !== "https:" ||
    config.publicOrigin !== origin.origin ||
    origin.username ||
    origin.password
  ) {
    throw new Error("Public service origin must be an HTTPS origin");
  }
  if (
    !config.audience.trim() ||
    config.audience === config.desktopClientId ||
    config.audience === config.mobileClientId
  ) {
    throw new Error(
      "API audience must be an API identifier, not a native client ID",
    );
  }
  for (const id of [config.desktopClientId, config.mobileClientId]) {
    if (!/^[A-Za-z0-9_-]{16,128}$/.test(id))
      throw new Error("Invalid native client ID");
  }
  return Object.freeze({ ...config });
}
