# Paperclip-inspired team experience for ion

Status: **implementation plan, not implemented**. Researched on 2026-09-29.
The companion [Paperclip research note](RESEARCH-PAPERCLIP.md) records the
first-party product and UI evidence. This plan adapts the flow to a native IDE;
the [ion product decisions](README.md) remain authoritative. Earlier Nebius,
phone, gateway, and sandbox live reviews remain open.

## Product thesis

Paperclip's useful pattern is a human setting direction and limits, agents
breaking the work into assigned tasks, and a control plane making progress,
spend, and decisions inspectable. Its older quickstart documents a CEO-led
strategy approval before visible task execution; newer releases simplify setup
and start in a chat task where a general agent clarifies the goal and proposes
an item-by-item plan for approval. The stable lesson is the review gate before
autonomous execution, not a required CEO persona.
[Paperclip quickstart](https://docs.paperclip.ing/guides/getting-started/five-minute-path/),
[v2026.831.0](https://github.com/paperclipai/paperclip/blob/master/releases/v2026.831.0.md),
[v2026.916.0](https://github.com/paperclipai/paperclip/blob/master/releases/v2026.916.0.md).
ion should make the **project and deliverable** the top-level object: a
developer asks for an outcome, reviews a short execution plan, and sees a team
work against the repository. The user is the final authority for scope,
credentials, host access, consequential actions, and completion.

Do not present an IDE user with mandatory CEO/employee/company setup. Map the
concepts as follows:

| Paperclip concept | ion concept | Why |
| --- | --- | --- |
| Company goal | Project objective or one task's outcome | Keeps work tied to a repository and deliverable. |
| CEO/manager | Lead agent for a run | A role within the task, not a permanent fake employee. |
| Employee | Named specialist preset with capabilities | Reuse roles across tasks without an org chart prerequisite. |
| Issue/subissue | Durable work item with parent, assignee, dependency, and evidence | Explains what is being done and why. |
| Heartbeat | Bounded wake/run attempt | Allows work to pause between triggers and accounts for each attempt. |
| Board approval | User decision with an exact, immutable proposal | Keeps scope and important effects under human control. |
| Routine | Automation definition producing an ordinary task run | Makes scheduled work inspectable and recoverable. |

Paperclip models hierarchical tasks and agent heartbeats, while routines create
or link to execution issues rather than running invisible work. These are the
precedents for the mapping, not existing ion behavior.
[Task model](https://github.com/paperclipai/paperclip/blob/master/doc/PRODUCT.md),
[routines](https://docs.paperclip.ing/reference/api/routines/).

## The intended end-to-end flow

Example: the user asks ion to "make the mobile task review work offline and
show me what changed."

1. **Describe the outcome.** The New task sheet captures objective, project,
   selected daemon, optional deadline, budget, and whether team mode is on.
   The daemon checks repository, model, workspace, and requested capabilities
   before spending tokens. A missing Nebius key or disconnected host is shown
   as a concrete setup blocker. The user can still create an unstarted draft.
2. **Propose a plan.** A lead creates a small set of typed work items: inspect
   the current mobile cache; implement the offline state; review accessibility
   and recovery; test the result. Each shows its reason, evidence required,
   owner role, dependencies, execution host, file scope, and allowed tools.
   The plan is saved before any specialist starts.
3. **Make the first decision.** The user can approve the exact plan, edit its
   scope, request revision, or keep the task manual. A revision creates a new
   version with an understandable diff; the older decision remains in history.
   Approval grants only the proposed scopes, host, limits, and plan version.
4. **Execute visibly.** The lead dispatches ready work items to specialists.
   Read-only research can run concurrently. Write tasks use separate worktrees
   or explicit file ownership; merge/apply occurs only through the daemon's
   reviewed change path. Every attempt records its trigger, actor, host,
   tool use, usage, artifacts, and outcome. The user can open the transcript
   for detail without having to read it to know the run status.
5. **Escalate only real decisions.** An agent can ask for a missing choice,
   request a wider tool grant, propose a new role or host, or submit a concrete
   consequential effect. The decision desk shows the exact target and proposed
   content, a reason, alternatives, and what stops while waiting. Denial or
   revision returns a typed outcome to the agent; it does not silently retry.
6. **Review a deliverable.** A work item reaches `in_review` only with its
   artifact and verification result. The user sees changed files, diff,
   tests, limitations, and a concise reviewer report. Accepting the work item
   is separate from merging a PR or publishing. The run completes only when
   required work items and final review criteria are satisfied.
7. **Continue later.** Closing desktop or phone does not invent a completed
   state. The owning daemon retains the plan, decisions, attempts, and next
   runnable work. Reopening shows an accurate catch-up summary and asks for
   action only where needed.

This is inspired by Paperclip's strategy gate, delegation, approval lifecycle,
run history, and budget stops. Its [approval guide](https://docs.paperclip.ing/guides/day-to-day/approvals/)
supports approve, reject, and request revision; its [agent guide](https://docs.paperclip.ing/guides/org/agents/)
shows the trigger-to-run record arc. ion's proposed sequence adds IDE-specific
diff and worktree review.

## UI layout and UX specification

### Navigation and information hierarchy

Keep ion's existing native three-part workspace: project/task sidebar, main
task transcript, optional right panel for files and diffs. Add a compact
project-level **Overview** and **Decisions** entry above the task list. The
sidebar badge must count *all* pending user actions from the same query used by
the Decisions screen: model questions, tool/file approvals, plan decisions,
budget overrides, connector proposals, and recovery choices. Paperclip's
documented Inbox is a human triage view, and its command palette lists
Dashboard, Inbox, Tasks, Projects, Goals, Agents, Costs, and Activity. ion can
compress those into a project overview, decision desk, task graph, and existing
usage page rather than adding eight top-level destinations.
[Paperclip Inbox](https://docs.paperclip.ing/guides/day-to-day/issues/#the-inbox),
[command palette](https://docs.paperclip.ing/guides/day-to-day/command-palette/).

Desktop composition (structural sketch, not a new visual theme):

```text
┌──────────────┬────────────────────────────────────┬───────────────────────┐
│ Project      │ Task objective · host · status      │ Selected work item    │
│ Overview     │ Plan revision · budget · Pause/Stop │ brief / artifact      │
│ Decisions  2 │────────────────────────────────────│───────────────────────│
│ Tasks        │ Work: ready → running → in review   │ Diff / files / checks │
│  task A      │────────────────────────────────────│                       │
│  task B      │ Ordered conversation and activity  │                       │
│             │                                    │                       │
│             │ Composer / pending decision         │                       │
└──────────────┴────────────────────────────────────┴───────────────────────┘
```

The right panel remains optional. At a narrow width, Work becomes a sheet
opened from the task header and the diff remains a selectable surface, so the
conversation and active decision retain room. On phone, the structure becomes
`Needs you → active work → task feed`; a selected task opens its transcript,
and plan/artifact/diff use focused sheets. The phone should expose status and
answer/stop controls without trying to reproduce a desktop three-column IDE.

The overview reads in this order:

1. **Needs you:** count and oldest pending decision, then the top three
   actionable rows. Each row says what will happen on approval and names the
   task/host. The count includes all decision kinds, never only one approvals
   table.
2. **Work in motion:** running, waiting, blocked, in review, and recently
   finished work items, with owner and next expected event. A disconnected
   daemon is labelled *offline/stale*, not *stopped*.
3. **Goal and plan:** the current approved objective, plan version, progress
   through dependent work, and a visible Replan action.
4. **Budget and activity:** consumed/remaining tokens or estimated cost with its
   confidence/source; next automation run; recent consequential effects.

The detail view for a team task has a slim **run header** above the transcript:
objective, plan version, running/blocked status, owner daemon, workspace,
budget, and Pause/Stop. Under it, a collapsible **Work** section displays a
dependency list, not an always-visible org chart. A selected work item opens
its brief, allowed tools, attempt history, artifact, and verification in the
right panel. The transcript stays the ordered source of provider events; the
work section is a durable summary projection. For a solo task the header stays
small and Work can be hidden.

Team setup is a preview, not a staffing form. Next to the proposed plan, show
the suggested roles and a one-line reason for each, the exact tool/file grants,
execution host, model, and per-role cap. The user can remove a specialist or
choose solo execution before approval. A proposed new role or wider grant
during a run creates a decision with a configuration diff. Later, reusable
**team recipes** can prefill this preview for code review, bug triage, release
preparation, or research; installing one must show every role, tool, routine,
workspace, and external connection it would add. Paperclip's Team Catalog
previews its imports before installation, which is the transferable UX idea.
[Paperclip Team Catalog](https://docs.paperclip.ing/guides/org/team-catalog/).

The decision desk is a full-width, keyboard-operable list with filters **Now**,
**Later**, **Resolved**. Its row contains request type, task and actor, age,
host, exact effect summary, and expiry. Opening one shows proposal content,
affected files/accounts, policy/grant and diff when applicable, plus **Approve
once**, **Deny**, and **Request changes**. A common type must not be buried in
the transcript while the badge says zero. Older Paperclip issues describe
that failure for its approval routes; a later report says its dashboard count
missed task-thread confirmation cards. Treat these as cautions about keeping
one normalized decision feed and a single count source, not as claims that
the current Paperclip UI still behaves that way.
[Discoverability report](https://github.com/paperclipai/paperclip/issues/4254),
[count mismatch report](https://github.com/paperclipai/paperclip/issues/9679).

On phone, make **Needs you** the first card in the selected daemon's task feed.
The phone can review, answer, steer, pause, and stop within its existing grants;
it must show the same immutable proposal revision and fingerprint as desktop.
Offline decisions remain viewable with *stale* status, while their action
buttons are disabled. Direct daemon and personal gateway approvals currently
have distinct authorities; the UI may combine their display only if each
decision carries its source and routes the answer back to that authority.

### Interaction and visual states

- **First run:** show an example outcome, one default lead and two optional
  read-only specialists, plus a preflight. Do not require a team roster first.
- **Plan draft:** distinguish proposed from approved scope with words and
  icons, never color alone. A revised plan shows added/removed work and grants.
- **Running:** show the currently active work item and its host, an activity
  pulse, and the latest durable event. No continuously animated board is needed.
- **Waiting on user:** one focused decision card and the tasks it blocks;
  preserve the rest of the plan. Keyboard focus lands on the card only when
  the user opens it, never when a stream event arrives.
- **Waiting on dependency / host:** explain the dependency or connectivity
  issue and offer retry only if the operation is known to be safe to retry.
- **Budget warning / stop:** show the measured usage, limit, and whether the
  amount is provider-reported, estimated, or unknown. A hard stop stays paused
  until the user raises the limit or changes scope.
- **Uncertain effect:** show exactly which external or sandbox command may
  have run and the manual reconciliation step. Do not auto-replay it.
- **Complete:** show accepted deliverables and verification, with a route to
  the full trace. Completion is not inferred from a fluent model summary.

Native details: reuse the sidebar's virtualized `list()` and cached row
snapshot; build overview aggregates on the daemon/background executor and
render from an in-memory projection. Keep diff and transcript virtualized.
All actions need focus targets, `tab_index`/`tab_stop`, visible focus, and
conventional keyboard behavior. Honor reduced motion and keep status text
beside icons. This follows the repository's `AGENTS.md` performance and
accessibility requirements; it is a design constraint for implementation.

## Current ion inventory and gap

| Existing code | Reusable behavior | Work still needed |
| --- | --- | --- |
| [`nebius.rs`](../../crates/waku-core/src/driver/nebius.rs) | Native Nebius worker, tool loop, credential isolation, write-ahead journal, approval, interrupted-effect handling. | Keep provider transport here; move team graph and decisions out of a private per-provider journal. |
| [`workflow.rs`](../../crates/waku-core/src/driver/nebius/workflow.rs) | Typed researcher/reviewer contracts, read-only grant, bounded sequential child, artifact, recovery. | Durable work items across turns, user-visible plan, scheduler, concurrency after workspace isolation, new role policy. |
| [`daemon.rs`](../../crates/waku-core/src/daemon.rs) and [`protocol.rs`](../../crates/waku-protocol/src/protocol.rs) | Daemon-owned runtime, typed RPC, attached clients, ordered commands. | Typed plan/work/decision/budget/automation commands and read models; avoid client-authored whole-graph state. |
| [`server.rs`](../../crates/waku-core/src/server.rs) | Sequenced live events, replay cursors, slow-subscriber bounds. | Durable control-plane events and snapshot/cursor recovery beyond the bounded in-memory event journal. |
| [`schema.ts`](../../db/schema.ts) and [`persistence.rs`](../../crates/waku-core/src/persistence.rs) | SQLite migrations and narrow session-list rows. | Independent indexed tables for work items, attempts, decisions, artifacts, policies, schedules, and audit; no transcript blob scans for a dashboard. |
| [`sidebar.rs`](../../src/app/sidebar.rs), [`transcript_view.rs`](../../src/app/transcript_view.rs), [`composer.rs`](../../src/app/composer.rs), [`right_panel.rs`](../../src/app/right_panel.rs) | Virtualized native task list/transcript, keyboard-operable per-task approval, diff/files. | Overview, decision desk, plan/work panel and shared pending counts. |
| [`gateway-client.ts`](../../apps/mobile/src/lib/gateway-client.ts), [`gateway.tsx`](../../apps/mobile/src/app/gateway.tsx) and [`task-relay.ts`](../../packages/personal-gateway/src/task-relay.ts) | Scoped phone proposals, mirrored task events, cursor recovery, stop/steer/answer. | Team summary and one decision feed with provenance, not a second scheduler in the gateway. |
| [`usage_history.rs`](../../crates/waku-core/src/usage_history.rs) and [`usage_page.rs`](../../src/app/usage_page.rs) | Historical usage visualization and rate-based estimates for some providers. | Live Nebius per-attempt accounting and enforceable caps; current estimates are insufficient for a hard monetary stop. |
| [Milestone 7](14-phone-asr-cloud-execution.md) and [Nebius sandbox research](RESEARCH-NEBIUS-EXECUTION.md) | Selected desktop/cloud daemon can own a whole task; ConTree is researched. | Child work routing between hosts, explicit file transfer, result import, and live account validation. |

The provider-owned `ThreadGoal` currently shown by [`goal_dialog.rs`](../../src/app/goal_dialog.rs)
is documented as a Codex thread feature. A new ion project objective must have
its own name and storage so it is never mistaken for that provider feature.

## Proposed domain and daemon design

### Durable records

Use stable IDs and revisions. A suggested first schema:

| Record | Required fields and invariants |
| --- | --- |
| `objective` | Project/session scope, user text, success criteria, owner, version, status. A work item always traces to one objective. |
| `plan_revision` | Immutable proposed graph, creator, created time, scope digest, prior revision, decision ID. Exactly one revision is active for dispatch. |
| `work_item` | Parent, objective, title, reason, assignee role, dependencies, scope, expected evidence, status, active attempt. No dependency cycles; completion requires evidence. |
| `run_attempt` | Trigger, actor, host, provider/model, start/end, status, usage, error, transcript reference, idempotency key. A retry is a new attempt. |
| `artifact` | Work item, kind, content/blob reference, origin, hash, verification status. A text summary is not a substitute for a diff/test artifact. |
| `decision` | Kind, task/work item, authority, requester, exact payload/hash, revision, status, expiry, review context, answer and actor. Revisions invalidate old approval. |
| `grant` | Actor/role, tool, resource, host, effect class, expiry, policy version. A specialist cannot broaden its own grant. |
| `budget_policy`/`usage_ledger` | Scope, metric, cap, warning threshold, accounting source, reserved/observed amount. One usage event counted once. |
| `automation`/`automation_run` | Owner, trigger, timezone, next fire, concurrency and catch-up policy, target project, template revision, run ID. One trigger firing maps to at most one run. |
| `audit_event` | Monotonic sequence, actor, source, entity, action, pre/post revision, timestamp, nonsecret details. Append only. |

The scheduler is a daemon module behind a small interface: `propose_plan`,
`decide`, `dispatch_ready`, `record_attempt`, `reconcile`, and snapshot/query
methods. It owns graph transitions, policy checks, and recovery; provider
drivers receive one bounded work contract and emit events/results. This seam
has two real adapters immediately: the existing native Nebius driver and a
deterministic fake used to verify scheduling. Add other runtime adapters only
when a real use case requires them. The desktop and phone use the same typed
protocol and do not implement their own state machines.

Possible states:

```text
draft → awaiting_plan_decision → ready → running → in_review → completed
                                 ↘ blocked ────────↗
                                    ↘ cancelled
running → waiting_on_decision → ready or cancelled
running → interrupted/uncertain → reconciliation → ready or blocked
```

Only the daemon may transition states. `blocked` must name a reason and the
unblock action. `completed` requires evidence plus a verifier result or an
explicit user acceptance for work that cannot be mechanically checked.
Changing scope creates a new plan revision; existing completed items keep
their history and are revalidated if their assumptions changed. Exactly one
active attempt can own a work item. Use a transaction/lease with fencing token
for claiming, so two wakeups or clients cannot launch the same item.

### Decisions and authority

Normalize user-facing decisions from Nebius file/tool approvals, plan review,
gateway connector proposals, provider questions, budget incidents, and
uncertain-effect recovery into one **read model**. Keep execution authority
with the existing owner: the local/cloud daemon for its tools, personal gateway
for gateway-managed external calls. A decision record stores an `authority`
reference; approving in one client sends the command to that owner and awaits
its recorded acknowledgement. Never infer approval from a notification tap.
The `pending` count is a query over this read model, used by every badge,
dashboard, and phone view.

Decision kinds need different actions: a tool action can be allowed once or
denied; a plan can be approved or sent back for revision; a missing answer gets
typed options; an uncertain effect needs reconciliation, not an Allow button.
All approval payloads are immutable; if content, host, target, grant, or branch
SHA changes, mint a new decision. Consequential external writes remain
one-time and journaled before dispatch. This follows ion's current PR and
NemoClaw behavior and Paperclip's review/revision model.
[ion workflow](10-workflows-github.md),
[Paperclip approvals](https://docs.paperclip.ing/guides/day-to-day/approvals/).

The initial policy should be explicit:

| Action | Default treatment | User-facing record |
| --- | --- | --- |
| Read files already inside the granted workspace; summarize evidence | Run within the approved role scope | Work item activity and cited artifact. |
| Decompose work or delegate within an approved plan, grant, host, and cap | Run, subject to dependency and cycle validation | Plan revision and assigned work item. |
| Add a role, tool, account, path, host, or budget; change success criteria | Pause affected work for review | Configuration/scope diff with request changes. |
| Edit code or apply a remote patch | Show exact diff under existing file approval policy | One-time approval and resulting file/hash. |
| Publish, send, merge, delete, or control the computer | Require exact action approval | Target, content/effect, authority, and durable outcome. |
| Repeat an effect with uncertain outcome after interruption | Do not repeat automatically | Reconciliation decision with observed evidence. |

### Scheduling, budgets, and execution hosts

Start with **event-driven** dispatch when an approved plan has ready work, a
dependency finishes, or a decision arrives. A worker claims one ready item,
runs within step/time/tool limits, commits the attempt, and releases its lease.
Add manual repeat next. Scheduled automations then use a daemon scheduler that
persists next fire, deduplicates by trigger/time key, and records skipped,
missed, and failed fires. Choose explicit concurrency policy: `skip`, `queue`,
or `replace` only after safe cancellation is proven. A routine creates an
ordinary work item/run linked to the trigger. Paperclip documents scheduled,
manual, and webhook routine triggers and concurrency/catch-up controls; for
ion, webhooks should wait until the personal gateway has a reviewed trust
model. [Paperclip routines](https://docs.paperclip.ing/reference/api/routines/).

Budget caps apply at objective, run, and role/automation scope. Start with a
hard **token or model-call cap** because those can be counted locally. Add
monetary hard stops only with verified Nebius usage/pricing data and a reserve
before each call; show estimated currency separately until it can be enforced.
Check every model invocation and scheduled wake against the remaining cap.
On exhaustion, stop dispatch, persist the incident, and create a reviewable
decision to change the cap or scope. Paperclip's budget model warns before a
hard stop and pauses the affected scope; ion should retain the same clear
operator control without claiming an estimated cost is an exact bill.
[Paperclip budgets](https://docs.paperclip.ing/reference/api/costs/).

The first slice runs every child on the task's selected daemon and repository.
The owner-managed cloud daemon already supports an entire task from phone.
Cross-host teams come later: each child declares `local`, `owner_cloud`, or
`nebius_sandbox` as a target plus an explicit workspace manifest, network
scope, tool grant, and result contract. A cloud sandbox gets only selected
files and returns artifacts/patches; it never sees the desktop vault or direct
daemon token. Nebius Sandboxes/ConTree is a separate beta product with its own
project-scoped authorization, not something the Token Factory inference key
automatically enables. [Nebius sandbox research](RESEARCH-NEBIUS-EXECUTION.md).
NemoClaw remains the optional, approved one-off command adapter described in
[Milestone 6](13-nemoclaw-adapter.md); it does not become the scheduler.

### Event and persistence strategy

Do not put the growing graph or decision list inside `AgentSession` JSON or
the Nebius message journal. Add indexed SQLite tables and transactional
mutations in `db/schema.ts`/`waku-core::persistence`; retain a bounded event
feed for clients. Snapshots carry a graph revision and an audit cursor. A
reconnecting client fetches a snapshot, then applies later events; if its
cursor predates retention, fetch a fresh snapshot. `server.rs` currently
retains only a bounded in-memory runtime event journal, so it cannot alone
serve as an audit log or scheduler recovery source. The daemon must recover
leases and in-flight effects from disk at startup, distinguish safe retry
from uncertain external effects, and never replay an uncertain write.

Projection rules: list endpoints return compact rows and counts; detail loads
one work item or run; transcript and artifact blobs load on demand. Long
collections use paged/virtualized views. Background tasks compute aggregate
counts once per collection update, with a generation guard, then notify GPUI.
No scheduler, SQLite query, filesystem walk, subprocess, or network call is
reached from a render path.

## Build sequence and verification checkpoints

Each phase is useful on its own. Keep the existing live review checklists open
until the user supplies Nebius credentials and a real phone/cloud/sandbox run.
No phase should claim those checks have passed because its static checks pass.

### Phase 0 — Freeze the interaction contract

- Write a small fixture with one objective, three work items, one plan revision,
  one pending decision, an artifact, and a failed attempt. Use it to agree on
  desktop/phone labels and state meaning before adding views.
- Specify which actions are one-time, repeatable, or uncertain after crash.
  Confirm the first team roles and their exact tool grants.
- Record initial success criteria: user finds every pending decision from the
  overview and phone; every work item has a reason; no background run occurs
  without a trigger and budget; the final diff/verification is reviewable.

### Phase 1 — Durable work graph and read model

- Add protocol types and SQLite migrations for objective, immutable plan
  revisions, work items, attempts, artifacts, decisions, and audit events.
- Implement daemon claim/transition/reconciliation logic with dependency-cycle
  validation and fencing. Expose compact snapshot/query commands and events.
- Adapt current `delegate_specialist` to create a first-class child record and
  reuse its bounded read-only execution. Keep old Nebius journals resumable.
- Verify restart after plan save, child start, child completion, and interrupted
  effect; verify two clients cannot claim or decide the same revision twice.

### Phase 2 — Plan review and native task UX

- Add New task preflight, proposed plan version, revision comparison, and
  approve/request-changes controls. Keep ordinary solo task creation simple.
- Add Overview, task Work section, run header, and artifact/diff drill-down
  using cached projections and existing virtualized list infrastructure.
- Verify keyboard reachability and visible focus for all controls, reduced
  motion, light/dark legibility, narrow desktop window layout, and thousands
  of history rows without per-frame I/O. The repository forbids unsolicited
  visual tests; request visual validation when implementing visible changes.

### Phase 3 — One decision desk

- Normalize existing pending per-task permissions and gateway proposals into
  a source-labelled, daemon-queryable decision feed. Preserve the original
  owners and use exact payload hashes, status revisions, expiry, and idempotent
  responses.
- Use the same feed for desktop badge, overview, full desk, and phone count.
  An action opens the exact approval surface already owned by its runtime.
- Verify a pending file edit, provider question, PR, and gateway action each
  appear once; a resolved/expired decision disappears from Now everywhere;
  a disconnected owner cannot be approved from stale mobile state.

### Phase 4 — Bounded automation and budgets

- Add per-run token/call cap and actual Nebius usage ledger. Block new model
  calls when the cap is exhausted; keep estimates labelled as estimates.
- Add manual repeat automation, then scheduled triggers with timezone,
  next-run preview, missed-run and overlap policy. Each firing creates a
  visible ordinary task/run with a dedupe key.
- Verify restart across a scheduled fire, DST boundary, duplicate trigger,
  daemon offline period, budget threshold, and cancellation. The user can
  pause a schedule without deleting its history.

### Phase 5 — More capable teams and execution targets

- Add explicit write-capable roles only after separate worktrees, file-scope
  claims, reviewed patch application, and deterministic conflict handling.
- Add one remote read-only specialist behind a host adapter with explicit file
  manifest and returned artifact. Nebius sandbox integration needs actual
  account entitlement and a live owner review; keep a local fake for CI.
- Prove cancellation, timeout, duplicate dispatch, reconnect, artifact hash,
  and uncertain remote effects before allowing parallel writes or automatic
  task routing.

## Release boundary and open decisions

The first shippable Paperclip-like slice is: **one project objective, one lead,
two read-only specialist roles, a user-approved plan, durable work items and
attempts, one unified decision desk, a bounded token budget, and a final
reviewable artifact/diff**. It can run on the existing selected daemon.
Scheduling, write-capable parallel specialists, multiple hosts, and reusable
team templates follow only after that slice is stable.

Before implementing, settle these product choices in the first fixture:

1. Whether team mode defaults on for every Nebius task or is suggested only
   when a task needs multiple specialties. Recommendation: suggest, with a
   one-click solo path.
2. Which plan changes need user approval. Recommendation: new host, tool,
   budget, external account, or outcome scope always needs a new decision;
   wording-only task decomposition within approved scope may proceed with a
   visible revision.
3. Whether automated runs can continue when no client is connected.
   Recommendation: yes on the owning daemon within existing grants and cap,
   with notifications only for actionable decisions, completion, or failure.
4. Whether the project objective is permanent or task-scoped. Recommendation:
   start task-scoped; add a reusable project objective after the work graph and
   dashboard prove useful.

The current research is based on published source/docs and repo inspection;
there is no claim that the proposed ion experience has been built or visually
validated. The [Paperclip research note](RESEARCH-PAPERCLIP.md) tracks where
its documented UX and current implementation may differ.
