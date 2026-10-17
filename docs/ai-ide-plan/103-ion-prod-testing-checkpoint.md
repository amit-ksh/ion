# ion-prod testing checkpoint

Integrated 2026-10-17: `ion-prod` fast-forwarded to implementation commit `946b753` from `codex/ion-agent-teams`. Locked dependencies are installed in the main checkout; mobile, shared-client and gateway TypeScript checks pass there.

The checkout's pre-existing planning notes are preserved in the named Git stash `ion-prod pre-integration owner planning notes 2026-10-17` (`29ce4e94c592a771b92e26af31d2db97a2778a66`). The merged plan contains the newer Teams decisions. Local `.lavish` files remain untouched.

This checkpoint is ready for owner testing. The full document/harness objective is still open. Unstarted Amp/Cursor bounded Teams execution, Fx ordinary-task Teams invocation and other unstarted harness adapters remain out of scope.

## Start the desktop

From the existing checkout:

```powershell
cd D:\personal\apps\waku
bun install --frozen-lockfile
bun ./scripts/dev.ts
```

If the development watcher is already running, keep using that watcher; do not launch another one. The desktop needs its existing Windows Rust/MSVC prerequisites. Save credentials through the app's setup/settings screens.

## Review order

- [ ] Open ion, finish or skip onboarding, and verify the setup indicator leads to the missing configuration.
- [ ] Open Dashboard, Inbox, Decisions, Teams, Routines, Workflows, Apps, Artifacts, Harnesses and Usage. Review empty states before creating data.
- [ ] Create a small team with one lead and two specialists. Select each agent's harness and Local environment. Confirm hierarchy, permissions and limits.
- [ ] Start an ordinary solo task. Then start a separate task with `@teamname` (or use the Teams page's New task action). Verify child assignments, results, Stop, retry and user decisions.
- [ ] Use only the already configured harnesses. Confirm unsupported/unstarted paths are unavailable and native account/model preferences stay selected.
- [ ] Run a small workflow with an explicit approval step. Inspect decisions and notifications across projects; verify user actions unblock the intended run.
- [ ] Produce HTML, PDF and the additional Office/text formats. Inspect Reading/Source/original export and the artifact/task/workflow links. An extracted Office reader does not promise the complete application's page or slide layout.
- [ ] Keep retention at its default: retain until explicitly cleaned. Review protection for active tasks/reviews/workflows before enabling or executing cleanup.
- [ ] Review usage/cost labels and existing remote controls using configured resources. Do not treat estimated/model accounting as provider invoice reconciliation.
- [ ] Follow [the detailed Teams release checklist](20-agent-teams-release-checklist.md) for existing integration acceptance.

## Phone and browser documents

A fresh native build is required for the new HTML resource host. Expo Go or a JavaScript refresh of an old phone binary cannot add this module.

Android, with Android SDK/JDK/device configured:

```powershell
cd D:\personal\apps\waku\apps\mobile
bun run android
```

iPhone: build the same checkout on a configured Mac/Xcode host using `bun run ios`, or use the project's configured EAS build workflow. Then pair with the desktop daemon. See [mobile setup](../../apps/mobile/README.md).

Browser preview:

```powershell
cd D:\personal\apps\waku\apps\mobile
bun run web
```

Use HTTPS or localhost for the document service worker. Run [chapter 102's document checklist](102-scoped-html-resource-hosts.md#owner-review-checklist) on direct pairing and gateway evidence: separate SVG/CSS/font files, variable/cascade fidelity, original exports, access revocation and background/unmount closure.

## Verified at this checkpoint

- Complete phone HTML/shared-resource matrix: **94 cases, 3,074 assertions**, zero final failures/skips. The isolated final run took 5.43 seconds. An existing CSS grammar test timed out during the concurrent run; it passed with its existing timeout unchanged when run alone.
- Production Java origin: **46 actual HTTP/lifecycle assertions** after Java 8-target compilation on JDK 17.
- Mobile TypeScript, strict new-test types, Expo Autolinking for Android/Apple and formatting/diff checks.
- Earlier native publication/retention/gateway/desktop evidence is recorded in [chapter 101](101-html-resource-transport.md); this phone-only increment does not replace those checks.

## Still open

- Android Kotlin/Expo application build and actual WebView acceptance.
- Apple Swift/Expo compilation and WKWebView/device acceptance.
- Real browser service-worker registration and CSP/resource-rendering acceptance.
- Phone performance/accessibility/large-text review and combined storage presentation for retained resource originals.
- Existing harness signed release/platform/native-account acceptance, including the implemented Kimi host.
- Live Nebius/speech/gateway/device checks that require the owner's credentials and infrastructure.

No visual/device test or paid/cloud allocation was performed for this checkpoint. Source-level verification is separate from those remaining gates.
