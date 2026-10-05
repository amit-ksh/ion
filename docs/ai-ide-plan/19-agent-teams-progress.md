# Agent teams: implementation progress and review checklist

Updated 2026-10-05. Development branch: `codex/ion-agent-teams`, based on
`ion-prod` in a separate managed worktree. This records the foundation, invocation, continuation, native adapter, daemon dispatch, review/rework and token-budget
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
  query and stop runs. Protocol version is **16**; desktop/daemon/mobile clients
  must rebuild together.

`PrepareTeamRun` prepares a durable queue for an existing idle root task. It does
not launch a harness inline; the daemon dispatches ready assignments in the
background. `GetTeamRun`, `GetTeamAudit` and `StopTeamRun` return the
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
- Team status appears in the task. This checkpoint originally queued work only;
  the dispatcher increment below connects ready native workers. Normal-agent
  invocation for external harnesses remains pending their authenticated bridges.
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
- This adapter checkpoint supplied the trusted worker. The dispatcher increment
  below connects it to actual queued work. Additional harnesses and remote hosts
  remain under development.

## Daemon dispatch increment (2026-10-05)

Native adapter checkpoint: `d2a770f`.

- The server supplies one publisher to the owner daemon before accepting clients.
  Its Teams dispatcher scans bounded, indexed pages of eligible runs, materializes
  each run's outbox, claims ready assignments and launches native worker threads.
  A broken project does not prevent other runs from being considered. Queue,
  settings, stop and completed-worker changes wake it; trusted solo-agent queues
  are discovered by the same polling path.
- Children execute as real tasks. Waiting parents release coordinator capacity,
  retain their event stream and resume only after their watched child outcomes
  settle and the preceding worker joins. The same attempt/runtime and open turn
  continue, including after owner restart. Unknown live hosts stay uncertain.
- Worker events update daemon-owned transcripts incrementally and retain exact
  replay cursors. Text, reasoning and tool order are preserved; stable tool IDs
  update existing rows. Saves are batched at most once per 120 ms during streaming,
  with final persistence at lifecycle boundaries. Client saves cannot overwrite
  team task state. Desktop attachment also hydrates team setup blockers and final
  transcripts when no live worker exists.
- Readiness checks distribution policy, enabled harness, project folder, explicit
  pinned model and the actual Nebius account catalog. Missing setup consumes no
  attempt, remains queued with a concrete reason, and can recover after setup.
  Repeated identical reasons do not flood audit events. A model change requires
  a new run because existing profiles remain pinned.
- Current execution supports the native Nebius adapter on Local, including the
  saved Local fallback policy when no sandbox host can start. Other selected
  harnesses remain queued with a setup reason. No harness/model substitution or
  automatic workload routing occurs. Actual remote execution is still pending.
- Every completion reserves a durable per-run model-call slot before the request.
  The default cap is 64 calls per run, configurable from 1 to 4,096 through the
  run limits. Calls with unknown provider outcomes remain counted across yields,
  retries and restarts. Racing reservations and repeated steps cannot exceed the
  cap. Exact invocation replay from protocol 13 retains the new default cap; a
  changed cap still rejects the replay. This does **not** enforce token or dollar
  budgets; those remain below.
- Stop signals active workers immediately; capacity remains held until their local
  worker actually joins. A joined worker's known terminal failure is recorded as
  a blocker without automatic retry. Expired leases can be reconciled only to
  this same confirmed Local attempt. Removing a team task first stops its run;
  closing a client attachment leaves daemon-owned work running. The server rejects
  client Start/OpenTerminal attempts before they can replace that stream, even
  if the request reuses the same runtime ID.
- The dispatcher limits live workers to 16 and resident streams to 256 globally,
  in addition to pinned team and shared-agent limits. Transcript projection and
  provider/catalog I/O run on background threads, never in render.
- Task projection stores the assignment status separately from its immutable
  binding. Stopped unstarted children have no invented execution turn. Restarted
  unknown hosts show their reason while retaining coordinator capacity; they
  cannot silently start another worker.
- Protocol **14** adds aggregate model-call limits and task execution status.
  Rebuild desktop, daemon and clients together. The UI now shows actual task status, and native invocation
  tools report daemon dispatch rather than the former queue-only placeholder.


## Result review and rework increment (2026-10-05)

Daemon dispatch checkpoint: `1400e86`.

