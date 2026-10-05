# Agent teams: implementation progress and review checklist

Updated 2026-10-05. Development branch: `codex/ion-agent-teams`, based on
`ion-prod` in a separate managed worktree. This is the first implementation
increment of [the agreed plan](18-agent-teams-implementation.md).

## Implemented in this increment

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
  query and stop runs. Protocol version is **11**; desktop/daemon/mobile clients
  must rebuild together.

`PrepareTeamRun` prepares a durable queue for an existing idle root task. It does
not launch a harness. `GetTeamRun`, `GetTeamAudit` and `StopTeamRun` return the
shared state. Delegation, claims, reports and reconciliation remain trusted
internal operations, pending authenticated harness bridges.

## Remaining development

- [ ] Wire each delegation to the materializer and dispatcher wakeup; drain
  additional batches during normal operation.
- [ ] Replace the legacy single-prompt Team task preparation with a run start
  action and daemon-owned dispatch.
- [ ] Native Nebius adapter: bind leases to driver sessions, enforce grants,
  expose delegation/report tools, return child results and resume parents.
- [ ] Bounded context handoffs, parent waiting/resume, review/rework, required
  output contracts and acceptance checks beyond recorded evidence.
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

The existing Team “New task” action still prepares one lead prompt and uses the
lead's saved model. It is not hierarchical team execution. Sandbox-configured
profiles/teams do not silently use that legacy local action.

## Developer verification

Run from this worktree:

```powershell
cargo test --locked -p waku-core teams:: -- --test-threads=4
cargo test --locked -p waku-core control::tests:: -- --test-threads=4
cargo test --locked -p waku-protocol protocol::tests::
cargo check --locked -p waku -p waku-daemon
bun run protocol:generate
bun run protocol:check
cargo fmt --all -- --check
git diff --check
```

Verified 2026-10-05 against this worktree:

- Teams regression suite: **19 passed**.
- Existing control store suite: **5 passed**.
- Protocol wire contract suite: **7 passed**.
- Native app and daemon: `cargo check --locked -p waku -p waku-daemon` passed.
- TypeScript bindings regenerated and the generator's `--check` passed.
- TypeScript client: **23 tests passed**, and its TypeScript type check passed.
- Rust formatting and `git diff --check` passed.

Cargo checks/tests reused the existing dependency cache with
`--target-dir D:/personal/apps/waku/target`; source changes live in the new
worktree. No app executable was launched or replaced by these check/test runs.
The existing dev watcher was not restarted. Changes are uncommitted.
No live provider, sandbox, mobile build or visual acceptance is claimed.

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
