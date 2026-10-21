# Relay usage fix: server deployment

21 October 2026. Source milestone **`3b8a8b6`** on `ion-prod`; measurements and
remaining native/live acceptance are in [158](158-desktop-relay-usage-fix.md).

## Verified rollout

The existing `ion-cloud` Worker at `https://api.build-with-ion.tech` was updated
from `b20ab52f-1155-424b-af51-eedc9957cd0f` to
**`d948eb00-1607-497d-9dd8-9e7dc75dbcbb`**, at 16:31 UTC / 22:01 IST.

The tested and deployed JavaScript SHA256 is
`fd5179abf23cf27b43758729e00ab1a924c45c6485d5b77e90d78b27e8e165f7`.
Normalized source-map comparison with the previous deployed bundle found only
five changed modules: personal gateway `automation-relay.ts`, `http.ts`,
`portfolio-relay.ts`, `push.ts` and `task-relay.ts`. Both maps have 61 sources.
The new desktop binary has **not** been packaged or installed yet.

Before upload, the helper required the expected previous version at 100%, a
clean committed backend source tree, the exact tested bundle, existing operator
secrets and absent sandbox policy. Deployment used `--keep-vars` with the same
bindings, existing SQL classes/migration, private R2 bucket and domains. No
secret value, service switch, budget policy, namespace data or permission changed.
No provider inference, app action, sandbox import or paid compute was started.

Before and after, checks passed for:

- Health and bootstrap: HTTP 200, static publishing available, sandbox unavailable.
- Anonymous service controls and both new gateway batch paths: HTTP 401.
- Speech-model HEAD: HTTP 200, 146,029,600 bytes and the pinned SHA256.
- The original operator secret names, unchanged.
- Zone routes: wildcard `*.build-with-ion.tech/*` points to `ion-cloud`, and
  `www.build-with-ion.tech/*` remains the null-script website exclusion.

### Route guard reconciliation

The first deployment helper exited with an assertion after the upload and
successful public checks because its route hash included ephemeral route IDs.
The failed receipt was preserved; deployment was **not retried**.

A separate read-only check compared just sorted `pattern`/`script` values. They
match the preserved pre-deployment semantic hash
`a9675ce4713523fb2eda7ddc6d2db43e4f707cb2c2dee3e5ac01760e0e2258e8`.
The expected two routes and website exclusion are intact. Final read-only checks
confirmed the new version at 100%, unchanged secret names and the exact bundle.
The initial guard failure is reconciled; no rollback or route write was needed.

## Native account transport

The existing Windows Rust acceptance test
`cloud::tests::live_managed_gateway_catalog_and_stop_controls` passed:
**one passed, zero failed** against the new deployment. It used the signed-in OS
vault account to read scoped service controls, connected-app catalog and combined
usage; it did not start inference, dispatch a valid app action or enable a switch.
This is transport/account evidence, not a native window or microphone pass.

Credential test executable SHA256 and rollout receipts are recorded in
[159-relay-deployment-receipt.json](159-relay-deployment-receipt.json).

## Desktop dependency correction and upgrade regression

The first full desktop typecheck found three references to `waku_core` in the
task bridge. The GUI intentionally has no daemon-core dependency. The shared
cadence and advisory-lock modules were moved to the already shared
`waku-credentials/src/gateway_sync/` boundary; both the daemon owner and GUI now
use that API. No dependency on daemon databases/providers was added to the GUI.
The full combined check **passed**:

```powershell
cargo check --locked --offline --release -j 1 -p waku -p waku-core -p waku-client
```

This validates the merged desktop, core and client types after the correction;
release linking, packages and native interaction acceptance remain separate.

The actual workerd SQL regression also passed a pre-receipt-schema recovery
scenario in its **disposable local test database**. It retains tasks, grants,
queued commands, uncertain claims and idempotency while recreating only the new
snapshot-receipt table; the next reconciliation restores the receipt. Production
data was not removed or reset, and these fixture routes are not exported by
AccountRelay. The original idle/write measurements remain unchanged.

## Live usage baseline and limits

The confirmed pre-rollout rolling 24-hour namespace query is recorded in 158:
115,287 requests, 964,342 SQL rows read, 26,043 rows written and 212,992 stored
bytes. A separate UTC-calendar-day query, `2026-10-21T00:00:00Z` through
`2026-10-21T16:28:57.563Z`, returned **87,538 requests**, **735,446 rows read**,
**21,908 rows written**, and the same maximum stored bytes.

The selected periods matter. Neither confirms the user's dashboard meter label,
denominator, selected window or account plan. Billing history remains denied
(403 / code 10000). Existing analytics access was used without scope expansion.
There is no measured post-rollout reduction or attribution to an individual
desktop/phone yet. The new server suppresses unchanged writes for old clients,
but request batching also requires the refreshed native client.

## Remaining acceptance

1. Finish real native owner/bridge recovery tests. The full desktop/core/client
   check, five native HTTP tests, three isolated scheduling/lock
   tests, 41 relay tests, eight fresh Worker integrations and the workerd SQL
   counter regression have passed.
2. Prepare a fresh source snapshot and Windows/Android packages. Older packages
   do not contain the merged redesign or relay cadence/batching changes. Do not
   relax their old provenance verifiers to accept the new tree.
3. Measure matched idle/active native windows with the new client; verify multiple
   windows, reconnect/account switching, approvals, Stop and lost acknowledgements.
4. Identify the actual 90% metric and reconcile pilot traffic against the existing
   plan and budget. Task 154 remains open; do not claim an all-user Free capacity
   pass or upgrade any plan to resolve it.

Ignored operational receipts: `ion-relay-worker-deployment.json` (initial guard
failure), `ion-relay-route-reconciliation.json`,
`ion-relay-worker-deployment-reconciled.json`, `ion-relay-native-live-read.log`,
`ion-relay-worker-bundle-comparison.json`, `ion-relay-live-namespace-20261021.json`
and `ion-relay-live-namespace-today-20261021.json`. No credentials are included.
