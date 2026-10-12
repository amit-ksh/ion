import {
  portfolioId,
  validateGatewayBillingQuery,
  type GatewayBillingQuery,
} from "@waku/client";
export interface GatewayBillingRoute {
  owner?: unknown;
  scope?: unknown;
  exportId?: unknown;
  after?: unknown;
}
export function gatewayBillingRequest(route: GatewayBillingRoute): {
  query: GatewayBillingQuery;
  ownerId?: string;
} {
  for (const value of [route.owner, route.scope, route.exportId, route.after]) {
    if (value !== undefined && (typeof value !== "string" || !value))
      throw new Error("Invalid billing route");
  }
  const ownerId = route.owner as string | undefined,
    scopeId = route.scope as string | undefined,
    exportId = route.exportId as string | undefined,
    after = route.after as string | undefined;
  if (ownerId !== undefined && !portfolioId(ownerId))
    throw new Error("Open billing from the owner Usage view");
  let query: GatewayBillingQuery;
  if (!scopeId) {
    if (exportId) throw new Error("Open an export from its billing account");
    query = {
      kind: "cloudBilling",
      query: { after: after ?? null, limit: 32 },
    };
  } else {
    if (!ownerId) throw new Error("Open a billing account from its owner");
    if (!exportId)
      query = {
        kind: "cloudBillingReports",
        query: { scopeId, after: after ?? null, limit: 16 },
      };
    else {
      if (after !== undefined && !/^[1-9][0-9]{0,5}$/.test(after))
        throw new Error("Invalid billing charge cursor");
      query = {
        kind: "cloudBillingCharges",
        query: {
          scopeId,
          exportId,
          after: after === undefined ? null : Number(after),
          limit: 32,
        },
      };
    }
  }
  validateGatewayBillingQuery(query);
  return { query, ownerId };
}