- New runs require parent review of child results by default. Submitted output
  enters **In review**, releases its execution slot and resumes a waiting parent
  with a bounded handoff. A dependent consumer remains queued until acceptance.
  Root completion requires every current child to be accepted; historical
  superseded results stay connected to their correction tasks.
- `review_child` is a structured native tool bound to the live parent lease and
  its direct child assignment. It records feedback and document/declared check
  evidence. Self-review, another parent's child, stale leases, changed receipt
  replay, stopped runs and unsubmitted results are rejected. Concurrent exact
  replays produce one review and one correction task UUID.
- Rework atomically retains the rejected result, publishes its review, and queues
  a separate task for the same pinned agent, objective, output contract and
  access. It does not repeat the original attempt. Its input includes the full
  bounded parent feedback and prior document IDs. Queued consumers are redirected
  to the corrected output. Older correction-input dependencies do not prevent
  later rounds from starting.
- Correction rounds are capped at two per result lineage by default, configurable
  from zero to eight through run limits; the aggregate work/model-call/deadline
  caps also remain in force. Exhausting the cap leaves the candidate unaccepted;
  the parent receives the reason and can report the remaining blocker.
- Delegation can pin an output contract: minimum unique documents, required
  formats and required declared check labels. Reporting rejects missing output,
  duplicated documents and superseded document reuse. Reading prior evidence
  still uses provenance authorization without demanding a completion contract.
  Declared check labels do **not** certify that a command or test was executed.
  Semantic output quality remains the parent's recorded review; executed check
  receipts and independent verification are still pending.
- Native tools expose contract delegation, declared checks and parent review.
  Child authorization is derived from the intersection of parent and pinned
  child grants. Correction workers receive the original objective and full
  feedback without copying unrelated conversations or changing harness/model.
- Accepted/superseded child status persists in a background batch even after the
  run exits the dispatch index. Task labels distinguish awaiting parent review
  from replacement by a correction task. Closing an attachment still does not
  cancel the worker. Stop and deadlines block pending reviews as well as work.
- Protocol **15** adds review/contract projections and review limits. Rebuild
  desktop, daemon and clients together. Older persisted runs retain their
  previous review policy and empty contract defaults; changing that policy is
  a changed invocation, not an exact replay.
- Acceptance rechecks current document provenance and structural output
  requirements. A removed document prevents acceptance while still permitting
  a correction request with valid observed evidence. Handoffs explicitly mark
  truncated output/feedback excerpts; full feedback is retained for corrections.

## Token reservation increment (2026-10-05)

Result-review checkpoint: `03640c3`.

- New runs pin a shared budget of 2,000,000 tokens by default. `maxTokens` is
  configurable from 1 to 1,000,000,000 through the trusted run-limits interface.
  Agent, parent, correction, retry and resumed requests all use the same ledger.
  A user-facing budget editor and dollar-price reservations remain pending.
- The trusted adapter reads the pinned model's exact verbose catalog entry and
  reserves its entire advertised input context plus the requested 4,096 output
  tokens before a completion request. This deliberately conservative allowance
  covers system instructions and tool definitions without a tokenizer guess.
  A model whose full allowance cannot fit is blocked before that request.
  Missing, invalid or duplicate metadata cannot authorize billable execution.
- Reservations and aggregate limits commit in one transaction. Parallel agents
  cannot each spend the last allowance; repeated model steps cannot replay a
  provider request. Call, model and allowance provenance remain daemon-owned.
- Native requests ask for streamed usage and cap completion tokens including
  reasoning. Only a complete stream with consistent nonnegative input/output/
  total usage can reduce a reservation to actual token counts. A malformed,
  missing or interrupted receipt retains the complete allowance. Cancellation,
  retry, parent yield and owner restart never clear it.
- Exact repeated receipts produce one accounting/audit update. A changed receipt
  cannot release a second allowance. The opaque call receipt can finish
  accounting after Stop/lease expiry without authorizing additional execution.
  A provider receipt above the advertised allowance is retained as a violation
  and blocks all later reservations for that run.
- Run snapshots expose model calls, consumed tokens, outstanding token
  reservations, unknown usage, historical unmetered calls and allowance
  violations. All ledger reads and provider metadata work remain on daemon/host
  workers. Streaming transcript cadence and the solo usage event remain intact.
