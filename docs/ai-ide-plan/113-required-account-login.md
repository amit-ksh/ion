# Required account login: desktop and mobile

## Scope

The owner requires login before IDE access. Desktop and native mobile are in scope; the owner explicitly deferred browser companion authentication. Optional provider/app/speech setup remains skippable after successful account login.

## Entry and session behavior

- Start locked while the saved OS-vault / SecureStore account is checked.
- Refresh expiring credentials through the existing rotating-refresh-token boundary and verify the workspace identity against `/v1/session` before unlocking.
- Display browser-based Auth0 sign-in, retry and cancellation/error feedback. There is no guest/local-mode bypass.
- Recheck the account in the background every minute. Phone foreground activation also covers the IDE while it verifies the session. Verification failures return to the locked screen; they never fall back to cached IDE access.
- Desktop sign-out locks access immediately, including when remote revocation cannot be confirmed. Account verification still runs off the UI thread.
- Desktop browser and document webviews are hidden while locked; task submission actions reject behind the gate.
- Mobile daemon/runtime/navigation providers mount only after verification, so cached routes and daemon credentials cannot expose the IDE before login. Sign-out unmounts them and clears/cancels the query cache. Notification navigation mounts inside the authenticated subtree.
- Existing daemon tasks may continue running on the host; locking the client does not cancel already-authorized background work.

## Deployment dependency

Auth0 dashboard setup is reported complete by the owner. Login also requires the deployed shared API and its `/v1/bootstrap` and `/v1/session` endpoints. Set `ION_CLOUD_PUBLIC_ORIGIN` on desktop/daemon and `EXPO_PUBLIC_ION_CLOUD_ORIGIN` for the phone build. Without a configured origin, the client shows a setup-unavailable state and keeps the IDE locked. DNS, trusted TLS and the operator deployment from document 111 remain pending.

## Deferred acceptance

- [ ] Fresh install shows only sign-in, with no task/project/preview flash.
- [ ] Valid login opens the existing onboarding / New task flow.
- [ ] Canceled, expired or replayed login callbacks leave the IDE locked.
- [ ] Valid saved credentials renew and verify; missing, invalid, revoked or ambiguous refresh credentials require login again.
- [ ] Sign-out immediately hides the IDE, documents and browser panels; offline revocation does not unlock it again.
- [ ] Phone deep links, restored routes and notifications cannot bypass login.
- [ ] Switching accounts clears cloud data and phone query caches; a stale asynchronous check cannot restore the prior account.
- [ ] Keyboard navigation, native window controls and readable light/dark login states.

Feature, device and visual acceptance remain deferred at the owner's request. A compile/type check does not replace the above acceptance checks.

## Compile/type verification (2026-10-19)

- `cargo check -p waku -p waku-daemon --offline` passed.
- Mobile, web and shared-client TypeScript checks passed.
- `git diff --check` passed.
- Feature, device and visual acceptance tests remain deferred.
