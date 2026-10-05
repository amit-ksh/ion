# Agent teams: implementation progress and review checklist

Updated 2026-10-05. Development branch: `codex/ion-agent-teams`, based on
`ion-prod` in a separate managed worktree. This records the foundation, invocation, continuation and native adapter
increments of [the agreed plan](18-agent-teams-implementation.md).

## Foundation implementation

- Agent definitions retain their own harness/model and add an explicit Local or
  Nebius sandbox environment, target reference, local fallback policy, expected
  output, workspace/tool/app authorization data and shared concurrency limit.
  Legacy profiles migrate to Local, read-only team access and one active task.
- The existing native editor can save/edit environment, target, expected output,
  concurrency and workspace/command/network access. Existing exact app grants
  are preserved; an app action grant editor is still pending.
- A daemon-owned coordinator pins complete team/profile definitions and limits
  in each run. Edits and deletion of current definitions do not rewrite a run.
- Transactional delegation checks direct reporting relationships, scope
  intersection, existing dependencies, ancestor deadlocks, depth and work caps.
  A stable delegation ID cannot be reused with changed arguments.
- Every child gets a durable task UUID and outbox record. A task database
  materializer uses create-if-absent with this UUID, preserving existing
  transcripts. It confirms persistence before acknowledgment. A bounded batch
  replays on owner daemon startup; the dispatcher will drain later batches.
  The coordinator rejects a claim before that acknowledgment.
- Claims use opaque internal leases, enforce team and shared agent concurrency,
  deadlines and attempt limits. The strictest active profile limit remains in
  effect when a shared agent is edited between runs.
- Explicit sandbox selection is preserved. The trusted dispatcher may request
  local fallback after a confirmed failure before effects, with the same pinned
  harness/model/access. No Auto option exists and fallback cannot loop back to
  the sandbox. This is coordinator logic, not a deployed sandbox integration.
- Expired leases and owner daemon restart fence old workers, mark the outcome
  uncertain and retain capacity until the host confirms the outcome.
- Stop cancels queued work and blocks new delegation/claims. Running work stays
  canceling until its host confirms cancellation; unknown work stays uncertain.
- Results require a bounded summary and document or check evidence. Documents
  must belong to the assignment or its handoffs in the same project. Parents
  cannot complete with unfinished children. Repeated identical reports produce
  one durable result event; changed reports are rejected.
- Audit events are indexed and paged by cursor. Typed daemon commands prepare,
  query and stop runs. Protocol version is **13**; desktop/daemon/mobile clients
  must rebuild together.

`PrepareTeamRun` prepares a durable queue for an existing idle root task. It does
not launch a harness. `GetTeamRun`, `GetTeamAudit` and `StopTeamRun` return the
shared state. Delegation, claims, reports and reconciliation remain trusted
internal operations, pending authenticated harness bridges.

## Invocation increment (2026-10-05)

Foundation committed as `d093fa9` on `codex/ion-agent-teams`.

- The existing New task flow stays intact. Ordinary prompts use their selected
  solo harness. A leading `@teamname <task>`, `@{Team Name} <task>` or a qualified
  address from @ suggestions opts into Teams. Team suggestions use the existing
  keyboard cursor, Enter/Tab acceptance and Escape dismissal alongside files.
  Team queue submissions remain available without an installed solo harness;
  a cached name lookup keeps that control consistent with keyboard submission.
- The Teams page's New task action inserts an explicit team address and its
  objective. It no longer concatenates member instructions into one lead prompt.
- Duplicate names require the picker's UUID-qualified address. Deleted qualified
  teams, missing objectives, stale definitions and file/name collisions cannot
  silently start a solo task. Quoted/code/later mentions are ordinary text.
- The daemon queues the run with pinned profiles, then materializes its root
  through the same durable outbox as children. It preserves typed text and the
  lead's saved harness/model. Exact repeated submissions reuse the run/task IDs.
