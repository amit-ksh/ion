# Ion native redesign

## Scope and isolation

Worktree: `C:/Users/ksami/.codex/worktrees/1052/waku`.
Branch: `codex/ion-native-redesign`.
Starting commit: `59218a1`; inherited tracked QA fixes and untracked build/test sources were snapshotted in `c9d94d4`. Untracked QA evidence and Android build output remain intact. No reset, clean, or modification of the ion-prod checkout is authorized by this work.

The persistent goal was created for the full desktop/Android redesign, identity refresh, implementation, and final UX audit. The user explicitly requested autonomous execution without questions. Product/context and design choices therefore come from the supplied brief, actual reference images, and source; no interview or approval is implied. This session builds directly in the native source. It does not set a standing Impeccable workflow preference.

## Reference read

All seven supplied PNGs were opened. Their transferable traits are compact hierarchy, neutral surfaces, restrained lime emphasis, rounded but economical groups, labelled navigation, deliberate spacing, and nearby actions. Decorative charts, invented billing data, stacked glass, and expansive green backgrounds are excluded.

Official sources inspected:

- [Nebius](https://nebius.com/) and [media kit](https://nebius.com/media-kit): identity context only; Ion keeps its own mark.
- [Android layout/navigation](https://developer.android.com/design/ui/mobile/guides/layout-and-content/layout-and-nav-patterns): navigation matched to window size.
- [Android touch target guidance](https://support.google.com/accessibility/android/answer/7101858): 48dp interactive regions.

## Initial audit

- Desktop canvas/sidebar/composer have broad green tints; workspace pages independently override a subset of colors, creating inconsistent surfaces.
- Mobile accent washes still use coral while foreground accents use lime/green. Some dark secondary labels are too dim.
- Shared desktop icon buttons are 22px; Android header actions and composer affordances require a touch-target audit.
- Existing native and mobile features and real data are the source of truth. Existing virtualization and streaming work must be retained.
- This worktree has no active Ion process/watcher. Another checkout owns an Android connection; this session will not install over or control it.

## Direction contract

Mode: Operate. Native workbench with clear task context, a quiet canvas, distinct control surfaces, one intentional primary action, and short contextual labels.

First viewport: compact Work navigation and task history frame the active task; the composer is a clearly bounded place to act, with explicit model/harness/environment context and microphone feedback. Mobile keeps primary destinations reachable with a thumb and uses ordinary opaque Material surfaces.

Hierarchy: title/action row, contextual filters, content. Use whitespace and row alignment before adding a container. Cards hold an object or a form group, not another full page.

Memory: Ion's original dotted stem and rounded loop become a crisp, reproducible vector mark with lime on charcoal. It must remain legible at launcher size.

Motion: existing reduced-motion-aware feedback only; no decorative perpetual animation or new frame clock. No I/O or whole-session scans are introduced into render paths.

Verification: build and behavior checks are separate from visual acceptance. Capture only isolated fresh builds when available; document exact unavailable capabilities. Final UX review records findings and fixes, plus the evidence limits.

## Milestones

1. Preserve QA baseline and establish direction.
2. Shared neutral/lime tokens and native component system.
3. Desktop shell, work surfaces, composer, settings/onboarding.
4. Android shell, navigation, controls, forms, and states.
5. Reproducible Ion vector identity and platform icon assets.
6. Meaningful verification, bounded UX audit/fixes, documentation, commits.

## Result and evidence

Implementation milestones were committed in `a2e4f3f` (native workbench and
identity) and `98f48cb` (Android navigation, Work, Apps and revision-safe editors).
Final workbench, composer target-spacing and Linux icon coverage is in `7753631`.
The inherited QA snapshot remains separate in `c9d94d4`.

The final [UX audit](ux-audit.md) records implemented coverage, meaningful checks,
the reviewer's seven resolved source findings, a freshly reproduced-and-fixed
Teams crash, and unavailable device/visual acceptance. The formal visual verdict
is **recapture**, not whole-surface approval. `validation.json` contains sanitized
artifact hashes and export source-inclusion checks; bulky build logs remain in
the ignored worktree cache. The source-derived design system is `DESIGN.md`.
