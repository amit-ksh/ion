# Desktop and Android redesign integrated into ion-prod

21 October 2026. The user explicitly authorized this merge in the implementation
chat after referencing **Redesign Ion desktop and mobile**. That chat was read
before integration. Source branch `codex/ion-native-redesign`, commit `81827ff`,
is now an ancestor of `ion-prod` through merge **`fc956df`**.

## Included and preserved

- Full committed desktop/Android design, tokens, app identity/icons, workbench,
  editors, navigation, appearance control and native sheet refinements.
- Current implementation work: account-bound publishing/service controls,
  recording Stop recovery, concise MCP errors, Android model labels and stale
  connection fencing, artifact-only Deploy, mention placeholders, hidden
  Tasks/Projects work links, Settings label and native menu icon fixes.
- Owner relay batching/idle milestone `29021c9` and its tests remain included.
  The wider Cloudflare usage task in 154/156 is **not complete**.

Ten conflicted files were resolved individually. Visual changes came from the
redesign while keeping current account, connection and cache safeguards.
Mobile Apps now uses the existing **workspace-bound** service transport for its
three POST routes; generic requests cannot bypass those account checks. Changing
accounts rejects a delayed connect/disconnect before network dispatch.

Both findings in review 150 are corrected: confirmed app disconnect invalidates
the completed connect operation, and incomplete remote revocation remains
visible after local sign-out. The actual callbacks plus ManagedApps/ServiceStore
fixture created two distinct links across connect/disconnect/connect and one
revocation warning. No live provider operation was performed.

## QA preservation

Before integration, the 111 tracked QA changes had SHA256
`8014669bb58260b00dcf000f846689680e51699d6ce24e84a4e2d7cd3fc22947`.
They were backed up and saved to recovery stash
`33b5e3fb961ccb590c6438b83857964cd4f13e06`. The stash applied cleanly after the
merge. Those changes were already represented in the redesign's inherited QA
snapshot; no additional tracked diff remained after restoration.

Six colliding untracked source files matched incoming content and are now
tracked. Their original bytes remain in the workspace safety snapshot.
All 821 other recorded untracked paths still exist. The recovery stash/snapshot
is retained until release acceptance. The separate redesign checkout was not
edited or archived.

## Verification

- Mobile TypeScript check: passed after resolving the merged POST method type.
- 42 focused tests / 357 assertions: passed, covering design contrast/assets,
  managed account/auth isolation, Apps, automation recovery, execution
  capabilities, draft revisions, team hierarchy, error summaries and device
  labels.
- Actual Apps/sign-out callback/backend fixture: passed both corrections.
- Seven merged Rust files: syntax parsed successfully with rustfmt; this is
  **not** a full Rust compilation or native UI acceptance.
- Conflict-marker scan and staged whitespace checks: passed.
- Merge ancestry and QA/untracked-file preservation checks: passed.

## Remaining release gates

Fresh full native compilation/protocol export, Windows/Android packages and
upgrade acceptance are still required. Prior packages do not contain this
merged redesign. Memory was below the existing package-build guard; no extra
APK, installer, physical microphone test or visual/device acceptance was claimed.
Carry forward desktop visual/keyboard, real microphone/paired-phone, TalkBack,
live Apps OAuth/account-switch and performance checks from the existing queue.
No live service was redeployed and no paid resource or permission was changed.
