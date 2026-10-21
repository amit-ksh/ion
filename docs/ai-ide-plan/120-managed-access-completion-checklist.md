# Managed access: development and acceptance checklist

20 October 2026. Branch: `ion-prod`. A checked implementation item is not a
claim that its real-provider/device acceptance is complete. Existing unrelated
QA changes remain preserved. Commit each completed milestone separately.

Active goal scope: Windows desktop, Android (Moto G32), shared backend services
and deployments. iOS implementation, builds, signing and acceptance are excluded.
Previously deferred/unstarted harness adapters also remain outside scope.

## 1. Gateway, admission and login

- [x] Deploy gateway-only Cloudflare Worker at `api.build-with-ion.tech`.
- [x] Keep owner/judge admission; reject unsigned/spoofed identity requests.
- [x] Real owner and judge Auth0 PKCE login through the shared client.
- [x] Rotating refresh and refresh-token revocation for both roles.
- [x] Judge denied owner-control writes; account-scoped usage verified.
- [x] One bounded shared inference request: 200, SSE completion, settled usage.
- [x] Restore original switches; judge subsequently confirmed both switches OFF.
- [x] Commit gateway milestone: `060a329`.
- [x] Actual Rust PKCE callback, Windows credential-vault storage, rotating
  refresh and managed desktop publish-only registration. See [129](129-native-account-drive-and-android-acceptance.md).
- [ ] Rebuilt Windows GUI login/account switching and return visit.
- [x] Owner-confirmed Android sign-in and nonvisual installed-app cold start on
  authorized Moto G32.
- [ ] Actual mobile refresh/logout and account switching on Moto G32.

Evidence and remaining storage/compute prerequisites: [119](119-live-gateway-pilot.md).

## 2. Shared Composio app connections — initial provider acceptance complete

- [x] Saved operator key verified against read-only toolkit, auth-config and
  connected-account endpoints (all 200). No credential/account state printed.
- [x] Implement backend sessions mapped to the verified Ion workspace identity.
- [x] Hide project-wide/operator accounts, credential envelopes and MCP capabilities.
- [x] Implement account-checked catalog, connect and revoke routes.
- [x] Persist uncertain operations and session initialization; no automatic effect retry.
- [x] Add persistent request limits and disabled Instant/Composio sandbox/proxy defaults.
- [x] Six local backend regressions pass, 120 assertions; Worker type check passes.
- [x] Verify actual provider session response shapes and connection catalog: real
  owner-scoped session returned 201 and catalog returned 200 (50 items, no connections).
- [x] Verify public Worker routing/isolation in real local workerd: five Worker
  integration tests pass, including authenticated app identity and restart persistence.
- [x] Install project key exclusively as a backend secret after verification.
- [x] Connect native Apps UI and app mentions, with account-bound background loads.
- [x] Owner-consented real Google Drive connection and exact approved read-only
  quota action, with provider log ID and restored owner controls.
- [ ] Disconnect/reconnect acceptance and actual second-account live denial of
  discovery, revocation and execution. Preserve the current working connection
  while user interaction is unavailable.
- [x] Commit connection implementation: `d05daf4`; deploy backend secret/session
  catalog. Real owner OAuth/action acceptance now passes; native interaction and
  second-account gates remain. See [129](129-native-account-drive-and-android-acceptance.md).

## 3. Approved app actions, teams and workflows

- [x] Extend existing exact approvals to managed session execution and pin the
  authenticated account, connected account, tool version and arguments.
- [x] Enforce ownership at discovery and again immediately before dispatch.
- [x] Persist dispatch/uncertain outcomes; reconcile instead of replaying effects.
- [x] Route normal chat, team grants and workflow actions through this boundary.
- [x] Backend regressions cover denial, modified arguments, foreign connection IDs,
  disconnects, provider errors, restart and duplicate submission. Native account
  replacement is rejected by the account-bound transport; actual UI acceptance remains.
- [x] Complete one authorized, read-only real provider tool call with a Composio
  log ID: owner-consented Drive quota action passed. See [129](129-native-account-drive-and-android-acceptance.md).
- [x] Commit execution backend milestone: `b7ba442`; native routing/account binding
  is documented in [126](126-managed-native-execution.md).

## 4. Managed Nebius inference across existing execution paths

- [x] Audit native chat, team continuation/resume and workflow model calls for
  consistent managed-vs-personal selection and account binding.
- [x] Preserve optional existing personal credentials; label the usage source.
- [ ] Verify real streaming, tool calls, cancellation, session expiry, service OFF,
  exhausted budget and account switching in actual native flows.
- [x] Keep unsupported/unstarted harness adapters outside scope.
- [x] Commit native managed execution/account-binding milestone after passing
  targeted native and credential regressions; see [126](126-managed-native-execution.md).

## 5. Managed Token Factory sandboxes

