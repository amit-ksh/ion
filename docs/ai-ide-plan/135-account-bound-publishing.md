# Account-bound native publishing

21 October 2026. A source audit found that native publication preparation and
mutations ran in the background through the generic current-account transport.
A sign-in change between opening review and dispatch could therefore attribute
an old upload to the replacement account. UI generation checks discarded late
results, but did not themselves bind the authenticated outbound effect.

## Fix

- Capture the cached Ion workspace when opening the publication review.
- Carry that workspace through initial listing, uploads, view tickets, visibility,
  unpublish/republish, version removal and the post-action listing.
- Use a dedicated hosting transport that validates method/path/workspace, checks
  identity before refresh and again after refresh, and constructs the request
  using that account's token. A replacement identity is rejected before refreshing
  it; expired, missing or foreign-refreshed sessions cannot authorize the upload.
- Remove hosting routes from the generic service transport to prevent accidental
  reuse of the current-account path. Existing spending-control/speech calls remain.
- Account invalidation closes the publication view and clears its retained source,
  snapshot and workspace. File reads and network work stay off the UI thread.
- The opt-in native production publication test now uses the same bound transport.

## Verification state

- Added fixture regression for refresh, account replacement/logout and invalid
  publishing methods/routes before dispatch.
- Added a separate regression for a foreign identity returned during refresh.
- The initial account replacement/path regression passed (one test), and
  `cargo check --offline -p waku` passed with the complete native UI/transport fix.
- The broader account fixture suite passed **20 tests, zero failures**, including
  both publishing regressions. Seven opt-in live tests remained ignored; the
  isolated real Windows-vault probe was explicitly excluded because host approval
  access is unavailable. These are fixture results, not fresh live-vault acceptance.
- Logs: `.waku-cache/ion-publishing-account-cloud-regressions.log` (exit 0) and
  `ion-publishing-account-desktop-check.log` (successful desktop compilation).
  The first targeted regression log is retained separately.
- Job handles/logs are recorded in `.waku-cache/ion-managed-active-jobs.json`.
  That initial fixture run did not include GUI acceptance or a live vault probe.
- Approval access returned at the scheduled 7:05 AM continuation. The actual
  signed-in native account passed the opt-in static publication test through the
  new bound transport: private denial, idempotency, HTML/asset bytes, SPA routing,
  server rejection, unpublish and republish. The disposable site was deleted,
  owner controls stayed unchanged and no paid compute was called.
- Live evidence: `.waku-cache/ion-account-bound-live-publication.json` (passed,
  cleanup confirmed) and the corresponding log/exit marker (one live test passed).
  This verifies the real vault/network request path; account replacement and
  foreign-refresh rejection are covered by the fixture regressions above.
- The isolated real Windows Credential Manager test subsequently passed under
  restored host access: one test, dummy entries only, automatic namespace cleanup.
  Evidence: `.waku-cache/ion-account-bound-native-vault.log` and exit marker.
- A subsequent live native managed-service check passed after the hosting routes
  were removed from the generic transport. It read the signed-in account's
  connected-app catalog, service controls and combined usage, checked the combined
  micro-USD representation and recorded spending below the implementation ceiling.
  Its conditional OFF-denial checks ran according to the controls returned; the
  test does not print those controls, so this result alone is not a new snapshot
  asserting every switch is OFF. No service was enabled, no provider tool was
  executed and no model generation or compute launch was requested. Evidence:
  `.waku-cache/ion-account-bound-managed-service-read.log` and exit marker,
  one ignored opt-in test passed. Provider invoice reconciliation remains open.
- Refreshed Windows release packaging and nonvisual upgrade acceptance passed;
  see [137](137-account-bound-windows-release-acceptance.md). Interactive
  account-switch acceptance remains pending. No approval bypass was attempted.

The package in plan 134 predates this change and is preserved with its original
hashes/source record. Plan 137 contains the refreshed candidate. Live backend publishing was already
accepted in plan 131; the subsequent live test above verifies the newly edited
native request path. The mobile counterpart is recorded in plan 136.
