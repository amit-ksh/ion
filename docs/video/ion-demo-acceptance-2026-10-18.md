# Ion recording rehearsal checklist

Do not record yet. **Not Ready for the complete requested demo matrix.** Read [the acceptance report](../qa/2026-10-18-ion/README.md) for per-scenario results and exact dependencies. The verified sequences below can be rehearsed; the final installer gate is recorded there.

## Current 0.1.24 verified native additions

The rebuilt watcher app passes these flows. **Current 0.1.24 installer acceptance also passes**, including actual installed Gmail approval, free OpenCode question/read, microphone cancellation, Usage links and restart. Earlier installed 0.1.23 checks below remain historical evidence.

- [x] New task in the disposable project → OpenCode → exact model `opencode/deepseek-v4.1-flash-free` (DeepSeek V4.1 Flash Free). Harmless exact-reply smoke returns QA_OPENCODE_FREE_NATIVE_OK (323–324). Leave expired paid OpenCode routes unchanged.
- [x] Use the actual provider question tool to ask which QA file to read; select README.md, submit and verify the real read activity and `# Ion QA disposable project` response (329–330).
- [x] Actual two-question request: Format with Markdown/Plain text, then Label with a typed answer. Select Markdown, type QA_TYPED_LABEL, use Back/Next to verify retention, Submit. Provider returns QA_OPENCODE_FREE_MULTI_OK, Markdown, QA_TYPED_LABEL in order (346–350). Do not fabricate provider question events.
- [x] Stop while a free OpenCode question is pending; the question disappears and the task is stopped. A new harmless turn returns QA_OPENCODE_FREE_RETRY_OK (335–337).
- [x] In an ordinary free OpenCode task ask it to list teams, invoke the exact QA Free Source Team once, await it, and read its document. Approve the displayed Nebius/DeepSeek/local agent with no filesystem/shell/network/apps grants. Verify the model returns QA_FREE_SOURCE_TEAM_OK only after its actual23-byte Markdown artifact is read (351–356). This uses the real ordinary-task bridge, not direct @team invocation.
- [x] Nebius connected actions: exact Gmail profile or Slack auth metadata only → exact pinned account/action/version/arguments approval → durable successful receipt and safe Markdown summary. One Gmail and both Slack connections pass; no mailbox/conversation content or external messages. Keep raw personal profile JSON out of a demo recording.
- [x] QA Connected Nebius Workflow completes Gmail → explicit stage gate → separate Slack action approval →2/2 completed stages with actual preceding Markdown input (338–339, connected-debug-workflow-audit.json). Do not enlarge access automatically after a gate; a preceding document requires the separately saved input-reader grant.
- [x] Denied/canceled actions have no app start; an agent with only Slack grant asking to describe Gmail gets the actual host denial, failed tool icon and0 app executions (321–322,342–343). Deliberate Slack API error executes once, reports failure and does not retry.
- [x] Settings → Usage → OpenCode shows actual free-model token receipts with cost unavailable; Open task routes to its exact task (344–345). Nebius filter shows actual DeepSeek model alongside the generic Nebius harness label (358). Zero catalog rates do not prove a reported charge.

Exact new evidence: [opencode-free-native-audit.json](../qa/2026-10-18-ion/opencode-free-native-audit.json), [free-opencode-source-team-audit.json](../qa/2026-10-18-ion/free-opencode-source-team-audit.json), [connected-debug-workflow-audit.json](../qa/2026-10-18-ion/connected-debug-workflow-audit.json). All new saved fixtures are bounded and disposable. Total Nebius allowance remains$15 for the entire pass.

## Verified baseline flow: Codex task and human decision

- [x] Open the actual native app and Projects → Open folder.
- [x] Select `D:\personal\apps\waku\.waku-cache\ion-qa-disposable-20261018`.
- [x] Choose Codex → GPT-6-Luna; choose Supervised.
- [x] Submit: `QA smoke task. Work only inside this disposable project. Read README.md and owner-data-sentinel.txt, then reply with exactly QA_CODEX_OK followed by the sentinel text. Do not edit files, invoke teams, browse, send messages, or use any paid cloud resources.`
- [x] Show the streamed activity and exact pending read command.
- [x] Open Decisions → Review. Confirm it opens this task, then Allow once.
- [x] Verify the final response contains `QA_CODEX_OK` and `Must remain after removing this project from Ion.`
- [x] Attempt removal while waiting for approval: Ion refuses with a Stop-first message.
- [ ] Fresh Codex approval/read rehearsal is blocked by the current account usage limit (retry time shown as 10:00 AM). Earlier actual native Codex task/approval/delegation checks and installed exact replies passed; do not promise a fresh live Codex segment before allowance is restored.
- [x] Reopen history after restart and confirm output and usage remain (278–279).

