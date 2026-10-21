# Combined pilot spending envelope

20 October 2026. Applies to controlled backend dispatches after deployment.

The owner authorized a target below USD 10 and an absolute USD 15 cumulative
implementation/testing ceiling, retaining the earlier USD 10 monthly ceiling.
Ion reserves at most **USD 7/month and USD 12 lifetime across all controlled
services**, leaving USD 3 for retained storage/network and other non-runtime
charges. This is an application dispatch limit, not a provider invoice guarantee.
Unrelated usage of personal keys or other account resources is outside this ledger.

## Implementation

- `Budget.reserve` checks the combined envelope in the same SQL transaction as
  per-service/account limits and reservation creation.
- Existing inference/compute counters count immediately; no migration resets
  historical spending or unknown holds.
- Controlled meters are inference (`tokenFactory`), app compute (`cloud`),
  Token Factory sandbox (`sandbox`) and Composio (`composio`). New paid dispatchers
  must reserve through this boundary before sending provider requests.
- Unknown outcomes retain full reservations. Verified settlement is idempotent;
  provider overages remain debt and block subsequent dispatches.
- Month rollover resets only the monthly comparison; lifetime spending and
  unknown holds remain. Service/account limits continue to apply independently.
- Runtime envelope configuration can only narrow these ceilings, and is copied
  and frozen to prevent accidental widening after construction.

## Verification

`bun test packages/ion-cloud/src/combined-budget.test.ts packages/ion-cloud/src/spending.test.ts`

9 tests passed, 69 assertions: mixed-service admission, unknown holds, overage
debt, historical counters, restart/month rollover, atomic rollback, per-account
limits and immutable configuration. This does not prove provider invoice totals
or mean that the planned sandbox/app-hosting services are deployed.

No additional paid resource was started to implement or test this envelope.
Deployment and verification of the updated live gateway remain acceptance gates.

## Later release verification

On 21 October the same two local suites passed **nine tests, 71 assertions**
against the current source (`.waku-cache/ion-account-bound-budget-recheck.log`,
exit 0). Subsequent native service reads against the deployed gateway accepted
the combined micro-USD usage representation and recorded spending below the
implementation ceiling. No model generation, provider tool effect or compute
launch was requested by those reads. These checks do not reconcile provider
invoices or release the Drive action's unknown-cost reservation; those gates
remain open. Current package/upgrade evidence is in plans 137–138.
