# Mobile publishing account boundary

21 October 2026. Follow-up to the desktop fix in plan 135. The mobile publication
review checked its original account, but the generic request transport then
authorized the current session independently. A queued confirmation or operation
could therefore use a replacement account after that earlier check.

## Implementation

- Introduce a dedicated mobile hosting request that requires the original
  workspace, validates the route/method and authorizes against that exact
  workspace through the serialized native session boundary.
- Remove hosting from the generic managed-service transport. The browser adapter
  still rejects native managed services.
- Carry the account from artifact review into upload. Attach the listing's local
  workspace to every returned site; view tickets, visibility, unpublish/republish,
  rollback, inactive-version deletion and follow-up listing use that scope.
- Discard late post-action listings and version-removal results after selection,
  account or screen generation changes. Serialize deletion with the other controls.
- Invalidate old account views on sign-in replacement even when the phone has not
  created a managed gateway pairing. Preserve personal gateway pairing data.

## Verification

- Account-controller and shared session regressions: **16 passed, zero failures**,
  132 assertions. New cases cover original-account dispatch, stale account/logout
  denial before network access, view invalidation without a paired phone, invalid
  routes/workspaces and rejection through the generic transport. Existing session
  tests cover rotating refresh, foreign refresh identity, concurrent requests,
  interrupted refresh and queued logout denial.
- Mobile TypeScript check passed after the final source changes.
- Evidence: `.waku-cache/ion-mobile-publishing-account-tests.log` and
  `ion-mobile-publishing-account-final-typecheck.log`, with exit markers.
- These are local transport/session tests. The rebuilt APK, signature, installed
  hash and nonvisual Moto upgrade/cold start subsequently passed in
  [138](138-account-bound-android-release-acceptance.md). Real account switching,
  confirmation dialogs and publication controls remain acceptance gates. No new
  phone login, provider app effect or paid compute was performed.

The Windows release refresh started from `d7e69ad` before these mobile-only source
changes. Keep its exact provenance; the Android refresh in plan 138 includes this milestone.