## Conditional beats

Only use a beat when its row in the acceptance report has a native/live Pass:

- Nebius coding: valid key Test, selected actual model, bounded task, streamed tools, exact decision and output. **Owner cap: $15 total Nebius usage for this pass.**
- Teams: actual lead/child assignments and submitted/accepted result; show pinned harness, model, access and environment. A saved definition alone is configuration evidence.
- Workflow/routine: actual stage gate and continuation, then a saved paused routine and its verified firing history.
- Artifacts: agent-produced original, native preview, original export/hash and task/project links. Office extraction does not prove Office layout fidelity.
- Usage: Ion activity filters and task link; keep CLI history separate and retain missing/estimated cost labels.
- Speech/mobile: actual recording → Stop → editable inserted transcript; actual paired phone only after native physical-device acceptance.
- Apps: use only the verified profile/auth metadata flows above with exact approvals. Cloud execution remains prerequisite-blocked. Never send real messages, deploy, publish externally or provision resources during rehearsal.

## Verified candidate rehearsal: team, workflow and documents

Use the disposable project only. Saved fixture definitions and output IDs are in `docs/qa/2026-10-18-ion/live-team-snapshot.json`, `live-workflow-snapshot.json`, `live-document-run.json` and `live-routine-status.json`.

- [x] Teams shows **QA Demo Team** with **QA Lead** and its child **QA Evidence**, alongside the separate **QA Document Team**.
- [x] Open the completed QA Demo Team run. Show both accepted assignments, `qa_review`, 12/24 model calls and 70,333 tokens. Open the HTML result from its actual artifact link.
- [x] Start **QA Gated Workflow**. Before approval, Decisions shows its exact Evidence-stage gate; Summary remains unstarted.
- [x] Decisions → Open → Approve stage continues the run. The actual MD report and subsequent HTML summary remain after restart.
- [x] **QA Paused Routine** has one scheduled completed run and a `QA_ROUTINE_OK` report. Leave the schedule paused (`nextRun: 0`).
- [x] Artifacts displays generated document cards with task/project provenance. Search `qa-large.html` returns the exact 2,100,069-byte original.
- [x] Small HTML Reading view renders `QA_DOCUMENT_OK`; Source shows the verified original.
- [x] Rebuilt large-HTML Reading view displays `QA_LARGE_HTML_OK`; native captured-PDF preview displays `QA PDF OK`.
- [x] Native Save copy exported the agent-created PDF; SHA-256 exactly matches the 596-byte published original. Large Unicode text reaches its actual tail/head markers with End/Home (200–201).
- [x] Enter `@team:{QA Document Team}` through the native composer. The actual completed run used 2/24 calls and produced exactly `<qa>QA_NATIVE_TEAM_OK</qa>`; open its XML artifact.
- [x] An ordinary supervised Codex task created `qa-documents/qa-codex.pdf`, invoked the saved Nebius team to inspect/publish it, and read its actual result. Native preview displays `QA_AGENT_PDF_OK`. The first model-only PDF attempt failed truthfully at its finite bound; use the verified file-publication flow for rehearsal.
- [x] Separate disposable removal fixture: confirm removal while approval is pending → refused; Deny command → Stop task; Cancel chat removal → retained; confirm inactive chat/project removal → source README, Git and sentinel preserved.
- [x] Final watcher-native JSON branch selects stage 3, skips stage 2 and waits for approval. Verify actual stage-one bytes before approving. A distinct isolated worktree receives that code, changes it to QA_HANDOFF_STAGE_TWO and publishes QA_HANDOFF_OK; source stays QA_HANDOFF_BASELINE (302–303, final-handoff-audit.json).
- [x] DeepSeek Flash through the existing Nebius account creates a real missing file using JSON null, reads it and publishes evidence in four calls (304, alternate-model-create.json). Use this verified model for this beat. Lightning twice supplied string None and overstated success; do not demonstrate that failing variation.
- [x] Rebuilt tool errors retain their exact arguments/output and show a failure icon even when the disclosure has content (298, 300). A later valid read/edit can still succeed.

The saved hierarchy, workflow, routine and document fixtures are in the **watcher-owned native QA profile** (`temp/`). The installed profile retains its existing owner data and separate installer smoke tasks; a read-only QA error-probe team is added there for final package acceptance. Do not copy/reset whole profiles. Both are actual native Windows apps; no HTML prototype is used.

The original QA Demo/Document teams use Nemotron Lightning, Local execution, workspace read allowed and writes/shell/network/apps disabled, capped at 24 calls / 2.2M tokens / ten minutes. The isolated handoff uses 16 calls / 2.2M / ten minutes; DeepSeek create comparison uses 8 / 2.2M / five minutes; installed read-only error probe uses 8 / 1.2M / five minutes. All use one parallel assignment. Keep total Nebius tests below $15. Exact successful handoff used 12 calls; alternate creation used 4; installed error/recovery used 7. Never apply worktree changes to source during rehearsal.

