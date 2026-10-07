# Agent Teams release checklist

Updated 2026-10-07. Implementation branch: `codex/ion-agent-teams` in the managed
worktree. These changes have not been integrated into `ion-prod` by this task.
Use [implementation progress](19-agent-teams-progress.md) for checkpoint evidence
and [the confirmed specification](18-agent-teams-implementation.md) for scope.

## Implemented and covered by local regression tests

- [x] Reusable teams, direct reporting hierarchy and per-agent harness/model,
  instructions, expertise, expected output, permissions and concurrency.
- [x] Explicit Local / Nebius sandbox selection; no Auto mode. Confirmed unavailable
  sandbox can fall back locally with the same selected harness/model/grants.
- [x] Preserve normal New task; invoke a team with `@teamname`, `@{Team Name}`
  or the qualified picker address. Ambiguous names require explicit selection.
- [x] Normal native Nebius agents can invoke saved teams with source permissions,
  wait for their actual outcome and read accepted generated documents.
- [x] Durable task creation, pinned definitions, nested delegation, bounded private
  handoffs, transactional capacity and lease fencing across restart.
- [x] Actual local Teams execution for native Nebius, restricted Claude, Codex,
  OpenCode, Pi, Oh My Pi, DeepSeek, Kimi Code and Grok Build. Exact model/account/
  transport restrictions and fixture evidence are recorded in
  [adapter and remote execution setup](22-harness-adapters-remote-execution.md).
- [x] Parent acceptance/rework, linked correction tasks, output contracts, generated
  documents and child/dependency scope checks.
- [x] Important decisions and exact Composio account/action/version/argument approval,
  single execution with durable acknowledgement and AI-generated result documents.
- [x] Isolated coding workspace, serialized writers, bounded exact file writes,
  directory creation, file deletion and actual before/after change documents.
- [x] Source-task Stop propagation; closing an attachment does not stop owner work.
- [x] Consumed/unknown token accounting and aggregate model-call/token limits.
- [x] Explicit continuation from a safe checkpoint after a known Local worker exit,
  retaining task, attempt, host, grants, model and budget history.
- [x] Native cached run/hierarchy/task/document/status projections and background RPCs.
- [x] Human-reviewed combined source changes from native edit receipts, exact source
  conflict checks, explicit approval, durable application receipts and interrupted
  application reconciliation. Created/deleted files and directories are included.

- [x] Local owner restart recovery: explicit previous-host stop confirmation,
  exact file/directory/delete outcome checks, user-labeled app evidence, durable
  confirmation replay and repaired native continuation without repeating effects.

- [x] Immutable UTF-8 file/folder input handoffs from the normal task composer,
  pinned to the run and available through intersected read grants.

- [x] Bounded text follow-ups from an existing active lead task on desktop/phone,
  durable transcript materialization, delivery at model boundaries and unchanged
  run authorization. Phone Stop also handles a queued team without a live runtime.

These checked items record implementation and automated verification. They do not
assert live account acceptance, visual validation or production release readiness.

- [x] Durable sequential workflow stages, per-stage teams, exact human gates,
  aggregate limits, immutable prior report handoffs and interval routine firing.
- [x] Phone workflow/routine history and instructions, saved-definition Run now,
  schedule enable/pause, exact stage gates and Stop, with persisted submission
  identity and fresh-owner/focus fences. Desktop retains definition editing.

## Development still required for the full planned release

- [x] Antigravity native direct Gemini and OpenCode 2 standalone direct OpenAI
  Teams adapters: Local, registered completion and recovery wiring.
- [x] Fx configured direct OpenAI/Nebius adapter implementation and dispatch wiring.
- [ ] Fx official native Linux/macOS fixture acceptance (no compatible Linux runtime
  is available on this Windows development host).
- [ ] Amp and Cursor Teams adapters: verified native account/request caps, effects,
  auxiliary-call boundary and complete per-call usage. These remain development.
- [ ] Unverified OAuth/subscription/custom provider transports for the additional
  harnesses; preserving solo availability does not establish a Teams contract.
- [ ] Normal-task team invocation bridges for external solo harnesses.
- [x] Registered immutable ConTree worker: actual selected-harness completion,
  readiness, bounded transfer, durable operation identity, cancellation, restart
  reconciliation, usage and pre-execution Local fallback. Live setup/review pending.
- [x] Verified operator binding after a lost sandbox acknowledgement, terminal
  process verification and explicit remote checkpoint continuation. Exact input
  witnesses are required; legacy requests and live setup remain review items.
- [ ] Full remote project/command execution, remote checks, resource/egress controls
  and GPU/ML job provisioning. Separate bounded command jobs are implemented;
  the selected-harness completion worker still returns model actions to the owner.
- [ ] Executed test/check receipts and enforceable command isolation. Declared check
  labels and model-written test claims are not executed validation.
- [ ] Pinned price/currency and monetary/cloud spending reservations and usage.
- [ ] Additional resource/egress grants and host enforcement for general commands.
- [ ] Image/PDF input interpretation through a verified selected harness.
- [ ] Persistent Inbox unread/acknowledge/archive state and user notification policy.
- [ ] Additional artifact hashes/provenance/preview formats and global usage ledger.
- [ ] Registered-owner portfolio/gateway projection and remote workflow controls.
- [ ] Cloud speech live partial streaming and deployment/device acceptance.

Elapsed-interval routines and sequential workflow execution now reuse the Teams
coordinator; see [workflow/routine behavior and review](21-workflows-routines.md).
Calendar schedules, conditional stage graphs, retries, document-format gates and
code integration across stages remain development work. Full release completion
also requires the unchecked execution and accounting items above.

