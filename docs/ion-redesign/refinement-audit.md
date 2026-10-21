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
  `.waku-cache/dev-refinement.log` and `.err`.
- Mobile TypeScript: passed after the final reviewed policy fixes; the result
  is tracked in `.waku-cache/refinement-types-final.log`.
- Initial focused suite: 27 passed. After further changes, the combined test
  process hit an internal Bun 1.2.1 segmentation fault; it is not a test pass.
  Re-running runtime/capability tests separately passed 14 tests (47 assertions), including local
  history, steering, goal operations, team follow-ups and cloud rejection. A
  separate non-React suite passed 24 tests (225 assertions) covering palette,
  icons, Apps, account policy, revisions, hierarchy and model layout. Final
  automation persistence/policy tests passed 27 tests (93 assertions).
- Android APK: assembly in progress using a separate `.redesign` application ID,
  local native dependency sources and an isolated Gradle cache. The first attempt
  exposed cross-drive codegen and missing Expo script-distribution errors, which
  were corrected in the local build environment without dependency version edits.
  A second attempt exposed missing compiled Expo config-plugin distributions.
  Restoring those files made direct Expo configuration pass. Subsequent attempts
  exposed a cross-drive Expo CLI path and Metro dependency-junction visibility;
  the isolated CLI copy and temporary Metro config resolved both. Direct Android
  bundling passed with 2,347 modules. APK assembly is retrying with cached native
  compilation. The temporary config is environment-only and is not product source.
- Android emulator: an isolated API 36 x86_64 image and AVD were installed. WHPX
  acceleration is available. The initial Vulkan boot stalled; a diagnostic boot
  used software rendering with Vulkan disabled and reached boot-completed state.
  No physical-device input/install
  has been performed.
- Account-gated Android screens require a signed-in emulator. The user has been
  confirmed they can sign in once the APK is installed. Authentication is left
  to the owner; no account-gated device pass is claimed yet.

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
**recapture** until phone/tablet and the outstanding desktop state matrix exist.

## Environment repairs

Disk exhaustion during setup and a generated native PDB linker failure were
resolved by deleting only this worktree's disposable generated caches and moving
its Android caches to `D:/codex-cache/ion-redesign-1052`. No user source, inherited
QA evidence, other checkout data, paid resource or production deployment was
removed or changed.
