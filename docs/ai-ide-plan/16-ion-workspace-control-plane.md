# ion workspace control plane: teams, decisions, routines, and workflows

Status: **architecture proposal; native screens/definition storage partially implemented**.
Current implementation and remaining work: [native review checklist](17-native-workspace-review.md).
Prepared 2026-09-30 from the
user's eight-area brief and four supplied Paperclip screenshots. This chapter
extends the [task-level Paperclip plan](15-paperclip-inspired-ion-experience.md).
Where that plan compressed navigation into Overview and Decisions, this newer
explicit brief takes precedence: the eight requested areas are first-class
destinations. The [ion product decisions](README.md) still govern BYOK,
credentials, execution ownership, and milestone review. The screenshots are
interaction references, not a request to copy Paperclip's styling or its
fictional company hierarchy.

## Product decision

ion becomes a personal control plane for work across projects. A user defines
reusable agents with expertise, instructions, and narrow resource grants;
assembles them into teams; gives a team an outcome; reviews its plan; and sees
the resulting tasks, decisions, artifacts, notifications, and resource usage
in one place. The team can delegate within an approved plan. Wider access,
external effects, and final acceptance return to the user.

**Teams is the core feature.** Dashboard, Decisions, Inbox, Routines,
Pipelines, Artifacts, and Usage are views and triggers over the same durable
task/run records. They must not become eight separate schedulers or stores.

