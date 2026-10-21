# Ion refinement audit

This pass follows the user's request to finish validation, replace the New agent
model textbox, add Apps icons, pin equal Save/Close actions, dismiss sheets from
outside, simplify mobile chat and paired desktops, and disable mobile cloud
services. It does not replace the earlier audit receipts with inferred passes.

## Implemented behavior

- Desktop agent models use a menu backed by cached provider probes, with a
  Harness default option. Changing harness requests discovery outside render.
- The desktop form scrolls independently of its footer. Save is primary; Save
  and Close each take half the available footer width. Outside clicks and Escape
  close the editor. An open dropdown gets its own dismissal first.
- Desktop Apps reuse available package/GitHub/document icons. Android Apps use
  the native symbol library for recognizable app categories and a generic
  extension fallback; no invented brand logos or remote image requests.
- Mobile home centers conversation: model selection above the prompt, one
  project/desktop setup entry, and the composer. The always-visible configuration
  card and repeated context syntax footer are removed. Setup remains available
  from its sheet rather than losing project/worktree/branch functionality.
- Mobile agent and team editors use fixed, equal-width Save/Close footers.
  Required fields, revision conflicts and explicit reload recovery remain.
- Paired desktops show a connection row and one options action per device.
  Edit, reconnect, disconnect and remove live in that action sheet. Account,
  gateway, cloud billing and tutorial text no longer crowd this screen.
- Mobile Shared/cloud/gateway routes are unavailable. Cloud services, speech
  sessions and hosting requests are rejected before account/network access.
  Nebius is excluded from mobile harness/model discovery and task submission;
  pinned team profiles are checked before new work/follow-ups. Account sign-in,
  managed Apps and desktop-local Parakeet dictation remain separate capabilities.
  Existing desktop cloud capabilities and saved cloud definitions are retained.

## Fresh desktop checks

The worktree's dev watcher rebuilt and launched the Windows debug binary. The
prior watcher was recovered only after process inspection confirmed it absent;
the separate Windows packaging process was left running.

At 1536 × 816, actual app interaction verified model menu opening and selection,
the primary/equal footer, footer position while scrolling, and outside dismissal.
The menu can extend beyond the sheet without dismissing it when selecting a
model. Appearance and home rendered in the light theme; the original System
preference was restored afterward. A subsequent fresh build also rendered the
populated Apps catalogue and the Workflows empty surface. At the supported
982 × 816 compact window, the workflow editor filled the available content width
and kept both footer actions visible and equal. Evidence includes
`desktop-apps-dark.png`, `desktop-workflows-dark.png`,
`desktop-workflows-compact-dark.png` and
`desktop-workflow-sheet-compact-dark.png`.

The compact check exposed an Escape focus defect immediately after opening an
editor. Trigger callbacks now focus the Name field. A scoped GPUI action binding
handles Escape from both controls and text fields, preserving dropdown dismissal
precedence. The worktree watcher rebuilt successfully, and the actual 982 × 816
app then passed focus and Escape dismissal. The before/after evidence is
`desktop-editor-focus-compact-dark.png` and
`desktop-editor-escape-compact-dark.png`.

Evidence is in `evidence/refinement/`: `desktop-agent-dark.png`,
`desktop-model-menu-dark.png`, `desktop-agent-scrolled-dark.png`,
`desktop-click-away-dark.png`, `desktop-settings-light.png` and
`desktop-home-light.png`. These establish the named interactions and surfaces,
not full provider, populated workflow or long-transcript acceptance.

## Verification status

- Native watcher build: passed; the latest successful launch is recorded in
  `.waku-cache/dev-device-audit.log` and `.err`.
- Mobile TypeScript: passed after the final reviewed policy fixes; the result
  is tracked in `.waku-cache/refinement-types-final.log`.
- Initial focused suite: 27 passed. After further changes, the combined test
  process hit an internal Bun 1.2.1 segmentation fault; it is not a test pass.
  Re-running runtime/capability tests separately passed 14 tests (47 assertions), including local
  history, steering, goal operations, team follow-ups and cloud rejection. A
  separate non-React suite passed 24 tests (225 assertions) covering palette,
  icons, Apps, account policy, revisions, hierarchy and model layout. Final
  automation persistence/policy tests passed 27 tests (93 assertions).
- Android APK: release assembly passed with 728 tasks; the isolated
  `sh.waku.mobile.redesign` x86_64 APK is 54,176,806 bytes. APK Signature Scheme
  v2 verification passed. Its Hermes bundle and source map contain the current
  edited screens and capability guards. Installation on `emulator-5580` passed,
  and Android resolved/launched `sh.waku.mobile.MainActivity` successfully.
  This is an emulator review package, not a physical-device release validation.
- Android emulator: an isolated API 36 x86_64 image and AVD were installed. WHPX
  acceleration is available. The initial Vulkan boot stalled; a diagnostic boot
  used software rendering with Vulkan disabled and reached boot-completed state.
  An Android System UI ANR appeared under the software-rendered emulator;
  choosing Wait recovered the visible Ion account gate. That environment issue
  is preserved as `android-system-ui-anr.png`; `android-account-gate.png` records
  the recovered actual app screen. No physical-device input/install was performed.
