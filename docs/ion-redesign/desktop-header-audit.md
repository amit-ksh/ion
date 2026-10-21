# Desktop header and Android follow-up

This receipt covers the October 21 follow-up: desktop appearance in the header,
connected-mobile presence, the project/session hierarchy from the supplied
screenshots, and the remaining Codex/audio/dark-sheet checks. Earlier receipts
remain historical evidence, with the updates below superseding their deferred
APK and dark-sheet entries.

## Implemented

- The desktop header exposes System, Light and Dark through the existing native
  appearance menu and persisted theme setting.
- An authenticated live Android/iPhone connection appears at the top of desktop.
  The transport tracks connected clients, deduplicates reconnects by client ID,
  and removes disconnected clients. The UI polls in one sequential background
  task; rendering reads an in-memory snapshot and notifies only on changes.
- Projects appear below Workspace, including projects with no chats. Folder
  headers collapse; indented chats use single-line rows with a neutral selected
  background and Show more. Status symbols and session context menus remain.
- Each project has a vertical three-dot menu with its name, chat count, path,
  rename and delete. Rename preserves IDs and paths, commits with Enter or an
  outside click, and cancels with Escape. Delete retains the existing busy-task
  protection and confirmation; deleting project details does not delete source.
- Android sheets apply status-bar contrast through a small native view attached
  to the actual Material dialog. It updates on attachment, focus and theme
  changes, without a repeating animation or polling timer.

## Approved APK

The owner authorized one updated APK build and **emulator-only** validation.
Gradle completed successfully in 28m 22s (51 executed, 677 up-to-date tasks).
The first sandboxed invocation failed before Gradle started because its isolated
cache was inaccessible; the successful invocation reused that existing cache.
No second successful APK build was performed.

- Package: `sh.waku.mobile.redesign`, x86_64 release, 54,177,682 bytes.
- SHA-256: `f215e124a54d32e2b9500e4eafa8e5b17fc2b030ec9afa86da8e02a27a6f0ab0`.
- APK v2 signature verification passed; update installation on `emulator-5580`
  succeeded and preserved sign-in.
- Hermes source-map contents matched the sheet, native-view wrapper, daemon
  context, recording prompt, composer, new-task screen, runtime-error helper and
  editor footer at build validation. The APK contains `IonSheetSystemBarsView`.
- The formerly deferred short recording copy, editor keyboard dismissal and
  bounded provider error are included in this APK.

## Observed checks

- Mobile TypeScript: passed (`tsc --noEmit -p apps/mobile/tsconfig.json`).
- Final focused dictation, stream and runtime-error tests: 19 passed,
  63 assertions, zero failures. Final mobile TypeScript also passed.
- Generated TypeScript protocol bindings: exporter `--check` passed.
- Live-transport regression: `mobile_presence_requires_authentication_and_tracks_live_clients`
  passed, covering desktop exclusion, failed authentication, client-ID deduplication,
  multiple devices, and removal after socket disconnect.
- The final desktop build passed and the existing watcher launched the current
  binary, newer than `sidebar.rs`. The app and daemon are from this worktree.
- The paired emulator was reported as `Android` by the transport. Project rename
  persisted through save/reload. The disposable project was restored to its
  original name before testing chat.
- The actual Android Codex chat returned the requested `ION_REVIEW_OK` assistant
  reply and reached idle. The former thread-open timeout did not reproduce.
  A separate `GitLab` MCP startup event returned HTTP 422. Its bounded warning
  banner still appears even though Codex completes; the integration's endpoint
  or authentication needs owner attention. No global MCP configuration was changed.
- A source-only follow-up names the failed MCP integration in that banner
  (`GitLab couldn’t connect…`) instead of claiming the agent could not connect.
  Its classification tests and TypeScript pass. **It is not in the installed
  APK**: `mobile-composer.tsx` and `runtime-errors.ts` are the two explicitly
  unbuilt files. No second APK was built.
- Dark Material sheet: white system icons are legible over the dark scrim.
- Light Material sheet: dark system icons are legible over the light scrim.
- Tapping the outside `Close sheet` backdrop dismissed the sheet.
- Captures: `evidence/refinement/android-header-update-dark-sheet.png` and
  `evidence/refinement/android-header-update-light-sheet.png`.
- The recording confirmation displays the shortened two-sentence copy. Android
  reports microphone permission granted. Starting recording returned to idle
  without an observable active recording state on this muted emulator; no
  successful recording, discard/finalization or transcript is claimed. The
  cause is undetermined, so it is not attributed solely to disabled host audio.
- `android-header-update-codex.png` records the real reply plus the GitLab warning;
  `android-header-update-record-prompt.png` records the short recording prompt.
- The disposable chat and project were removed. Deleting project details left
  a sentinel in its source folder intact; other session IDs were preserved.

The machine-readable receipt is [desktop-header-validation.json](desktop-header-validation.json).

## Validation limits

The Windows Computer Use helper failed twice, including after its prescribed
reset, with `failed to write kernel assets: The system cannot find the path
specified (os error 3)`. Native desktop visual/keyboard acceptance is unverified;
no other Windows automation was substituted and no screenshot pass is inferred
from compilation.

The physical Moto g32 was not installed on or controlled. This review emulator
runs with host audio disabled, so physical microphone quality, audible playback,
ARM performance and high-refresh hardware acceptance remain unverified. Spoken
TalkBack, live Apps OAuth/account switching and macOS/Linux launchers retain
their earlier limitations. No production checkout, paid resource or deployment
was changed.
