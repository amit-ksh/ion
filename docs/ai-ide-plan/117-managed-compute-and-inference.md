# Managed compute and shared inference implementation

20 October 2026. Candidate branch: `codex/ion-cloud-auth`. This extends checkpoint
116. Source implementation and local verification are separate from live release.
No paid VM, real provider request, public route or cloud resource was created.

## Implemented source

- `Budget` stores integer micro-USD held/spent counters, by service, UTC month,
  lifetime and account. Reservations and lifecycle decisions share one SQL commit
  in local SQLite and Durable Objects. Unknown outcomes retain their full hold;
  valid inference usage or conservatively estimated elapsed VM time after provider-confirmed stop settles exactly once. Overages are recorded as debt.
  A month rollover does not erase lifetime or uncertain spending.
- The shared inference proxy admits approved text/tool models only, enforces a
  conservative UTF-8/template context envelope, one active request per account,
  bounded output, 90-second upstream timeout, 500 KB request and 2 MB response.
  It uses the pinned Token Factory HTTPS endpoint, never follows redirects and
  never stores prompts or keys in its ledger. Missing/contradictory usage keeps
  the hold. Global/lifetime/per-account limits apply before dispatch.
- Shared inference preserves real provider SSE order for text/reasoning/tool
  deltas, with four active streams globally and one per account. The success
  terminator is withheld until final usage and stream completion validate.
  Disconnects, oversized/malformed events and missing usage keep the reservation;
  partial tool deltas cannot become successful execution without that terminator.
- Native solo agents, local Teams and workflow generation use personal credentials
  when configured, otherwise account-bound shared inference. Managed access tokens
  refresh per request and cannot move an existing task to a replacement account.
  Remote execution-host model credentials remain separately configured; no master
  Token Factory key or native account token is placed in untrusted app containers.
- Worker owner switches activate only configured capabilities. Judges cannot
  change them. Desktop/mobile settings show shared usage/reservations and current
  observed VM state. A Token Factory switch cannot reset a paused Cloud session.
- The gateway tracks a single pinned app VM, controller leases, build leases,
  demand/idle expiry, bounded startup/runtime, conservative spending reservation,
  coalesced mutations and confirmed stopped state. Queued reviewed builds wake
  compute; authenticated build polling keeps its lease while active. Authorized
  dynamic-site visits renew demand, with a retryable cold-start response.
- `services/compute-controller` is the separate operator **Linux** pull controller.
  It uses the official pinned gRPC SDK; credentials remain outside the app VM.
  It persists mutation intent/operation IDs, verifies project/instance, observes
  provider operation completion and attempts stop on gateway outage/deadline.
  Its host must be online; a dead controller cannot guarantee provider shutdown.
- App workers inspect actual Docker state after VM restart and restart existing
  stopped containers. Missing/unrecoverable runtime containers become uncertain
  and their sites stop routing, requiring review/new publication.
- HTML/static content stays in private R2 and does not depend on VM availability.
  The verified local 110M speech path and first-use model manifest remain as in 116.

Native desktop/mobile builds now default to `https://api.build-with-ion.tech`;
operators can override the origin at build/runtime for controlled builds. Judges
do not set an environment variable, configure a gateway host or supply the shared
Token Factory key. The operator still has to make that default origin live.

## Policy inputs

Copy the example JSON files into private operator configuration and replace all
placeholder/null values. They intentionally fail validation unchanged:

- `packages/ion-cloud-worker/compute-policy.example.json`: actual instance,
  conservative micro-USD/hour and runtime bounds. Runtime ceilings are at most
  USD 7/month and USD 22 lifetime, leaving USD 3 of each requested Cloud envelope
  for retained-resource/rounding/tax allowance. Verify the allowance; delayed
  provider billing prevents an absolute invoice guarantee from source estimates.
- `packages/ion-cloud-worker/token-factory-policy.example.json`: approved IDs,
  reviewed context/output limits (the example caps input context at 131072; verify the actual model supports this envelope, including the native tool schemas) and current conservative input/output prices
  in micro-USD per million tokens. Defaults cap shared inference at USD 10/month,
  USD 25 lifetime and USD 1 per account. These are independent Cloud/TF ledgers.

Worker secrets (run Wrangler from `packages/ion-cloud-worker`):

