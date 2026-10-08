# Agent Teams release checklist

Updated 2026-10-08. Implementation branch: `codex/ion-agent-teams` in the managed
worktree. These changes have not been integrated into `ion-prod` by this task.
Use [implementation progress](19-agent-teams-progress.md) for checkpoint evidence
and [the confirmed specification](18-agent-teams-implementation.md) for scope.

## Current scope correction (2026-10-08)

The user deferred adapters that have not been added yet. Keep all implemented
adapters and native Nebius/Codex/Claude/OpenCode normal-task Teams connections.

The following additions are **out of scope for the current implementation**:

- Amp and Cursor Teams completion adapters.
- Remaining normal-task Teams bridges, including Pi and Oh My Pi.
- Additional unsupported OAuth/subscription/custom provider/account routes.

These are deferred work, not completed capabilities or current release gates.
Existing adapter setup, supported-account review and live acceptance still apply.
Remote filesystem/commands/GPU development remains in the plan. Historical
checkpoint sections below describe the scope at their date; this correction
supersedes references there to unfinished adapters as required development.

## Implemented and covered by local regression tests

- [x] Reusable teams, direct reporting hierarchy and per-agent harness/model,
  instructions, expertise, expected output, permissions and concurrency.
- [x] Explicit Local / Nebius sandbox selection; no Auto mode. Confirmed unavailable
  sandbox can fall back locally with the same selected harness/model/grants.
- [x] Preserve normal New task; invoke a team with `@teamname`, `@{Team Name}`
  or the qualified picker address. Ambiguous names require explicit selection.
- [x] Normal native Nebius, Codex, Claude and OpenCode agents can invoke saved teams with source
  permissions, wait for their actual outcome and read accepted generated documents.
  New Codex threads register native dynamic tools; reopening retains those tools.
  OpenCode uses a native plugin with source identities observed on its shared
  server; its real account/plugin review is listed in the adapter setup doc.
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
- [x] Pinned per-stage required HTML/Markdown/text/CSV/JSON output documents,
  real lead output contracts and final-stage evidence revalidation; requirements
  remain visible on desktop/phone and immutable for existing runs.
- [x] Phone workflow/routine history and instructions, saved-definition Run now,
  schedule enable/pause, exact stage gates and Stop, with persisted submission
  identity and fresh-owner/focus fences. Desktop retains definition editing.

## Development still required for the full planned release

- [x] Antigravity native direct Gemini and OpenCode 2 standalone direct OpenAI
  Teams adapters: Local, registered completion and recovery wiring.
- [x] Fx configured direct OpenAI/Nebius adapter implementation and dispatch wiring.
- [x] Fx official native Linux x86_64 fixture acceptance with the actual 0.0.13
  binary, fake key and localhost provider; twelve Linux transport regressions.
- [ ] Fx macOS and live selected-account/registered-host acceptance.
- [x] Native Codex normal-task bridge, exact source-turn scope, durable invocation
  replay, bounded approval/wait work and actual app-server start/resume fixture.
  Imported Codex threads created without dynamic tools cannot retrofit them through
  the verified native resume interface; start a new Ion Codex task for team tools.
- [x] Native Claude SDK MCP normal-task bridge, observed main-session tool identity,
  exact grants/replay/cancellation and actual native start/resume/pending-approval fixture.
- [x] Registered immutable ConTree worker: actual selected-harness completion,
  readiness, bounded transfer, durable operation identity, cancellation, restart
  reconciliation, usage and pre-execution Local fallback. Live setup/review pending.
- [x] Verified operator binding after a lost sandbox acknowledgement, terminal
  process verification and explicit remote checkpoint continuation. Exact input
  witnesses are required; legacy requests and live setup remain review items.
- [ ] Full remote project/command execution, remote checks, resource/egress controls
  and GPU/ML job provisioning. Separate bounded command jobs are implemented;
  project snapshots now support a sealed isolated workspace within explicit
  transfer approval. The selected-harness completion worker returns actions to the owner.
- [x] Pinned bounded remote command check gates use original successful receipts
  and current source witnesses at submission and parent acceptance. Descriptive
  check labels do not prove execution.
- [ ] Local/fallback/general project checks and live command isolation acceptance;
  bounded remote gates do not establish these broader capabilities. Local Linux
  installed-entrypoint and owner-path acceptance now passes.
