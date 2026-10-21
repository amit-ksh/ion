# Initial source audit and visual evidence gate

Reviewed the changed native/mobile source since `c9d94d4` and the new Work,
Settings, Apps, agent/team editor, workbench, definition hook, catalog and
hierarchy source. No source was edited and no build, watcher, UI action,
account operation or device action was run. The parent is applying fixes;
line references below describe the source at the initial review.

## Formal finish disposition

`disposition: recapture`

The finish-reviewer contract requires fresh captures before its five-section
visual review. `.impeccable/review/` has no fresh native captures. Historical
`docs/qa/2026-10-18-ion` images cannot prove this build. Required evidence is
the freshly rebuilt desktop application and Android phone, plus expanded
Android/tablet if shipped, in light/dark and with the requested compact/large
text checks. The parent reports no current Ion window available yet, no
Android emulator and a connected device reserved by another session. This
is a validation limitation, not a claim that either build rendered correctly.

## Supplemental source findings

This source audit is separate from the formal visual gate. Its initial source
disposition is **fix**; it does not grant visual acceptance.

1. **P1 — Every new managed Apps operation is rejected locally.**
   `apps/mobile/src/lib/cloud-account-controller.ts:93–104` permits bootstrap,
   speech, service and hosting routes only. The new catalog/connect/disconnect
   calls in `apps/mobile/src/app/apps.tsx:27,39` therefore always throw
   `Invalid managed service request` before reaching the existing cloud
   endpoints. Add the exact three Apps routes to the allowlist and exercise
   the actual request boundary, including rejection of unrelated paths.

2. **P1 — Refresh silently bypasses concurrent-edit protection for drafts.**
   `apps/mobile/src/app/agent-editor.tsx:36–43` and
   `apps/mobile/src/app/team-editor.tsx:26–30` hydrate once per owner/id;
   refreshing or returning to the screen updates the snapshot without
   updating the form. `apps/mobile/src/hooks/use-workspace-definition.ts:25`
   then saves the old draft using that newer snapshot revision. A desktop
   change can consequently be overwritten by a save that passes the revision
   check. Bind the expected revision to the hydrated draft and require
   reconciliation/reload when a fresh snapshot differs. Do not replace the
   draft's base revision merely because a query refreshed.

3. **P2 — Reconnection can strand a definition in Saving.**
   `use-workspace-definition.ts:19` resets busy/lock on scope changes only;
   `:35` clears them only if the original client is still active. A connection
   replacement for the same profile/address during a pending save makes both
   conditions false, leaving the form locked until remount. Reset or retire
   an operation token when the client changes as well as the scope, preserving
   protection against stale completions.

4. **P2 — Agent required-field validation misses instructions.**
   `apps/mobile/src/app/agent-editor.tsx:46–47` enables Save for a name and
   harness with blank instructions, but
   `crates/waku-core/src/control.rs:668` rejects that record with
   `An agent needs instructions`. Mark the field required and include trimmed
   instructions in validity, with useful local recovery copy.

5. **P2 — Expanded Android still uses the compact bottom bar.**
   `apps/mobile/src/components/workbench-navigation.tsx:32–35,49` only adds
   horizontal padding at 600dp; it never changes the bottom row to a navigation
   rail or drawer. Compose the shell by window size so expanded Android uses
   its native navigation pattern, then verify both layouts on real Android
   captures. No tablet appearance was inferred from the source.

6. **P2 — Light warning/error text falls below AA.**
   The semantic roles in `apps/mobile/src/constants/theme.ts` are used as
   normal-size validation/error text in the new editors. Warning `#a66b20`
   on canvas `#f5f6f2` is **4.077:1**, and danger `#c64a42` is **4.339:1**;
   on muted `#ecefe8` they are **3.810:1** and **4.054:1**. Desktop mirrors
   those colors. Darken the light semantic foregrounds and cover them in
   `scripts/ion-design.test.ts`, whose existing loop omits semantic roles.

7. **P2 — Composer touch expansions overlap adjacent targets.**
   `apps/mobile/src/components/mobile-composer.tsx:976` uses a 2dp cluster
   gap while icon/send controls add 4dp hit slop. Although their nominal
   targets are now 48dp, adjacent expanded hit regions overlap and fall short
   of the Android reference's 8dp spacing. Keep at least 8dp between controls;
   validate the resulting wrap with recording/finalizing states at 320dp and
   large text.

## Source observations to preserve

- PRODUCT.md exists and describes the actual native/adaptive product. The
  local Impeccable config declares a code build; no approved generated comp
  was supplied, so an open comp phase is not evidence of a skipped approval.
- The shared desktop/mobile colors, restrained primary fills, original Ion
  geometry, labelled Work/Settings destinations and explicit Local/Nebius
  controls form a coherent source direction. Their rendered finish remains
  unscored.
- The team graph validator rejects duplicates, missing managers, cycles and
  missing agents. Preserve these invariants while fixing draft concurrency.
- New owner mutations use revision checks and try to suppress stale-owner
  completions. Preserve that intent while binding state to draft and client.
- Dashboard/Inbox were removed from primary mobile history navigation;
  Decisions is reached from Work and Usage from Settings. The old Inbox route
  remains an implementation route rather than an advertised destination.
- No new adapters, server/cloud work, iOS implementation or publishing path
  appears in the reviewed diff. Existing static publishing behavior was not
  executed or re-proven by this source audit.

## Verification gaps

No current screenshots, approved comp, QUALITY BAR card or original seven
reference PNGs were provided to this reviewer. Direction was read from
PRODUCT.md, the implementation note and the parent's supplied brief. There
was no HTML detector because this is native. The parent reports native
compile, Android TypeScript, Hermes export and focused tests passing before
the latest edits; these are build/source evidence, not device acceptance.

Still require native keyboard traversal/visible focus, Android system Back,
IME/insets, 320dp composer/microphone states, large font scale, expanded
navigation, semantic colors, real Apps OAuth/status/disconnect and stale
owner/reconnect/conflict recovery. Long transcript performance, provider
payload ordering, workflow/routine behavior and artifact/static publication
were not re-executed by this audit.