- [x] Verify existing project key's sandbox access using read-only discovery:
  `whoami` accepted the key but all spawn/list/cancel/import permissions are false.
  Activation or project authorization is required before live sandbox acceptance.
- [ ] Select and verify compatible registered images for supported team harnesses.
- [x] Backend owns project credentials, provisioning, account-bound command
  approvals, bounded file transfer, status, results and artifact retrieval.
- [x] Implement durable limits, runtime/idle deadlines, cancellation, recovery,
  owner stop and confirmed resource shutdown. Keep this separate from app-hosting VMs.
- [x] Test denied approvals, cross-account access, unavailable image, timeouts,
  restart and uncertain command outcomes without duplicate effects.
- [ ] Perform a short real sandbox lifecycle only after price/limit verification;
  verify stop/deletion and retained billable resources afterward.
- [x] Commit sandbox backend milestone: `afe9b1e`; recovery hardening and native
  routing are documented in [125](125-managed-token-factory-sandbox.md) and [126](126-managed-native-execution.md).

## 6. Combined spending enforcement

- [x] Implement the latest authorization: aim below USD 10; USD 15 cumulative ceiling
  for this implementation/testing work, preserving the earlier USD 10 monthly limit.
- [x] Enforce a combined SQL envelope across inference, sandbox, app compute and
  Composio meters, including historical counters and a USD 3 retained-cost allowance.
- [ ] Reconcile non-ledger provider charges and verify the updated envelope live.
- [x] Add Composio request/usage limits appropriate to its actual plan; keep
  Instant Tools disabled unless explicitly budgeted and authorized.
- [x] Test combined reservations, historical counters, restart/month rollover,
  unknown holds, debt/exhaustion and transactional rollback: nine budget tests pass.
- [ ] Verify owner stop and actual provider billing within the new envelope live.
- [x] Commit budget implementation milestone: `0a5f63a`. See [121](121-combined-spending-envelope.md).
  Deploy it before additional paid provisioning.

## 7. Speech model distribution and local dictation

Latest user direction: use private R2 through Ion's API as the default download
origin. Hugging Face publication below remains historical evidence for the same
artifact. See [128](128-r2-speech-and-publishing.md).

- [x] Migrate the verified model to private R2, switch the bundled manifest to
  Ion's HTTPS endpoint, and verify full GET/HEAD plus real native first-download
  and cache reuse. Preserve the original byte count, hash and model notices.

- [x] Exact converted 110M Q8 model staged with known size/SHA-256.
- [x] Bounded/cancellable/checksummed download and local runtime implemented.
- [x] Publish the exact compatible GGUF with attribution to `amit-ksh/ion-stt`,
  pinning revision `e2d275da00383be64c0ee95f340ff37e3329c407`.
- [x] Support observed Hugging Face CDN redirects anonymously, retaining
  content-size/hash checks, partial-file handling, time bounds and cancellation.
- [x] Test actual native first-use download and cache reuse; regress corrupt,
  oversized, canceled and incomplete downloads.
- [x] Run installed CPU runtime with the downloaded model against fixed speech
  and silence fixtures; both pass.
- [ ] Test desktop microphone permissions and actual recorded dictation.
- [ ] Test phone dictation relayed to the paired desktop. Phone inference remains
  on the paired desktop for this release.
- [x] Commit speech distribution milestone: `d9e93b5`. See [122](122-hugging-face-speech-release.md).

## 8. HTML and app publishing

**Scope update — 21 October 2026:** the owner now allows **static app publishing
only** on Cloudflare. Backend app servers, cloud build execution and the Nebius
app-host/controller provisioning and sleep/wake gates below are deferred, not
remaining dependencies for this static release. Keep Token Factory inference
and sandbox acceptance separate. See [130](130-cloudflare-app-deployment-feasibility.md)
and [131](131-static-only-publishing.md).

- [x] Enforce static-only publishing, review prebuilt desktop exports, reject
  server runtimes/build commands/secrets and prevent legacy runtime activation.
- [x] Verify static publication with Cloud OFF: service 44/44, workerd 7/7,
  native export 3/3, and one real native-account production acceptance passed;
  temporary site removed, all owner switches unchanged.


- [x] Activate R2, create the private bucket and deploy the production storage
  binding. Complete HTTPS model GET/HEAD passed exact size and SHA-256 checks.
- [x] Verify backend private storage bindings, bounded uploads, ownership,
  content isolation and real publishing/revocation URLs. Native GUI preview and
  publishing interactions remain in [132](132-physical-device-acceptance.md).
- Deferred: Ubuntu controller and isolated app build/runtime host, images,
  DNS/TLS and networking. Static publishing uses the existing Cloudflare path.
- Deferred: server build logs, app health/secrets, compute sleep/wake and
  lifecycle/provider-stop acceptance for backend app runtimes.
- [x] Local workerd cross-account publication controls and content/gateway
  isolation; real owner public/private access, DNS/TLS and revocation.