- The owner signed in on the isolated review emulator. Sign-in survived APK
  updates. It is paired to this worktree's debug daemon through an adb loopback
  mapping; no LAN exposure, physical-device install or other session takeover.
- Actual signed-in device checks passed for clean home, Apps category icons,
  paired-desktop row/options, sheet outside dismissal, model menu selection,
  equal fixed Save/Close actions and Save/reopen persistence. The selected
  `gpt-6-luna` model persisted on the saved disposable agent.
- Actual Android IME testing found the footer and composers behind the keyboard
  despite `adjustResize`. Keyboard-aware roots now use Android height avoidance.
  The installed APK confirms the composer and equal Save/Close footer above the
  keyboard. Scrolled content also stays behind an opaque themed navigation bar.
- The owner requested a transparent hamburger. The installed app confirms the
  fill is removed and the labelled 48dp touch target remains.
- A concurrent edit through a second client advanced the disposable agent from
  revision 1 to 2. After foreground refresh, Android kept the draft, disabled
  Save and exposed Reload from desktop. Explicit reload showed the updated
  expertise. This checks the real daemon revision boundary without an overwrite.
- Compressed Android accessibility hierarchy excludes the closed task drawer and
  the model picker's off-page provider rows. An uncompressed diagnostic dump
  includes non-important views; it was not used as a TalkBack pass. Full spoken
  TalkBack traversal remains unperformed.
- A live Codex chat was attempted with a short no-tools prompt. The daemon
  returned “Codex did not finish opening its thread”; the Android error surface
  rendered. Provider streaming, long-transcript smoothness and successful chat
  completion are not verified by that failed attempt. Its verbose provider
  failure included HTML and filled the composer area. A source-only follow-up
  bounds the banner to three lines and replaces HTML responses with a short
  desktop-setup recovery message; the original runtime diagnostic is retained.
- The original light status-icon defect is fixed in the installed APK. Ordinary
  light/dark surfaces have contrasting system icons. The native Material sheet
  can still darken status icons over a dark scrim; full modal system-bar contrast
  is not accepted.
- The final request shortened the dictation explanation to “Record up to 60
  seconds. Review the transcript before sending.” TypeScript passed. Per the
  owner's explicit instruction, this copy-only change was **not rebuilt** and
  the installed APK still contains the earlier wording.

The independent reviewer initially hit its account usage limit, then completed
its bounded read-only review after the window reset. It found a stale composer
availability condition, live steering without the cloud guard and automation
execution bypasses. These were fixed: local composers remain editable, live
cloud steering is rejected, automation starts/enabling validate all target teams,
and Nebius workflow generation is removed from mobile. Automation history,
stopping, declining and pausing remain available. Future-stage pinned profiles
are not exposed by the public run projection, so positive workflow gate approval
is explicitly a desktop action until that capability can be verified on mobile.
The reviewer rechecked these fixes and found no additional material mistake.
Automation persistence/policy tests and runtime tests were rerun; no device
pass is inferred from that review. Formal full visual disposition remains
**recapture** for the outstanding desktop state matrix, hardware, provider and
recording acceptance. Focused emulator passes do not certify those areas.

## Environment repairs

Disk exhaustion during setup and a generated native PDB linker failure were
resolved by deleting only this worktree's disposable generated caches and moving
its Android caches to `D:/codex-cache/ion-redesign-1052`. No user source, inherited
QA evidence, other checkout data, paid resource or production deployment was
removed or changed.

## Device evidence and limits

All device evidence is from `emulator-5580`, API 36, software rendering. The
normal phone viewport is 1080 × 2400 at 420dpi. Tablet evidence uses
1920 × 1200 at 240dpi (1280 × 800dp), including dark home/rail, team editor
with keyboard, light Work and the real Apps catalogue. The compact check uses
840 × 1800 at 420dpi (320dp width) with font scale 1.3. Layout overrides are
restored after testing. Final captures have the `android-final-` prefix.

The isolated APK is signed, installed and launched. Its Hermes source map matches
all ten final layout/editor source files recorded in `refinement-validation.json`.
The later shortened dictation copy and explicit keyboard dismissal on editor
Save/Close, plus the bounded provider-error banner, are listed as unbuilt source
changes. The installed APK already keeps
those controls above the IME; it can leave the IME visible briefly after closing.
No additional APK build was started after the owner asked to defer small changes.

Recording permission/finalization, audible transcription and actual audio quality
are not certified on this emulator, which runs with host audio disabled. Real Apps
OAuth/connect/disconnect, account-owner switching, publication/export, macOS/Linux
launchers, full provider transcript states and high-refresh hardware performance
remain outside the observed passes. No paid resource was provisioned or production
checkout changed. Disposable validation records are removed after capture.