## Verified local speech prerequisite

- [x] Existing NeMo-Speech 0.1.0 and cached Nemotron transcribe the disposable Windows-generated voice fixture. Normalized words match exactly.
- [x] Settings → Providers → Speech → Transcribe WAV: choose `D:\personal\apps\waku\.waku-cache\ion-qa-voice-20261018.wav`, then Insert into composer and verify editable unsent text.
- [x] Rebuilt microphone continues after a recoverable discontinuity; Cancel preserves the draft; Stop finalizes silent input and reports no speech. Controlled spoken-microphone insertion still needs an actual spoken sample; WAV insertion is separately verified.

## Current service/device gates

- Nebius: owner-saved key Test succeeds with 28 models; live tasks and teams succeed. Rebuilt workflow generation succeeds using the actual live model catalog. Never put the key in this document or chat.
- Composio: owner connected Gmail and two Slack accounts; real exact approved metadata actions, gated workflow, deny/cancel/access/error cases now pass in rebuilt0.1.24. No email/conversation content or external messages are in scope.
- OpenCode: explicitly selected free `opencode/deepseek-v4.1-flash-free` passes. Old paid Console/OpenRouter credentials remain expired and unchanged. Ordinary free success does not establish the unsupported bounded Zen retry transport.
- Claude models/harness are excluded by owner request.
- Codex now reports account usage limit. Native steering and fresh live recording beats require restored allowance; goals/questions also need a supported eligible CLI mode. Do not replace this with a fabricated question event.
- Moto G32: disconnected; ADB is installed and lists no devices. Current offline Android rebuild hits cached Ninja's 260-character path limit, including the shorter-cache retry. An older APK build is not current native acceptance. iPhone 15 and an authorized Apple build/signing host are unavailable.
- Hosted speech, registered remote execution and billing export acceptance need their exact existing endpoints, grants and infrastructure; a Token Factory model key does not supply those.

## Current installed 0.1.24 acceptance — verified

- [x] Live steering on the same free-model task: request a real question asking “May I finish the steering test?” and wait. While it is pending, type `QA_STEER_FREE_CHANGE. Change the final answer in this currently running turn to exactly QA_STEER_FREE_CHANGED_OK. Keep waiting for my real Continue answer if the question is still pending. No other actions or tools.` Press Enter to queue it, then click Steer. Select Continue → Submit. Verify the changed marker; base, steer and final answer share one actual turn ID (379–383, installed-free-steering-after.json). This is live steering, not a follow-up shortcut.
- [x] Installer `target/release/ion-0.1.24-x86_64-Setup.exe`, 33,373,298 bytes, SHA-256 `f93bf64cfa4aa6b8e557749bb892bfebb9268eca2f123723cbc20a92fff46c23`. Source/package checks, installation and exact installed binary hash pass. Package is unsigned; no signing credential configured.
- [x] Open installed app in `ion-qa-disposable-20261018`. Select OpenCode → DeepSeek V4.1 Flash Free. Submit: `Reply with exactly QA_INSTALLED_FREE_0124_OK. Do not use tools, read/edit files, invoke teams, call apps, browse, send external messages, publish, deploy or provision anything.` Verify the exact response (362–365).
- [x] In that task submit: `QA_INSTALLED_FREE_READ_0124_OK. Use your actual question tool to ask Which QA file should I inspect? with choices README.md and Do not read. Wait for my real answer. If I choose README.md, use the actual read tool to read ONLY README.md in this disposable project. Then reply QA_INSTALLED_FREE_READ_0124_OK followed by the exact first heading. Do not read any other file, edit files, run shell, invoke teams, use apps/network, send messages, publish, deploy or provision.` Select README.md → Submit. Verify actual question/read activity and the heading `# Ion QA disposable project` (366–369).
- [x] Fresh QA Connected packageGmail Team run: Decisions shows exact Gmail profile account/action/version and `{user_id:me}`. Approve only that action. Run completes in four Nebius calls with one app start/settlement and actual Composio log ID. Artifacts → QA_CONNECTED_GMAIL_OK opens the safe report; keep raw personal profile JSON out of recordings (360–361,370).
- [x] Type the unsent draft QA_DICTATION_PRESERVE_0124. Start microphone; installed bundled recognizer runs while Ion stays visible. Cancel → no speech process remains and the unsent draft is preserved (371–373). Controlled spoken input is still prerequisite-blocked.
- [x] Settings → Usage → OpenCode shows actual free-model receipts and cost unavailable; Open task returns to the exact task. The reply/question/read check had four receipts with 73,715 input / 211 output tokens (374–375); subsequent steering adds its own receipts, so totals grow on repetition.
- [x] Close only the idle installed app, reopen once, verify both exact responses, actual tool events, usage and draft. Open the same Gmail report after restart. Byte/hash audits pass; clear only the temporary unsent QA draft afterward (376–378). Do not close the watcher-managed debug app.
- [x] Final combined QA Nebius ledger: 168 calls, 886,022 input / 29,508 output tokens; known-token estimate $0.12015540, not reported charges. Missing generator ledger requests are not zero. Four sentinels/source baseline unchanged; one watcher; no pending QA decisions or nonterminal QA teams; routine paused/nextRun0.

