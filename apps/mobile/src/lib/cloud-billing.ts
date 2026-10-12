import type {
  WakuClient,
  CloudBillingPage,
  CloudBillingReportPage,
  CloudBillingChargePage,
} from "@waku/client";
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const ID = /^[A-Za-z0-9_-]{1,160}$/;
export type BillingView =
  | { kind: "accounts"; page: CloudBillingPage }
  | { kind: "reports"; page: CloudBillingReportPage }
  | { kind: "charges"; page: CloudBillingChargePage };
export type BillingRoute = {
  scope?: string;
  exportId?: string;
  after?: string;
};
export async function readCloudBilling(
  client: Pick<WakuClient, "request">,
  route: BillingRoute,
): Promise<BillingView> {
  if (!route.scope) {
    if (route.exportId || (route.after && !UUID.test(route.after)))
      throw new Error("Invalid billing account page");
    const reply = await client.request({
      type: "loadCloudBilling",
      after: route.after ?? null,
      limit: 32,
    });
    if (reply.type !== "cloudBilling" || reply.page.scopes.length > 32)
      throw new Error("Invalid billing account response");
    return { kind: "accounts", page: reply.page };
  }
  if (!UUID.test(route.scope))
    throw new Error("Open a billing connection from the account list");
  if (!route.exportId) {
    if (route.after && !ID.test(route.after))
      throw new Error("Invalid billing export cursor");
    const reply = await client.request({
      type: "loadCloudBillingReports",
      scopeId: route.scope,
      after: route.after ?? null,
      limit: 16,
    });
    if (
      reply.type !== "cloudBillingReports" ||
      reply.page.scopeId !== route.scope ||
      reply.page.reports.length > 16 ||
      reply.page.reports.some((r) => r.scopeId !== route.scope)
    )
      throw new Error("Billing exports belong to another account");
    return { kind: "reports", page: reply.page };
  }
  if (
    !ID.test(route.exportId) ||
    (route.after && !/^[1-9][0-9]{0,5}$/.test(route.after))
  )
    throw new Error("Invalid billing charge page");
  const after = route.after ? Number(route.after) : null;
  if (after !== null && after > 100_000)
    throw new Error("Invalid billing charge cursor");
  const reply = await client.request({
    type: "loadCloudBillingCharges",
    scopeId: route.scope,
    exportId: route.exportId,
    after,
    limit: 32,
  });
  if (
    reply.type !== "cloudBillingCharges" ||
    reply.page.report.scopeId !== route.scope ||
    reply.page.report.exportId !== route.exportId ||
    reply.page.charges.length > 32
  )
    throw new Error("Billing charges belong to another export");
  return { kind: "charges", page: reply.page };
}
