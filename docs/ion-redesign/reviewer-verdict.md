# Source verdict after 98f48cb

This pass scores only the seven findings in `reviewer-initial.md` against
commit `98f48cb` and the current files. No source edits, builds, tests, device
operations or new broad audit were performed.

## verdict

1. **Resolved — Managed Apps request boundary.**
   `apps/mobile/src/lib/cloud-account-controller.ts:99` now permits exactly
   `/v1/apps/catalog`, `/v1/apps/connect` and `/v1/apps/disconnect`.
   `cloud-account-controller.test.ts` exercises all three through the actual
   controller and verifies unrelated Apps routes, traversal and query suffixes
   are refused before network activity. This resolves the local feature
   blocker; real OAuth and account status still require runtime verification.

2. **Resolved — Draft concurrency.**
   `apps/mobile/src/lib/definition-write.ts` refuses to adopt a newer revision
   for an existing draft. Both editors bind the revision during hydration,
   disable Save when the current revision differs and expose an explicit
   `Reload from desktop` action. Reload is the action that replaces the draft.
   `definition-write.test.ts` covers an unseen desktop edit, unbound draft and
   uncertain prior save. The source no longer silently overwrites refreshed
   desktop changes under a newer expected revision.

3. **Resolved — Reconnect lock.**
   `apps/mobile/src/hooks/use-workspace-definition.ts:22` now retires the
   operation and clears busy/lock on both scope and client changes. Success,
   failure and cleanup are guarded by the operation ticket and original
   scope/client, preventing old completions from clearing a newer operation.
   Reconnection on the same owner no longer leaves the initial save locked.

4. **Resolved — Required instructions.**
   `apps/mobile/src/app/agent-editor.tsx:51` includes trimmed instructions in
   validity, and the field is labelled `Instructions (required)`. The form
   no longer offers Save for the blank-instructions record the daemon rejects.

5. **Resolved in source — Expanded Android navigation.**
   `apps/mobile/src/app/_layout.tsx:103` composes a left navigation rail at
   600dp and wider. `workbench-navigation.tsx` supplies an actual column rail
   with a right separator and safe-area top inset; compact layouts retain
   the labelled bottom bar. Phone/tablet rendering and large-text fit remain
   unverified on Android hardware or emulator.

6. **Resolved — Light semantic foreground contrast.**
   The mobile and desktop light warning/danger/success roles now use
   `#875014`, `#ac3933` and `#237640`. `scripts/ion-design.test.ts` includes
   semantic foreground roles across the six workbench backgrounds in both
   themes. This corrects the initial measured AA failures. Rendered disabled
   states and composited surfaces still belong to device validation.

7. **Resolved in source — Composer target spacing.**
   `apps/mobile/src/components/mobile-composer.tsx:978` has the requested
   8dp outer cluster gap. `phone-dictation-control.tsx:162` now also has an
   8dp group gap. Adjacent 48dp controls with 4dp hit slop therefore meet at
   their expanded edges instead of overlapping, including when the recording
   text is empty. Narrow/large-text recording states still need device checks.

## remaining

- All seven listed findings are resolved in source. Their device-dependent
  behavior is not certified, and this source verdict covers only those fixes.
- Formal visual disposition remains **recapture**. Fresh
  `.impeccable/review/desktop-home-dark.png` and
  `.impeccable/review/desktop-teams-dark.png` exist, but the full desktop
  and Android viewport/theme/state evidence is still missing. The Teams
  capture was inspected only for the reported crash path; no full-surface
  visual acceptance is implied.
- **Resolved for the reproduced crash path — Desktop primary-button hover
  registration.** The current 1382×864 Teams capture shows Work > Teams
  rendered with its New team/New agent controls and empty state. The parent
  observed that exact navigation succeeding in the freshly rebuilt isolated
  app. `.waku-cache/dev-redesign-recovery.log` records launch from this
  worktree's `native-target/debug/waku.exe`, and the corresponding `.err`
  records a successful dev build in 58.93s. Together these support the fix
  of the prior Teams-opening crash. Other interactions remain outside this
  narrow check.
- **Earlier Android export evidence is disqualified.** The parent found
  zero application routes in its source maps because the cross-drive root
  `node_modules` junction prevented route inclusion. The earlier Hermes
  export cannot establish that the Android application was bundled. The
  parent reports replacing that dependency view with a worktree-local view:
  expo-router copied locally, cached external packages linked read-only and
  the `@waku` workspace packages mapped to local source.
- The replacement readable export at `.waku-cache/android-source-final`
  contains 2354 source-map modules. This reviewer directly verified that
  its `sourcesContent` exactly matches the current `_layout`, `work`,
  `agent-editor`, `team-editor`, `apps`, `workbench-navigation` and
  `definition-write` source files. This establishes inclusion of those
  current routes/modules in the readable export.
- **Final Hermes bytecode/export source proof verified.** The bundle in
  `.waku-cache/android-export-verified` is **5,342,780 bytes** with Hermes
  magic `c61fbc03c103191f`. Its map contains **2328 sources**. This reviewer
  independently verified all **16** source entries listed in
  `docs/ion-redesign/validation.json` against current files after normalizing
  CRLF, including both editors, Work, Apps, navigation, theme, composer,
  dictation, account controller and definition hook. Current file hashes and
  the bundle/source-map SHA-256 hashes also match that receipt. This proves
  current-source inclusion in the final Hermes export; it does not prove
  APK assembly, installation or Android runtime/visual acceptance.
- The parent reports the final focused tests at **19 pass, 0 fail, 213
  expectations**, mobile TypeScript exit **0**, and the earlier general
  suite at **492 pass, 28 skip, 0 fail**. These tests were not rerun by this
  reviewer. Android's isolated offline Gradle attempt lacks foojay resolver
  plugin 1.0.0; no emulator is installed and the connected device remains
  reserved. Installation, phone/tablet rendering and runtime acceptance are
  unproven.

Source disposition: **ship for the scored fixes only**. Formal visual
disposition: **recapture**.