## Earlier installed 0.1.23 acceptance — historical

- [x] Final installer SHA-256 `3f6d87d6174415837b60fd52e255eaaed3d15a4573153f79a0f4a4953bfabda3`, 33,367,262 bytes; exact installed binary match and no Windows restart.
- [x] In installed app, New task in `ion-qa-disposable-20261018` → submit `@team:{QA Installed Error Team} Run exactly the saved read-only QA probe. First read missing-qa-file.txt once (expected recoverable error), then README.md, save QA_INSTALLED_TEAM_ERROR_OK Markdown and report_result. No source edits, other teams, shell/network tools, apps or external effects.`
- [x] Expand Worked → tool calls. Expected missing-file call displays the failure icon; valid README read follows. Open its actual published artifact; exact marker and heading remain after restart (306–308, 314–316).
- [x] Local microphone starts its exact bundled helper without a foreground console. Cancel preserves the unsent draft and does not leak feedback into another task (309–311). Controlled spoken input and hosted/phone paths remain blocked.
- [x] In ordinary installed Nebius task submit `Reply with exactly QA_CLOSEOUT_INSTALLER_NEBIUS_OK. Do not use tools, read/edit files, invoke teams, browse, publish, deploy or send external messages.` Exact reply and restart persistence pass (312–314).
- [x] Earlier closeout estimate was $0.05184630. Current combined estimate is above; neither is an invoice charge. Original source bytes and paused routine remain preserved.

For repeated file creation, use the verified QA Alternate Create Team/DeepSeek model and its isolated workspace. For an ordinary Codex-created PDF after quota restoration, choose a fresh disposable filename; do not rerun an exclusive-create prompt against the existing qa-codex.pdf. Do not leave workflow gates waiting while rebuilding—the ten-minute deadline remains in force.

## Exact launch and test commands

```powershell
Set-Location -LiteralPath 'D:/personal/apps/waku'
& 'C:/Users/ksami/AppData/Local/Programs/Waku/waku.exe'
Get-FileHash -LiteralPath './target/release/ion-0.1.24-x86_64-Setup.exe' -Algorithm SHA256
cargo test --locked -p waku-credentials --lib composio::tests -- --test-threads=4
cargo test --locked --workspace --lib --bins -- --skip claude --test-threads=4
cargo test --locked -p waku-core --lib driver::nebius::team::tests -- --skip claude --test-threads=4
bun test packages/waku-client packages/personal-gateway apps/mobile apps/web scripts/branding.test.ts scripts/dev-watch.test.ts
bun node_modules/typescript/bin/tsc --noEmit -p packages/waku-client/tsconfig.json
bun node_modules/typescript/bin/tsc --noEmit -p apps/web/tsconfig.json
bun node_modules/typescript/bin/tsc --noEmit -p apps/mobile/tsconfig.json
bun node_modules/typescript/bin/tsc --noEmit -p packages/personal-gateway/tsconfig.json
node --test scripts/mobile-check/cli.test.mjs services/speech/acceptance/acceptance.test.mjs services/speech/deployment/deployment.test.mjs
& ./docs/qa/2026-10-18-ion/verify-installer-0.1.24.ps1
```

Use Node 22+ for the Node-only tests; Bun 1.2.1 does not implement `node:test`. Existing pinned speech `ws` dependency was installed with its frozen lockfile. Missing host/account/device cases stay ignored/blocked with their listed prerequisites. These commands rerun checks; the recorded counts above are the evidence from this pass.

Release build command: `bun scripts/bundle-windows.ts`. The source guard intentionally rejects a changed source tree until a fresh candidate snapshot is prepared. Existing watcher is already running; do not start another `bun ./scripts/dev.ts` or run a debug bundle. Legacy executable/profile directory names remain compatibility identifiers; visible branding is ion.


Full-matrix recommendation remains **Not Ready** for the dependencies in the acceptance report. The verified sequences above are suitable for rehearsal. **Do not record videos yet.**
