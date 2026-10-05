# Agent teams: implementation proposal

Prepared 2026-10-05. This proposes the next Teams sprint; it does not claim
that arbitrary team execution is already implemented. The user confirmed
automatic delegation within configured limits. At agent creation the user
chooses Local or Nebius sandbox; there is no automatic environment mode.
Nebius agents fall back to a compatible local machine when sandbox execution
cannot start or a known failure permits a safe retry.
Each agent also has a user-selected harness; teams may mix harnesses. Harness
selection is independent of environment and remains unchanged on local fallback.
No app code changes accompany this plan.

## What exists and what changes

The native app saves reusable agent profiles and multiple teams, validates one
lead and valid reporting parents, and prepares a task from the team's
instructions. That preparation currently combines all members into one prompt.
The native Nebius runtime separately offers sequential researcher/reviewer
delegates with read-only file tools and its own restart journal.

The next implementation gives every assigned member a real, observable child
task. The daemon coordinates the team independently of the selected lead's
harness. Existing solo tasks and saved definitions remain usable.

Verified starting points:

- [Profile/team records](../../crates/waku-protocol/src/control.rs).
- [Definition store and document collection](../../crates/waku-core/src/control.rs).
- [Current team task preparation](../../src/app/workspace_control.rs).
- [Existing specialist execution](../../crates/waku-core/src/driver/nebius/workflow.rs).
- [Harness dispatch and controls](../../crates/waku-core/src/driver/mod.rs).
- [Daemon lifecycle](../../crates/waku-core/src/daemon.rs).
- [Existing isolated Git worktrees](../../crates/waku-core/src/worktree.rs).

## User experience

1. Create agents with names, expertise, instructions, expected output, harness,
   model, permitted resources, and an environment selector with exactly two
   choices: Local or Nebius sandbox. For Nebius, show that local fallback is
   enabled and check local readiness as well. An agent is a saved
   profile; it uses compute when assigned work, rather than continuously running
   an idle model conversation.
   The creation form has separate **Harness**, **Model** (where supported),
   and **Environment: Local / Nebius sandbox** controls. Every member, including
   the lead, uses its own saved choice; there is no team-wide forced harness.
2. Assemble several teams. Each has one lead; members report to another member.
   An agent may join multiple teams with different reporting lines. Shared
   agent concurrency limits apply across those teams.
3. Choose a project and a team in New task, then describe the outcome. The task
   records the selected team/profile versions, acceptance criteria, permitted
   selected environments, permitted local fallback, access, budget, and
   autonomy policy. The confirmed default lets the
   lead delegate automatically within those limits.
4. The lead breaks the outcome into assignments. A manager delegates to direct
   reports; those reports may delegate to their own reports. The daemon checks
   each assignment against the hierarchy and permitted scope.
5. Specialists return documents or patches, evidence, checks performed, and
   blockers. Review findings create bounded rework assignments. The lead
   assembles the result and reports completion against the acceptance criteria.
6. Important decisions appear in Decisions; progress and completion appear in
   Dashboard and Inbox. Opening an assignment shows its actual task transcript.
   Stop team stops new dispatch and requests cancellation of active descendants.

Example: a lead assigns investigation to a researcher and implementation to an
engineering manager. The manager assigns the fix to an engineer and validation
to a reviewer. Findings return through the manager to the lead. The final
report and patch remain connected to the original project task. Role names
are examples, not a hardcoded list of supported agent types.

The Teams screen will offer a team selector, hierarchy chart and accessible
roster, agent configuration drawer, and live assignments. Reporting hierarchy
and assignment dependencies are distinct views: a reporting line says who can
delegate, while a dependency says which output must exist before work starts.

## Daemon-owned coordination module

Introduce a Teams module under `crates/waku-core/src/teams/`. Its interface
hides persistence, scheduling, grant evaluation, context assembly, and restart
reconciliation. Proposed commands are `StartTeamTask`, `SubmitDelegation`,
`ReportAssignment`, and `RespondTeamDecision`; querying returns a compact
`GetTeamRunSnapshot`. Existing stop/steer controls are extended to team runs.
The protocol carries typed records; desktop and mobile consume the same state.

The daemon owns the work queue. A lead model proposes assignments and receives
results; it does not own the scheduler or need to stay running while children
work. Ready work is claimed transactionally with a lease and fencing token.
Paused, stopped, or superseded runs cannot dispatch through an old lease.

Persist immutable team/profile revisions, team runs, work items, dependencies,
attempts, handoffs, decisions, scoped grants, and audit events in indexed tables
alongside the existing control definitions. Mutable definition records remain
compatible; a run pins a published snapshot. A profile edit affects later runs;
applying it to a current run requires a visible revision change.

The control DB and task DB are separate today. Materializing a child therefore
needs a durable outbox and preallocated task UUID, followed by create-if-absent
and reconciliation. A crash between databases must not create a second child.
Do not assume an atomic transaction spans both existing stores.