- [ ] Pinned price/currency and monetary/cloud spending reservations and usage.
- [ ] Aggregate host resource/egress grants and enforcement for general commands;
  pinned bounded-command Linux rlimits are implemented.
- [ ] Image/PDF input interpretation through a verified selected harness.
- [x] Persistent current-notification Inbox read/acknowledge/archive/restore state
  and desktop task notification policy, with independent owner revision and
  concurrency/restart/capacity checks.
- [ ] Complete owner notification history, global unread badge and phone Inbox UI.
- [x] Artifact content hashes and original task/team producer metadata, bounded
  verified read/export/report/review/handoff and agent-only client publication.
- [ ] Embedded artifact previews/additional interpretation and global usage ledger.
- [ ] Registered-owner portfolio/gateway projection and remote workflow controls.
- [ ] Cloud speech live partial streaming and deployment/device acceptance.

Elapsed-interval routines and sequential workflow execution now reuse the Teams
coordinator; see [workflow/routine behavior and review](21-workflows-routines.md).
Stage retries, binary document publication and
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

Current protocol is **33**. Rebuild desktop, daemon and clients together. The
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
- Deferred: Amp/Cursor Teams contracts and additional unsupported account routes.
  Fx's Linux fixture does not establish Windows Local or macOS support acceptance.

## 2026-10-07 — native GPU estimate receipts

- [x] Immutable owner SKU/quantity/component approval, explicit hourly ceiling,
  exact currency and startup/cleanup/extra allowances; absent historical fields
  preserve original signatures and cannot authorize new production launches.
- [x] Fixed native v1 calculator request binds the entire original typed Job body.
  Exact integer decimals, SKU/aggregate equality, metadata/units and upward
  reservation arithmetic reject incomplete, ambiguous or excessive estimates.
- [x] Sealed receipt freshness/body/policy verification and sanitized transactional
  storage before one create; original eligible replay has no new reads or POST.
- [ ] Live native Job estimation support and exact account SKU/quantity/default
  semantics. Unsupported estimation prevents launch; Compute approximation is
  not an implemented alternate contract.
- [ ] Selected billing-account/contract identity, retained storage/transfer/tax
  accounting and authoritative usage/finality reconciliation. Estimate/lifecycle
  assumptions do not establish an invoice cap; all monetary reservations stay held.
- [ ] Remaining agent GPU submission, image/device/egress/CUDA, historical inventory
  and dataset/checkpoint/retention integrations. See the
  [research note](RESEARCH-nebius-gpu-price-receipt.md) and command guide review list.

## 2026-10-07 — native release packaging and registered process identity

- [x] Reproducible multi-stage worker release recipe using reviewed tracked source,
  supplied pinned bases and unchanged release profile/lockfile. No build HOME,
  Cargo cache, source tree or native credentials are copied to runtime.
- [x] Manifest version 2 explicitly pins Linux UID/GID 1000 and `/home/ion`.
  Instance POST includes UID/GID and PATH/HOME; the worker verifies effective
  identity/home/cwd before native account/catalog lookup. Original input pins
  identity through recovery; old omitted fields retain legacy request semantics.
- [x] Nonroot runtime setup creates private `/ion/work` and home/config directories
  and removes inherited setuid/setgid bits. The release ABI check has no request.
- [x] 21 remote/worker Rust tests and desktop/daemon compile gates passed, including
  identity/version errors, durable submission and exact recovery restrictions.
- [x] Actual Linux release image built from `ad21a87`; ten executable test groups
  passed offline under UID/GID 1000, read-only root, dropped capabilities and
  no-new-privileges. Correct identity reaches only fake help; mismatches block
  native lookup. Source/digest/ELF evidence recorded in the worker package guide.
- [ ] Install/accept each exact native CLI/runtime/catalog privately, add version-2
  `/ion/host.json`, configure host-native authentication and register immutable UUID.
- [ ] Live complete worker/usage/stop/restart/lost-acknowledgement acceptance remains
  separate. The recipe alone does not establish a deployed cloud execution host.
- [x] Current Amp native catalog/plugin fixtures and Cursor 1.0.36 SDK/bridge
  research saved. Amp's complete bounded actor usage and Cursor's hard completion
  cap/unexpected effects/native-account acceptance remain unimplemented.

