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
- [x] Pinned bounded remote command check gates use original successful receipts
  and current source witnesses at submission and parent acceptance. Descriptive
  check labels do not prove execution.
- [ ] Local/fallback/general project checks and Linux/live command isolation
  acceptance; bounded remote gates do not establish these broader capabilities.
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

Current protocol is **31**. Rebuild desktop, daemon and clients together. The
progress document records the exact automated checks for each committed increment.
No live provider billing, app account mutation, cloud deployment, visual test or
second development watcher was performed during these checkpoints.

## 2026-10-07 — additional harness review

- [ ] Antigravity 1.3.0/1.3.1: select native Gemini credentials and exact low/high
  slug; verify thinking tokens are included in usage and output reservation.
- [ ] OpenCode 2 beta-17823: verify standalone execution preserves the original
  service and native selected direct account; test conflicting/OAuth profiles.
- [x] Fx 0.0.13: official Linux x86_64 binary verified against the release digest;
  actual Rust adapter fixture and twelve gate regressions passed with a fake key,
  localhost provider and external networking disabled.
- [ ] Fx: accept the actual native account/model limits and immutable registered
  host; verify live usage and macOS separately before claiming those targets.
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
- [x] Fixed Linux command/guard CLI acceptance in a pinned Docker image, including
  real child isolation, failed outcomes and private exclusive result collection.
- [ ] Imported image/live command acceptance from the
  [command and GPU review guide](23-remote-command-gpu-jobs.md).
- [x] Command lost-acknowledgement operator binding, separate command recovery
  labels, exact original input/policy witness and continuation without resubmission.
- [x] GPU durable launch/cancel claims, immutable owner spending allowances,
  count/concurrency/deadline checks and saved resource identities. Unknown or
  terminal outcomes do not release reservations. Agent GPU launch remains gated.
- [x] GPU VM/disk inventory and one-time deletion claim; typed completed deletion
  plus absence proofs release compute capacity while output/unknown cost remain held.
- [x] GPU lost creation/cancellation/deletion acknowledgement binding: native
  recovery controls, original typed request/key/input verification and revision
  fencing, with no repeated mutation or reservation release.
- [ ] GPU recovery for missing historical VM/disk inventory; lost acknowledgements
  without sufficient native operation/input evidence remain unresolved.
- [x] GPU result volume and owner signed S3 receipt reader, original command/file
  validation, durable generated artifact, replay and assignment-scoped readback.
- [x] GPU durable waits for approved bound jobs, owner background polling, Stop/
  deadline uncertainty and native same-attempt continuation with real receipts and
  verified cleanup. Monetary reservations remain held independently.
- [x] Exact owner GPU tenant/project/network/preset/resource approval, fresh native
  identity/quota/capacity reads and durable sanitized preflight receipts.
- [x] Atomic cumulative GPU/vCPU/RAM/disk resource limits across runs, stricter
  outstanding approvals and conservative provider quota/capacity headroom. Unknown legacy
  quantities/terminal states stay reserved until original cleanup is verified.
- [ ] Live GPU resource-policy/readiness/cleanup-capacity review from the command
  guide, including actual native omissions, capacity units and quota identities.
- [ ] GPU agent submission, verified monetary/egress enforcement, large
  dataset/checkpoint volumes, retention policy and live acceptance.
- [x] Persistent post-cancellation result collection and cleanup maintenance with
  saved backoff, fenced expiring claims and the shared four-worker GPU pool.
  Runtime receipt/cleanup settlement retains canceled status and monetary holds.
- [x] Provider GPU job timeout minimum (one hour), separate short command deadline
  and unchanged original team deadline; invalid new jobs fail before reservation.
- [ ] Live post-cancellation/restart/permission/concurrency review from the command
  and GPU guide; GPU submission still needs the launch policy integrations above.
- [x] Enforce pinned executed command receipts in required output-contract gates,
  with exact arguments/files/time, current-attempt identity and source freshness.
- [ ] Live executed-check review, including source changes, parent rework, missing
  output and restart replay; see the command guide.

## 2026-10-07 — trusted Linux child socket boundary

- [x] Fixed root-owned guard runs after UID/GID demotion and before user exec;
  exact accepted kernel, trusted libseccomp, empty capabilities, sanitized env,
  closed FDs, socket/io_uring/namespace/theft/compatibility-ABI denial and inherited
  no_new_privs/filter. Missing support stops before user code.
- [x] Nine Linux CLI guard checks plus six worker checks in a pinned image,
  including testing with Docker outer seccomp disabled. Root collector can still
  write genuine isolated receipts; child cannot forge the result volume.
- [x] New owner GPU preflight requires image-bound socket-mechanism approval before
  reads/reservation. Legacy original-policy replay/recovery retains signatures;
  missing approval cannot authorize a new launch.
- [ ] Accept each provider GPU image/kernel and CUDA workload. Exclude RDMA/NIC/
  VFIO devices, inherited handles, network filesystem and IPC/file proxies; check
  actual device access, threading, private collector mount and receipt persistence.
- [ ] Review account/billing, complete egress/runtime enforcement and remaining
  GPU agent submit integrations before enabling the target. Local Docker tests
  do not establish cloud/GPU acceptance.

## 2026-10-07 — Fx and native Linux worker acceptance

- [x] Official Fx Linux x86_64 0.0.13 binary digest/provenance and actual Rust
  launch/profile fixture, with exact model/task, one metered request, no native
  tools and output clamp. Three Fx and twelve gate tests passed without external
  networking; all twelve catalog adapters now have native transport/CLI fixtures.
- [x] Actual Linux debug harness worker built offline from the unchanged lockfile.
  Existing protocol regression and three actual-binary argument/input-boundary
  checks passed, without request echo or unsanitized diagnostics.
- [ ] Build the production release/registered image, configure its selected native
  account/bootstrap and accept actual inference/stop/restart with your resources.
- [ ] Amp/Cursor Teams contracts and other account transports remain development.
  Fx's Linux fixture does not establish Windows Local or macOS support acceptance.
