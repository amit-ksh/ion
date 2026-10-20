# Desktop Ion Cloud sign-in and publishing

Implemented in `codex/ion-cloud-auth`, based on `ion-prod`.

## User flow

1. Open **Settings → Models & Speech → Ion Cloud**, or the **Remote access** onboarding step.
2. Choose **Sign in**. Ion opens the system browser for Auth0 email login.
3. Return to Ion, then explicitly choose **Connect this computer**.
4. Existing task, review, automation and document publishers use this account. The computer receives only the `publish` device grant. Phone controls remain a separate phone consent choice.
5. **Disconnect computer** stops local cloud publishing while retaining login. **Sign out** removes the saved account and device before remote revocation. Offline or damaged-state revocation uncertainty is reported.
6. Local coding tasks remain usable without a cloud account. All onboarding steps can still be skipped.

A personal/self-hosted pairing is preserved. Remove it before linking Ion Cloud; disconnect Ion Cloud before pairing a personal gateway. Managed service-file pairings retain priority and are never silently replaced. A corrupt saved cloud account exposes **Clear saved sign-in** for recovery.

## Configuration required before live login

Use the actual deployed API HTTPS origin, with no path or trailing slash:

```powershell
$env:ION_CLOUD_PUBLIC_ORIGIN = "https://YOUR-ACTUAL-CLOUD-HOSTNAME"
```

Set it in the environment of **both desktop and daemon**. Release packaging can embed the same public value at compile time; a runtime value overrides it. No hostname is invented or shipped as a working service. Without it the account card reports cloud login unavailable.

| Setting | Value |
| --- | --- |
| Issuer | `https://dev-amit-ksh.au.auth0.com/` |
| Audience | `https://ion-cloud-api` |
| Desktop Native client | `M7iCZjpo5bzOVO2E5nrvKzDJ00p3ZWy0` |
| Desktop Allowed Callback URL | `http://127.0.0.1:43891/ion/callback` |
| Mobile Allowed Callback URL | `ion://auth/callback` |
| API scope | `ion:gateway` |

The audience is an API identifier, not the cloud service hostname. Confirm Native public applications, Authorization Code and Refresh Token grants, API offline access, rotating refresh tokens and the intended email connection. There is no client secret, password handler or user authentication database.

Sign-out revokes tokens and the computer grant; it does not call a browser logout endpoint. Login requests use `prompt=login`. Do not register a fabricated logout URL or assume browser cookies have been erased.

## Native implementation

- Authorization Code + PKCE S256, cryptographically random verifier/state and exact callback validation. Bootstrap must match the pinned Auth0 configuration.
- The loopback callback listener accepts only the expected path, state and Host. Login can be cancelled, has bounded HTTP reads and expires after five minutes. Codes, tokens and provider error details never appear in callback pages.
- Session identity is verified through the authenticated cloud `/v1/session`. Tokens are bound to the cloud origin, issuer, audience, client and workspace.
- Tokens live only in the OS vault. Two bounded chunk slots and an atomic manifest commit accommodate Windows Credential Manager's [2,560-byte limit](https://learn.microsoft.com/en-us/windows/win32/api/wincred/ns-wincred-credentialw). Each ASCII chunk is at most 1,200 characters / 2,400 UTF-16 bytes. Explicit sign-out also removes orphaned chunks from interrupted transactions.
- Desktop and daemon coordinate with the same OS file lock. Refresh persists a one-shot marker before consuming a rotating token; a crash, lost response or failed token commit requires a new login and never replays that token.
- Managed gateway requests send account JWTs in `Authorization`, with `X-Ion-Device-Token` and `X-Waku-Device-Id` for the computer grant. The personal gateway keeps its original contract.
- Every managed request from an existing/cached client rechecks saved identity and device capability under the lock before dispatch. Sign-out and grant replacement prevent subsequent requests from stale publishers.
- The existing desktop task bridge and daemon automation/portfolio/document publishers share this transport. Authenticated gateway capability discovery is included, so daemon relay detection works in managed mode.
- UI renders cached metadata only. Vault, lock, callback and HTTP work runs on background workers. Controls have persistent keyboard focus, visible focus and Enter/Space handling.

## Verification

Commands from the worktree root (reuse an existing target directory if needed):

```powershell
cargo test -p waku-credentials --offline
cargo check -p waku -p waku-daemon --offline
bun test packages/ion-cloud/src packages/ion-cloud-client/src apps/mobile/src/lib/cloud-account-controller.test.ts apps/mobile/src/lib/gateway-account-auth.test.ts
bun run --cwd packages/ion-cloud check
bun run --cwd packages/ion-cloud-client check
```

**35 native tests pass**, including the real Windows vault fixture. **36 cloud/client/mobile account contract tests pass**. Cloud/client TypeScript checks and desktop/daemon Rust checks pass. Native tests cover PKCE vectors, real loopback callbacks, cancellation/account replacement during token exchange, corrupt storage, bounded atomic vault writes, ambiguous refresh recovery, concurrent requests, distinct-process locking, publisher HTTP headers/routes and cached publisher rejection after sign-out. A Windows-only test round-trips large dummy data through an isolated vault namespace and removes it afterwards. It does not use the user's saved account.

No visual tests or installed-app acceptance were run. The existing dev watcher was not restarted. A type-check is not an installer or device acceptance.

## Owner acceptance checklist

- [ ] Deploy the API on a Nebius target and configure its real HTTPS hostname.
- [ ] Register the exact desktop callback above in the supplied Native client.
- [ ] Confirm email connection, API scope, offline access and refresh rotation.
- [ ] Build/launch desktop and daemon with the same public origin.
- [ ] Cancel browser login; no tokens or computer grant should be saved.
- [ ] Sign in, explicitly link the computer, restart and verify saved login.
- [ ] Open the same account on iPhone 15 and Moto G32; verify published tasks.
- [ ] Exercise a routine/workflow and document preview through the daemon.
- [ ] Disconnect/sign out while desktop and daemon are active; further relay calls must stop. Test offline sign-out and review/revoke an uncertain device.
- [ ] Sign in with another account; prior account tasks and grants stay isolated.
- [ ] Verify self-hosted pairing and local coding tasks still work.

Hosted speech, HTML artifact publication, static/application deployment and per-user managed Composio ownership are separate implementation increments. This change creates no paid resources, public deployment, installer or APK.