- Protocol **16** adds the pinned token cap and usage projection. Older runs keep
  their previous call-only limit (`maxTokens: null`); old unmetered calls are
  explicitly counted. Exact user/agent invocation replays recover the saved
  limits, including the old review policy; they cannot silently enable a new cap.
  Explicit run-limit requests still reject changed arguments.
- This enforces token admission under the provider's advertised context and
  completion limits. It does not bound currency billing, cloud VM charges,
  connector costs or another app's usage. No live provider acceptance is claimed.

Provider interfaces verified against the current [verbose model catalog](https://docs.tokenfactory.nebius.com/api-reference/models/list-models),
[chat completion request](https://docs.tokenfactory.nebius.com/api-reference/inference/create-chat-completion)
and [public OpenAPI schema](https://api.tokenfactory.nebius.com/openapi.json).
The schema defines `RichModel.context_length`, `max_completion_tokens` including
reasoning, and `stream_options.include_usage`; actual account payloads still need
live acceptance once the key is available.

## Native team activity checkpoint

The native Teams page now has Teams and Runs tabs. The reusable roster follows
preorder so each manager's direct reports remain together, with an explicit
Reports to label. Runs are read through a bounded, project/team-filtered daemon
collection; opening one run fetches its full pinned hierarchy. Each assignment
shows its real task and parent task links, selected harness, actual host/fallback,
status, blocker, submitted summary, parent feedback and generated documents.
The run header exposes Stop team and durable token/call totals, including
allowances held for missing receipts. Team task transcripts link back to the run.

A single background poll runs only while the relevant workspace page is open.
Generation checks discard superseded requests, unchanged polls do not notify,
and rows are cached and virtualized. Run collection reads use one SQLite
snapshot and do not hydrate transcripts. Opening an uncatalogued child task
hydrates that task on a worker and guards against a changed user selection.
Pagination exposes older runs; project filters default to all projects.
Dashboard and Inbox use real run summaries. Team waiting/review states are no
longer mislabeled as solo human decisions. Artifacts refresh from the same durable
control store. Human planning decisions are now persisted as described in the checkpoint below;
parent review and execution blockers are not fabricated approvals.

Verification: 85 team regressions passed, including bounded pagination,
project/team filtering, deleted-definition history, setup blockers and metered
usage projections. The 7 protocol wire tests and 23 TypeScript client tests
passed; native app/daemon compilation, TypeScript type checking and generated
bindings checks passed. The collection pagination was then refined to immutable
creation-time ordering and verified separately before committing.

No visual test or app launch was performed, following AGENTS.md. User review:

- [ ] Open Teams → Runs, then follow lead and child task links.
- [ ] Confirm siblings remain under the correct manager; compare the pinned
  roster after editing a reusable agent or team definition.
- [ ] Inspect queued setup blockers, review feedback, correction tasks and documents.
- [ ] Stop a running team and confirm it reaches Stopped after workers join.
- [ ] Use project filters and Previous/Next with more than 32 saved runs.
- [ ] Confirm parent review appears in Runs rather than as a user decision.
- [ ] Inspect used/reserved tokens and missing-receipt counts.
- [ ] Review keyboard operation and text legibility in both themes.

## Editable run limits checkpoint

Team creation/editing now exposes assignment count, shared parallel tasks,
delegation depth, attempts, model calls, token cap, correction rounds, timeout
and parent-review policy. Save validates bounds in the coordinator's control
store as well as the UI. A clear token field explicitly means no token cap;
new teams default to 2,000,000 tokens and required child review.

Both user @team invocation and the native solo-agent invocation bridge resolve
these defaults when creating a new run. Exact invocation replay loads the old
run limits before comparing identity, so editing a template cannot enlarge or
rewrite an existing run's authorization. Legacy teams without a limits field
retain the documented default for new runs; old pinned runs keep their original
policy. Token limits remain distinct from currency and remote-host cost limits.

Verification: 87 team checks passed, including user/agent invocation with
saved limits, edits after queueing, legacy replay, concurrent invocation and
atomic rejection of invalid templates. Native app/daemon compilation passed.
The regression suite caught and verified the fix for opening a control-store
connection inside a held read transaction.

User review:

- [ ] Set a small token or model-call limit in a team, save, and start @team.
- [ ] Confirm the run header reflects those limits and stops before another
  billable native completion when no allowance remains.
- [ ] Edit the team's limits and confirm the existing run remains unchanged;
  a new run should use the edited limits.
- [ ] Enter invalid or zero limits and confirm saving fails without changing
  the stored definition.
- [ ] Review required/optional parent-review behavior and correction limits.

## Human team decisions checkpoint

A lease-bound `request_decision` tool checkpoints and parks an assignment until
an authenticated user approves or declines its exact proposal. The durable
record includes its project/task, pinned team revision, attempt, host and answer
revision. Parking revokes the old lease and releases capacity. A restart resumes
once under a fresh generation, delivering the full approved/declined receipt to
the same harness and model. Stop and deadlines cancel unanswered decisions;
conflicting or stale answers cannot authorize progress. Approval does not change
filesystem, connector, network or spending grants.

Inbox, Decisions and Teams → Runs show the full proposal with task/run links.
The background collection is bounded and project filtered, has paginated history
and a pending total for Dashboard. Answering is a background RPC with optimistic
revision checks. There is no model tool that can answer its own decision.
These are planning decisions. Privileged action-specific approvals belong to
the corresponding execution adapter and remain separately listed below.

Verification: **94 team tests passed**, including seven new coordinator/native
adapter checks for private checkpoints, lease fencing, shared capacity, restart,
exact replay, racing verdicts, decline, cancellation/deadlines, collection bounds
and rejection of model-supplied identity/host overrides. All 8 protocol wire tests, native app/daemon compilation, generated binding
checks, 23 TypeScript client tests and TypeScript type checking passed. Rust
formatting and diff checks passed.
No visual or live provider test was performed.

User review:

- [ ] Ask a team for a concrete decision; inspect its full proposal in Decisions.
- [ ] Approve and confirm the original task resumes with the selected harness.
- [ ] Decline and confirm the team revises its plan or reports a blocker.
- [ ] Restart while a decision is pending; confirm it survives without duplicates.
- [ ] Stop a waiting team; confirm its pending decision becomes canceled.
- [ ] Review project/task filters, history pagination and Dashboard pending totals.

## Claude Code team bridge checkpoint

Claude Code can now execute team assignments through the same lease-bound tool
loop as native Nebius. Each bounded CLI request supplies the pinned agent's
instructions, private journal and current Ion tool catalog through stdin. Claude
returns JSON actions; Ion validates and executes them, returning durable receipts
on the next request. Delegation, child wait/review/rework, human decisions,
documents and results retain the coordinator's existing controls.

The CLI runs in an empty temporary directory with its built-in tools, external
MCP servers, project/user customizations, slash commands and hooks disabled.
One request permits one model turn. Process output and duration are bounded;
Windows job containment and Unix process groups stop descendant processes on
cancellation/timeout. No raw stderr or authentication payload enters a transcript.
A canceled or malformed completion retains its reserved token allowance.

Readiness checks the installed executable's required flags and authenticated
Claude account without starting a billable turn. The user's binary override is
honored. Supported exact models currently include Sonnet/Opus 4.6 and 5.5 and
Haiku 4.5. Aliases/custom provider routes remain blocked until their limits can
be verified. Reservations use the supported model's full context/output bounds;
receipts count uncached, cached-read and cached-created input tokens. Another
model, missing usage or an unbounded turn result cannot release the allowance.
No other harness is substituted for a queued unsupported agent.

The bridge currently exposes Ion's document/read/delegate/review capabilities.
Workspace edits, command execution and apps remain separate execution work.
Claude's own unrestricted coding tools are not exposed under a read-only grant.
Provider cost estimates are not treated as authoritative currency receipts.

Verified contracts: [CLI reference](https://code.claude.com/docs/en/cli-reference),
[usage accounting](https://code.claude.com/docs/en/agent-sdk/cost-tracking),
[Sonnet model specifications](https://platform.claude.com/docs/en/models/sonnet-4-6/overview).
Installed CLI help was also inspected locally.

Verification: the **100-test team suite passed**, including the mixed
Nebius/Claude hierarchy regression. The added process bridge fixture then ran a
real bounded subprocess through the production Claude adapter, published a real
attributed document and completed the coordinator task with two metered requests
without provider billing. All **6 Claude adapter tests** and the pinned-harness
substitution regression passed. Native app/daemon compilation, formatting and
diff checks passed. No app launch, visual check or live Claude account execution
was performed.

User review:

- [ ] Authenticate Claude Code locally and select an exact supported model.
- [ ] Create a mixed Nebius lead / Claude reviewer team and start it with @team.
- [ ] Inspect each child task's own harness/model, evidence and parent review.
- [ ] Confirm unsupported/custom models show a setup blocker without an attempt.
- [ ] Cancel during a Claude request; confirm the process stops and unknown usage
  remains reserved rather than being shown as zero.
- [ ] Compare metered tokens with the CLI receipt once a live run is available.

## Remaining development

- [x] Wire native delegation to the materializer and dispatcher wakeup; drain
  additional batches during normal operation.
- [x] Replace legacy single-prompt Team preparation with explicit @ addressing
  and daemon-owned durable queue creation.
- [x] Add the native normal-agent team invocation queue bridge.
- [x] Implement native Nebius team start/dispatch on Local.
- [x] First authenticated external harness bridge: Claude Code.
- [ ] Additional external harness bridges: Codex, OpenCode and the remaining catalog.
- [x] Native Nebius worker with lease-bound delegation/report/document tools,
  enforced read grant and bounded resumed child context.
- [x] Connect native workers to the daemon dispatcher, durable transcripts,
  setup readiness, model-call quotas and attached runtime events.
- [x] Enforce native per-run token reservations before billable execution.
- [x] Add the user-facing token and execution limits editor.
- [ ] Add dollar-price reservations and cost limits.
- [x] Durable parent waiting/resume and bounded child result handoffs.
- [x] Native adapter delivery of bounded child handoffs and document evidence.
- [x] Native parent review/rework and structural document/check output contracts.
- [ ] Executed check receipts, independent semantic acceptance and review gates
  for additional harness adapters.
- [x] Per-call pinned-model token allowances and durable provider usage receipts.
- [ ] Per-call cost reservations and currency enforcement; token limits do not
  enforce dollar budgets or remote host charges.
- [x] Composio exact connected-account/action/version grant editor and approved execution.
- [ ] Additional filesystem/resource/egress policy and general command containment.
  A prompt, worktree or saved grant is not permission enforcement.
- [ ] Team readiness matrix and supported model validation for each harness.
- [x] Native hierarchy/roster with live child links, blockers and results; cached
  run projections across Dashboard, Inbox and Artifacts. Solo-style false decisions
  are removed; human planning decisions now have persisted pause/resume receipts.
- [x] First bounded external harness bridge and mixed-harness coordinator regression.
- [ ] Additional mixed-harness adapters and live authenticated acceptance.
- [x] Serialized writers, isolated coding worktrees and generated patch documents.
- [ ] User-approved integration into the original checkout and uncertain-effect recovery.
- [ ] Registered Nebius execution host: actual harness launch, explicit transfer,
  host readiness, remote status/cancel/checkpoint and tested local fallback.
- [x] Persisted human planning decisions bound to exact proposal/revision/host.
- [x] Exact app-action approval with durable no-replay execution receipts.
- [ ] Effect-specific filesystem integration/cloud approvals.
- [ ] Routine scheduling and workflow execution over the same coordinator.

The Team “New task” action now prepares an explicit team address in the normal
composer. Both user addressing and native agent invocation queue a real run.
Ready native Nebius assignments now execute through the daemon's dedicated Teams
worker with parent review and bounded correction tasks. Mixed harnesses,
coding/tools, independent acceptance verification and remote execution remain
under development. Team tasks never enter the old solo action.

## Developer verification

Run from this worktree with dependencies installed:

```powershell
cargo test --locked -p waku-core team -- --test-threads=4
cargo test --locked -p waku-core control::tests:: -- --test-threads=4
cargo test --locked -p waku-core server::tests:: -- --test-threads=4
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
  **83 passed**, including **53 coordinator tests**, **11 native adapter tests**
  and **12 dispatcher tests**. The six continuation
  regressions cover three-level execution with one slot, restart, stale leases,
  exact replay, racing resume, shared capacity, cancellation/deadlines and
  uncertain children with escaped/Unicode handoffs. The native adapter checks
  a three-level document/review chain, pinned definitions, denied tools and
  identity overrides, private/traversal reads, bounded replies, invalid waits,
  unfinished children, cancellation before effects and plain text without a
  completion receipt. Document checks cover atomic replay, concurrent publication
  and assignment/handoff provenance.
  Dispatcher checks execute a three-level chain through real adapter tools,
  restart waiting parents, retain uncertain host outcomes, preserve transcript
  event order and stable tool identities, recover missing setup without a new
  task, enforce aggregate model-call caps and hold cancel capacity until the
  actual worker joins. Server ownership coverage keeps the team stream alive
  after client Close and rejects solo Start over the same task. Legacy request
  replay accepts only the documented default for the added run limit.
  Review checks cover pending result capacity, acceptance before dependent
  dispatch, racing/exact rework receipts, unchanged agent/model/access, two-round
  correction limits, stop/deadline fencing, missing formats/checks/documents,
  removed evidence, superseded output reuse and legacy defaults. Sixteen
  superseded children with escaped Unicode evidence fit the handoff budget.
  The native dispatcher executes a correction task through real adapter tools
  and restores the exact review status from persisted task details. A separate
  regression protects that update when a matching catalog hides stale detail.
  Token checks cover concurrent shared reservations, exact/racing receipts,
  unknown usage across parent yield and restart, accounting after Stop, pinned
  model identity, provider overage and invalid limits. Native checks prove denial
  before the completion callback and actual usage settlement before the next
  request; missing usage retains the full allowance. Stream checks reject
  inconsistent/partial receipts without losing the legacy solo occupancy event.
  User and native-agent invocation replay retains legacy pinned limits without
  accepting changed work. A daemon regression verifies the visible budget blocker.
The following unaffected suites also passed at the daemon-dispatch checkpoint:

- Runtime publisher cursor/replay regression: **1 passed**.
- Server lifecycle and reconnect suite: **17 passed**, including the team-owned
  attachment and rejected solo-start regression.
- Team addressing parser: **3 passed**.
- Native composer completion suite: **11 passed**.
- Existing control store suite: **5 passed**.
Current token-budget increment verification:

- Protocol wire contract suite: **7 passed** (including review and token limits).
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
Native adapter checkpoint: `d2a770f`. Daemon dispatch checkpoint: `1400e86`.
Result-review checkpoint: `03640c3`. The token-budget checkpoint follows the
verification above. No live provider, sandbox, mobile build or visual acceptance
is claimed.

## Isolated coding workspace increment (2026-10-05)

Claude execution checkpoint: `9bcec51`.

- Direct user team invocation now uses the saved lead's exact configured access;
  repeated invocation retains its original run grants. Normal-agent invocation
  remains conservatively read-only until its source authorization bridge is added.
- Coding runs reserve a deterministic Git branch and shared workspace before
  dispatch. The source must have a clean committed baseline; a dirty source,
  missing Git project, submodule or custom checkout filter shows a setup blocker.
  No stash/reset or implicit working-tree snapshot is performed. Once ready,
  the branch/base/workspace remain pinned across restart and source edits.
- Every team task uses that same persisted worktree for its workspace and review
  panes. The run shows its branch/base and an Open workspace action.
- Local writers are serialized transactionally per run. Yielding frees the
  writer slot; a running, stopping, uncertain writer or unresolved file effect
  prevents a competing writer from starting. Separate runs have separate branches.
- Native Nebius and the Claude bridge expose granted bounded `write_file` and
  `publish_change` tools. Exact previous content is required for replacement;
  null creates a missing file without replacing a concurrent creation. Paths
  reject traversal, links, private files, alternate streams and device names.
  Parent directories must exist and content is limited to 16 KiB per file.
- File effects are reserved before writing and completed with durable receipts.
  Exact replay returns the receipt; changed requests reject. Interrupted effects
  retain a pending receipt, block later writers/completion and are never repeated.
- `publish_change` creates an AI document with base commit, branch, exact original
  and final content and edit receipt IDs. It confirms that the actual current
  file still matches the receipt. Parent acceptance retains that evidence.
- The source checkout remains unchanged. Integration/merge approval, general
  shell/check execution, directory creation/deletion and uncertain-file recovery
  are still implementation work; completion of a coding assignment means its
  isolated change is submitted and reviewed, not merged or tested automatically.
- Trusted subprocess containment is shared by harness and fixed Git operations:
  bounded output/time, drained pipes and descendant cancellation. Git checkout
  hooks/global filters/fsmonitor are disabled; local checkout filters block setup.
- Protocol **20** adds the optional persisted workspace projection. Rebuild the
  desktop, daemon and clients together.

Verification: **108 team tests passed**, including six workspace regressions
and the real adapter delegation/edit/document/review chain. **8 protocol tests**,
**23 TypeScript client tests**, TypeScript checking, native desktop/daemon checking,
Rust formatting and diff whitespace checks passed. No live provider or visual
acceptance was run. This checkpoint is committed after those checks.

User review:

- [ ] Enable workspace write on the team lead and coding specialist.
- [ ] Start `@teamname` against a clean committed Git project.
- [ ] Confirm a dirty project shows a setup blocker without changing its files.
- [ ] Observe a specialist edit and generated change document; open its workspace.
- [ ] Confirm the lead reads/reviews the document and the run reaches Completed.
- [ ] Confirm the original project checkout is unchanged until reviewed integration.
- [ ] Restart and confirm the same branch, base commit and changed files remain.

## User review of invocation and native dispatch

- [ ] Send a plain New task prompt; confirm normal solo behavior.
- [ ] Start a prompt with `@teamname` or choose a team from @ suggestions.
- [ ] Use a spaced team name, duplicate names and an unknown qualified team.
- [ ] Press Enter/Tab/Escape and use arrows in the shared @ picker.
- [ ] Confirm a team reference in quotes, code or later text does not route work.
- [ ] Confirm path mentions such as `@./engineering` still address files.
- [ ] Confirm the queued task preserves the lead harness/model and typed prompt.
- [ ] Refresh/restart and confirm the same task/run survives without duplicates.
- [ ] Without setup, confirm a queued task shows the actual setup reason and
  consumes no attempt. With ready native agents, observe child execution and resume.
- [ ] Refresh a completed team task and confirm its transcript and documents remain.
- [ ] Observe a child enter In review; confirm the parent accepts it or creates
  a separate correction task, preserving the rejected document and feedback.
- [ ] Check that a consumer waits for accepted corrected output and that a
  parent cannot finish with an unaccepted child.
- [ ] Request missing documents/formats/check labels; confirm reporting returns
  feedback and does not commit a completed result. Declared checks are not executed tests.
- [ ] Inspect the run token totals; confirm missing usage remains reserved and
  budget exhaustion shows a blocker before another provider request. Current
  default is 2,000,000 tokens; customize new-run limits in the Team editor.
- [ ] Close the team task attachment while children run; reopen it and confirm
  the same run continues. Use Stop separately to cancel the owning team run.
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
- Configure Composio privately and grant exact account/actions in the Agent editor. Merely connecting an app does not grant execution access.

These resources are not required for the coordinator's local regression tests.


## Approved connected-app execution increment (2026-10-05)

Previous coding workspace checkpoint: `65cbccf`.

- The Agent editor can load connected Composio accounts, search/page published
  actions, grant exact account/action/version combinations and remove grants.
  A newer version requires an explicit reselection. Existing runs keep their
  original grant. Old unversioned grants remain visible but cannot execute.
- Both native Nebius and the bounded Claude bridge expose granted action
  metadata and `request_app_action`. All app invocations currently require a
  user decision, including reads; no inferred read-only category is trusted.
- Decisions shows the complete host-generated account, action, published version
  and canonical arguments. Approval is bound to the pinned assignment, attempt,
  team definition and actual execution host. The model cannot substitute another
  account, version, payload or host after approval. A decline sends no mutation.
- After approval, the worker checks the exact connected account is active and
  the requested version still belongs to its toolkit. The private Composio key
  stays at the credential boundary. Calls use explicit `connected_account_id`,
  `version` and structured `arguments`; proxy execution, instant-account fallback,
  natural-language execution and authentication overrides are unavailable.
- Execution is reserved before the one POST and acknowledged with a durable
  receipt. Exact replay returns that receipt. A timeout/malformed acknowledgement
  retains its pending effect and blocks claims, later model spending, file edits,
  yields and successful completion. It never silently sends the action again.
  Recovery controls for such uncertain effects are still a separate checkpoint.
- App results become AI-generated JSON documents with task/project provenance.
  Models receive a bounded excerpt and document ID; private session/auth envelope
  fields are excluded, common authentication keys are redacted, and oversized
  provider data is explicitly marked truncated. Provider failures do not pretend
  the intended work succeeded. Stop can record a late acknowledgement of an
  already-started effect without authorizing another action.
- The editor's RPC/network work runs in background tasks. Action searches are
  bounded to 20 rows, with pagination and generation guards. Existing keyboard
  menu/button primitives are reused; no new rendering I/O or watcher was added.
- Protocol **21** adds versioned grants and optional exact app-action decisions.
  Rebuild desktop, daemon and clients together.

Composio implementation references: [tool listing](https://docs.composio.dev/reference/api-reference/tools/getTools),
[pinned tool metadata](https://docs.composio.dev/reference/api-reference/tools/getToolsByToolSlug),
[exact execution request](https://docs.composio.dev/reference/api-reference/tools/postToolsExecuteByToolSlug)
and [connected-account status](https://docs.composio.dev/reference/api-reference/connected-accounts/getConnectedAccountsByNanoid).

User review after private Composio setup:

- [ ] Open the Agent editor and load connected apps; choose an active account.
- [ ] Search actions and grant only the exact actions the agent needs; save.
- [ ] Grant the same exact account/action/version to its lead so delegation can
  intersect those permissions. Merely connecting an app never grants execution.
- [ ] Ask the team to use an action. Confirm it appears in Decisions and Inbox
  with the complete account, version and arguments before execution.
- [ ] Decline one request and confirm no app action is sent.
- [ ] Approve one request and verify the actual provider outcome and generated
  result document, then the lead's review and final result.
- [ ] Disable a connected account before approving; confirm setup blocks the
  action without switching accounts or sending a mutation.
- [ ] Confirm a missing acknowledgement shows uncertainty and never retries
  automatically. Inspect the provider log before reconciling the outcome.
- [ ] Edit the saved grant/version after a run starts; confirm its pinned grant
  remains unchanged and a new run uses the updated grant.

No live app account was modified. Private keys, cloud targets and authenticated
provider acceptance remain user setup. Additional harness adapters, remote Teams
execution, executed checks, cost budgets and integration/recovery remain work;
this increment does not mark end-to-end Teams implementation complete.

Verification for this increment: **115 team tests passed**, including exact
app approval/replay, grant expansion rejection, declined/preflight-failed calls,
unknown acknowledgements and native approval → execution → document → completion.
**4 Composio credential tests**, **23 TypeScript client tests** and TypeScript
checking passed. Desktop/daemon checking passed. Protocol bindings were regenerated;
**8 protocol tests**, Rust formatting and whitespace gates passed.
No provider billing, live app action, visual test or app launch was performed.


## Normal-task team execution increment (2026-10-05)

Previous connected-app checkpoint: `ad74d63`.

- A normal native Nebius task can invoke a saved team with its configured grants.
  Full access / auto-accept-edits permits saved file/app grants when the lead has
  no shell or general network grant. Other modes request permission for the exact
  saved lead, harness/model, environment, access and run limits. Read-only teams
  can start without that prompt. Arguments cannot supply identities or permissions.
- An identical tool-call replay returns its original run and authorization without
  prompting again. Definition changes, project overrides and changed objectives
  reject the replay; each source still has a maximum of four invocations.
- `await_team` waits for up to 30 seconds on the provider worker and returns actual
  state. Setup blockers, uncertainty and elapsed waits never imply completion.
  `read_team_document` exposes only accepted documents from a completed run owned
  by this source task and project, with an explicit bounded/truncated excerpt.
- Explicit source-task Stop revokes its own team runs and cancels their workers.
  Unrelated runs continue. Close attachment remains separate from Stop. The queue
  path also closes cancellation during the invocation transaction.
- App actions still require their own exact Decisions approval; invocation approval
  cannot enlarge the saved team grant. External solo harness invocation bridges
  and the remaining execution/recovery/integration capabilities are still work.

User review:

- [ ] Ask a normal Nebius task to delegate to a saved read-only team.
- [ ] In Ask mode, start a team with file write access; verify the exact scope prompt.
- [ ] Decline and confirm no run starts; allow and inspect the isolated workspace.
- [ ] Let the source wait for the run and read the accepted generated document.
- [ ] Stop the source task while its team runs; confirm only its runs are stopped.
- [ ] Close an attachment and confirm this does not stop the owner run.

Verification: **118 team tests**, **23 TypeScript client tests**, TypeScript
checking, desktop/daemon checking, Rust formatting and diff whitespace gates
passed. No live provider, visual acceptance or app launch is claimed.
