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
- [ ] Actual rebuilt Windows app login, account switching and return visit.
- [ ] Actual mobile-client Auth0 login/refresh/logout on Moto G32.

Evidence and remaining storage/compute prerequisites: [119](119-live-gateway-pilot.md).

## 2. Shared Composio app connections — implementation complete, live OAuth pending

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
- [ ] Connect/disconnect/reconnect one real user-owned app and verify another Ion
  account cannot discover, revoke or use it.
- [x] Commit connection implementation: `d05daf4`; deploy backend secret/session
  catalog. Real OAuth and action acceptance remain pending. See [123](123-managed-app-connection-foundation.md).

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

- [x] Activate R2, create the private bucket and deploy the production storage
  binding. Complete HTTPS model GET/HEAD passed exact size and SHA-256 checks.
- [ ] Verify private storage bindings, bounded uploads, artifact ownership,
  HTML content isolation, preview and publishing/revocation URLs.
- [ ] Configure the existing Ubuntu controller and a budget-approved isolated
  app build/runtime host, required images, DNS/TLS and networking.
- [ ] Verify build logs, app health, runtime secrets, sleep/wake, cancellation,
  idle/runtime limits, restart recovery and confirmed provider stop.
- [ ] Test cross-account publication controls and isolation of deployed content
  from the authenticated gateway. Verify charges retained after stopping compute.
- [ ] Commit publishing milestone.

## 9. Managed onboarding, release and final acceptance

- [x] Make sign-in/model selection/user app connections the normal onboarding;
  move personal provider keys to optional Advanced settings without deleting them.
- [x] Show accurate unavailable, disconnected, expired-session and budget states.
- [ ] Preserve agreed Ion styling; validate keyboard operation, account-cache
  clearing, labels and Ion branding in rebuilt UI.
- [ ] Run targeted regressions and the required release gates; reuse the existing
  watcher. No native visual test unless directly authorized for this session.
- [ ] Build and test a new installer; verify clean install, launch, upgrade and
  bundled speech runtime. Record source revision and artifact hashes.
- [ ] Build Android only from the merged/verified source; test on authorized Moto
  G32, including native login, pairing and microphone acceptance.
- [ ] Stop all resources started for acceptance and record the actual result.
- [ ] Commit each completed milestone and provide exact evidence/blockers to
  **Test and fix Ion** when the agreed handover conditions are satisfied.

## Operator participation still needed

- R2 activation complete; real authenticated publication acceptance remains.
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

See [126](126-managed-native-execution.md) for persistent account ownership,
chat app actions, managed remote execution and actual remaining operator gates.
R2 is active and production model download is verified. Sandbox execution
permissions remain false, the native app is signed out, and Moto G32 is connected
and authorized. Remaining live gates are not recorded
as a passed test. No billable Nebius resource was started for this increment.