## 2026-10-08 — normal Codex tasks invoking teams

- [x] Shared source-task catalog/invoke/status/wait/document module used by native
  Nebius and Codex; authoritative task/project, saved lead grants and pinned limits.
- [x] Five Codex dynamic functions; only the live main thread/turn can call them.
  Native peer RPC IDs never grant access or become Ion approval IDs.
- [x] Bounded background queue, interruptible approvals, exact durable replay,
  changed-call rejection, foreign/late call rejection and cancellation races.
- [x] Actual native Codex 0.162.0-alpha.2 app-server through Ion's driver, private
  fake account and localhost Responses fixture: one approved run, correct result
  ID and saved tools/readback after native process exit and thread resume.
- [ ] With your native Codex account, start a new Ion Codex task and ask it to use
  a saved team. Inspect the exact lead/model/Local-or-Nebius grant before approving.
- [ ] Decline once, then approve; verify only the approved invocation queues work,
  actual assignment status is shown and queue acknowledgement is not completion.
- [ ] Reopen that task; ask for status and a completed accepted document. Stop during
  an approval/wait and verify its own team runs stop without affecting other tasks.
- [ ] Accept additional native app-server releases/account routes with your setup.
  Other external solo bridges and Amp/Cursor are deferred. Broader remote/GPU work
  remains in scope.

## 2026-10-08 — normal Claude tasks invoking teams

- [x] SDK MCP server with exactly five shared source-task functions. The daemon
  supplies the task/project; saved lead harness/model/environment, grants and
  budget remain authoritative.
- [x] Main native assistant tool-use observation plus exact `claudecode/toolUseId`
  correlation. Reject foreign/subagent/unobserved/changed calls and unknown
  arguments/metadata. Changed observations cancel outstanding grants.
- [x] One shared bounded Codex/Claude approval worker; separate Ion approval IDs,
  native invocation identity across peer RPC changes and restart, interrupted
  approvals and native `control_cancel_request` handling.
- [x] Background native shutdown with two-second EOF grace and owned process-tree
  termination. No blocking wait or process I/O is introduced into render or Drop.
- [x] Actual Claude **2.1.289** through Ion's production driver with an isolated
  fake account/localhost provider: start, one approved invocation, confirmed process
  exit, resume/read the same run, and close during outstanding approval without
  another run. Production user configuration is retained outside the private fixture.
- [ ] With your Claude account, start a normal Ion Claude task and ask it to use
  a saved team. Approve/decline the exact lead grant, inspect the durable queue
  and resulting document, reopen and read the same run. Stop during approval
  and verify a late approval cannot start work.
- [ ] Run an actual mixed-harness team to completion with your selected models and
  configured app accounts; the source bridge's synthetic responses only establish
  correct native transport, authorization, durable queue and readback.

The native fixture and behavioral proof are documented in the adapter guide.
Other solo bridges and Amp/Cursor are deferred by the current scope correction.
Live native account acceptance and broader remote/GPU development remain open.

## 2026-10-08 — installed command worker and owner path

- [x] Exact shipped manifest/socket boundary accepted by owner preflight; invalid
  manifests reject before a command reservation or create POST.
- [x] Actual Linux CLI binary transfer, identity/socket/environment, failed outcome,
  timeout/log, orphan, unsafe-output and private exclusive collector acceptance.
- [x] Missing supervisor capabilities reject before child code; Windows checkouts
  preserve valid LF Linux scripts.
- [x] Native exported receipt through production owner bundling, durable reserve,
  one create/observe, atomic generated artifact, scoped paged output and replay.
- [ ] Review the imported live image/kernel/library, supervisor capabilities and
  real operation/receipt contract. Repeat after runtime/image changes.
- [ ] Complete broader project/resource/egress, GPU device/FUSE and billing gates
  listed in the [command/GPU guide](23-remote-command-gpu-jobs.md).

## 2026-10-08 — approved child resources

- [x] Versioned private command-resource approval, bounded quantities and exact
  image support before remote reads/reservation/create. Model arguments cannot alter it.
- [x] Fixed native Linux hard CPU/address-space/file/process/descriptor limits,
  no core dumps, stricter inherited ceilings and removed setup environment.
