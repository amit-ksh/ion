# Managed native execution and account ownership

20 October 2026. Windows desktop and Android scope; iOS and deferred harness
adapters remain excluded. This is an implementation and fixture-verification
record, not a claim of completed live-provider/device acceptance.

## Native execution paths

- Native Nebius chats expose connected-app discovery, pinned action schemas and
  exact account/version/arguments approvals. Prepared effects are journaled before
  dispatch; interruption reports an uncertain effect instead of repeating it.
- Teams use existing per-agent grants, parent delegation and user decisions.
  Workflows and routine-created runs inherit their original account at creation.
  Timer execution never adopts whichever user happens to be signed in later.
- Each managed conversation, team journal, queued run, workflow and routine pins
  its account in persistent state. Account replacement, logout and a personal/
  managed payer change cannot silently resume old work under a new identity.
  Legacy unscoped work cannot automatically adopt a managed payer: create a new
  task under the intended login.
- Credential transports reject replacement identities before token refresh and
  immediately before the authenticated request. Managed app preparation carries
  the original account through metadata, delayed approval and dispatch.
- Without explicit personal sandbox IAM configuration, registered remote harness
  and command clients use the authenticated gateway. The app receives reviewed
  manifests and public policy, never the operator project key.
- Agent creation retains explicit Local or Nebius environments. Registered
  sandbox choices are filtered by harness using a background-loaded cache.
  No automatic environment mode was added. Existing explicitly configured local
  fallback remains limited to confirmed pre-launch failures; uncertain dispatch
  cannot fall back or resubmit.
- Solo native-chat sandbox commands require an exact approval showing immutable
  image, argv and timeout, transfer no local files, preserve actual exit codes,
  and cancel/observe the original operation when stopped.
- Cloud Account usage distinguishes reported charges from reserved inference,
  sandbox and app-action charges. Missing billing data stays unavailable.

## Verification

- Desktop and daemon `cargo check --offline` passed with the native bridge.
- Native command suite: 22 passed, one ignored Linux-worker fixture test.
- Native harness terminal receipt regression passed.
- Eight managed native regressions pass. Account scope tests verify persistence
  after restart, changed login rejection and delayed-action identity checks,
  workflow stage inheritance and a later routine run using the saved owner.
  A separate rerun verifies routine edits cannot change that owner and a denied
  edit rolls back the definition revision.
- Credential tests cover account replacement before refresh, managed app routes,
  sandbox route escape rejection and exact account binding: five passed; one
  operator-login acceptance test remains ignored in the ordinary suite.
- Mobile suite: 428 passed, 27 skipped native-fixture acceptance tests; TypeScript
  passed. Skipped checks are not counted as acceptance.
- The read-only native credential-vault live check was attempted and stopped
  before HTTP because this app's native account is not signed in. Earlier shared
  TypeScript Auth0 checks are recorded separately in [119](119-live-gateway-pilot.md).

Correctness-only native tests used `profile.test.package.waku-core.opt-level=0`
to avoid repeating lengthy optimizer passes. Production release optimization is
unchanged. No native visual testing or watcher restart was performed.

## Live acceptance still required

1. Sign in inside the rebuilt desktop app and authorize one real app connection;
   verify the actual Composio action log ID and account switching.
2. Enable the intended Nebius sandbox caller permissions and register inspected
   immutable worker images with verified price/retention ceilings. Current
   `whoami` accepts the saved key but all relevant execution permissions are false.
3. Activate Cloudflare R2. Current bucket-list requests return code 10042.
   HTML/app publication remains unavailable until storage and an isolated runtime
   controller are configured and actually tested.
4. Reconnect Moto G32, then accept native login, pairing and microphone permissions.
   ADB currently reports no connected device. No iOS work is required.
5. Accept Windows clean-install/upgrade, real microphone and paired-phone behavior
   using newly packaged artifacts. Do not substitute source fixtures for devices.

No Nebius instance, sandbox, image import, disk or app worker was created for this
increment. No new paid inference or app action was executed. Live service switches
were left unchanged. Work is not declared end-to-end complete while these gates
remain open.

## Gateway deployment

Recovery hardening source `bc9d21c` is deployed as Worker version
`33865782-96d7-4d4b-b02d-1811d10dd615`. Public `/v1/health` and bootstrap
return 200; anonymous effect/sandbox requests and a spoofed catalog request
return 401. Bootstrap reports `managedSandbox: false`, matching the absence of
a reviewed production sandbox policy. No live service control was changed.