## Your live review when resources are available

- [ ] Attach a UTF-8 requirements file/folder in New task and invoke `@teamname`.
  Verify the original prompt and file chips remain in the root transcript; a
  permitted specialist can read those exact inputs and produce a separate artifact.
- [ ] Try an image/PDF, private path or oversized selection and confirm the error
  leaves the prompt and attachments available for correction.
- [ ] Send a text follow-up on desktop and phone in the existing lead task while
  it is running or waiting on children. Verify one human message, the same task/turn/attempt and a delivered
  status in the run. Confirm the final result addresses the additional request.
- [ ] On the phone, interrupt a follow-up acknowledgement and retry. Verify a
  single message, unchanged history and no solo provider startup. Stop a queued
  team before its worker starts and verify its undelivered messages are canceled.
- [ ] Edit a draft while sending a follow-up and verify the newer draft is retained.
- [ ] Queue a follow-up after a temporary Local failure. Verify it remains queued
  until Continue from checkpoint, with the same grants and budget history.
- [ ] Stop before delivery and verify the follow-up is canceled. A follow-up cannot
  approve a pending decision; resolve that decision first. Use New task after
  completion or to supply additional files. Maximum: eight messages, 4 KiB each.
- [ ] Save the Nebius Token Factory key privately in Settings and Test it.
- [ ] Install/authenticate the exact selected Claude models/harness on Local.
- [ ] Configure Composio privately, connect an active account and grant exact actions
  to the lead and specialist so delegation can intersect the grants.
- [ ] Start a three-level team task in a clean committed Git project; follow the
  actual child task links, selected models and host labels through completion.
- [ ] Review one generated document and one correction task; verify unaccepted
  child output cannot finish the parent.
- [ ] Approve and decline exact app actions and inspect real provider outcomes.
- [ ] Create a source directory/file and remove a small obsolete file in the isolated
  workspace; inspect the change document and confirm the source checkout is intact.
- [ ] Open a completed coding run's Review changes, inspect the combined diff and
  Apply reviewed changes. Verify only those changes appear in the original project
  and remain uncommitted. Repeat Apply and confirm newer edits are preserved.
- [ ] Change the original source while reviewing. Verify conflicts prevent Apply;
  restore a compatible clean baseline and Refresh review before approval.
- [ ] In a disposable project, interrupt source application. Inspect the receipts
  and private backups, repair the source, and explicitly confirm its outcome.
- [ ] Restore provider access after a temporary request failure and use Continue
  from checkpoint; verify task identity and budget history remain unchanged.
- [ ] In a disposable Local run, interrupt a file or app acknowledgement and open
  Review recovery. Stop the previous host before confirming it. Inspect file
  before/after content and preserve private stage bytes before clearing stages.
  Confirm applied or restored only when the actual filesystem matches.
- [ ] Verify an uncertain app outcome in the connected account and enter evidence.
  Confirm succeeded or failed; inspect its generated receipt labeled as user
  evidence. Prepare continuation and Continue from checkpoint; verify the same
  task/attempt and budget history, with no repeated edit or app mutation.
- [ ] Stop a run during recovery and verify it cannot continue; recorded outcomes
  may still be reconciled without starting new work.
- [ ] Stop a source/team task and verify real children stop; close an attachment
  separately and confirm the owner run continues.
- [ ] Check keyboard operation, light/dark legibility and long run/transcript lists.
- [ ] Supply a Nebius target and bounded cloud budget for remote acceptance after
  the registered host adapter is implemented. A key alone is not a remote host.

## Verification baseline

Current protocol is **28**. Rebuild desktop, daemon and clients together. The
progress document records the exact automated checks for each committed increment.
No live provider billing, app account mutation, cloud deployment, visual test or
second development watcher was performed during these checkpoints.

## 2026-10-07 — additional harness review

- [ ] Antigravity 1.3.0/1.3.1: select native Gemini credentials and exact low/high
  slug; verify thinking tokens are included in usage and output reservation.
- [ ] OpenCode 2 beta-17823: verify standalone execution preserves the original
  service and native selected direct account; test conflicting/OAuth profiles.
- [ ] Fx 0.0.13: verify the official native binary on Linux/macOS, exact configured
  model limits and account, one provider call and no native effects.
- [ ] With live access, verify all three adapters through delegation, review, stop,
  checkpoints, registered worker and pre-execution Local fallback.
- [ ] Amp/Cursor: finish the outstanding development contracts in the adapter guide
  before enabling Teams. Credentials alone do not complete those adapters.

## 2026-10-07 — command jobs and GPU lifecycle transport

- [x] Bounded credential-free remote command/file worker, real exit/log/file/hash
  receipts, generated summary artifact, explicit grants and pinned policies.
- [x] Durable command reservation and bound operation cancellation/restart; no
  repeated submission or Local retry after an uncertain outcome.
- [x] Native command/output tools and recovery of exact argument-bound receipts.
- [x] Separate Serverless AI Jobs lifecycle transport and localhost contract tests.
  Agent GPU launch remains disabled pending the integrations below.
- [ ] Linux/image/live command acceptance from the
  [command and GPU review guide](23-remote-command-gpu-jobs.md).
- [ ] Command lost-acknowledgement operator binding and richer recovery projection.
- [ ] GPU durable waits, monetary/resource/egress enforcement, dataset/checkpoint
  volumes, real output retrieval, cleanup verification and live acceptance.
- [ ] Enforce executed command receipts in required output-contract check gates.
