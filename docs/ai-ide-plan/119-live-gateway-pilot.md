# Live gateway pilot and QA handover gate

20 October 2026. Workspace: `ion-prod`. Existing unrelated QA changes and
artifacts are preserved. This records a partial live release, not full E2E acceptance.

## Verified

- Cloudflare zone `build-with-ion.tech` is active. Wrangler OAuth and the scoped
  DNS token work. Only `api.build-with-ion.tech` was reserved; root and `www`
  website routing were not changed.
- Worker `ion-cloud` is deployed with SQLite Durable Objects, HTTPS and three
  private secrets: approved account membership, the shared inference key and its
  spending policy. Credentials and account subjects stay in the private operator
  folder/Worker secrets, not this document.
- Live `/v1/health` and `/v1/bootstrap` return 200. Session, controls and inference
  requests without a real access token return 401, including spoofed account
  headers. The unhosted speech binary returns an explicit 503.
- Real owner Auth0 PKCE login and gateway admission pass. A live rotating token
  refresh and refresh-token revocation at sign-out pass through the shared client.
  One bounded streaming inference returns 200, model deltas and `[DONE]`; the
  spending ledger settles USD 0.000046 with zero held funds. The prior service
  switch is restored successfully. Evidence: `.waku-cache/ion-live-owner-check.json`
  (tokens stayed in memory; no access/refresh token is in that report).
- Real judge login, rotating refresh and refresh-token revocation pass. Owner
  control writes return 403; shared inference is denied while OFF. Judge usage
  excludes the owner request from its account spend, while the global ledger
  retains it. Both Cloud and Token Factory switches are OFF. Evidence:
  `.waku-cache/ion-live-judge-check.json`. The first judge attempt used the owner
  account and was rejected by the acceptance runner before any switch mutation.
- Token Factory model-catalog authentication succeeds. The current exact model ID
  is `nvidia/nemotron-3-super-120b-a12b`; native defaults and the policy example
  now match it.
- The official public catalog showed USD 0.30 input / 0.90 output per million
  tokens and a 256K context. The pilot policy reserves at USD 0.40 / 1.10, caps
  context at 131072 and output at 4096. Shared limits are USD 10/month,
  USD 25 lifetime and USD 1/account. These limits do not meter unrelated personal
  key use or act as a provider invoice guarantee.
- The model staged in `dist/ion-speech-110m` is 146029600 bytes with SHA-256
  `21ca92b8f0299c6e7d28eb9caa25235bf0a55eaa6511e8e46fbbc27e96900876`.
- Moto G32 USB debugging is authorized. This is device connectivity evidence,
  not proof that the new mobile release passed physical acceptance.
- Correct accessible Nebius project: `project-e04f8x7qbc00cshy8v3t19`
  (`default-project-eu-west2`). `ion-pilot` points to it; VM listing succeeded with
  zero instances. No VM, sandbox, disk or controller service was provisioned or started.

## Changes verified locally

- Gateway-only deployment omits R2. Login/relays/shared inference continue to
  work; hosting and model binaries fail closed, and unavailable publishing is
  not advertised. Full and gateway-only configs use the same script and Durable
  Objects for later storage activation without replacing account state.
- Compute policy accepts a validated operator project ID. The separate Linux
  controller requires `ION_COMPUTE_PROJECT_ID` and still rejects a VM belonging
  to another project. The previous inaccessible project is no longer hardcoded.
- Real local workerd/SQLite/R2 regressions: 4 passed, including the new no-R2
  test. That test failed against the old implementation before the fix.
- Gateway/native-client regressions: 38 passed, 228 assertions. Compute spending
  subset: 7 passed, 56 assertions, including a foreign-project observation test
  that failed before the project-configuration fix.
- Worker and Linux-controller TypeScript checks pass. Desktop/daemon offline
  compile passes with existing warnings. No new EXE/APK has been built.

## Current blockers

1. **Remaining Auth0 acceptance:** the Desktop pre-login rejection is resolved.
   Owner login, rotating refresh, logout and bounded inference now pass live.
   Judge login, refresh/logout, denied owner controls and account usage isolation
   now also pass live. Both service switches are confirmed OFF. Mobile
   authorization still needs its own real-device check; Desktop success does not
   prove Mobile access.
2. **R2 activation:** the authenticated bucket API returned code 10042, R2 not
   enabled. The owner will complete activation later. No bucket/model upload was
   attempted after this result. Model downloads and cloud publishing remain blocked.
3. **Compute/hosting:** intentionally not provisioned while R2 is unavailable.
   Still requires supervised Linux controller setup, scoped credentials, a
   budget-reviewed isolated app VM, verified images/networking and confirmed stop.
4. **Physical acceptance:** new desktop/mobile builds, native account login,
   pairing, microphone and first-use/cached speech must pass. iPhone acceptance needs signed iOS tooling/build access.

The first two Auth0 attempts failed before login. After application access was
updated, the third completed owner acceptance. The test temporarily enabled
Token Factory for the single bounded request and restored its prior switch state
before revoking its refresh token. Public capability `sharedInference: true` means
configured, not that the owner has enabled it. No compute resource was started.

## Final QA handover

The user requested handover to the existing **Test and fix Ion** chat after
implementation and verification. Its thread ID is
`01a14e55-9a4b-7480-9e15-91a66d35fd0c`. The final handover has **not** been sent:
the gates above are not yet satisfied. Do not call incomplete live checks passed.

After Auth0/R2/operator acceptance, provide that chat the actual source revision,
build paths, this record, deployment and resource-stop evidence, and exact remaining
device cases. Preserve the existing dirty QA workspace and archived merge backup.
Unstarted harness adapters remain out of scope. Do not grant blanket permission
for paid resources, external messages or owner-data cleanup through the handover.

## Primary references

- [Official Nemotron Super catalog](https://tokenfactory.nebius.com/models/catalog/text2text/nvidia%2Fnemotron-3-super-120b-a12b)
- [Auth0 per-application API access](https://auth0.com/blog/developers-guide-api-access-policies-auth0/)
- [Cloudflare R2 activation](https://developers.cloudflare.com/r2/get-started/)