Each assignment records its superior, assignee/profile revision, root task,
  team revision, objective, dependencies, output contract, grants, selected
  environment, actual host, fallback reason,
harness/model, attempt history, budget, and result. Status is explicit:

`queued → running → waiting on decision → in review → completed`

Alternative states include `blocked`, `failed`, `interrupted`, `canceling`,
`canceled`, and `uncertain`. The reason and authoritative host are visible.
A model saying "done" is not sufficient: required outputs and validation must
be present. Uncertain external effects require reconciliation before retry.

## Delegation and context

- Expose structured team commands through each capable harness integration.
  Native Nebius tools are the first adapter; add existing CLI harnesses through
  supported integration mechanisms after inspecting their actual contracts.
- Derive the caller's agent/run identity from the integration session, not an
  identity supplied in model arguments. Scope its invocation capability to
  that assignment and revoke it on stop or expiry.
- Validate membership, direct reporting relationship, acyclic dependencies,
  input/output contract, available resources, quotas, and host readiness before
  accepting work. No delegation outside the selected team in the first sprint.
- A child receives its own profile instructions, assignment, relevant files,
  selected prior documents, and bounded handoff summaries. Other agents' full
  transcripts and credential material are not copied into every prompt.
- Completing a child emits one durable result event. A waiting parent receives
  the result through its adapter, or is resumed once when that adapter cannot
  accept a message during an active turn. Correlation IDs deduplicate delivery.
- Limit delegation depth, active work, attempts, wall time, and model calls.
  Bounded rework prevents an engineer/reviewer loop from consuming indefinitely.

## Harnesses and resource access

Profiles may select Nebius, Claude, Codex, OpenCode, Antigravity, and the other
existing entries. Lead and specialists may use different harnesses. Selection
requires installation/authentication on the actual chosen host.

The harness picker lists the existing catalog and shows readiness for the
selected environment. Model choices use that harness's actual discovery or
supported configuration. Pin harness/model in the agent revision and assignment;
delegation must launch those selections rather than inherit the lead's harness.
Example: a Nebius lead, Codex engineer and Claude reviewer can share one team.
On sandbox failure, local fallback uses the same harness/model. If it is missing
or unsupported locally, show a setup blocker; never substitute another harness
or model without the user's configuration change.

An adapter declares its real capabilities: task lifecycle, delegation bridge,
permission handling, enforced file/tool/app scope, steering, and recovery.
Initial work proves the coordinator with native Nebius, then one mixed-harness
vertical slice with installed CLIs. Other harnesses become team-ready only
after their required capabilities are implemented and checked. Solo availability
is not a claim of team readiness.

Effective access is the intersection of the agent profile, team, project,
parent assignment, and task authorization. Connected Composio accounts need
exact account/action grants; a connected account alone is not execution access.
Secrets remain in the vault or credential-owning connector module.

Prompt text and a Git worktree do not enforce machine access. Native tools must
check grants at execution. External CLIs need enforced permissions or isolation
that satisfies the selected scope. If a harness cannot enforce a requested
restriction, show the missing capability and block that configuration; do not
silently launch it with broader access. A team cannot gain a denied capability
by delegating to another agent. Existing computer-use distribution constraints
remain in force; team execution does not itself unlock computer control.

## Workspace and host behavior

Begin with read-only parallel work and serialized writers. Reuse the existing
worktree module for isolated coding assignments once integration is proven.
Choose an explicit base commit or reviewed working-tree snapshot so existing
uncommitted changes are handled deliberately. Worktrees isolate edits, not
credentials or filesystem permissions. For non-Git projects, serialize writers
until a suitable isolation strategy exists.

A coordinator collects patches, applies them in an integration workspace,
checks conflicts, runs required validation, and returns a reviewable final
diff. A conflict creates a repair assignment; conflicting patches are never
silently overwritten. Publishing or merging follows the task's approval policy.

Use one owner daemon per team run. Full remote agents run through an explicitly
registered owner-cloud daemon or a sandbox adapter capable of launching the
chosen harness. The present ConTree command tool is not yet a full remote
agent host. Dispatch requires host readiness, workspace availability, permitted
egress, credentials on that host, and a cost cap. File transfer is explicit,
bounded, recorded, and excludes secrets; no implicit full project upload.

The user's final correction supersedes the earlier automatic/load-based
placement suggestions. Environment is selected per agent, not inferred from
the assignment's workload:

- **Local:** execute the agent's assignments on the user's machine.
- **Nebius sandbox:** execute on the configured Nebius sandbox, using the
  chosen harness when that target supports it. Local fallback is part of this
  profile's execution policy. There is no Auto option and no load-based routing.
- **Profile fields:** persist `execution_environment` as `Local` or
  `NebiusSandbox`, plus the target reference and permitted fallback grants.
  Pin these in each run revision. Environment changes affect subsequent runs
  unless explicitly applied to a current run at a safe boundary.