```sh
wrangler secret put MEMBERS_JSON
wrangler secret put COMPUTE_POLICY_JSON
wrangler secret put COMPUTE_CONTROLLER_TOKEN
wrangler secret put TOKEN_FACTORY_POLICY_JSON
wrangler secret put TOKEN_FACTORY_KEY
wrangler secret put HOSTING_WORKER_TOKEN
wrangler secret put HOSTING_WORKER_ORIGIN
wrangler secret put HOSTING_SECRETS_KEY
```

Use private stdin/files, not values in command history. The controller uses the
matching compute token and an explicit scoped Nebius credentials file, per its
README/systemd example. Hosting uses the existing private-network/XFS/gVisor
runbook. Prices, instance IDs, actual image digests and credentials cannot be
filled from the public project ID alone.

## Verification record

- Complete gateway/lifecycle/runtime-recovery regression: **95 passed, 1 existing skipped HTML fixture case, 0 failed; 655 assertions across 19 files** before the final streaming-concurrency case. The final focused streaming/spending suite passes **6 tests, 53 assertions**, including an incomplete-stream hold and success-terminator denial. Focused native credentials/account/speech regression: **43 passed, 1 opt-in installed-model smoke case excluded from the generic run**; that model smoke passed explicitly at checkpoint 116.
- Focused mobile account/dictation regression: **16 passed, 80 assertions**. Native-client PKCE/session/request plus mobile-account regression after the default-origin change: **22 passed, 130 assertions**. Final Worker/controller/hosting/shared-package/mobile typechecks pass.
- Full local workerd/SQLite/R2 suite: **3 passed, 0 failed**, including the final live-SSE proxy path. The configured pilot test passes signed native login admission, approved
  inference, real SQLite reservations/settlement, reviewed app publication,
  authenticated VM wake, Cloud OFF stop, and state after runtime restart.
- Worker/controller/hosting/mobile TypeScript checks pass.
- Final native desktop/daemon compile check passed, including onboarding/shared usage and account-cache changes. No visual app test was requested or performed.
- Linux controller SDK types were checked. A real SDK import on this Windows
  environment fails because its `fs-ext` native module is not available; the
  Linux prerequisite and rebuild instructions are explicit in its runbook.
- Temporary redirect diagnostics were removed. The real Worker regression found
  that workerd rejects `redirect: error`; requests now use `manual` plus explicit
  status rejection. A Bun-only test did not catch this difference.

## Remaining live release checklist

- [ ] Authenticate Cloudflare CLI; create private R2/DO resources and install DNS,
  API/site routing and HTTPS without intercepting root/www website traffic.
- [ ] Supply approved Auth0 owner/judge subjects, disable signup, and verify native
  callbacks, scope, refresh and logout on real accounts.
- [ ] Upload the exact staged 146 MB model; verify the public file hash before
  distributing first-use installers.
- [ ] Provision the dedicated Nebius app worker and a supervised **separate Linux
  operator controller**. Verify project/region, scopes, actual prices, persistent
  disks, gVisor/XFS isolation, private worker HTTPS and pinned runtime images.
- [ ] Configure secrets/policies, observe provider STOPPED, then owner-enable the
  desired service. Verify real start/build/health/URL/idle-stop/wake/Cloud-OFF,
  controller-outage recovery and provider-confirmed stopped state.
- [ ] Exercise an approved model against the actual shared key; reconcile provider
  usage/prices with the ledger and check each spending limit and owner OFF switch.
- [ ] Build the release EXE/APK with actual origins; fresh install, microphone,
  first-use/cached model and Moto G32/iPhone 15 acceptance remain physical checks.
  iOS signing/build still requires the Apple toolchain/account.

The source was merged into `ion-prod` on 20 October 2026; see checkpoint 118
for integration evidence. It is not live. Existing QA changes were preserved.
No unstarted harness adapters were added.

## Operational limitations

Cloud accounting uses a conservative elapsed-time estimate plus configured rate
bounds; it is not a Nebius invoice reader. Retained storage and other services
remain the operator's responsibility. Unobserved/ambiguous starts keep their hold
and require operator reconciliation; changing the owner switch does not refund
unknown spending. Shared streaming has a 90-second request bound. Phone dictation initially uses the existing selected-desktop transport;
it does not add a managed-gateway audio relay or interrupt a daemon job remotely.

## Primary references

- [Official Nebius Node SDK](https://github.com/nebius/js-sdk), pinned to 0.2.81;
  generated instance/operation types were inspected from that installed package.
- [Nebius instance start/stop protocol](https://github.com/nebius/api/blob/main/nebius/compute/v1/instance_service.proto).
- [Token Factory API specification](https://api.tokenfactory.nebius.com/docs).