- Every materialized task carries its daemon-owned team binding. A per-task
  startup gate and indexed membership check prevent a team task entering a solo
  runtime, including the gap before outbox replay. Client state saves cannot
  overwrite the daemon's pinned team task. Membership checks are batched per
  save rather than reconnecting for each projection. Restart hydration restores
  the binding; outbox replay can adopt older rows without losing their transcript.
- The native Nebius **normal task** runtime advertises `list_teams`, `invoke_team`
  and `get_team_runs`. It can choose an existing team when useful without the
  user having to type a mention. Its trusted workspace context supplies the
  source task/project. Model arguments cannot change those identities or grants.
- Agent invocations are correlated by source task and provider tool-call ID.
  Identical repeats and concurrent requests yield one durable run; changed
  arguments are rejected. Each source task can create at most four team runs.
  Invocations preserve the source link, use conservative read-only authorization
  and the coordinator's depth/work/concurrency/attempt/deadline defaults.
- Normal task catalog refresh replays one bounded outbox batch on a daemon worker.
  Direct user submission materializes its own run so an unrelated broken project
  cannot block that submission. Restart never blindly repeats an interrupted
  invocation; the agent inspects its own durable runs first.
- Team status appears in the task. UI and tool responses explicitly say execution
  is unavailable. The current bridge **queues work only**: it does not launch
  child harnesses or claim completion. Normal-agent invocation for external
  harnesses remains pending their authenticated tool bridges.
- Team attachment handoffs and team follow-ups are pending. Rejected submissions
  retain their text and attachments. No new task-creation screen was added.

## Parent continuation increment (2026-10-05)

Invocation checkpoint committed as `c64810c`.

- A trusted adapter can persist a private JSON checkpoint and yield while it
  awaits up to 16 of its own direct children. Checkpoints are capped at 128 KiB
  and never appear in task projections or public wire responses.
- Yield commits the checkpoint and releases the execution slot atomically.
  Three-level teams therefore work even with a team concurrency limit of one.
  Waiting parents survive owner restart without being treated as live hosts.
- Resume requires all watched children to have settled; an uncertain child
  keeps its parent waiting. Resume checks shared/profile/team capacity again,
  retains the same attempt, host, harness/model and access, and consumes no retry.
- A fresh lease generation on resume rejects old workers' renew, delegation,
  checkpoint and report calls. Racing resumes issue one continuation; exact
  repeated suspension is idempotent and changed replay arguments are rejected.
- Child handoffs contain bounded summary/check evidence, artifact IDs, task
  links and omission counts. JSON escaping counts toward their context budget.
- Stop cancels waiting parents; active children still require host confirmation.
  Deadlines block waiting and queued assignments. Live expired hosts remain
  uncertain until reconciled.
- These are coordinator operations. Actual adapter dispatch, child execution,
  review/rework and live task projections remain the next implementation work.

## Native adapter increment (2026-10-05)

Parent continuation checkpoint committed as `54c26cd`.

- A native Nebius Teams worker runs separately from the existing solo driver.
  Its opaque lease selects the authoritative pinned assignment and profile;
  caller projections cannot substitute objective, harness/model or grants.
  It requires a Local attempt (including confirmed Local fallback) and an
  explicit Nebius model. A cloud model API does not change the selected host.
- Structured tools delegate to direct reports, await their results, publish/read
  assignment documents and report a result or blocker. Models cannot supply
  caller/task/project identity, host, model, access grants or trusted receipt IDs.
- Parent model messages and pending tool calls are checkpointed before effects.
  Yield commits the completed tool reply and continuation with lease revocation.
  Resume retains that conversation and adds only bounded child handoffs.
  No unfinished effect is blindly replayed as a continuation.
- The worker retains a limit of 16 model calls across yields, at most eight tools
  per response, 64 KiB of message context, 128 KiB of private checkpoint, bounded
  tool replies, 4,096 requested output tokens and 1 MiB of SSE response data.
  These bounds are not dollar-budget enforcement; reservations remain pending.
