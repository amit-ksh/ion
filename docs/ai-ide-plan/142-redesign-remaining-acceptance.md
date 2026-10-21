# Redesign follow-up: remaining acceptance

21 October 2026. This is an addition to the existing completion/testing queue,
including the Milestone 1 identity and keyboard review. It does not reopen passed
implementation or authorize a build, merge, device test or provider change.

## Source and scope

Reviewed receipts from `C:/Users/ksami/.codex/worktrees/1052/waku`, branch
`codex/ion-native-redesign`, commit `81827ff`:

- `docs/ion-redesign/desktop-header-audit.md`
- `docs/ion-redesign/desktop-header-validation.json`

The redesign's latest test authorization was **one APK build, emulator only**.
That build was used. The Moto G32 was not touched by the redesign follow-up.
Prior main-branch Moto acceptance is separate evidence. Carry the redesign limit
forward until separately authorized; do not infer permission for a second APK,
physical microphone capture or a merge from this queue update. Preserve watchers,
unrelated source/data, current connections and global MCP configuration.

## Outstanding tasks and existing acceptance links

The user's six additional UI tasks are tracked once in
[146](146-requested-sidebar-and-artifact-ui-tasks.md), UI-01–06. Device identity,
project ellipsis/menu icons, Work navigation and bottom Settings extend RD-01;
mention-placeholder guidance and artifact-only deployment link the existing
composer/publishing checks. They remain queued, not passed by old receipts.

| ID | Task and acceptance | Status / existing queue |
| --- | --- | --- |
| RD-01 | Desktop visual and keyboard matrix: Light/Dark/System; header appearance, connected-mobile status, expandable project/chat hierarchy, Show more, details/rename/delete, visible focus, Enter/Escape, outside dismissal and narrow windows | Unverified. Extends completion section 9 and plan 132 Windows acceptance. Computer Use failed after reset with `failed to write kernel assets: The system cannot find the path specified (os error 3)`. Compilation does not pass this gate. |
| RD-02 | Diagnose Android Start recording returning to idle without observable recording. Verify active recording, discard, Stop/finalization, transcription, background/disconnect recovery and draft preservation | Open diagnosis; extends plan 132 dictation rather than duplicating it. On emulator-5580/API 36 permission was granted, prompt observed and host audio disabled. Cause undetermined; do not attribute the failure solely to muted host audio. Real microphone/playback quality, physical ARM and high-refresh acceptance remain unverified and subject to separate authorization. |
| RD-03 | Investigate GitLab MCP startup HTTP 422: endpoint/authentication and useful integration failure state | Open integration issue. Actual Android Codex returned `ION_REVIEW_OK` and reached idle; previous thread-open timeout did not reproduce. Owner involvement may be needed. Do not silently disable or change global MCP settings. |
| RD-04 | Include and validate integration-specific MCP warning copy in the next independently authorized package refresh | Source tests/typecheck passed; installed APK does not contain this correction. Changed redesign files: `apps/mobile/src/components/mobile-composer.tsx`, `apps/mobile/src/lib/runtime-errors.ts`. Message names GitLab rather than claiming the agent failed. Extends the existing release refresh; no additional APK build is authorized by the handoff. |
| RD-05 | Complete linked audit gaps: spoken TalkBack, live Apps OAuth/account switching, long transcripts/provider tools/reasoning and high-refresh performance | Pending; link existing account, app and transcript acceptance, do not requeue completed focused tests. macOS/Linux launcher checks remain deferred outside this Windows/Android goal unless scope is explicitly extended. |

## Already passed in the separate redesign

Keep these receipts; do not requeue them as wholly unfinished:

- Final desktop build and watcher relaunch; authenticated Android connection
  reporting and the live-presence regression.
- Project rename persistence and project-details removal preserving source.
- Mobile TypeScript and current generated protocol bindings.
- Nineteen focused tests, 63 assertions, zero failures.
- One signed x86_64 APK installation on emulator-5580 preserving sign-in.
- Light/dark Material sheet status-bar contrast, outside sheet dismissal and
  shortened recording confirmation. These do not pass recording/transcription.
- Actual Android Codex reply/idle; disposable QA chat/project cleanup.

Redesign APK SHA-256:
`f215e124a54d32e2b9500e4eafa8e5b17fc2b030ec9afa86da8e02a27a6f0ab0`.
Package `sh.waku.mobile.redesign`, x86_64, 54,177,682 bytes. This is separate
from main-branch ARM64 release artifacts and is not a physical-device pass.

## Completion rule

RD-03 diagnosis: [145](145-gitlab-mcp-endpoint-diagnosis.md) reproduces HTTP 422
HTML from the configured endpoint and observes a 401 OAuth challenge from the
documented MCP endpoint. Correction/authenticated acceptance awaits owner
approval; no global configuration or cache was changed.

RD-02 source hardening: [143](143-phone-dictation-stop-recovery.md) records a
separately reproduced native Stop/Discard exception cleanup defect and its fix.
Ten isolated hook lifecycle scenarios and mobile TypeScript pass. The emulator
startup return-to-idle symptom and actual recording/transcription remain open;
these fixtures do not establish its cause or pass device acceptance.

Track RD-01–05 in `.waku-cache/ion-managed-task-queue.json`. Reuse the
[completion checklist](120-managed-access-completion-checklist.md) and
[device/interaction checklist](132-physical-device-acceptance.md) for shared
gates. Verify source provenance before any later integration/package refresh;
do not overwrite the implementation checkout with redesign files automatically.