Paperclip supplies useful precedents: a control layer above agent runtimes,
agent roles and assignments, issue-backed work, human approvals, routines that
produce auditable execution, budgets, and artifact/run history. ion maps those
patterns to a native IDE and its daemon-owned local or owner-cloud workspaces.
Paperclip's current first-party behavior and version caveats are recorded in
[the product research](RESEARCH-PAPERCLIP.md) and
[architecture research](RESEARCH-PAPERCLIP-ARCHITECTURE.md); the four user
screenshots establish the desired page concepts here. The later organization
screenshot and user correction make **multiple teams with editable reporting
hierarchies** a required Teams interaction. A CEO persona and automatic
"hiring" remain optional metaphors.
Paperclip's documented implementation uses a database-backed control layer
with runtime adapters and task checkout; ion should reuse that ownership
pattern while retaining its existing native daemon, SQLite, and typed wire
protocol rather than copying Paperclip's web/server stack.
[Paperclip architecture](https://github.com/paperclipai/paperclip/blob/master/docs/start/architecture.md).

## Vocabulary and scope

| Term | Meaning in ion | Relationship |
| --- | --- | --- |
| Portfolio | The explicit set of projects and owner daemons registered by this user in ion. | Bounds every "All projects" query; it is not a new shared account or tenant. |
| Project | Repository or workspace and its owning daemon/host. | Contains tasks, pipelines, routines, and artifacts. |
| Task | One user-visible outcome with a conversation, status, and acceptance criteria. | May be created manually, by a routine, or by a pipeline. |
| Work item | A bounded delegated step with reason, owner, dependencies, grants, and required evidence. | Belongs to one task or pipeline run. |
| Agent | A versioned, reusable profile: expertise, instructions, model/host preference, limits, and proposed capabilities. | Runs only through the effective grants of a task. |
| Team | A versioned roster, lead, team-scoped reporting tree, delegation rules, and default limits. | An agent may join multiple teams; a task pins the team and hierarchy version it approved. |
| Run attempt | One actual invocation of an agent/harness on one host. | A retry creates another attempt; it never rewrites history. |
| Routine | A schedule or manual repeat definition. | A firing creates a normal task or pipeline run. |
| Workflow | A prompt-generated, user-edited, project-specific sequence/graph of typed stages and gates. | Internally represented by a versioned pipeline definition; a run pins its approved version. |
| Decision | One immutable question or proposed effect for the user. | May block a work item, stage, task, or budget scope. |
| Inbox item | A notification or attention event with read/acknowledge state. | May link to a decision; reading it never approves the decision. |
| Artifact | A durable document such as HTML, PDF, or Markdown, with origin, hash, format, and review state. | May be produced by any work item or workflow stage; workspace file references are a separate type. |

The initial portfolio is all projects on one selected ion daemon. Later, the
desktop can aggregate only the additional owner daemons the user has
explicitly registered; each row must carry its owner and last synchronization
time. An offline owner is
**stale**, never silently shown as idle or finished. The phone first shows the
selected daemon's portfolio; a truly cross-host phone view waits for a
personal gateway projection and routes actions to the authoritative daemon.

## Navigation and screen contract

Use ion's native sidebar and existing task transcript/file panel. Keep the
global control pages separate from the active project's coding workspace:

```text
WORKSPACE                     WORK             AGENTS             ORGANIZATION
New Task                      Tasks            Pinned agents      Teams
Search                        Routines         See all agents     Harnesses
Dashboard                     Workflows                           Usage
Inbox [unread count]          Artifacts
Decisions [action count]      Projects                           Settings
```

The sidebar follows the supplied Paperclip grouping and scan order while using
ion's native density, icons, and task transcript. The workspace selector is at
the top; the agent list gives quick access to profiles. **Teams** opens the
multi-team hierarchy, rather than treating every agent as one organization.
Do not add company goals, cases, or conference rooms without a separate user
requirement. A project filter sits in each global page's header and defaults
to **All projects**. Search covers task, agent, document, and run titles with
source labels.

| Destination | Primary question | First viewport / main action | Detail route |
| --- | --- | --- | --- |
| **Dashboard** | What is happening now across projects and agents? | Needs-you strip, active/blocked tasks, live agent cards, project health; choose an item to inspect. | Task, agent, project, or decision. |
| **Decisions** | What must I choose? | Prioritized pending list with project, actor, affected scope, age, expiry, and exact effect preview; approve/deny/revise where appropriate. | Immutable proposal and source context. |
| **Inbox** | What changed while I was away? | Unread/Recent/Issues/Completed filters with grouped notifications; mark read or open source. | Task, run, artifact, or decision. |
| **Teams** | Who reports to whom, and who may delegate? | Multiple team selector; hierarchy chart by default; accessible roster; create/edit agent and team reporting lines. | Agent profile, team version, and delegation flow. |
| **Harnesses** | Which execution runtimes can an agent use? | Full supported catalog, runtime kind, and live setup/readiness state; link to agent setup. | Provider settings and model discovery. |
| **Routines** | What repeats, when, and under whose limits? | Active/Paused/Draft tabs, next fire, last outcome, owner, project, run history; create routine. | Trigger and delivery editor. |
| **Workflows** | How should this project repeat a process across apps? | Prompt composer, connected-app picker, generated draft, editable typed stages, and recent runs. | Stage editor and run detail. |
| **Artifacts** | Which documents did work produce? | Cross-project HTML/PDF/Markdown library with format, project, task, producer, date, and review state. | Native document preview and provenance. |
| **Usage** | Which agents, models, and harnesses consumed resources? | Tokens, estimated/verified cost, model calls, runtime/compute and budget incidents; filter by period/project/agent/model/host. | Run-level usage and source ledger. |

**Name choice:** use **Usage** in the sidebar and **Usage & costs** as the
page title. "Timeline" implies chronological work handoffs; reserve an
**Activity timeline** inside a task, agent, or pipeline run for those events.
The existing Settings → Usage page can become a source of historical data, but
its current provider-rate estimates are not yet an enforceable cross-agent
ledger.

### Visual direction for the control plane

Use a light, Nebius-green glass aesthetic as the default across these pages.
A restrained lime-to-mint glow belongs behind the workspace; the sidebar,
header, cards, and drawers use translucent neutral surfaces so the task data
remains the focal point. Use brighter green on selected navigation, primary
actions, and small emphasis areas. Do not place body text directly on the
gradient or use pale green for text on white. Preserve red and amber for their
error and warning meanings.

The native implementation should test primary, secondary, placeholder, status,
and document-preview text against the lightest and darkest backgrounds each can
actually sit on: at least 4.5:1 for ordinary text and 3:1 for large text and
control boundaries. Keep focus outlines visible. Support dark mode and the
system's reduced-transparency setting. Blur large fixed panels selectively;
avoid per-row blur in virtualized task lists and transcripts. The
[HTML prototype](PROTOTYPE-ion-workspace.html) records this visual direction,
not production theme code.

### Dashboard

The first supplied screenshot shows live agent cards followed by totals and
charts. Keep that scan pattern, but place **Needs you** before general charts
so a pending action is not buried. The first row answers: pending decisions,
agents running, tasks blocked, active projects, and spend/usage confidence.
Below it, show a compact horizontally scrollable set of live agents with
current task, project, host, and elapsed state. The main list shows task and
project status with reasons for blocked/stale states. Charts become a lower
section with period selection and click-through, not the sole source of status.
The same daemon projection supplies cards, counts, and detail; no per-card
polling or transcript scans during render.

### Decisions versus Inbox

The second screenshot demonstrates a rich decision with evidence, recommended
action, alternatives, and due date. ion should show the **exact effect** above
the agent's explanation: file diff, PR body and branch SHAs, account target,
host/tool grant, or budget change. Show what is blocked and what approval
would permit. A model recommendation can be requested as a separate bounded
read-only run; it cannot decide on behalf of the user. Support **Now / Later /
Resolved**, snooze with expiry, and filters by project/type/age. Different
decision kinds get different controls: plan approval, typed answer, exact
action approval, revision request, or uncertain-effect reconciliation.

Inbox records completion, failure, blocked work, newly produced artifacts,
routine/pipeline events, scoped connected-app events, and decisions that need
attention. It deduplicates
repeated event delivery, groups noise from the same run, and supports unread,
acknowledged, and muted categories. A linked decision remains pending when its
Inbox item is marked read. The Decisions badge counts unresolved actionable
decisions; the Inbox badge counts unread notifications. Both derive from their
own canonical queries, so they cannot drift because of ad hoc UI flags.

### Teams: the main creation and delegation flow

The supplied organization screenshot makes reporting lines the primary Teams
view. Show a selector for **multiple teams**, each with its own lead and
editable parent for every member. Default to the hierarchy chart, with a
keyboard-operable roster showing the same parent and status information. A
chart node opens the agent detail; the chart is never the only way to operate
the team. An agent may belong to multiple teams with different reporting lines.
Team setup is four short steps:

1. **Define expertise:** agent name, purpose, expertise tags, success examples,
   instructions, and expected output. The instruction editor distinguishes
   system policy, user-authored profile, and task context; untrusted project
   text cannot edit an agent profile.
2. **Choose execution:** model/provider preference, local/owner-cloud host,
   harness, concurrency, per-run budget, and fallback behavior. A missing
   Nebius key or unavailable host yields a draft, not a broken active agent.
   The chooser lists all 13 entries in `ProviderKind::ALL`: Amp, Claude Code,
   Codex CLI, Cursor CLI, DeepSeek Harness, Fx, OpenCode, OpenCode 2, Grok
   Build, Kimi Code, Nebius Nemotron, Oh My Pi, and Pi. Nebius is the native
   ion runtime with Token Factory credentials; the other entries use their
   respective external integration setup. Selection never implies the runtime
   is installed, authenticated, or available on the chosen host. The live UI must show
   per-host readiness and model discovery or a clear setup action. A newly
   selected but unverified harness keeps the profile in Draft.
3. **Grant resources:** project paths and read/write scope, tools, skills,
   connected apps/accounts, network destinations, and expiry. Show an
   intelligible capability summary. "Can propose PR" is distinct from
   "may publish without review"; consequential effects retain exact-action
   approvals. Credentials stay in vault/gateway and are never copied to the
   model or specialist context.
4. **Assemble the team:** choose one lead, specialists, each member's parent,
   shared objective, default decision gates, and team budget. Reject cycles,
   missing parents, and a member reporting outside the team. Preview effective
   grants and test with a small read-only task. Saving creates a versioned
   reusable team; a task pins the approved hierarchy version.

From **New task**, the user can choose Solo or a Team, select a project, state
an outcome and acceptance criteria, and preview the proposed plan. The lead
may assign work to agents below them in the selected team's hierarchy, only
within the approved plan, project, host, effective grant, and budget. A
delegated task records the superior, assignee, team version, objective, and
handoff. Child grants are an **intersection** of the
agent profile, team policy, project policy, and task approval. Agents cannot
broaden access by editing instructions or delegating to a stronger role. A
new agent, app connection, path, host, or budget during a run creates a
reviewable configuration diff in Decisions.

Team activity is visible as `queued → running → waiting on decision → in
review → complete`, with `blocked`, `failed`, `interrupted`, and `uncertain`
states that name the reason. The task transcript is the detailed record; the
team view shows the work graph and handoffs. Read-only agents can run in
parallel. Writers start sequentially until isolated worktrees, file claims,
patch review, and conflict handling are reliable.

### Routines and Workflows

The fourth screenshot gets the entry point right: title and work instructions
first, delivery controls second. A routine draft is harmless and **paused**
until its trigger and project/owner are valid. The form shows the next three
fire times in the selected timezone, agent/team, task or workflow definition,
host, cap, overlap policy, missed-run policy, and notifications. Start with
manual repeat and calendar schedule; webhook/event triggers require the
personal gateway trust and replay design. Every fire creates a normal visible
task or workflow run with a unique trigger key; pausing preserves history.

The user creates a workflow by describing the process in a prompt and choosing
a project and connected apps/resources. The model proposes a **draft**, not an
executable script: a named list of typed stages, dependencies, agents/teams,
app actions, artifact requirements, and approval gates. The user can add,
remove, reorder, and edit stages before saving a version. Missing app
connections remain explicit setup steps; connecting an app scopes an account,
actions, and expiry separately from the workflow prompt. Credentials remain
in the vault/gateway.

The saved workflow is internally a project-scoped, versioned pipeline state
machine. Each stage has typed input/document requirements, an assigned agent
or team, host/tool/app grants, retry rule, timeout, budget, and success/failure
transition. The daemon validates the proposed graph, app scopes, and grant
intersection before activation. Gates can be automated validation or a user
decision. A run pins its definition revision, displays the current stage and
documents satisfying prior gates, and resumes after restart. A routine may
trigger a workflow; the workflow does not own its clock. Typed stage kinds
(`agent_task`, `validation`, `human_gate`, `approved_action`) prevent arbitrary
prompt text from becoming executable code or a hidden transition. Publishing,
deploying, merging, and other consequential app actions require an exact effect
preview and a one-time approval.
Paperclip's current [pipeline tutorial](https://github.com/paperclipai/paperclip/blob/master/docs/pipelines-tutorial.md)
documents definitions, case instances, stage transitions, review gates,
version conflicts, routine triggers, and issue links. That proves a useful
state model, not that every Paperclip build exposes a finished visual editor.
ion should ship a linear typed editor before branching graphs. There are no
fixed release/deploy/review workflow types; those are examples of prompts a
user may provide.

### Artifacts and Usage

Artifacts is primarily a **document library** for durable HTML, PDF, Markdown,
and similar deliverables produced by tasks, specialists, routines, and
workflows. Cards show a document preview and format; detail opens a readable
document with project/task/run, producer and profile revision, creation time,
hash, verification, and review state. Workspace file references may appear as
a separate, explicitly labelled source type: they can disappear when their
workspace is removed and cannot satisfy an archival requirement. Store durable
content once and index metadata separately. Search/filter metadata
without loading blobs; open a specific artifact to fetch content. A summary
without source evidence is labelled as such.

Usage records one ledger entry per provider/model response, harness attempt,
tool call class, and billable host resource when observed. Token counts and
prices carry a source (`provider_reported`, `local_measured`, `estimated`, or
`unknown`) plus rate/version and currency. Join each entry to agent, task,
project, model, host, routine/pipeline run, and attempt, so filters agree and
one event is never counted twice. Show BYOK provider **estimated cost** as an
estimate until a verifiable bill/usage source exists; do not call it an ion
charge. A hard token or call cap can be enforced locally first. Monetary hard
stops require a conservative reservation before each call and trustworthy
pricing. A budget incident pauses the affected work and creates a decision.

## Technical architecture

```text
GPUI desktop / paired phone
  └─ typed query + command client
       └─ owning ion daemon per project
            ├─ control store (SQLite + append-only audit)
            ├─ work scheduler + dependency claims
            ├─ decision and grant engine
            ├─ routine scheduler + pipeline runner
            ├─ artifact index + usage ledger
            └─ execution adapters
                 ├─ native Nebius coding runtime (first)
                 ├─ local tools / worktrees / connected-app gateway
                 └─ owner cloud or sandbox adapters (later, explicit)
```

The **Control Plane** is a deep daemon module behind typed commands and
queries. Its interface is small: create/revise a plan, decide, dispatch or
pause work, record an attempt/effect, and query versioned projections. It owns
state transitions, grant checks, leases, and recovery. The Nebius driver is
an execution adapter: it receives one bounded contract and returns events,
usage, and artifacts. A deterministic fake adapter can exercise the same
interface without a live key. NemoClaw and Nebius sandbox remain optional
execution adapters, never alternative sources of truth.

The current code has valuable starting seams: [`db/schema.ts`](../../db/schema.ts)
stores narrow project/session rows; [`protocol.rs`](../../crates/waku-protocol/src/protocol.rs)
already has typed daemon requests, task snapshots, and sequenced runtime
events; [`nebius/workflow.rs`](../../crates/waku-core/src/driver/nebius/workflow.rs)
has bounded sequential read-only researcher/reviewer delegation and a text
artifact; [`usage_page.rs`](../../src/app/usage_page.rs) has historical usage
UI; [`sidebar.rs`](../../src/app/sidebar.rs) virtualizes task navigation.
These are **existing capabilities**, not evidence that reusable teams,
portfolio decisions, routines, pipelines, or a cross-agent ledger already
exist. `LoadTaskState`/`SaveTaskState` and provider journals should not become
the new graph database; add indexed SQLite tables and narrow, version-checked
mutations. Keep transcript bodies and artifact blobs out of list queries.

### Durable records and invariants

| Record family | Essential fields / rule |
| --- | --- |
| `agent_profile`, `agent_profile_revision` | Name, expertise, instructions reference, model/harness preference, status, revision. Published revisions immutable. |
| `team`, `team_revision`, `team_member` | Team-scoped lead and parent per member, profile revisions, default caps. An agent may join several teams; a task pins one hierarchy revision. Every member reaches the lead without a cycle. |
| `resource_grant`, `app_connection_ref` | Actor, project/path/tool/account/host/action scope, expiry, policy revision. Store references, never secrets. Effective grant is an intersection. |
| `task`, `plan_revision`, `work_item`, `work_dependency` | Outcome, acceptance criteria, approved plan version, assigned agent/team, required evidence, status, reason. Dependency graph acyclic. |
| `run_attempt`, `execution_effect` | Actor, trigger, host/harness/model, lease/fence, input digest, start/end, outcome, effect idempotency key. Retry is a new attempt. |
| `decision`, `decision_response` | Authority, kind, exact payload digest, source revision, status/expiry, affected scope, answer/actor. Changed payload invalidates approval. |
| `notification` | Source event identity, category, recipient, unread/acknowledged state, target link. One source event yields at most one item per recipient/category. |
| `routine_definition`, `routine_fire` | Versioned trigger/timezone/template, next fire, catch-up/overlap, dedupe key, resulting task/run. A fire is claimed once. |
| `pipeline_definition`, `pipeline_stage`, `pipeline_run` | Source prompt and connected-app refs, edited stage proposal, pinned version, typed transitions, document contracts, current stage, gates. No hidden transition chosen by model text. |
| `artifact`, `artifact_link` | Document format, durable content reference/hash, producer, origin, verification/review, retention. Workspace references have separate storage status. |
| `usage_event`, `budget_policy`, `budget_incident` | Metered units, source/confidence, rate revision, allocation IDs, cap/threshold, pause reason. Unique usage identity prevents double count. |
| `audit_event` | Monotonic owner-local sequence, actor, entity, old/new revision, nonsecret effect summary. Append only. |

The scheduler claims ready work transactionally with a lease and fencing
token. Before an effect, it journals the immutable intent and checks the
current grant, decision, target revision, budget, and host. A crash during an
external write yields `uncertain` until reconciled; it never blindly replays
the write. Long-running work resumes from durable state rather than a client
window. The routine scheduler uses the same claim discipline. Pipeline stages
materialize ordinary work items and decisions, so their status and outputs
appear consistently throughout ion.

### Queries, events, and multiple hosts

- `GetPortfolioSnapshot(filters)` returns compact dashboard counts/rows,
  agent activity, pending decision count, unread count, next routine fires,
  and usage summary with a revision and `observed_at`.
- `ListTeams/Agents`, `ListDecisions`, `ListNotifications`, `ListRoutines`,
  `ListPipelines/Runs`, `ListArtifacts`, and `QueryUsage` are paged/filterable;
  detail queries load only one graph, proposal, artifact, or ledger slice.
- Mutations are typed (`PublishAgentRevision`, `ApprovePlan`, `RespondDecision`,
  `PauseRoutine`, `StartPipelineRun`, etc.) and include an expected revision and
  idempotency key. The daemon returns the committed revision or a conflict.
- A durable audit cursor supports snapshot-then-event recovery. Existing
  bounded in-memory runtime replay is still useful for live transcript
  streaming, but cannot serve as the sole control-plane history.
- Portfolio aggregation across several daemons keys identities by
  `(owner_daemon_id, entity_id)`, records each source's freshness, and routes
  commands back to the owning daemon. The personal gateway may later mirror
  safe projections for phone; it never becomes a second scheduler or silently
  executes a stale decision.

Build GPUI list rows from in-memory projections, refreshed in one background
pass with a generation counter. Virtualize long task, notification, artifact,
and ledger lists. Rendering never reaches SQLite, a subprocess, the network,
or vault. All mouse actions have keyboard focus/operation and visible focus;
status uses text/icon as well as color, and reduced motion is honored.

## End-to-end examples

### A user asks a team to fix a bug

1. In **Teams**, the user creates a Lead, Researcher, Engineer, and Reviewer,
   chooses a repository and scoped file/app access, and saves Team V1.
2. In **New task**, they choose Team V1 and state an outcome. The daemon checks
   key/host/workspace availability. The lead proposes work items, owners,
   dependencies, grants, and a budget. The user reviews **Plan V1** in
   Decisions; approving it records the exact revision.
3. The scheduler runs read-only research, then an isolated Engineer attempt.
   A proposed file edit appears as a one-time exact diff decision. The Inbox
   also records that attention is needed; reading it does not approve.
4. The Reviewer attaches evidence and tests. The final artifact appears in
   Artifacts, the attempt's token/model/host measurements appear in Usage,
   and Dashboard shows the task ready for user review.
5. Acceptance records completion. A later plan/grant change produces V2 and
   another decision; prior history remains inspectable.

### A prompt-generated release workflow runs every Friday

1. The project owner prompts ion to prepare notes, build/check, request sign-off,
   publish approved assets through a connected app, and verify. ion proposes
   stages. The user edits their agents, document gates, app scopes, host, caps,
   and transitions before saving Workflow V1.
2. A Routine schedules Workflow V1 and previews its next fires. On Friday,
   one fire creates a version-pinned run and ordinary child tasks. Dashboard shows
   the run; Inbox reports failures or completion.
3. The release gate appears in Decisions with notes, checks, target, exact
   publish payload, and rollback path. Publishing is not inferred from a
   model's success statement.
4. Artifacts holds the HTML release notes, PDF build report, and other durable
   documents; Usage allocates model/harness/host consumption to the workflow
   and agents.
   A daemon restart or duplicate timer cannot create a second run for the
   same trigger key.

## Build sequence and review gates

This is the proposed next program of work, **not a claim of completion**.
Earlier Milestones 2–7 still have user-owned live checks (Nebius key, phone,
gateway, cloud host, and sandbox as applicable). Do not mark them complete
because this control-plane design is approved or its unit checks pass.

| Phase | Deliverable | Verification checkpoint |
| --- | --- | --- |
| **0. Interaction contract** | One realistic fixture covering two projects, multiple hierarchical teams, one plan, two decisions, a routine, a prompt-generated workflow, HTML/PDF documents, notification, and usage. Resolve labels and state transitions against the screenshots. | Every count links to the same underlying record; reporting lines, planned vs approved grants, and fresh vs stale owner are unmistakable. |
| **1. Durable control core** | Versioned tables, typed commands, audit cursor, leases, grant evaluator, budget ledger, and read projections. Adapt existing Nebius child journal without breaking older tasks. | Restart and two-client tests cover claims, decisions, duplicate effects, and snapshot/cursor recovery. |
| **2. Teams vertical slice** | Create/edit multiple teams, hierarchy revisions, agent profiles, and effective access; delegate from a superior to a lower member, approve a plan, and collect a document. | Cycles and out-of-team managers are rejected; a child cannot gain a parent-denied capability; instructions cannot override grants. |
| **3. Attention surfaces** | Dashboard, global Decisions, Inbox, agent activity, project filters, and source-labelled freshness. | Each pending action appears exactly once in Decisions, its badge and Dashboard count match, and Inbox read state is independent. |
| **4. Artifact and usage portfolio** | Searchable HTML/PDF/Markdown document library and preview, run provenance, provider token ledger, harness/host measurements, estimated/unknown cost labels, hard token/call caps. | Document previews retain origin and format; filter totals reconcile to attempt ledger; blobs stay out of list queries; estimated cost is never displayed as verified billing. |
| **5. Routines** | Draft/manual repeat, schedule, timezone preview, pause, overlap/catch-up and idempotent fire, notification policy. | DST, offline daemon, restart, duplicate fire, and cancellation yield one traceable task/run per intended trigger. |
| **6. Workflows** | Prompt to draft generation, connected-app scope picker, user-editable typed stages, document and human gates, and run detail. | Arbitrary prompts produce reviewable drafts; missing app access cannot silently activate; edits do not alter running versions; no publish/deploy occurs without exact approval. |
| **7. Federated hosts** | Aggregate registered local/owner-cloud daemon projections and route decisions to their owners; later phone gateway mirror. | Disconnected host is labelled stale, counts identify missing sources, and stale remote decisions cannot execute. |

Deliver **multiple Teams + hierarchy + plan approval + one delegated document + Dashboard/Decisions**
as the first useful slice before adding the workflow generator. It proves
the requested main feature and the shared record model while keeping the
first release small enough to validate with a real Nebius account.

## Decisions to carry into implementation

These are recommendations, not blockers to writing the plan:

1. **Team onboarding:** suggest a default Lead/Researcher/Reviewer template,
   but allow a solo task without creating a team first.
2. **Agent instructions:** user-editable Markdown with version history;
   enforce access separately in the daemon, never through prompt text alone.
3. **Cross-project authority:** start with all projects on one owner daemon;
   label host and freshness from day one so federation does not change the
   meaning of status later.
4. **Decision routing:** one global Decisions read model; the original daemon
   or personal gateway remains the authority for the effect. If the owner is
   disconnected, actions are read-only until refreshed.
5. **Workflow editor:** prompt generation produces a draft; typed stage list,
   dependencies, app connections, and approval gates remain editable before
   activation. The graph is secondary to a keyboard-operable stage list.
6. **Cost language:** default to tokens and measured runtime. Show currency
   estimates with source and pricing date; do not imply exact provider bills.

## Source and implementation references

- Paperclip first-party [core concepts](https://github.com/paperclipai/paperclip/blob/master/docs/start/core-concepts.md), [agent guide](https://docs.paperclip.ing/guides/org/agents/), [decisions](https://docs.paperclip.ing/guides/day-to-day/decisions/), [routines](https://docs.paperclip.ing/reference/api/routines/), [costs](https://docs.paperclip.ing/guides/day-to-day/costs/), and [UI routes](https://github.com/paperclipai/paperclip/blob/master/ui/src/App.tsx). These support the precedent; the ion-specific architecture above is an inference and proposal.
- [Paperclip research and UI evidence](RESEARCH-PAPERCLIP.md), [architecture source review](RESEARCH-PAPERCLIP-ARCHITECTURE.md), [task-level ion mapping](15-paperclip-inspired-ion-experience.md), and [existing teams/automation plan](05-agent-teams-automations.md).
- ion implementation seams: [schema](../../db/schema.ts), [wire protocol](../../crates/waku-protocol/src/protocol.rs), [Nebius specialist workflow](../../crates/waku-core/src/driver/nebius/workflow.rs), [usage UI](../../src/app/usage_page.rs), and [sidebar](../../src/app/sidebar.rs).

No live Paperclip deployment, ion UI implementation, or visual QA is claimed
by this document. The supplied screenshots illustrate desired behavior; they
are not bundled into the repository because their clipboard paths are
temporary.
