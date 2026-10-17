import {
  portfolioId,
  portfolioReadGrant,
  portfolioDirectoryGrants,
  validatePortfolioJob,
  validatePortfolioQuery,
  type PortfolioQuery,
  type PortfolioJob,
  type PortfolioResult,
  type PortfolioSubmission,
} from "@waku/client";
import {
  readGatewayPortfolio,
  submitGatewayPortfolio,
  type GatewayPairing,
} from "./gateway-client";

interface Transport {
  maxPolls?: number;
  submit(
    pairing: GatewayPairing,
    input: PortfolioSubmission,
  ): Promise<PortfolioJob>;
  read(
    pairing: GatewayPairing,
    input: PortfolioSubmission,
  ): Promise<PortfolioJob | null>;
  wait(): Promise<void>;
}
const transport: Transport = {
  submit: submitGatewayPortfolio,
  read: readGatewayPortfolio,
  wait: () => new Promise((resolve) => setTimeout(resolve, 1_000)),
};
/** All requests are reads. The caller fences focus/pairing/filter changes; one
 * nonce follows a lost submit acknowledgement and every subsequent poll. */
export async function loadGatewayPortfolio(
  pairing: GatewayPairing,
  publisherId: string,
  query: PortfolioQuery,
  nonce: string,
  current: () => boolean,
  api: Transport = transport,
): Promise<PortfolioResult> {
  validatePortfolioQuery(query);
  if (!portfolioId(publisherId) || !portfolioId(nonce))
    throw new Error("Invalid history request identity");
  const grant = portfolioReadGrant(query);
  if (
    grant
      ? !pairing.grants.includes(grant)
      : !pairing.grants.some((value) =>
          portfolioDirectoryGrants.includes(value),
        )
  )
    throw new Error(
      "Pair this phone with the required owner history read access",
    );
  const input: PortfolioSubmission = {
    publisherId,
    idempotencyKey: nonce,
    query,
  };
  const allowed = () => {
    if (pairing.expiresAt <= Date.now())
      throw new Error("History read grant expired; pair this phone again");
    if (grant && !pairing.grants.includes(grant))
      throw new Error("History read grant revoked; pair this phone again");
    if (!current()) throw new Error("History request scope changed");
  };
  const transfer = [
    "workflowDocument",
    "workflowPdfPage",
    "workflowHtmlManifest",
    "workflowHtmlSource",
  ].includes(query.kind);
  let readmissions = 0;
  const maxPolls = api.maxPolls ?? 60;
  if (!Number.isSafeInteger(maxPolls) || maxPolls < 1 || maxPolls > 240)
    throw new Error("Invalid history poll bound");
  const deadline = Date.now() + 60_000;
  allowed();
  let job: PortfolioJob | null;
  try {
    job = await api.submit(pairing, input);
  } catch (error) {
    allowed();
    job = await api.read(pairing, input);
    if (job === null) throw error;
  }
  for (let poll = 0; poll < maxPolls && Date.now() < deadline; poll++) {
    allowed();
    if (job === null && transfer) {
      if (readmissions >= 2)
        throw new Error(
          "Document transfer receipt was evicted repeatedly; refresh to try again",
        );
      readmissions++;
      job = await api.submit(pairing, input);
      allowed();
    }
    if (job !== null) validatePortfolioJob(job, pairing.id, input);
    if (job?.status === "complete" && job.result) return job.result;
    if (job?.status === "rejected" || job?.status === "expired")
      throw new Error(
        "Owner history request expired or was rejected; refresh to try again",
      );
    await api.wait();
    allowed();
    job = await api.read(pairing, input);
  }
  throw new Error(
    "Owner history is taking longer than expected; refresh when the owner is reachable",
  );
}
