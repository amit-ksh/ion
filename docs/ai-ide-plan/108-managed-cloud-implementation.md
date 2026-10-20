# Managed Ion Cloud implementation — 2026-10-19

## Branch and scope

- Branch: `codex/ion-cloud-auth`.
- Worktree: `C:/Users/ksami/.codex/worktrees/ion-cloud-auth/waku`.
- Base: `ion-prod`, commit `2b79018d4532d35fe56020ff14bdbeac1b473ed9`.
- The main checkout's uncommitted QA/development changes were not copied or
  modified. Only the two managed-platform planning documents were carried over.
- This is the first implementation increment of plan 107. It does not mean all
  managed cloud features are complete.

## Completed

- [x] Confirm public Auth0 OIDC discovery matches the supplied tenant.
- [x] Add API access-token verification with pinned issuer/audience/RS256,
      native-client allowlist, required scope and cached JWKS.
- [x] Bind personal workspace identity to verified issuer/subject; ignore email
      and caller-supplied workspace selectors for authorization.
- [x] Reuse the existing gateway's pairing, grant checks, revocation, exact
      approvals and durable relay state under per-account isolation.
- [x] Separate reusable gateway runtime from its standalone network listener.
- [x] Add persistent pilot admission cap and graceful request draining.
- [x] Add public bootstrap and authenticated account session endpoint.
- [x] Add native PKCE transport with system-browser handoff, one-use state,
      exact callback validation, expiry/cancel/supersession and API-verified identity.
- [x] Add a loopback-only service entry point and operator deployment examples.
- [x] Document current contract and the compatibility boundary with personal
      self-hosted gateways.
- [x] Keep unavailable hosted speech, publishing and deployment capabilities
      disabled in bootstrap.

## Verification

- New cloud/client tests: **15 passed**, 0 failed.
- Existing personal gateway suite: **74 passed**, 1 existing skipped test,
  0 failed. The existing skip concerns HTML source HTTP evidence/revocation.
- TypeScript checks pass for cloud, cloud-client and personal-gateway packages.
- Actual ephemeral loopback listener: health 200, bootstrap 200,
  unauthenticated session 401.
- No visual tests were run, in accordance with the repository instruction.
- A real Auth0 sign-in has not been performed: client/API/connection/callback
  dashboard settings cannot be proven by public discovery.

Run commands and configuration:
[cloud README](../../packages/ion-cloud/README.md).

## Remaining implementation, in order

Native phone and desktop login, renewable sessions and managed publisher headers
are implemented. See [phone integration](109-mobile-cloud-login.md) and
[desktop integration](110-desktop-cloud-login.md).

The managed speech, HTML/static publication and selected HTTP app deployment
paths are now implemented in source. See [implementation 111](111-managed-services-app-integration.md).

Remaining work before pilot acceptance:

1. Configure the real Nebius hosts, DNS/TLS, runtime/model artifacts and budget.
2. Run the deferred API/security, model, deployment and native-device checklist.
3. Build updated Windows and native mobile packages after operator configuration.
4. Per-user managed Composio ownership is a separate increment; operator-wide
   discovery/execution remains disabled across managed tenants.

Unstarted harness adapters remain outside the agreed scope.

## Owner setup and review checklist

- [ ] Confirm both supplied Auth0 client IDs are Native public applications.
- [ ] Confirm the API identifier is `https://ion-cloud-api`, with RS256 and the
      `ion:gateway` permission; enable Authorization Code / Refresh Token grants,
      offline access and rotating refresh tokens.
- [ ] Confirm the intended email connection is enabled for both clients.
- [ ] Register desktop `http://127.0.0.1:43891/ion/callback` and mobile
      `ion://auth/callback` in their respective Auth0 Native clients.
- [ ] Select the real cloud HTTPS hostname. The audience identifier is not that
      hostname.
- [ ] Supply Nebius deployment target and budget before provisioning resources.
- [ ] Test desktop login and the same account on iPhone 15 and Moto G32.
- [ ] Test another account cannot see tasks, devices, documents or approvals.
- [ ] Test expired login, refresh failure, device revoke, app restart and
      offline desktop behavior before release.
- [ ] Accept streaming speech / publishing / app deployment after their
      implementation increments. None is deployed by this change.

## Cleanup completed

All eight older Git worktrees were clean and their HEAD commits were already
ancestors of `ion-prod`. Their branches remain available. Their ignored
diagnostic caches were preserved as eight ZIP archives under:

`D:/personal/apps/waku/.waku-cache/retired-worktrees-20261019/`

Retired worktrees:

- `ion-agent-teams`
- `ion-billing-discovery`
- `ion-gateway-billing`
- `ion-gateway-pdf`
- `ion-phone-html`
- `ion-resource-admission`
- `ion-speech-acceptance`
- `ion-speech-deploy`

The managed archive tool returned queued requests; Git checkout removal was
completed after explicit path, clean-state and merged-history checks. App
attachment metadata may take time to reconcile. The old agent-teams cache
archive was verified against all 3,820 source files before removal. Its shared
dependency/contract junction targets were retained.

About **713 MB** of additional redundant files were removed: downloaded Android
command-tools ZIP, empty extraction directory, duplicate JS export, Gradle
thread dumps, obsolete 0.1.19–0.1.22 installer extraction copies, installer
scratch files and empty Bun temp folders.

Preserved: APK in `dist/mobile`, Android SDK/Gradle caches, main dependencies,
current app/QA data under `temp`, credentials, source changes and current
installer/test outputs. Further cache removal can be evaluated after active QA
and packaging work ends.

## Follow-up: native phone integration

Phone browser login, SecureStore sessions, rotating refresh, consent-based pairing and managed gateway headers are now implemented. See [implementation 109](109-mobile-cloud-login.md). Desktop login and publisher integration are implemented in [increment 110](110-desktop-cloud-login.md); live acceptance still needs the hostname and Auth0 callback setup.