- Workspace read tools are offered only with read authorization, enforce the
  project root, reject traversal/links/private paths, and bound directory walks
  and UTF-8 excerpts. No workspace write, shell, arbitrary network or connector
  tools are exposed yet. Host isolation and exact resource policies for external
  harnesses remain pending.
- Completion requires document evidence; plain text cannot finish an assignment.
  Unknown documents, unfinished children and invalid waits return feedback.
  Blockers are recorded with an unsuccessful turn outcome. Yield emits a parked
  turn rather than a completion event; resume continues that open turn.
- Generated document payload, exact tool receipt, artifact provenance and audit
  event commit together in the control database. Repeated/concurrent publication
  yields one document; changed arguments or stale leases are rejected. The normal
  document reader supports these payloads alongside existing imported files.
  Parent reads are limited to its own work and child/dependency handoff scope.
- This adapter is available to a trusted host worker. The daemon dispatcher,
  durable transcript projection, readiness/budget gates and UI runtime attachment
  are not connected yet. User and normal-agent invocation remain queue-only;
  this increment does not claim that the application executes a queued team.

## Remaining development

- [ ] Wire each delegation to the materializer and dispatcher wakeup; drain
  additional batches during normal operation.
- [x] Replace legacy single-prompt Team preparation with explicit @ addressing
  and daemon-owned durable queue creation.
- [x] Add the native normal-agent team invocation queue bridge.
- [ ] Implement actual team start/dispatch and authenticated harness execution.
- [x] Native Nebius worker with lease-bound delegation/report/document tools,
  enforced read grant and bounded resumed child context.
- [ ] Connect native workers to the daemon dispatcher, durable transcripts,
  readiness/budget gates and attached runtime events.
- [x] Durable parent waiting/resume and bounded child result handoffs.
- [x] Native adapter delivery of bounded child handoffs and document evidence.
- [ ] Review/rework, required output contracts and acceptance checks beyond
  recorded evidence.
- [ ] Per-call token/model/cost reservations and budget enforcement. Current
  concurrency/work/depth/attempt/deadline caps do not enforce dollar budgets.
- [ ] Exact filesystem/resource/egress grants and Composio action grant editor.
  A prompt, worktree or saved grant is not permission enforcement.
- [ ] Team readiness matrix and supported model validation for each harness.
- [ ] Native hierarchy/roster with live child links, blockers and results; cached
  run projections across Dashboard, Inbox, Decisions and Artifacts.
- [ ] One validated external harness bridge, then additional mixed harnesses.
- [ ] Serialized writers, isolated coding worktrees and integration/patch review.
- [ ] Registered Nebius execution host: actual harness launch, explicit transfer,
  host readiness, remote status/cancel/checkpoint and tested local fallback.
- [ ] Persisted user decisions bound to exact action/revision/host.
- [ ] Routine scheduling and workflow execution over the same coordinator.

The Team “New task” action now prepares an explicit team address in the normal
composer. Both user addressing and native agent invocation queue a real run.
Hierarchical harness execution remains under development. Neither Local nor
Nebius configurations silently run through the old solo action.

## Developer verification

Run from this worktree with dependencies installed:

```powershell
cargo test --locked -p waku-core team -- --test-threads=4
cargo test --locked -p waku-core control::tests:: -- --test-threads=4
cargo test --locked -p waku-protocol protocol::tests::
cargo test --locked -p waku-protocol teams::mentions::
cargo test --locked -p waku-client composer_complete::tests::
cargo check --locked -p waku -p waku-daemon
bun run protocol:generate
bun run protocol:check
bun test packages/waku-client
bun run --cwd packages/waku-client check
cargo fmt --all -- --check
git diff --check
```

Verified 2026-10-05 against this worktree:

- Team coordinator, invocation, recovery and daemon integration checks:
  **43 passed**, including **32 coordinator tests** and **7 native adapter
  tests**. The six continuation
  regressions cover three-level execution with one slot, restart, stale leases,
  exact replay, racing resume, shared capacity, cancellation/deadlines and
  uncertain children with escaped/Unicode handoffs. The native adapter checks
  a three-level document/review chain, pinned definitions, denied tools and
  identity overrides, private/traversal reads, bounded replies, invalid waits,
  unfinished children, cancellation before effects and plain text without a
  completion receipt. Document checks cover atomic replay, concurrent publication
  and assignment/handoff provenance.
