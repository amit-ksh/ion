# Remaining Windows and Moto G32 acceptance

21 October 2026. This is the remaining interaction checklist, not a record of
passed tests. Follow plans 129 and 131 for completed native-account, provider
and static-publishing evidence. iOS and deferred harness adapters are excluded.

## Starting state

- Owner native Auth0 login, Windows Credential Manager refresh and managed
  publish-only desktop registration passed. The desktop account remains signed in.
- The owner previously authorized Moto G32 USB access and confirmed Android
  sign-in; upgrade, cold start and the primary user's microphone permission
  passed nonvisually. At 08:42 IST the device reported offline. A scoped host-side
  reconnect did not restore it; subsequent reads report device not found.
  That failure is preserved in `.waku-cache/ion-account-bound-moto-availability.json`.
  At 11:15 IST the phone was reachable again. The refreshed APK subsequently
  passed a replacement install, exact installed hash and nonvisual cold start;
  its existing microphone permission remained granted. No screenshots,
  microphone capture or app-data clearing were performed. Physical dictation
  and account interactions remain open; see plan 138.
- Owner Google Drive connection and one approved read-only quota action passed.
  Do not repeat that provider effect just to replace already recorded evidence.
- Cloud, Token Factory and app-action spending switches remain OFF. Static
  publishing works with Cloud OFF; it does not need a Nebius VM or controller.
- The exact R2 model bytes and packaged CPU runtime passed synthetic speech and
  silence. These checks do not establish physical microphone behavior.
- Windows and Android packages containing the account-bound publishing fixes
  passed packaging and nonvisual upgrade checks. Follow plans 137–138 for the
  current hashes and exact source/QA provenance. The separate redesign is excluded.

## Windows release installation

- [x] Record completed installer/ZIP hashes and source provenance. Preserve the
  existing 111-file QA patch; this is not represented as a pristine checkout.
- [x] Install the refreshed per-user EXE, preserving the existing user data and
  credential vault. Verify actual installed file versions and package membership.
- [ ] Open Ion and verify login returns to the existing account. Confirm the
  managed connection and user-owned projects/chats are retained after upgrade.
  The launched process and retention of all original chat/project/message IDs
  passed nonvisually. Native admitted-account service reads also passed; actual
  account restoration/connection display in the GUI remains open.
- [ ] Exit normally and reopen; verify account/session restoration and no stale
  publishing operation is replayed. Do not stop another task's development watcher.
- [x] Verify the refreshed portable ZIP's bundled speech helper and notices,
  model manifest URL/size/hash and fixed speech/silence results.

## Real desktop and paired-phone dictation

Redesign recording diagnosis RD-02 shares this acceptance gate; see
[142](142-redesign-remaining-acceptance.md). Emulator-5580 returned to idle after
Start recording despite granted permission. Cause remains undetermined; disabled
host audio alone is not an established explanation. Its prompt/contrast checks
passed, but recording/discard/finalization/transcription did not. This separate
redesign review was emulator-only and did not exercise the Moto G32.

- [ ] In desktop **Settings → Models & Speech**, download/verify Parakeet.
  Confirm the first-use progress and subsequent verified cache reuse.
- [ ] In a disposable task composer, dictate one short English sentence, Stop,
  review/edit the result and confirm it is not sent automatically.
- [ ] Start another recording and Cancel; verify no pending transcript is inserted.
- [ ] Use silence and denied microphone permission; confirm a recoverable message
  and preservation of the existing draft. Restore the original permission afterward.
- [ ] Keep the desktop online. On Android **Daemons**, add/select the connection
  shown in desktop Daemon settings. Managed gateway registration alone does not
  establish the direct daemon link used for speech and full task transcripts.
  Keep the daemon token in device credential storage, never in chat or reports.
- [ ] In a phone task, record a short English sentence and Stop. Verify the
  paired desktop performs local ASR and the phone receives an editable result.
- [ ] Verify phone Cancel, app backgrounding and selected-task/daemon changes
  discard pending recording text instead of inserting it in another task.
- [ ] Take the desktop offline during finalization; confirm failure preserves the
  draft and no audio is silently replayed to a different service or computer.

Do not record an unattended person's microphone to satisfy this checklist.
The 110M model supports English; this release does not claim multilingual ASR.

## Native tasks, teams and workflow interactions

Link RD-01/RD-05 to the existing desktop keyboard/theme/navigation, spoken
TalkBack and long-transcript/provider tool/reasoning audit. RD-03 is specifically
GitLab MCP startup HTTP 422; actual Android Codex completed and reached idle.
RD-04 tracks source-only warning copy in the next authorized package, not a new
build request. See plan 142 for passed receipts and test authorization limits.

- [ ] Create a disposable local task with an already supported harness. Verify
  start, streamed progress, completion, cancellation and return visit.
- [ ] Verify `@` includes files, teams and apps; `@@` filters AI teams; `@@@`
  filters apps. Selected mentions retain the intended account/context.
- [ ] Create a small team with explicit harness/environment choices. Verify lead
  delegation, bounded access and an approval-dependent action; reject the action
  and confirm it does not execute. No new harness adapter is in scope.
- [ ] Verify routine/workflow definition, one bounded run, cancel and restart
  recovery. An approval for one exact action must not authorize changed arguments.
- [ ] Use the connected app through a real chat/team/workflow only when the owner
  has enabled the app-action switch and accepted the exact action. Restore the
  switch after acceptance and record the provider receipt; avoid duplicate effects.
- [ ] For managed inference acceptance, enable only the reviewed service/model
  within the existing envelope, then verify streaming/tool calls/cancel and OFF
  denial. Keep the initial controls and restore them before finishing.

## Account boundaries and publishing

- [ ] Sign out and sign in as the judge using the actual native clients. Verify
  no owner chat, device, connected-app or private publication leaks across accounts.
- [ ] Confirm the judge cannot change owner spending controls. Returning to the
  original account restores its data without borrowing a stale background result.
- [ ] Publish a reviewed local static export with HTML/JS/assets and SPA routing.
  Check private/public access, a second version, rollback, unpublish and cleanup.
  Verify a second actual account cannot view/manage the private publication.
- [ ] Verify backend server runtimes, cloud build/start commands and runtime
  secrets cannot be selected or submitted. Static publishing needs no Cloud switch.
- [ ] Check the same publication controls in the refreshed Android application.

## External gates and closeout

- [ ] Token Factory sandbox: verify spawn/list/cancel access, compatible immutable
  worker image/manifests and real price bounds before any paid lifecycle test.
  Access passed for `aiproject-e00h49rtswkv9bz46s` in plan 141; reviewed worker
  images, pricing and real lifecycle remain pending. No sandbox was started.
- [ ] Reconcile provider invoices/unknown ledger holds. The Drive action's bounded
  reservation is not evidence of its final invoiced cost. Do not release an unknown
  hold merely to make usage look complete.
- [ ] Confirm stopped/deleted state for every resource actually started for tests,
  record artifact hashes/test evidence/remaining limits and commit the milestone.
- [ ] Hand over to **Test and fix Ion** only after the agreed required gates pass.

Keep desktop/mobile visual redesign review in its separate worktree. Its changes
must be tested and merged deliberately before rebuilding another final release;
the redesign task does not retroactively change this package's recorded source.