- **Readiness:** check compatible harness, authentication, workspace/inputs,
  app grants, required CPU/RAM/GPU and budget on the selected target. These
  checks validate the user's choice; they do not select a different environment.
  A cloud model API is separate from environment: a locally executing agent
  may still call Nebius or another remote model provider.
- **Fallback before execution:** unavailable sandbox, startup failure or a
  known failure before effects can cause local dispatch, provided local tools,
  inputs, resource capacity and grants are sufficient. Record the remote
  attempt and fallback reason; show Local (fallback) on the assignment. Do not
  repeatedly retry the sandbox and loop back between environments.
- **Fallback after interruption:** loss of connection is not proof the remote
  job stopped. Confirm remote state/cancellation and retrieve its outputs or
  checkpoint before retrying locally. Unknown external effects remain uncertain
  and require reconciliation; do not execute them again on the local machine.
  Resume at a confirmed checkpoint when supported, otherwise start a new
  attempt only when the previous outcome is known and retry is safe.
- **Compatible scope:** fallback does not widen file/app/network permissions
  or copy credentials between hosts. An exact approval bound to a remote host
  cannot authorize a different local effect. If local fallback cannot satisfy
  the selected scope, tools or hardware, show a concrete blocker.
- **Consequential effects:** deployment, publishing and other effects retain
  the applicable task approval policy in both environments. Selecting a Nebius
  agent does not itself provision a GPU or unlock computer-use capabilities.

A disconnected owner makes its projection stale. Controls remain disabled
until refreshed; another client does not become a second scheduler. Cancel
requests propagate through child task controls. A remote write with an unknown
outcome remains uncertain until the host can confirm its state.

## Approval and automation policy

User-confirmed default: submitting a team task lets
the lead plan, delegate, run permitted work, and review results within the
configured access and budget. The plan remains visible. An optional initial
plan gate is available for users who want it.

Scope/access expansion, a new execution target, a budget increase, consequential
external effects, and uncertain-outcome reconciliation create explicit user
decisions unless already covered by an applicable user authorization. Approval
binds the exact action and revision; changed arguments invalidate it. Ordinary
handoffs within the authorized task do not need a separate approval each time.

User-confirmed environment policy: choose Local or Nebius sandbox when creating
the agent. Use that choice for its assignments. If sandbox execution fails,
fall back to a compatible, authorized local machine when the previous outcome
permits a safe retry. No automatic environment selection is offered. Provider,
model, grants and caps remain pinned; fallback is not a provider substitution.

## Implementation sequence and checkpoints

1. **Durable foundation:** migrate/pin profile and team revisions; add work
   graph, typed commands, grants, attempts, audit cursor, and task outbox.
   Check hierarchy/dependency cycles, stale edits, shared concurrency and
   duplicate child creation across a crash.
2. **Native team execution:** replace prompt-only team start with daemon-owned
   assignments, general Nebius delegation, bounded handoffs, required outputs,
   reviews, stop and restart recovery. Check three levels of reporting and
   invalid/out-of-team calls; retain existing specialist/solo task behavior.
3. **Visible team activity:** hierarchy/roster, child task links, reasons for
   blocking, result documents, consistent Dashboard/Decisions/Inbox projection.
   GPUI renders cached snapshots; background reads use generation guards,
   and long lists are virtualized. Keyboard operation and legibility are gates.
4. **Mixed harnesses and isolated coding:** one validated external harness
   adapter, then additional adapters; integration workspace and patch review.
   Check unsupported capabilities, failed authentication, conflicts, cancel,
   and duplicate result delivery. Verify distinct lead/specialist harness/model
   selections persist and launch correctly, and local fallback preserves them.
   Use mocked adapters before live CLI tests.
5. **Registered cloud targets:** execute Nebius-selected agents in the sandbox
   with explicit transfer and confirmed remote lifecycle; add local fallback.
   Check selected-environment persistence, unavailable sandbox, compatible
   local fallback, interrupted remote jobs, stale decisions, uncertain effects,
   quotas and cancellation. Verify there is no Auto environment choice.

The first useful release proves a lead → manager → specialist chain, review,
one document, task links, a user decision, and recovery on the local native
runtime. Mixed harness and selected sandbox execution follow as explicit increments. Routine
scheduling and dynamic workflow execution reuse this module in later sprints.

No live provider, cloud, mobile, or visual acceptance is claimed by this plan.

## Confirmed user choices

1. **Autonomy:** delegate automatically inside configured limits. The user does
   not need to approve each child task or the initial plan by default.
2. **Execution environment:** the user chooses Local or Nebius sandbox at agent
   creation. Remove automatic environment mode. Sandbox failure falls back to
   local when compatible and safe; uncertain remote effects are reconciled first.
3. **Harness:** the user chooses a harness and supported model for each agent,
   including the lead. Different members may use different harnesses; fallback
   changes the execution environment, not the harness or model.

No preference question remains from this planning turn. Resource availability
and live acceptance remain implementation/setup gates rather than reasons to
change the confirmed behavior.