- Team addressing parser: **3 passed**.
- Native composer completion suite: **11 passed**.
- Existing control store suite: **5 passed**.
- Protocol wire contract suite: **7 passed**.
- Native app and daemon: `cargo check --locked -p waku -p waku-daemon` passed.
- TypeScript bindings regenerated and the generator's `--check` passed.
- TypeScript client: **23 tests passed**, and its TypeScript type check passed
  using the primary checkout's existing TypeScript dependencies.
- Rust formatting and `git diff --check` passed.

Cargo checks/tests reused the existing dependency cache with
`--target-dir D:/personal/apps/waku/target`; source changes live in the new
worktree. No app executable was launched or replaced by these check/test runs.
The existing dev watcher was not restarted. Foundation commit: `d093fa9`.
Invocation checkpoint: `c64810c`. Parent continuation checkpoint: `54c26cd`.
The native adapter increment is committed separately after verification.
No live provider, sandbox, mobile build or visual acceptance is claimed.

## User review of invocation (queue only)

- [ ] Send a plain New task prompt; confirm normal solo behavior.
- [ ] Start a prompt with `@teamname` or choose a team from @ suggestions.
- [ ] Use a spaced team name, duplicate names and an unknown qualified team.
- [ ] Press Enter/Tab/Escape and use arrows in the shared @ picker.
- [ ] Confirm a team reference in quotes, code or later text does not route work.
- [ ] Confirm path mentions such as `@./engineering` still address files.
- [ ] Confirm the queued task preserves the lead harness/model and typed prompt.
- [ ] Refresh/restart and confirm the same task/run survives without duplicates.
- [ ] Observe that queued work is not shown as executing or complete.
- [ ] In a native Nebius solo task, ask it to use a saved team; inspect the
  `invoke_team` call and its queue/status result. Requires a privately saved key.
- [ ] Confirm actor/project overrides, changed repeated calls and a fifth
  invocation are rejected; external harness bridges remain pending.

## User review when the execution adapters are ready

- [ ] Create multiple teams and assign one agent to more than one team.
- [ ] Configure different harnesses and supported models for lead, manager,
  specialist and reviewer. Confirm each real child transcript uses its choice.
- [ ] Select Local or Nebius sandbox per agent; verify no Auto entry appears.
- [ ] Edit a profile and confirm an existing run retains its pinned settings.
- [ ] Follow lead → manager → specialist delegation, review/rework and one
  document through actual child task links.
- [ ] Reject hierarchy violations and access expansion without doing the work.
- [ ] Verify shared concurrency and budget limits across projects/teams.
- [ ] Stop a team with active children; confirm the hosts stop and status follows.
- [ ] Restart the daemon during a child creation and an active assignment;
  confirm no duplicate child task or unconfirmed effect replay.
- [ ] Make a sandbox unavailable before start; confirm Local (fallback) with
  unchanged harness/model/access. Missing local requirements show a blocker.
- [ ] Disconnect a running sandbox job; confirm it stays uncertain until remote
  outcome reconciliation, rather than starting a second local job.
- [ ] Exercise important decisions and resulting Inbox/Dashboard notifications.
- [ ] Review light/dark legibility, keyboard focus and long virtualized lists.

## Setup still needed for live acceptance

- Install/authenticate the harnesses selected by your agents on their actual
  execution hosts. Unsupported or unready integrations must remain blocked.
- Add a Nebius Token Factory key privately in Settings for the native provider.
- Configure a Nebius sandbox target, host credentials and bounded cloud budget
  before testing actual remote execution. No target has been supplied yet.
- Configure Composio privately and grant exact account/actions when that adapter
  is implemented. Merely connecting an app does not grant execution access.

These resources are not required for the coordinator's local regression tests.
