# Mobile Ion Cloud login — 2026-10-19

## Implemented in `codex/ion-cloud-auth`

The native phone client now connects the shared login transport to the existing
first-use screen and **Daemons** connection settings. The existing direct-daemon
and self-hosted personal-gateway paths remain available.

- **Sign in to Ion Cloud** opens the system authentication browser with PKCE,
  exact callback/state validation and a fresh login prompt.
- The app validates the configured cloud bootstrap against the pinned Auth0
  issuer, API audience, mobile client ID and managed gateway address.
- Account identity comes from the API's verified access token. It does not use
  an unverified ID-token profile or email to select a workspace.
- Access and rotating refresh tokens are kept in Expo SecureStore. Account
  secrets are not stored in AsyncStorage, tasks, artifacts or browser storage.
- Refreshes are serialized. A persisted in-flight marker makes a crashed or
  ambiguous refresh require sign-in again instead of replaying the token.
- **Connect this phone** grants read access. **Allow task controls** explicitly
  adds provider approval review, task steering/stopping and workflow/routine
  controls. Updating access revokes the old device before replacing its grant.
- Managed gateway requests send both an account access token and a paired-device
  grant. Account/origin mismatch rejects before dispatch. Writes are not replayed
  automatically after a lost response.
- Switching accounts removes the old managed phone binding and task cache.
  Sign-out clears account access before remote revocation, removes the managed
  pairing/cache and resets cloud navigation. Offline sign-out reports that remote
  revocation could not be confirmed.
- A self-hosted pairing is never silently overwritten by cloud sign-in.
- Web uses the existing nonpersistent/direct behavior; native account login is
  deliberately unavailable there.

No user authentication database or client secret was introduced.

## Operator configuration

Set this **public build-time** value when building the native app:

```powershell
$env:EXPO_PUBLIC_ION_CLOUD_ORIGIN = "https://YOUR-ACTUAL-CLOUD-HOSTNAME"
```

This must be the deployed service's HTTPS origin. **Do not use the Auth0 API
audience `https://ion-cloud-api` as a substitute hostname.** No real service
hostname has been supplied yet. An unconfigured build displays that cloud
sign-in is unavailable and retains direct connections.

The mobile Auth0 Native application must allow:

```text
ion://auth/callback
```

Its client ID is `bZ4cOhwaVD4N9A1g882surgTm2kHaSyq`. The app now registers the
`ion` scheme while retaining the existing `waku` compatibility scheme.
Confirm Authorization Code and rotating Refresh Token grants, API offline access,
the `ion:gateway` permission and the intended email connection in Auth0.

The new `expo-web-browser` dependency and callback scheme require a **new native
build**. The previously delivered APK has not been replaced by this change.
Expo Go cannot stand in for the app's registered production callback scheme.

## Verification

The focused cloud/client/mobile account suite currently has **40 passing tests**,
including a full phone account workflow against the real managed gateway handler
using locally signed test JWTs. It covers read access, enabling controls, old-grant
revocation, logout, account switching, self-hosted compatibility, offline behavior,
bootstrap substitution, refresh concurrency and crash recovery.

Mobile, cloud-client and cloud TypeScript checks pass. The current Android Hermes export succeeds with 2,338 modules; compiled-string/source-map checks confirm the new login routes, managed headers and account audience/origin binding. This is a bundle check, not a new APK or device acceptance. The mechanical UI source
scan reported no findings. No visual tests were run because repository guidance
requires an explicit request.

Real Auth0 login, native browser return and SecureStore behavior on the owner's
iPhone 15 and Moto G32 still require device acceptance. Test JWTs do not establish
that Auth0 dashboard configuration or native installation is correct.

## Owner/device acceptance

- [ ] Deploy the managed gateway and configure its actual HTTPS hostname.
- [ ] Confirm the mobile Native client, callback, email connection, scope,
      rotating refresh and offline access in Auth0.
- [ ] Build/install a new native app with the public origin above.
- [ ] Cancel browser login: no account/device should be saved.
- [ ] Sign in and connect with task controls off: reads work, controls are absent.
- [ ] Enable controls and update access: the previous phone grant is revoked.
- [ ] Background/reopen the app and verify a refresh uses the saved account.
- [ ] Switch to another account: previous tasks and phone binding disappear.
- [ ] Sign out online: account access, phone pairing and cached tasks disappear.
- [ ] Sign out offline: local access clears and remote-revocation uncertainty is
      reported; review/revoke the device when online.
- [ ] Confirm direct daemon/self-hosted gateway connections still work.

## Remaining managed-platform implementation

Desktop system-browser login, cross-process OS-vault refresh coordination and
managed publisher headers are now implemented; see [implementation 110](110-desktop-cloud-login.md). Hosted speech,
HTML publication and app deployment follow the account/device connection work.
No Nebius resources or live service deployment were created by this increment.

## Worktree bundle verification

This Windows worktree reuses dependencies across C: and D:. The first Metro export resolved a shared Expo Router entry without the app routes and was rejected. A worktree-local Expo Router copy restored the complete 2,338-module graph; source-map and compiled-string checks confirm the native account adapter, settings UI, callback route and managed device headers are included. These local dependency copies and bundle outputs are ignored; a fresh normal installation keeps dependencies and app source together.