- [x] Original policy/body/environment witnesses and missing-field legacy recovery.
- [ ] Imported runtime/UID/inherited-limit acceptance; aggregate VM resource,
  allocation/egress and billing controls remain separate work.

## 2026-10-08 — bounded project snapshots

- [x] Explicit private source/file/count/run-byte approval; sealed isolated run
  source and actual Nebius grants; tracked/unignored inventory with private exclusions.
- [x] Atomic archive/staging reservation before upload; exact content-addressed
  upload identity, pinned instance file injection and original-operation recovery.
- [x] Worker verifies full archive and file hashes before exec; trusted snapshot
  receipt and whole-project source checks at report and parent acceptance.
- [x] Staging participates in pending-effect/Stop/restart fences; no compute after
  failed upload or Stop, and no repeat upload/compute after lost acknowledgement.
- [ ] Live imported-image/file-injection and namespace retention acceptance using
  [the project review checklist](23-remote-command-gpu-jobs.md#2026-10-08--project-snapshot-command-execution).
- [ ] Large returned files/datasets, dependency/network access, local general checks,
  aggregate host/cloud billing controls and full GPU training remain development.


## 2026-10-08: executable project archive acceptance

- [x] Windows tracked executable permissions, Unix current permissions, exact
  archive hash and rejection of unsafe/conflicted Git index entries.
- [x] Permission-only source changes reject original successful command receipts
  at result submission and parent acceptance.
- [x] Actual Windows-generated archive executes a script with the installed Linux
  worker; native owner receipt and full upload/replay/artifact/review acceptance.
- [ ] Verify the registered host's executable private workspace mount and run a
  real approved project command when the Nebius target is available.

## 2026-10-08 — Inbox controls and artifact evidence

- [x] Durable current-notification state and All / Issues only / Off desktop task
  notifications; organizing Inbox does not change execution authorization.
- [x] Agent-only artifact publication, original SHA-256/bytes/producer evidence
  and verification at consumer, submission, review and workflow boundaries.
- [x] Owner/native integration tests, desktop/daemon and shared/phone type checks.
- [ ] Complete the [Inbox/artifact live review](24-inbox-artifact-review.md).
- [ ] Historical notification index/global unread/mobile UI, embedded rich document
  previews and global usage ledger remain development, alongside the execution,
  accounting, calendar/graph and gateway/speech work listed above.

## 2026-10-08 — required workflow documents

- [x] Manual/generated editable requirements using current team publication formats.
- [x] Original root output contract at creation/replay, actual document submission
  enforcement and revalidation before next-stage or final completion.
- [x] Pinned desktop/phone projection, legacy compatibility, real dispatcher and
  focused owner integration acceptance; build and shared/phone type checks pass.
- [ ] Complete [the workflow document review](21-workflows-routines.md#your-review).
- [ ] Calendar/graph/retry/code integration, binary document publication and the
  other current-scope execution/accounting/Inbox/gateway/speech gates remain open.

## Calendar implementation checkpoint (2026-10-08)

Calendar routines, explicit missed/overlap policies and next-three-fire preview
are implemented in the Teams worktree. See the calendar section in
[workflows and routines](21-workflows-routines.md) for exact behavior and your
unchecked native/phone/DST review. Owner tests cover concurrent scheduling and
real team capacity. This does not close conditional graphs, historical/mobile
Inbox, global usage, remote/GPU, gateway or speech development. New harness
adapters remain deferred by user instruction.


## Historical and phone Inbox checkpoint (2026-10-08)

- [x] Transactional owner notification journal and bounded durable task outbox,
  retained resolved/deleted facts, indexed paging and global unread/pending counts.
- [x] Native and phone shared read/ack/archive controls; phone actual planning
  decision/proposal review and accepted-proposal prefill of the unsent New task.
- [x] Owner concurrency/restart/deletion/gate/artifact/rollback acceptance, team
  and persistence regressions, matching desktop/daemon/shared/phone type checks.
- [ ] Complete the expanded [desktop and phone review](24-inbox-artifact-review.md).
- [ ] Global usage, embedded previews, retries/code integration and broader
  remote/GPU/gateway/cloud speech gates remain open. New adapters stay deferred.