- [ ] Actual second-account live publication denial and native interaction.
  No app VM/disk was created; no retained app-host charge is assumed or invented.
- [x] Commit R2/publishing and static-only enforcement: `f07a489`, `59218a1` and
  live static acceptance `7ffbbdd`.

## 9. Managed onboarding, release and final acceptance

- [x] Make sign-in/model selection/user app connections the normal onboarding;
  move personal provider keys to optional Advanced settings without deleting them.
- [x] Show accurate unavailable, disconnected, expired-session and budget states.
- [ ] Preserve agreed Ion styling; validate keyboard operation, account-cache
  clearing, labels and Ion branding in rebuilt UI.
- [ ] Run targeted regressions and the required release gates; reuse the existing
  watcher. No native visual test unless directly authorized for this session.
- [x] Build refreshed Windows EXE/ZIP, record exact source/QA provenance and
  artifact hashes, verify package assets/R2 manifests and actual bundled CPU
  speech/silence fixtures. The current account-bound release is in
  [137](137-account-bound-windows-release-acceptance.md); plan 134 is historical.
- [x] Verify the real per-user upgrade, installed file hashes, process launch and
  preservation of all original database IDs. Post-upgrade native account reads pass.
- [ ] Verify clean install/fresh-user state, actual GUI account restore and return visit.
- [x] Build Android from merged/verified source `65a36d6` plus preserved QA inputs;
  verify its unchanged signing identity and exact installed bytes on Moto G32.
  Replacement install and nonvisual cold start passed; see
  [138](138-account-bound-android-release-acceptance.md).
- [ ] Verify refreshed Android account restoration/login/refresh/logout/switching,
  desktop pairing and actual microphone/dictation interactions.
- [ ] Stop all resources started for acceptance and record the actual result.
- [ ] Commit each completed milestone and provide exact evidence/blockers to
  **Test and fix Ion** when the agreed handover conditions are satisfied.

## Operator participation still needed

- R2 activation and real native-account backend publication acceptance complete;
  actual GUI and second-account checks remain.
- Hugging Face upload access supplied; model upload and native download are complete.
- Owner Google Drive consent, real read-only action, native login/refresh and
  Android login/upgrade/microphone permission are complete. Actual phone speech,
  pairing and remaining native interactions still need acceptance.
- Any paid resource action exceeding or materially changing the existing budget
  requires a concrete new authorization. Do not request secret values in chat.

## Current API references

- [Composio sessions](https://docs.composio.dev/reference/api-reference/tool-router)
- [Session configuration](https://docs.composio.dev/docs/configuring-sessions)
- [Connected accounts](https://docs.composio.dev/reference/api-reference/connected-accounts)
- [Scoped project keys](https://docs.composio.dev/reference/authenticating-to-composio/project-api-key-permissions)
- [Hugging Face uploads](https://huggingface.co/docs/hub/models-uploading)

## Latest independent verification

See [129](129-native-account-drive-and-android-acceptance.md) for real native
account/refresh/Drive and Android upgrade evidence, [131](131-static-only-publishing.md)
for live static publishing, [132](132-physical-device-acceptance.md) for the
remaining interaction checklist, and [133](133-backend-typecheck-acceptance.md)
for clean backend/Worker type checks and 44 service / seven workerd tests.

R2 model GET/hash and actual native first-download/cache checks passed. Both
supplied project IDs were rechecked on 21 October: sandbox `whoami` returns 200
with spawn/list/cancel/import false. No sandbox launch was attempted. The native
owner login/vault/refresh and desktop publish grant passed; Moto G32 remains
connected and its process was observed alive. Physical paired dictation and
account-switch behavior are not inferred from these facts.

Windows source `7ffbbdd` plus the preserved 111-file QA patch built successfully.
Package assets, licenses, R2 manifest embedding and bundled CPU speech/silence
acceptance passed; see plan 134. A later account-bound publication fix (plan 135)
passes 20 account fixture tests, desktop compilation and real native-account
static publication through the bound transport. Approval access returned at the
7:05 AM continuation; the disposable live site was removed, controls stayed
unchanged and no paid compute was called. The earlier installer predated this fix.
Its replacement has passed nonvisual upgrade/data-retention checks in
plan 137. Actual GUI account-switch acceptance remains open. The earlier backend-only
source reconciliation remains documented in plan 133. No new
Nebius compute or paid app action was started in these checks. The later
read-only Compute inventory probe stalled and was cancelled locally; the prior
post-create-denial inventories were empty, and no new inventory result is claimed.

The corresponding mobile publication transport now pins the reviewed account,
rejects generic hosting requests and invalidates old views without requiring a
gateway pairing. Sixteen controller/session regressions and the mobile TypeScript
check passed; see [136](136-mobile-publishing-account-boundary.md). The refreshed
APK, signature and actual Moto replacement install/cold start passed in plan 138.
Actual device account switching, pairing and physical dictation remain open.
