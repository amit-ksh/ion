# Agent teams: implementation progress and review checklist

## Current scope update (2026-10-14)

The user has reopened the remaining harness integrations and requested additional document formats, larger publications and fuller HTML/CSS phone rendering. This supersedes the earlier adapter deferral for the active goal. Unsupported native account/model/budget contracts remain implementation gaps, rather than completed capabilities. [Current document increment](63-phone-rendered-documents.md) records available behavior and remaining publication/resource work.

Updated 2026-10-12. Development branch: `codex/ion-agent-teams`, based on
`ion-prod` in a separate managed worktree. This records the foundation, invocation, continuation, native adapter, daemon dispatch, review/rework and token-budget
increments of [the agreed plan](18-agent-teams-implementation.md).

For the current implementation scope, remaining development and live setup, use the
[release checklist](20-agent-teams-release-checklist.md). Earlier sections below
record historical increments and their verification, not a claim that every
planned Teams capability is complete.

## Historical scope correction (2026-10-08)

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

The earlier ~75% implementation / ~25% remaining estimate included the now
deferred adapter work. It is a historical estimate, not a current percentage for
the reduced scope. Deferring work does not mark it implemented.

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

## Safe local checkpoint continuation increment (2026-10-05)

Normal-task invocation checkpoint: `02f604a`.

- A stopped Local worker can continue from its durable private checkpoint using
  the Teams run's **Continue from checkpoint** action. This is explicit user work,
  not automatic retry. The owner rejects it while the previous worker is finishing.
- The trusted native/Claude loop marks only checkpoints with no unfinished tool
  calls and remaining model-call capacity as recoverable. A newer unsafe checkpoint
  removes the marker. Legacy/opaque checkpoints do not silently become resumable.
- The continuation retains the same task and attempt, actual host, selected harness
  and model, pinned permissions and all consumed/reserved token history. A fresh
  lease generation fences the old worker. No new attempt or spending limit is granted.
- Queueing is transactional, checks the displayed run revision and is idempotent
  while the continuation is queued. It observes the original deadline, shared agent
  and team capacity, and a maximum of eight user continuations per assignment.
- Unconfirmed hosts, remote assignments, pending file/app effects, child/decision
  waits, stopped runs and exhausted safe checkpoints cannot use this action.
  A trusted reconciliation cannot claim success while an effect is still pending.
- The UI reads a cached availability projection and submits its RPC in the
  background, retaining the existing hierarchy, task links and keyboard controls.
  Protocol **22** adds this action and the defaulted availability flag; desktop,
  daemon and clients must rebuild together.

User review:

- [ ] Interrupt a model request temporarily and observe a blocked Local assignment.
- [ ] Restore provider access and use Continue from checkpoint on the run.
- [ ] Verify the existing task, harness/model, host, permissions and token history
      remain intact as the hierarchy finishes and publishes its final document.
- [ ] Confirm Stop or expiry prevents continuation.
- [ ] Confirm unknown host/file/app outcomes expose no safe-continuation action.

This increment covers safe checkpoints after a known Local worker exit. Confirmation
of a host left behind by owner restart and file/app outcome reconciliation remain
separate implementation work; no unknown action is retried by this control.

Verification: the **124-test Teams suite** passed, followed by **7 focused
checkpoint-continuation tests**, including the added runtime join → continue →
hierarchical completion test. **9 protocol tests**, current generated bindings, **23 TypeScript client tests**,
TypeScript checking, native desktop/daemon checking, formatting and whitespace
gates also passed. No live provider or visual acceptance ran.

## Structural coding changes increment (2026-10-05)

Safe continuation checkpoint: `d9dfba6`.

- Granted native Nebius and Claude agents can create one directory at a time
  in the isolated run workspace. Parents must exist; each creation has its own
  exclusive filesystem operation and durable receipt. Existing paths reject.
- `delete_file` requires the exact current UTF-8 content (maximum 16 KiB), read
  and write grants and a ready isolated workspace. It removes only that one file
  and preserves the exact removed bytes in the durable edit receipt for recovery.
  Directories, recursion, read-only files, private paths and links
  remain rejected. The original project checkout is unchanged.
- File writes, directories and deletions now share the same private reservation
  and acknowledgement module. Each stable request ID binds exact arguments;
  known receipts replay without another mutation, while pending outcomes block
  further effects, model calls and completion. An acknowledgement must match its
  run, work and attempt rather than silently succeeding on a missing record.
- Generated change documents support an explicit null final value for deletion
  and verify that the actual path is absent. Delete/recreate chains retain the
  original before value, final content and all file receipt IDs. Empty directories
  are not Git file changes and do not masquerade as a published file patch.
- A deletion first moves the actual file to a private stage and verifies its
  content. Matching bytes are already preserved in the committed receipt; the
  stage is removed so it does not contaminate Git review. Unexpected content
  stays at the private stage for reconciliation and is excluded from agent reads.
  An interrupted structural change remains pending and is never performed twice.

User review:

- [ ] Ask a permitted coding specialist to create a new source directory and file.
- [ ] Remove an obsolete small text file; inspect its exact generated change document.
- [ ] Confirm a deleted file has null final content and the source checkout is intact.
- [ ] Confirm exact removed bytes appear in the generated change document and
      recovery bookkeeping does not appear as an added file in the Git diff.
- [ ] Verify stale content, missing parents, directory deletion and denied grants
      produce feedback without changing an unrelated file.

No shell execution, automatic test claim, source integration or live provider
acceptance is implied by these structural tools. Remaining adapters, actual
sandbox execution, executed checks, cost caps and reconciliation/integration
continue to be recorded as development work.

Verification: **130 Teams tests passed**, including structural edits, exact
replay, scope/private-path denial, preserved deletion bytes, clean Git review,
unknown structural effects and the native create → write → delete → document →
result chain. Desktop/daemon checking, Rust formatting and whitespace gates
passed. Protocol/client checks from `d9dfba6` remain current; this checkpoint
makes no wire-contract change. No live provider or visual acceptance was run.

## Reviewed source integration increment (2026-10-06)

Structural coding checkpoint: `bd74d14`.

- A completed coding run can prepare an immutable combined review directly from
  its durable native edit receipts. Every final file/directory is checked against
  the actual isolated workspace. Extra staged, unstaged or untracked changes,
  discontinuous edit chains and changed worktree identities reject preparation.
  Creation, deletion, replacement and nested directory creation retain their
  original before values, final values and contributing receipt IDs.
- Models have no source-application tool. The native run's Review changes action
  opens the combined diff, and Apply reviewed changes approves the exact saved
  review identity/revision. All source binding, filesystem, Git and RPC work runs
  on background workers. Diff parsing/tokenization is cached there; expanded diff
  lines use the existing native renderer and virtualized list. Cached diff lines
  are capped at 50,000 across the review; Save review preserves the exact full
  before/after content and receipts for inspecting larger changes.
- Approval rechecks the actual pinned worktree result, original project/repository,
  source commit, clean checkout and exact before content before reserving any
  effects. Conflicts are shown in the review. It never stashes, resets, stages,
  commits or silently merges another edit. A changed base requires the user to
  reconcile that baseline rather than having the model overwrite it.
- The durable approval precedes each source effect's reservation. Repository-wide
  application serialization prevents another team application crossing an
  interrupted one. Before content remains in the saved review. A concurrent
  unexpected file is preserved in a private source backup, with an uncertain
  receipt; the effect is never replayed automatically.
- Completed applications return the same durable receipt on repeated requests,
  preserving newer user edits. Owner restart converts an unfinished application
  into an uncertain outcome. Explicit reconciliation reads/verifies source and
  requires private stages to be inspected/preserved and cleared before confirming
  either the exact reviewed result or the restored original source. A confirmed
  restoration closes the application without granting a replay. Reconciliation
  does not perform filesystem effects.
- Git metadata checks reject repository clean/smudge/process filters before status
  inspection, alongside the existing disabled hooks/fsmonitor/global configuration.
  Protocol **23** adds review/application/reconciliation RPCs and the exact change
  manifest. Desktop, daemon and clients must rebuild together.

User review:

- [ ] Complete a coding team task that creates a directory/file, updates an existing
      text file and removes an obsolete file. Inspect every combined change.
- [ ] Apply the saved review and confirm the original project contains the expected
      changes without an automatic commit or index update.
- [ ] Introduce a source conflict before approval and confirm Apply is unavailable
      or rejects without overwriting the newer file; resolve and refresh the review.
- [ ] Repeat a successful Apply after a new user edit and confirm it is preserved.
- [ ] In a disposable project, inspect/reconcile a partial application and verify
      automatic replay remains refused.
- [ ] Check keyboard operation and large expanded diffs when live UI review is run.

Verification: **137 Teams tests**, **10 protocol tests**, **2 desktop diff/cache
regression tests** and **23 TypeScript client tests** passed. Generated bindings
are current; TypeScript checking, desktop/daemon checking, Rust formatting and
whitespace gates passed. The source application tests used disposable repositories;
no live provider, account mutation, cloud deployment, visual test or second watcher
was run.

## Stopped Local host and effect recovery increment (2026-10-06)

Previous source integration checkpoint: `6627c5b`.

- Teams assignments expose Review recovery for an unknown Local host, pending
  file/app effects or a native checkpoint requiring receipt repair. The owner
  runtime rejects confirmation while its current worker has not joined. A host
  left behind by owner restart additionally requires explicit user confirmation
  that it has stopped; absence from the new daemon's worker map is insufficient.
- Every confirmation binds the run, work item, exact attempt and reviewed run
  revision. Exact retries return the same committed result; changed identities,
  assertions or evidence reject. The recorded human confirmation does not issue
  a new lease, reset usage, enlarge grants or start another attempt.
- File recovery checks the pinned isolated Git workspace and actual bytes or
  directory state. The user can confirm an applied result or restored original.
  Remaining private staging/deletion bytes must be inspected, preserved and
  cleared before confirmation. Recovery never rewrites a file or removes a
  private backup. Restored effects are excluded from change documents and source
  integration rather than represented as successfully applied edits.
- App recovery retains the exact approved account, action, version and arguments.
  Success/failure requires nonempty provider evidence entered by the user. It is
  labeled `confirmed_by: user` in a generated JSON artifact, preserving the
  distinction from machine provider acknowledgement. A failure confirmation
  does not grant permission to retry the original app request. Differing late
  acknowledgements cannot overwrite the durable recovered outcome.
- Prepare continuation repairs the private native journal using argument-matched
  durable file, document, delegation and parent-review receipts. Read-only or
  uncommitted calls receive explicit interruption feedback; no tool is executed.
  Settled app outcomes and document references are supplied as bounded data.
  Pending effects anywhere in the run, a child/decision wait, changed attempt,
  exhausted native steps or oversized context prevent preparation. The repaired
  checkpoint and safe marker commit atomically. Continue from checkpoint is a
  separate user action through the existing native dispatcher.
- The native recovery panel reuses cached diff parsing and virtualized lines.
  RPC, Git, filesystem inspection and receipt parsing run on background workers;
  generation checks reject stale responses. Controls use the existing keyboard
  focus/button and text-field primitives. The ordinary New task path is retained.
- Protocol **24** adds recovery review/confirmation commands and a paired recovery
  review/run snapshot. Rebuild desktop, daemon and clients together.

Local recovery does not establish an actual registered remote host. The remaining
harness adapters, external solo team bridges, actual Nebius execution, executed
checks, monetary/cloud caps, additional command/resource enforcement, attachment
handoffs and follow-up prompts remain in the release checklist.

User acceptance steps are included in
[the release checklist](20-agent-teams-release-checklist.md). Automated recovery
fixtures exercise interrupted receipt boundaries and actual native continuation;
live provider accounts, remote infrastructure and UI acceptance remain separate.

Verification: **146 Teams tests**, **11 protocol tests**, **24 TypeScript client
tests** and **2 native diff/cache tests** passed. Generated bindings are current;
TypeScript checking, desktop/daemon checking, Rust formatting and whitespace
gates passed. Recovery tests include an actual native continuation after a lost
app acknowledgement with the app callback count remaining one.

Recovery metadata lookup now selects blocked assignments before reading native
journals. A synthetic in-memory SQLite check with 64 running assignments, 60 KB
journals and 100 iterations measured 1.007 ms/query before filtering and
0.003 ms/query after filtering; this is query evidence, not live application FPS.
No live provider, account mutation, cloud deployment, visual test or second
development watcher was run.

## Immutable text input handoffs increment (2026-10-06)

Recovery checkpoint: `7bd5bc4`.

- New task keeps its existing composer and allows a team invocation with attached
  UTF-8 text files/folders. The daemon verifies managed attachment references,
  names, types and paths before preparing a run. Unsupported images/PDFs/binary
  inputs return a concrete error without consuming the draft.
- Files are copied into the control database in the same transaction as run
  preparation and the task outbox. Exact retries compare original presentation
  and use committed bytes after a lost queue acknowledgement; later upload
  edits or cleanup cannot change the model's inputs. This snapshot does not
  execute a command, overwrite project files or create an AI artifact.
- Up to eight attachments, 64 files and 2 MiB total are supported, with 256 KiB
  per file, folder depth eight and 256 inspected entries. Private/generated
  paths and links reject. Folder enumeration sorts deterministically.
- Native Nebius and the bounded Claude action bridge expose `list_inputs` and
  `read_input` under the assignment's intersected workspace-read grant. The
  manifest pages four files; reads return at most 200 lines and 8 KiB escaped
  data. Exact run/lease checks fence other runs, denied specialists and stale
  workers. User inputs are labeled data and do not enlarge any permission.
- Root task materialization retains original attachment chips separately from
  the original prompt. Metadata-only run projections contain no input bytes or
  host paths. The native composer retains newer draft edits if a queue response
  arrives after the user changes its text or attachment set. All input I/O runs
  on the daemon worker; streaming/render cadence is unchanged.
- Protocol **25** adds queue attachment metadata and the immutable input manifest.
  Rebuild desktop, daemon and clients together. Image/PDF interpretation and
  follow-up prompts remain release development items.

Verification: **152 Teams tests**, **11 protocol tests** and **25 TypeScript client
tests** passed. Generated bindings are current; TypeScript checking,
desktop/daemon checking, Rust formatting and whitespace gates passed. Fixtures
cover native delegation/resumption using the pinned input copy, exact queue
replay after upload modification, task presentation, denied/revoked read grants,
combined size/count bounds and unsupported input rejection. No live provider,
account mutation, cloud deployment, visual test or second watcher was run.

## Active lead follow-ups increment (2026-10-06)

Text input handoff checkpoint: `249a967`.

- The normal composer can send bounded text to an existing lead task. This queues
  up to eight messages of 4 KiB each under the run's original objective, pinned
  profiles, access, deadline and aggregate budget. Additional attachments and new
  work after completion use New task. Only the root task can use this RPC.
- Exact message IDs bind retries to their original text/presentation. The durable
  queue and task transcript outbox survive a lost acknowledgement or restart;
  replay preserves the same message and existing turn. Mobile/desktop projections
  handle the typed event and deduplicate by its ID.
- Delivery and the private native journal commit together under the current lease
  at a model boundary after all tool receipts. A result racing a queued message
  cannot complete before delivery. The next inference spends from the existing
  budget, and the lead may delegate within its existing hierarchy and access.
- Queued and child-waiting leads can receive a message. A recoverable blocked lead
  keeps the message queued until the user chooses Continue from checkpoint.
  Uncertain hosts, unresolved decisions, pending effects, unavailable adapters,
  exhausted call/step limits and oversized context reject new messages. A message
  does not approve a decision, resume a worker or grant permissions. Stop/deadline
  cancels undelivered messages.
- Native run rows display queued, delivered and canceled messages using cached
  projections and the existing virtualized list. Queue RPC and durable I/O run on
  background workers. Draft edits made during acknowledgement are preserved.
- The phone uses the same durable follow-up command for team-owned tasks,
  including tasks whose selected harness is Claude. It retains the exact message
  ID after transport failure, adopts acknowledgement against the latest streamed
  history and deduplicates the later event. It does not start a solo runtime or
  open a second turn. Phone Stop loads the current run revision and can stop a
  queued team without an attached runtime. Pinned model options remain hidden.
- Desktop and phone retain draft edits made while a send acknowledgement is in
  flight. Failed or unsupported submissions leave text and attachments available.
- Protocol **26** adds the follow-up command, durable metadata and typed runtime
  event. Rebuild desktop, daemon and clients together.

The release checklist still tracks the additional harnesses, external solo bridges,
remote execution host, executed checks, monetary caps and PDF/image interpretation.

Verification: **162 Teams tests**, **14 distinct protocol/wire/projection tests**
and **38 TypeScript client/mobile runtime tests** passed. Generated bindings are
current. Shared-client and full mobile TypeScript checking, desktop/daemon checking,
Rust formatting and whitespace gates passed. Mobile verification reused installed
workspace dependencies with a temporary mapping to this worktree's shared client;
no dependencies were installed. Tests exercise late follow-up delivery, exact
replay, restart/outbox recovery, decision/stop/deadline fences, pre-start context
bounds, actual native completion and preserved mobile history.

No live provider billing, app account mutation, cloud deployment, device/visual
acceptance or second development watcher was run. These checks establish this
increment; full Teams release development remains listed separately.

## Durable workflow/routine execution increment (2026-10-06)

Previous checkpoint: `5941901`.

Current protocol: **27**. Earlier version numbers below belong to their historical
checkpoints. Use the release checklist for current remaining development.

- Workflow stages have explicit saved teams and optional enforced human gates.
  The daemon pins the workflow and selected team/profile definitions. Actual
  Teams task IDs and the outbox are committed with stage start, so restart cannot
  create a second stage. Only real completion advances the sequence.
- Accepted prior report bytes become immutable bounded inputs under the next
  lead's read grant. A workflow shares overall model-call, token, assignment and
  deadline caps; unknown usage retains reservations. Coding stage workspaces
  remain individually isolated and reviewable.
- Saved routines stay paused; Enable schedule arms an elapsed UTC interval.
  Firing, next schedule and automation creation commit together. Missed intervals
  coalesce to the latest slot, and previous active/blocked runs cause recorded
  overlap skips. Pause affects future firings, while Stop fences future stages and
  durably propagates cancellation to the active team.
- Native forms select teams per stage and teams/workflows per routine. Run now,
  paged history, real stage task links and exact gate decisions reuse existing
  focused controls and virtualized lists. Dashboard/Inbox/Decisions share cached
  projections and global gate counts. Polling remains background and generation
  fenced; the owner uses its existing scheduler with bounded fair batches.
- Start retries retain the same ID across unknown transport outcomes. The native
  client distinguishes a completed daemon rejection from disconnect. An existing
  run is recovered; only a completed rejection plus confirmed absence clears the
  submission for correction. A timeout alone never allows another execution ID.
- Protocol **27** requires matching desktop, daemon and clients. Legacy unassigned
  drafts stay editable, and must receive explicit stage teams before running.

[Behavior, setup and review checklist](21-workflows-routines.md). Calendar schedules,
conditional graphs, stage retries, document gates, code integration, phone definition editing
and notification policies remain subsequent increments. Additional harnesses,
registered cloud execution and full monetary accounting remain release development.

Phone controls in this increment use the same owner commands. Task history now
links to Workflows & routines, with project filtering, saved instructions,
Run now, interval enable/pause, paged history and exact run/stage controls. The
phone stores a submission ID/revision before sending; unknown outcomes and
storage failures retain it across app restart. A completed rejection plus exact
absence permits correction. Concurrent taps share one request. Focus/foreground
and connection fences disable cached controls and prevent late navigation.
Definition creation/editing remains on desktop; no second phone scheduler exists.

The first broad Teams run passed 175 tests and timed out its two-stage runtime
fixture under heavy parallel filesystem contention. The same compiled test
passed alone in 24.59 seconds. The fixture now retains the existing 30-second
allowance per full team stage, with stage diagnostics and bounded test parallelism.
This changes the test wait, not the runtime scheduler. A separate late-owner
regression protects already-completed final stages without permitting late dispatch.

Verification: the broad Teams regression suite passed **176 tests** with four test
threads. After the separate late-owner correction, the final targeted automation
suite passed **13 tests**, including actual two-stage runtime execution and the new
late-completion fence. **5 control-store tests**, **20 native client tests**, **13
protocol wire tests** and **52 TypeScript client/mobile tests** passed. Generated
protocol bindings are current. Full mobile and shared-client TypeScript checking,
desktop/daemon checking, Rust formatting and whitespace checks passed. Mobile
verification reused installed dependencies through a temporary checked junction;
no dependencies were installed.

No live provider billing, app mutation, cloud deployment, phone/device acceptance,
visual test, second watcher or integration into `ion-prod` was performed. The review
checklist retains those acceptance items and the full release development gaps.

## Selected harness and registered remote worker increment (2026-10-06)

Previous checkpoint: `19ceaad`. Protocol remains **27**; no wire migration is
introduced by the private worker/operation ledger.

- Local Teams now use native Codex, OpenCode and Pi, alongside Nebius and Claude.
  Exact model metadata supplies conservative reservations. A one-use transport
  removes native tools and validates the complete provider receipt before the
  CLI receives it. Ion alone executes the bounded structured actions through its
  existing workspace, app, hierarchy and approval checks.
- Native CLIs run in isolated private roots with resource/plugin controls and
  contained process trees. OpenCode's auxiliary title/summary/compaction agents
  are disabled; explicit titles prevent a second provider call. OpenCode and Pi
  copy only the selected direct native account and exclude inherited key overrides.
- The versioned headless worker runs one selected harness completion in a pinned
  ConTree image. Host capabilities must match the exact harness/model and bound
  the actual native token limits. The owner sends bounded context and receives
  validated action/usage JSON. Host credentials are configured on that host;
  owner vaults, auth files and provider keys are not sent in the instance request.
- A durable remote ledger precedes POST and pins request, image, project and
  acknowledged operation. Stop/restart checks and cancels existing operation IDs.
  Cancellation acknowledgement does not release capacity; confirmed terminal
  state does. A missing spawn acknowledgement retains uncertainty and is never
  resubmitted or assumed stopped. Valid late usage can settle after Stop/restart
  without authorizing tools, new calls or resumed execution.
- Local fallback occurs on verified pre-execution readiness failure, with the
  same saved harness/model/grants. Once submission starts, uncertainty cannot
  create a second remote request or Local worker. Inference leases cover the
  bounded transport/cancellation wait and still clip to the run's deadline.
- Accepted HTTP sockets explicitly become blocking with bounded read/write
  timeouts. Windows inherits a listener's nonblocking mode; a delayed-request
  regression protects the transport from intermittent pre-request failures.

[Contracts, local/remote setup and live review](22-harness-adapters-remote-execution.md)
record the precise boundary. **The other nine catalog harnesses remain Teams
adapter development. Full remote files/commands, executed checks, resource/egress
and monetary controls, and GPU/ML provisioning also remain development.** The
current remote worker performs selected-harness completions; file/app actions
still execute on the owner through existing grants. This checkpoint is not the
complete planned Ion release.

Verification: the broad Teams regression passed **138 tests** (three optional
installed-CLI fixtures excluded). The final adapter/remote suite passed **24 tests**,
including installed **Codex 0.160.0**, **OpenCode 1.18.15** and **Pi 0.84.1** against
fixture-only loopback providers. The coordinator suite passed **124 tests**. A final
remote exit/receipt decoding test passed after accounting for optional integer
signal representations. Desktop/daemon checking, the Windows worker binary build,
Rust formatting and whitespace gates passed. The actual worker binary rejected an
invalid protocol with empty stdout and sanitized diagnostics. These suite counts
overlap; they are not an aggregate count of distinct tests.

Two cases in the first regression failed when Windows inherited the listener's
nonblocking mode. After the production/fixture fix, the complete regression and
final adapter suite passed. The native fixture deadline also covers cold CLI
startup, and failed fixture receipts now report only fake-account diagnostics.

No live provider billing, cloud instance submission, Linux image build/deployment,
app-account mutation, phone/device or visual acceptance, second development watcher
or integration into `ion-prod` was performed. Live target/account setup and the
remaining implementation contracts are recorded in the linked checklist.

## Oh My Pi and remote assignment loop increment (2026-10-06)

Previous checkpoint: `635be70`. Protocol remains **27**.

- Added Oh My Pi using its own native catalog/RPC, SQLite API credential and YAML
  model configuration. This is a separate adapter from Pi. Local dispatch,
  registered remote completion and supported restart recovery retain its exact
  provider/model. Direct OpenAI, Anthropic and Nebius API routes are supported;
  brokers, named profiles, overlays, custom model configurations and OAuth remain
  unsupported and block readiness.
- Native resource-disable flags, a private home/config/workspace, an OS/runtime
  environment allowlist and the one-request gate prevent native effects and
  additional inference calls. Model limits come from the exact bundled catalog.
  The native SQLite account is opened read-only and must be unambiguous.
- The official Windows v18.6.1 executable was downloaded into the ignored test
  cache and checked against the release SHA256SUMS. The actual executable ran
  against a fixture account/provider. Its initial model discovery request exposed
  a routing failure; pinning only the exact model's inference endpoint fixed it
  without allowing a second request through the gate.
- Fixed a real remote integration defect: the shared Teams loop previously required
  a Local attempt even when called by the remote adapter. Trusted Local and remote
  entry points now enforce their respective saved host. A complete fake-cloud
  assignment exercises the owner loop, one POST, exact remote identity, usage
  settlement, a durable generated-document receipt and blocker reporting. A Local
  adapter still rejects that sandbox claim before reserving or invoking a model.

Verification: **148 Teams tests passed**, including all four optional installed
Codex/OpenCode/Pi/Oh My Pi native executable fixtures, with fake credentials and
loopback providers. The earlier focused adapter/remote suite passed **29 tests**;
these counts overlap. Desktop, daemon and headless worker binary checking passed.
The final conservative `PI_CONFIG_DIR` validation was included in that build check.
Rust formatting and whitespace gates passed.

Six selected harnesses now have Teams completion adapters; **eight catalog harnesses
still require adapters**. Full remote files/commands, executed checks, resource and
egress controls, monetary limits, GPU/ML jobs and verified lost-ack recovery remain
implementation work. The remote worker currently returns actions for owner-side
execution. No live model billing, cloud submission/deployment, account mutation,
Linux image acceptance, visual test, second watcher or integration into `ion-prod`
was performed. See the adapter setup and release checklist for the remaining work.

## 2026-10-06 — bounded native DeepSeek Harness adapter

- Added a distinct native DeepSeek headless Teams adapter for published runtime
  0.1.5-rc.1, direct host API authentication and exact official V4 text model IDs.
  Readiness, Local dispatch, registered worker dispatch and recovery recognize it.
- Private empty-bundle composition contains no native effect producers. Version
  and complete configuration are checked before inference; the one-use gate pins
  the exact official Chat endpoint and model, strips tools, forbids retries and
  accounts complete provider usage. Native defaults are pinned explicitly.
- Verified the official Windows wheel SHA256 before extracting its executable
  into the ignored fixture cache. The actual binary completes against a fake key
  and loopback provider, with one request and no native tools. An initial test
  caught the published release's Chat route differing from newer Messages source;
  the adapter now matches the actual supported release.

Verification: the complete Teams suite passed **150 of 151 tests**; the installed
Pi fixture timed out during help startup. It failed once more in isolation, then
read-only native Node/shim help probes and the exact isolated Pi fixture passed.
The initial startup cause was not established; no timeout or permission bound
was relaxed. The three DeepSeek tests, including its actual native fixture, passed.
Desktop, daemon and headless worker binary checking passed. These checks use
fixture credentials and localhost, with no live provider or cloud execution.

Seven selected harnesses now have bounded Teams completion adapters. Remaining:
Antigravity, Amp, Cursor, Fx, OpenCode 2, Grok and Kimi, plus unverified OAuth/custom
transports. Full remote files/commands/GPU jobs and deployment acceptance remain
implementation work. The current sandbox worker executes completion only.

## 2026-10-06 — verified sandbox operation recovery and continuation

- Persist the complete bounded worker input privately before a sandbox POST. A
  second preparation for that call is refused, including an uncertain submission.
- Recover a lost operation acknowledgement only after the official API metadata
  matches the exact saved request, image, project, model, input, tool contract and
  process limits. UUID entry alone supplies no process or usage proof. Legacy
  submissions without the original saved input cannot be bound automatically.
- Native Review recovery shows operation identities and provides Verify and bind
  operation and Verify sandbox stopped. Cancellation acknowledgement does not
  release an unresolved operation; a terminal API state is required. Private
  inputs, outputs and credentials stay outside the recovery projection.
- Explicit preparation and continuation preserve the same attempt, selected
  harness, model, grants and actual execution environment. A recovered remote
  attempt cannot switch to Local. Unknown prior model usage remains reserved;
  cached actions and uncertain submissions are never replayed.
- Protocol 28 includes the remote recovery identities and confirmations. Generated
  client types and the existing request fixture are updated together.

Verification: **151 Teams tests passed**, with five optional installed-runtime
fixtures ignored in this regression run (those native contracts were checked in
previous adapter checkpoints). New cases cover changed metadata/input/model/access,
wrong image, stale revision, terminal process proof, confirmation replay, canceled
runs, missing legacy input and duplicate POST prevention. The resumed owner loop
creates one new request and document receipt while retaining the original unknown
budget. The protocol recovery roundtrip passed; the TypeScript client check and
**22 client tests** passed. Desktop, daemon and worker binary checking passed.
Formatting and whitespace gates passed. No visual test or live provider/cloud
request was performed; the app watcher and primary ion-prod checkout were untouched.

Remaining implementation: the seven other native Teams adapters, full remote
filesystem/commands and executed checks, resource/egress and monetary controls,
and GPU/ML jobs. Registered Linux image acceptance and real credentials/deployment
remain later user setup and review items in the release checklist.

## 2026-10-06 — bounded native Kimi Code adapter

- Added the verified native Kimi Code 2.1.1 adapter through Local readiness and
  dispatch, registered sandbox completion and both recovery checks. Exact coding
  model identities, official route and published native context limits are
  required. Renaming an alias cannot replace its selected underlying model.
- Read only the selected native direct account; support one literal key or its
  native key environment reference. OAuth, alternate headers/routes and model
  overrides remain unsupported. Source account configuration is never modified;
  malformed configuration errors cannot expose credential values.
- Create private tool-free/subagent-free native agent and configuration files,
  with no key in either. Disable auxiliary/background work, retries, updates,
  telemetry, cron, watchers, skills, hooks, MCP and plugins. Anchor project
  discovery to the private scratch workspace, including under an ancestor repo.
- Keep task JSON literal through native prompt templating. Bound raw and escaped
  input, native process output, wall clock and process tree. The one-use Chat
  gate enforces the exact model and positive completion cap before any upstream
  request, rejects alternate caps and retains complete metered usage.
- Verified the official Windows release archive SHA256:
  `325a42e7954854d4f737199b331bf3c05622e4a5e71bb74dd31a87df0400b92e`.
  The runtime and exact release source are cached only in the ignored fixture
  directory. No global install, native login or user account mutation occurred.

Verification: **three Kimi tests passed**, including the actual native executable
against a fake key and loopback provider, with one request, no native tools, exact
model/output cap and an invalid ancestor configuration trap. **Five shared gate
tests passed**. The final Teams regression suite passed **154 tests**, with six
optional native fixtures ignored; the Kimi fixture was explicitly run separately.
These counts overlap. Desktop, daemon and headless worker binary checking passed;
formatting and whitespace gates passed. No live provider billing, cloud operation,
Linux image acceptance or visual app test was performed.

Eight selected harnesses now have bounded Teams completion adapters. Remaining:
Antigravity, Amp, Cursor, Fx, OpenCode 2 and Grok, unverified OAuth/custom transports,
external solo-harness team bridges and full remote files/commands/GPU execution.
Later native/cloud acceptance steps are in the adapter guide and release checklist.

## 2026-10-06 — bounded native Grok Build adapter

- Added native Grok Build 1.0.46 to Local readiness/dispatch, registered sandbox
  completion and both recovery paths. Only exact `grok-4.6` and `grok-4.5`, the
  official direct API route and a compatible host-owned native key are supported.
  Cached/inline/OAuth/managed accounts, auth commands, custom model/route/header
  configuration and native login-policy overrides are refused before inference.
- Inspect the exact native version and effective private configuration before
  launch. Restrict environment inheritance and child credentials; exclude native
  skills, hooks, plugins, MCP, indexing, updates, telemetry, retries and background
  work. The private repository prevents discovery of any ancestor source checkout.
- Disable native connection prewarming. The one-use Responses gate refuses
  automatic title/auxiliary requests locally and forwards only the exact saved
  Teams instructions/input. Native context and tools are removed. The model and
  32,768 output cap are verified before billing; full 500,000 input tokens are
  reserved, with complete provider input/output usage required for completion.
- The official Windows runtime reports `1.0.46 (2765805b9442)`. Its npm SHA512
  integrity matched the published metadata, and the executable SHA256 is
  `47ad7fbc51767b8b301714111a1e452acc2c89912c08be260e1779a383302a4c`.
  Source snapshot `2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8` is newer than
  that native release; runtime acceptance relies on the actual binary fixture.

Verification: **three Grok tests passed**, including the actual native executable
against a fake key and localhost provider. It made one upstream request with
canonical literal input, no native tools, the pinned model/output cap and complete
usage. The initial fixture exposed an unframed native prewarm GET; source-backed
prewarm disabling resolved it. Missing native Responses fields and the final SSE
blank line were repaired in the fake provider fixture. No framing relaxation was
retained. **158 Teams regression tests passed**, with seven optional installed-CLI
fixtures ignored; the new Grok native fixture was explicitly run separately. These
counts overlap. Desktop, daemon and worker binary checking passed, as did formatting
and whitespace checks. No live provider billing, cloud operation, global install,
account modification, Linux image acceptance or visual app test was performed.

Nine selected harnesses now have bounded Teams completion adapters. Remaining:
Antigravity, Amp, Cursor, Fx and OpenCode 2; unverified OAuth/custom transports and
external solo-harness team bridges. Full remote project files, commands/executed
checks, resource/egress and monetary limits, and GPU/ML jobs remain development
work. The sandbox worker currently performs selected-harness completion only.
Setup and later acceptance are recorded in the adapter guide and release checklist.

## 2026-10-07 — Antigravity, OpenCode 2 and Fx adapter implementations

- Added private native Antigravity Gemini and OpenCode 2 standalone completions,
  with selected host-owned accounts and exact supported models/releases. Wired
  Local readiness/dispatch, registered worker execution and both recovery paths.
- Added Fx 0.0.13's configured direct OpenAI/Nebius ask contract and the same
  execution wiring. Windows Local readiness explains its Linux/macOS requirement.
  Official native Linux/macOS acceptance is still outstanding.
- Added canonical bounded transport input/output. Native context/history and tools
  are removed before a single authenticated provider call. Google title requests
  are rejected locally; native function/code/file effects never reach the client.
  Usage must include thinking and stay within the reserved call.
- Added Antigravity model discovery in a bounded background process, preserving
  exact low/high native slugs. Normal New Task/team invocation behavior is intact.
- Research found remaining Amp/Cursor native contract gaps. Their specific
  readiness errors retain the selected harness and stop before an attempt. The
  adapter guide records current official references and outstanding work.

Verification before the final small profile/timeout fixes: 166 targeted Teams
regression tests passed, 10 optional native fixtures ignored. OpenCode 2 and
Antigravity native fixtures were run separately with fake keys/localhost and
passed. Running them concurrently with filesystem-heavy regression tests exposed
fixture startup contention; their fixture timeout now matches the bounded native
startup. Antigravity's self-updating public fixture also exposed a patch release;
1.3.1's official SHA512 manifest was verified before extending acceptance.

The official OpenCode 2 npm integrity matched; executable SHA256:
`d24907b190298e03bd5a06a6d0a47767878660948db40e7040d3ea7df7ec7ae6`.
Antigravity 1.3.0 SHA256:
`c1b0001989c4051a41f5104484de182db5014fa155053c8088baacf1663063bf`;
1.3.1 SHA256:
`38f30c7dd1ed808f5cf98fe2014de3d30903035a4f0df02d3eb72a9ff8993741`.
Fx official v0.0.13 source and Linux release were cached for inspection. Public
binaries/source remain only in the ignored cache; no global installation or
account modification was requested. One initial OpenCode endpoint-schema fixture
failure reached the official provider with a fake key and received 401; the
compatible schema corrected it. No live billing was performed. No cloud operation,
full remote command/GPU execution, visual test or second watcher was performed.

Twelve Teams adapters are implemented; eleven have native transport/CLI fixture evidence.
Fx native acceptance, Amp/Cursor adapters, additional account transports and full
remote files/commands/GPU execution remain outstanding. See the release checklist
and adapter guide for development and later live-review gates.

Final verification after those fixes: **19 focused checks passed**, including
both actual native OpenCode 2 and Antigravity 1.3.1 executable fixtures. The checks
cover canonical Chat/Responses/Gemini requests, complete usage and thinking,
rejected effects/identity changes, private account/profile contracts and native
model discovery. The Fx installed-binary fixture remains ignored pending a
compatible Linux/macOS host. These counts overlap the earlier 166-test regression
run and are not an additional count of unique regression tests. Final desktop,
daemon and worker binary checking, formatting and Git whitespace checks passed.
No live account acceptance, cloud operation or visual test was performed.

## 2026-10-07 — bounded command/file execution and GPU lifecycle transport

- Added a separate credential-free Linux command worker and operator image
  registry. Actual Nebius attempts require command/read grants; Local/fallback
  attempts cannot submit cloud commands. Explicit file bundles, executable/time/
  count/layer limits, nonroot command execution under a separate trusted supervisor and disabled instance network are enforced.
- Commands execute with literal argv and a private home/environment. Real exit,
  timeout, bounded logs and selected binary output bytes return in versioned
  receipts; owner-side SHA256 verification rejects mismatched files.
- Added durable request reservation before POST, original operation binding,
  cancellation/terminal observation and restart GET/DELETE reconciliation. Unknown
  acknowledgements remain reserved and block continuation; no repeat or fallback.
- Receipt and generated JSON artifact commit together, including late/recovered
  outcomes. Native run/read tools and argument/provenance-bound journal recovery
  reuse those exact receipts. Output reads are assignment-scoped, 4 KiB pages;
  no source checkout write or implicit repository upload occurs.
- Added Linux child subreaping and bounded descendant collection for detached
  process groups. The Windows worker fixtures do not verify that Linux behavior;
  imported-image/native acceptance remains in the checklist.
- Started the separate Nebius Serverless AI Jobs transport: pinned image digest,
  explicit resource fields and injected request, create operation vs job identity,
  provenance verification, status and cancellation. It is not exposed for agent
  launch until durable waits, outputs and monetary/resource authorization exist.

Final verification: **31 Rust fixture/regression tests passed** (8 command jobs,
2 GPU API contracts, 15 sandbox/recovery checks and 6 owner recovery checks), plus
**4 Python worker tests passed** with real private Windows child execution. Desktop
and daemon checking, locked dependency resolution, formatting and whitespace checks
passed. Test code generation used a command-local core optimization override;
repository/app build profiles were not changed. An initial restart fixture used a
nonexistent ConTree `RUNNING` state; it was corrected to the verified `EXECUTING`
contract before final acceptance. No live provider billing, cloud submission,
resource deployment, account modification, app visual test or second watcher ran.

Remaining development: command lost-acknowledgement operator binding/recovery
projection; enforced executed-check output gates; full remote project/large file
support; GPU durable waits, output/volume handling, spending/egress/resource and
cleanup integration. Amp/Cursor Teams contracts and Fx native Linux/macOS fixture
acceptance remain outstanding. See [the remote job guide](23-remote-command-gpu-jobs.md)
and [release checklist](20-agent-teams-release-checklist.md). These are development
items, not merely later user review.

The final worker hardening uses a separate trusted supervisor and nonroot command
UID/GID, no supplementary groups and inherited no-new-privileges, protecting the
receipt collector from command-side signalling or native privilege elevation.
Command/GPU fixtures and desktop/daemon checking were repeated after the contract
change; Linux native acceptance remains outstanding.

## 2026-10-07 — command recovery binding and durable GPU reservations

- Review recovery now distinguishes command operations and model completions.
  Lost command acknowledgements can be bound only after verifying the complete
  original command/input/policy witness against the operation. The current run
  revision and attempt are fenced again when the binding commits. Repeating an
  accepted confirmation uses its saved receipt without another request.
- The real command receipt and artifact survive terminal observation, Prepare
  continuation and the user's Continue action. Native journal recovery matches
  the original arguments and never repeats the command POST. Protocol **29** adds
  the purpose label; legacy records default to model completion. No command/file/
  credential content is projected publicly.
- GPU launch now reserves its immutable request/body, selected target, private
  owner-approved currency/quote/expiry/spending allowance, count/concurrency and
  original team deadline before create. Exact replay returns its saved state.
  Lost acknowledgement retains the reservation and cannot invoke Local fallback.
- GPU Stop/restart observes the same bound job and claims cancellation before its
  single POST. It records creation/job/cancel identities separately and accumulates
  compute IDs. Terminal status and cancellation acknowledgement keep all spending,
  resource and continuation reservations. Unchanged observations do not increment
  revisions or append audit events on every poll. Stop attempts all resource
  classes before returning an error from an unavailable credential context.
- Agent GPU launch remains gated. Approved resource/subnet/volume policies,
  provider price/billing reconciliation, asynchronous native waits, authentic
  output receipts, cleanup proofs and GPU operator recovery remain development.
  Owner-approved launch allowances are not provider invoice totals or a verified
  ceiling on billing after an uncertain cleanup.

Verification: **37 core tests passed** (10 command, 6 GPU, 15 sandbox/recovery,
6 owner recovery), **1 protocol compatibility/privacy test passed**, and **22
client tests passed**. Client TypeScript checking passed using the existing
compiler/types from the primary checkout because this worktree has no `tsc`
installation. Focused test code generation used command-local optimization
settings for core/protocol/client; app profiles and dependencies were unchanged.
The initial command fixture omitted Queue continuation after Prepare; it was
corrected to follow the real user action before final acceptance.
Final desktop/daemon checking, formatting and Git whitespace checks passed. The
GPU identity/stable-observation checks and sandbox owner-loop tests were repeated
after the final safeguards; those runs overlap the counts above. Existing core
warnings remain unchanged.

No live account, provider billing, cloud submission/deployment, app visual test or
second watcher was used. Amp/Cursor contracts, Fx native acceptance, executed-check
gates and full remote project/large transfers also remain development. The full
goal is active; this checkpoint does not mark it complete.

## 2026-10-07 — GPU resource inventory and cleanup proofs

- Added durable VM/disk inventory and an immutable cleanup snapshot. The owner
  verifies project/platform/preset, desired disk attachments and observed disk IDs.
  Complete inventories cannot silently change. Missing startup/disk witnesses and
  unexpected filesystem attachments keep the resource reservation pending.
- Deletion is claimed before its single DELETE of the original AI job. Creation,
  job, cancellation and deletion identities remain separate. The transport never
  directly deletes a VM/disk and never repeats a lost deletion acknowledgement.
- Recovery checks the typed successful DeleteJobRequest and structured absence of
  the job, every captured VM and every disk before committing cleanup. Pending/
  failed operations, HTML 404s, permission failures and remaining resources retain
  the reservation. Verified cleanup releases the project compute slot and supplies
  host Stop evidence; command outputs, money and continuation remain independently
  fenced. Cleanup never manufactures a result artifact or billing receipt.
- Stop attempts all provided job records before reporting one observation failure.
  An inventory permission failure still allows cancellation of the original bound
  job. Resource checks execute on the existing owner worker path, not in rendering.
- The release checklist and remote-job guide include later live cleanup review and
  the remaining missing-inventory/lost-delete recovery work. Authentic GPU output
  retrieval and native waits remain development; job state does not contain a
  command receipt, and no speculative job-logs REST endpoint was added.

Verification: **45 focused core tests passed** (14 GPU, 10 command jobs, 15 sandbox
lifecycle/recovery and 6 owner recovery). Desktop/daemon checking, formatting and
Git whitespace checks passed. Test compilation used the same command-local core/
protocol/client optimization settings as the previous checkpoint. The first new
capacity fixtures tried to launch another effect within an unresolved run; they
were corrected to independent authorized runs in the shared project, preserving
Ion's pending-effect fence. One HTTP-fixture assertion was corrected to use the
already parsed JSON body. Existing warnings remain unchanged.

No live account, provider billing, GPU/cloud submission, resource deployment,
app visual test or second watcher was used. GPU output/volume/wait/price/egress and
operator recovery integration, Amp/Cursor Teams contracts, Fx native acceptance,
executed-check gates and full remote project/large transfers remain development.
The full goal remains active.

## 2026-10-07 — authentic GPU result transport

- Added an immutable optional result-volume policy to the private GPU launch
  allowance. It pins bucket ID/name, region and prefix. Before create, the owner
  verifies the same project, regular bucket type, active/unsuspended state and
  absence of anonymous access. Exact launch replay performs no bucket read/POST.
  Unsupported or changed volume policy fails before submission. Older no-volume
  reservations preserve their original signature and cannot acquire a result
  volume during replay.
- The isolated root supervisor keeps `/ion` private from its nonroot command,
  creates a fresh request UUID directory on the mount before execution, then
  exclusively writes/flushed-syncs the real receipt after descendants stop.
  Repeated/partial writes never rerun commands or overwrite an earlier receipt.
  The CPU JSON stdin/stdout contract remains unchanged. GPU collector success
  does not conceal the command's real nonzero exit.
- Added a fixed-origin HTTPS S3 GetObject reader with owner-only access/secret and
  optional session credentials. Secrets are not supplied to commands, images,
  artifacts or upstream-error diagnostics. Redirects and arbitrary URLs are
  unavailable. The signer matches independent AWS botocore 1.40.0 S3SigV4Auth
  vectors, including session tokens. JSON is capped at 512 KiB and every requested
  output's exact size/hash and the original request contract are verified.
- Result receipt and generated summary artifact commit atomically. Exact replay
  retains the artifact and revision; altered receipts cannot replace it. Results
  survive owner restart and support assignment/attempt-scoped 4 KiB binary pages.
  Collection is independent of Stop/resource cleanup and never releases money or
  task continuation. A missing object, malformed receipt or denied read remains
  unresolved. Agent GPU launch stays gated until the remaining integrations.
- Updated the worker/setup/review guides and corrected the older command
  lost-acknowledgement note to match its implemented verified operator binding.

Verification: **52 focused core tests passed** (21 GPU, 10 command jobs, 15 remote
lifecycle/recovery and 6 owner recovery), plus **6 Python worker checks** with real
child execution. The result tests cover authentic failed-command artifacts,
binary hashes/readback, restart/exact replay, altered contracts, another
assignment, early/unapproved receipts, missing/auth/redirect/partial/malformed/
oversized storage reads, and changed bucket/privacy/grant policy. HMAC was already
locked transitively; only the waku-core direct dependency was added to Cargo.lock.
Desktop/daemon compilation, Rust formatting and Git whitespace checks passed;
existing compiler warnings were unchanged.

No live GPU/account/billing/cloud deployment, app visual test or second watcher
was performed. Linux/FUSE result isolation/exclusive-write/flush semantics, GPU
image acceptance and bucket IAM/retention policy still need acceptance. Durable
waits and agent integration, verified billing/resource/egress enforcement, GPU
operator recovery, large project/dataset transfers, Amp/Cursor Teams contracts,
Fx native acceptance and executed-check gates remain development. The full goal
remains active.

## 2026-10-07 — durable GPU waits and native resume

- Added a durable GPU-wait record and native journal continuation for an already
  approved, bound, observed request. Unknown acknowledgements cannot be parked.
  The original Nebius attempt and command/read/network grants are required;
  unrelated unfinished effects remain fenced. No GPU submission tool was enabled.
- Suspension revokes the lease generation and releases the shared agent slot.
  The owner observes on independent background threads, with four concurrent
  polls and a five-second minimum cadence per job. The poll map is bounded to
  256 entries. Ordinary polling does not cancel; Stop or the original deadline
  claims cancellation once and retains uncertainty until resources are stopped.
- Runtime settlement requires both an authentic command receipt and verified
  cleanup. It clears the runtime effect fence, retaining the complete monetary
  reservation. The native adapter resumes once with the same task, attempt,
  host, harness, model, grants and budget history. It receives the actual exit,
  timeout and artifact context, and can read requested output pages. The tested
  nonzero command remains failed while the agent completes its failure analysis.
- The saved-proof path resumes without a new cloud request or environment
  credentials. Old lease generations remain unusable and exact settlement replay
  does not churn revisions. An active GPU effect still blocks other work in its
  own run; the released agent slot is available to independent runs, not parallel
  GPU work in the same team run.
- Updated worker, adapter and review guides. A canceled run can still use the
  owner collector, but persistent automatic post-cancellation collection is not
  implemented. GPU submission remains gated on verified monetary/resource/egress,
  image and readiness policy, independently of the completed wait mechanism.

Verification: **196 distinct core tests passed** across overlapping focused runs:
27 GPU checks (including the owner polling fixture), 105 coordinator checks,
23 native adapter checks, 18 daemon checks, 10 command checks and 15 sandbox checks.
The owner fixture holds real saved GPU waits and verifies the dispatcher remains
available, concurrency is bounded and repeated polling cannot duplicate a held
request. Desktop/daemon checking, Rust formatting and Git whitespace checks passed.
Protocol remains **29**; the wait and receipt context are private owner state.

Two new fixture failures were corrected: launch was moved after the original
model-call reservation rather than bypassing the pending-effect fence, and an
interrupted workspace row replaced an invalid foreign-key app row. An existing
decision test asserted obsolete prompt wording; it now verifies the actual tool
grants after approval, and the complete native suite passed. The final test build
emitted six existing warnings. Normal checking emitted eleven existing warnings
plus one unused retained GPU cancellation-identity warning; ts-rs and the existing
future proc-macro diagnostic remain.

No live GPU/account/billing/cloud deployment, app visual test, second watcher or
provider credential setup was performed. Remaining development includes GPU
submission policy/verified pricing and accounting/resource and egress enforcement,
operator recovery and persistent post-cancellation collection, full remote
project/dataset/large-file transfers, enforced executed-check receipts,
Amp/Cursor Teams contracts and Fx native Linux/macOS acceptance. The full goal
remains active; this is an implementation checkpoint.

## GPU original-operation recovery increment (2026-10-07)

- Protocol **30** adds bounded GPU creation/cancellation/deletion identities to
  recovery review and a typed confirmation carrying native provider ID strings.
  Client bindings were regenerated and checked against the exporter.
- The owner verifies a candidate using fixed-origin bounded GETs, the original
  idempotency key and exact typed request. Creation compares saved intent and
  verifies immutable injected bytes plus fixed environment values through the
  same job's SECRET read. Known sensitive history leaves/defaults are handled
  without dropping paths, names, mounts or unknown capabilities.
- Atomic binding rechecks run revision, current attempt, saved request signature
  and original mutation claim. It records compute identities seen during create
  recovery. Exact confirmation replay does not repeat network access or submit
  work. Binding retains money and never proves successful work or cleanup.
- Native recovery rows are prepared off frames and use the existing virtualized
  panel and keyboard controls. Each phase has its own identity and binding action.
  Settled/cleaned jobs do not produce stale lost-cancellation recovery notices.
- Missing operation/key/SECRET evidence remains unresolved. Historical missing
  VM/disk inventory recovery, enabled GPU submission, resource/egress/monetary
  integration, large transfer volumes and live acceptance remain development.
  Amp/Cursor and the broader adapter contracts remain open in the release guide.

Verification: **35 GPU tests**, **124 coordinator tests**, **1 protocol test**, the
protocol exporter check, desktop/daemon `cargo check`, formatting and whitespace
checks passed. Existing ts-rs, dead-code and proc-macro warnings remain. No live
account/cloud request, app bundle, second watcher or visual test was run.
See [the recovery contract research](RESEARCH-nebius-gpu-recovery.md) and
[the review guide](23-remote-command-gpu-jobs.md#original-gpu-operation-recovery).

## Executed remote command check gates (2026-10-07)

- Protocol **31** adds `requiredCommands` to the immutable output contract and
  `executedChecks` to assignment results. Empty extension fields are omitted so
  existing stored delegation/result replays remain compatible. Generated client
  bindings were regenerated and checked.
- Leads pin exact names, literal arguments, ordered explicit input/output paths
  and timeouts before delegation. The complete contract is bounded to 32 KiB and
  16 commands. Existing descriptive checks remain observations.
- Native `report_result.executed_checks` references original trusted owner command
  receipts from the same run/assignment/current attempt. Submission and parent
  acceptance both reject missing, foreign, old-attempt, failed, timed-out, canceled
  or output-error receipts, and any command/file/time mismatch.
- Before bundling, the owner captures the sealed isolated workspace and bounded
  edit epoch; it rechecks that context before reservation. Review verifies current
  scoped regular input bytes against the original immutable bundle. Changed source
  or owner edit history invalidates the receipt. Rework retains all requirements
  and must obtain new execution evidence for the correction.
- Owner workers perform verification. Bounded child handoffs preserve complete
  receipt names/IDs and report omissions; sixteen escaped handoffs remain within
  the 32 KiB context cap. No source I/O is added to UI frames.
- Native delegation/report parsing is covered through a real model tool loop.
  Forged Local claims do not complete the assignment. The existing shared-client
  recovery fixture now includes the required GPU recovery field exposed by the
  previous protocol revision.

Verification: **170 distinct core regressions** passed (127 coordinator, 19 command
and 24 native tool tests), including 13 new command-contract/source/review/native
cases. **22 client tests**, shared-client TypeScript checking, generated-binding
checking, desktop/daemon `cargo check`, formatting and whitespace gates passed.
TypeScript reused the primary checkout's existing compiler/types; no dependency
installation was needed. Existing ts-rs, dead-code and proc-macro warnings remain.
Initial fixture compile/path assertions were repaired before the final gates.
No live account/cloud request, app bundle, second watcher or visual test was run.

These gates prove the explicit remote file bundle at submission/acceptance, not
unlisted source, later edits or an entire project checkout. Local/fallback shell
checks, GPU training/metric gates, full remote project/dataset transfers, verified
prices/billing/resource/egress controls, GPU agent submission and post-cancellation
collection remain development. Amp/Cursor contracts, Fx native Linux/macOS
acceptance and live provider/cloud review remain open. The full goal stays active.
See [executed command requirements and review](23-remote-command-gpu-jobs.md#executed-command-checks).

## Persistent GPU maintenance and provider timeout correction (2026-10-07)

- The owner now polls original bound GPU jobs after terminal/blocked assignments
  and canceled runs leave the normal dispatch index. Previously saved jobs are
  adopted without migration-generated cloud requests or model execution.
- A durable token/expiry fences maintenance callbacks. Retry schedules survive
  owner restart with five-second to five-minute exponential backoff. An abandoned
  claim can be reclaimed after ten minutes; an older callback cannot overwrite
  the newer schedule. Unknown/unbound create requests remain unresolved.
- Maintenance shares the existing four-worker GPU wait pool, excludes busy work
  and runs provider I/O outside dispatcher/state locks. The actual owner tick is
  tested with five canceled runs and held provider workers: four start, a repeated
  tick starts no duplicate, and the fifth uses the next released slot.
- The original request and attempt are checked before settlement. Result reads
  target one original receipt, not every job in an attempt. Cleanup and approved
  result collection are attempted independently. Missing credentials do not
  invent success, and cleanup permission failure does not discard a valid receipt.
- A late nonzero receipt produces one durable artifact after cancellation/restart.
  Verified receipt plus cleanup closes the runtime fence, while the task stays
  canceled and all monetary reservations remain held. Saved settlement replays
  without credentials or provider calls. A foreign attempt cannot settle the job.
- Nebius documents a one-hour minimum job timeout. New launch/body/reservation
  validation now requires 3600–86400 seconds, preserving Ion's 24-hour cap,
  independent short command limits and the original team deadline. GPU fixtures
  use a valid one-hour provider lifetime with enough pinned team time. No team
  limit is silently expanded. See the cited launch research for provider details.

Verification: **187 distinct core regressions** passed (43 GPU, 130 coordinator,
19 runtime, with 5 overlaps), including **8 new maintenance/timeout cases**.
Desktop/daemon `cargo check`, formatting and whitespace checks passed. Two test
fixture assumptions were repaired before the final gates. Existing ts-rs,
dead-code and proc-macro warnings remain. No live account/cloud request, app
bundle, second watcher or visual test was run. Protocol/client interfaces did
not change in this increment.

Remaining development: enabled GPU agent submission, verified price/account/
billing reconciliation, image/quota/readiness/resource/egress enforcement,
historical missing-inventory recovery, large project/dataset/checkpoint transfers,
Amp/Cursor Teams adapters and the other adapter transport/bridge contracts.
Native Linux/macOS and live provider/cloud acceptance remain review gates.
The full goal stays active. See [maintenance and later review](23-remote-command-gpu-jobs.md#persistent-gpu-maintenance-after-task-cancellation)
and [GPU launch policy research](RESEARCH-nebius-gpu-launch-policy.md).

## Exact GPU resource policy and readiness (2026-10-07)

- New production owner launches require exact immutable tenant/project/region,
  network/subnet, image, platform/preset and GPU/vCPU/RAM/disk/shared-memory
  approval, with separate cumulative project and tenant quantity limits.
- Fixed-origin bounded native reads verify active project identity, exact preset,
  ready subnet/network, both applicable quota scopes/units/known usage and fresh
  generic capacity advice. Ambiguous, fabric-specific, stale or incomplete advice
  is rejected. Provider availability remains a snapshot, not a guaranteed allocation.
- A sealed request/body/policy receipt saves sanitized read queries/hashes,
  quota headroom and capacity identity/time in the same launch transaction before
  the single POST. It cannot be reconstructed from model input. Reservation checks
  a 30-second freshness window; all provider reads must complete within two minutes.
- Atomic cross-run reservations enforce the stricter outstanding project/tenant
  quantity limits and conservatively reduce fresh headroom by all uncleaned owner
  allocations. Matching capacity-advice headroom is also reserved atomically, with
  measurement age checked again at reservation. Unknown historical quantities
  block new allocation. Terminal/cancel states and restart retain capacity; verified original VM/disk
  cleanup releases quantities while money and missing outputs remain independent.
- Eligible original launch replay needs no new readiness reads or POST, preserving
  saved resource identities and original recovery/maintenance. Legacy policy
  signatures omit the new optional field; existing recovery remains compatible.
- Updated the launch research, adapter guide and release/live-review checklists.
  No additional cloud grants, model routing or environment selection were added.

Verification: **195 distinct core regressions** passed (51 GPU, 130 coordinator,
19 runtime, with 5 overlaps), including eight new resource/preflight test groups.
Tests cover request fidelity, changed approvals/native fields, missing/stale/
ambiguous/paginated advice, concurrent cross-run caps, tenant/quota headroom,
legacy uncertainty, restart, original cleanup releasing capacity and retained money.
Desktop/daemon checking passed. A new unused test-only re-export warning was
removed; final desktop/daemon compilation, formatting and whitespace checks passed. Existing ts-rs/dead-code/proc-macro warnings remain.

No live account/cloud request, credentials, deployment, bundle, second watcher
or visual test was used. GPU agent submission, provider prices/account/billing,
child egress and GPU image enforcement, missing historical inventory, large
project/dataset/checkpoint volumes, retention policy and live acceptance remain
development. Amp/Cursor and the remaining adapter account/transport/bridge contracts
remain open. The full goal stays active.

## Trusted Linux child socket boundary (2026-10-07)

- Added a fixed root-owned isolated Python launcher between child demotion and
  user exec. It verifies explicit nonroot identity/empty capabilities, trusted
  libseccomp and exact image-approved kernel, sets no_new_privs, rejects socket,
  io_uring, namespace/theft/ptrace/BPF and compatibility-ABI escapes, and closes
  inherited/launcher descriptors. Threads/fork/exec inherit the filter.
- The root result collector stays outside it. Missing library/unapproved kernel
  stops before user code with an authentic failed receipt. Worker JSON version 1
  and result paths/hashes/exclusive writes remain compatible.
- Image builds require a pinned base, exact signed libseccomp version and accepted
  kernel release list. New GPU owner preflight requires the immutable image's
  `child_network` approval before reads/reservation. Legacy missing fields retain
  exact stored signatures and original-operation replay/recovery, but reject new
  allocation. Agent GPU submission remains gated.
- Started the already-installed Docker Desktop hidden after confirming it was
  unavailable. Existing user images were preserved; disposable test containers
  were removed. No host credential or cloud resource was used. The local test
  image remains cached for continuing native Linux harness acceptance.

Verification: **52 GPU Rust regressions**, **nine real Linux guard CLI tests**
and **six worker tests** passed. The guard suite passed both normally and with
Docker outer seccomp disabled on x86_64 kernel
`6.18.40.1-microsoft-standard-WSL2`; the six worker checks also passed on Windows.
Test image used immutable `python@sha256:05cda9777409a9c3ffddd94a4c476b79f0769a0b4857f0c7ed9226b6800b0d6f`
and signed `libseccomp2=2.6.0-2`. Linux tests cover domains/raw syscalls, inheritance,
x32, descriptor/env closure, missing support and private authentic failed/exclusive
receipts. A fixture timestamp and nested script quoting were repaired before the
final checks. Desktop/daemon checking passed with existing ts-rs/dead-code/
proc-macro warnings; formatting and whitespace gates passed.

This is socket-mechanism evidence, not complete GPU isolation. Provider image/
device/CUDA acceptance must exclude RDMA/NIC/VFIO, connected handles and network
filesystem/IPC/file proxies. Full GPU launch, verified price/account/billing,
historical missing inventory, large project/dataset/checkpoint transfer, Amp/
Cursor contracts and other adapter transport/bridge contracts remain development.
Fx native Linux binary acceptance is now feasible locally and remains pending.
No live provider/GPU/deployment, app bundle, visual test or second watcher was run.
The full goal stays active.

## Fx official Linux fixture and native worker build (2026-10-07)

- Reverified the cached official Fx 0.0.13 Linux x86_64 executable against GitHub's
  release archive and asset SHA256. The native binary reports the exact accepted
  version. It ran Ion's existing Rust adapter fixture with a private home/work
  directory, fake key and localhost provider, without external networking.
- The fixture verifies the actual launch/profile plus bounded gate, one canonical
  Chat request, exact model/task, no native tools and the 32,768 output clamp.
  Three Fx tests, twelve transport-gate regressions and one worker protocol test
  passed on Linux (**16 distinct Rust tests**). Transport cases reject native
  effects, changed identities, excess usage and auxiliary/retry billing.
- Built the actual Linux `ion-harness-worker` debug executable offline, with the
  unchanged lockfile and tracked source `7106c17`. Three actual-binary checks
  rejected arguments, malformed input and oversized stdin with expected exit
  codes, no stdout/input echo and bounded sanitized diagnostics. This does not
  claim authenticated inference or a production release/deployment image.
- Twelve catalog adapters now have native transport/CLI fixture evidence. Fx's
  former missing-Linux-binary gate is closed; Windows Local remains unavailable,
  and macOS/live selected-account/registered-image acceptance is separate.

Pinned build base: `rust@sha256:2775a09d208ff0d7c1f50490c45b62db929e87ba1dcbc3f2132ac71a704bcdd3`
(Rust 1.97.1, Linux x86_64). Fx archive SHA256:
`53a3b30ce1541048f8fa4b42b7bc68161466bb2e3708db104449a58af3807430`;
executable SHA256: `00def23f9c68ef538d7d967394c3491910793461884fad0b983cb74011b9120c`.
The fixture image compiled the actual core test library with `--locked`, without
manifest edits or alternative dependency resolution. Public release/build assets
remain in ignored caches; disposable test containers were removed. One local
Docker base-reference syntax error was corrected before the offline worker build.
Existing ts-rs/dead-code/proc-macro diagnostics and an existing Linux-only test
unused import remain. Formatting and whitespace checks passed.

Remaining development includes Amp/Cursor Teams contracts, additional native
account transports/bridges, full project/dataset/large-file transfers, enabled GPU
submission, verified price/account/billing, complete GPU device/IPC/egress and
image/CUDA acceptance, historical missing inventory and checkpoint retention.
No user credential, live provider/GPU/cloud deployment, app bundle, visual test or
second watcher was used. The full goal remains active. See the
[adapter guide](22-harness-adapters-remote-execution.md#fx-native-linux-acceptance-2026-10-07)
and [release checklist](20-agent-teams-release-checklist.md).

## Exact GPU estimates and atomic price receipts (2026-10-07)

- Added immutable owner `nativeJobV1` pricing approval with exact SKU IDs,
  positive quantities, GPU/vCPU/RAM/disk component coverage, hourly ceiling and
  explicit startup/cleanup/extra spending allowances. Historical absent fields
  keep original serialized signatures and eligible original replay/recovery.
- Native v1 calculator requests use the complete original typed Job body and
  fixed owner origin/authentication. Estimates must match the project/name,
  currency, exact SKU set/quantities, hourly unit, per-SKU costs and total sums.
  Unsupported/denied estimates, ambiguous cost types and changed/missing fields
  stop before any reservation or job create; no Compute approximation is used.
- Checked integer decimal arithmetic handles equivalent decimal/scientific
  formats without floating point, rounds monetary micro-units upward and rejects
  negative/invalid, unsupported precision or overflowing values. The lifecycle
  estimate plus extra allowance must fit the approved full reservation.
- A sealed fresh receipt binds original request/body/policy, native request and
  response hashes, exact SKU/rate/currency witness and lifecycle calculation.
  The transaction stores sanitized price and resource witnesses with the launch
  reservation before one create POST. Replay after restart has no new calculator,
  readiness or create call. No raw input, IAM or unknown response fields enter
  the durable price witness. All monetary allowances remain held.
- Added the primary-source pricing research and updated implementation/release
  checklists. The public Any contract does not establish Job estimator support
  or identify the active billing account/contract; these remain unverified.
  Owner SKU coverage and lifecycle allowances do not establish an invoice cap.

Verification: **61 GPU Rust regressions** passed, including **nine new pricing
and exact-arithmetic test groups**. Cases cover exact native request fidelity,
transaction visibility before create, changed/incomplete/ambiguous estimates,
unsupported status/redirect responses, atomic receipt age/body/policy binding,
legacy restart replay, bundled SKUs, quantity equivalence, overflow and upward
reservation. Assertion/fixture errors were corrected before the successful run.
Desktop/daemon `cargo check`, `cargo fmt --check` and `git diff --check`
passed. Existing ts-rs/dead-code/proc-macro warnings remain. A Windows default
encoding error during documentation editing was repaired while preserving
original UTF-8 content.

Remaining development: native Job pricing/account acceptance, authoritative
billing reconciliation and retained storage/transfer/tax policy, full GPU agent
submission and image/device/IPC/egress/CUDA acceptance, historical missing
inventory, project/dataset/checkpoint transfers and retention, Amp/Cursor Teams
contracts and other native account transports/bridges. The full goal remains
active. No live account/cloud/GPU request, credentials, deployment, app bundle,
second watcher or visual test was used. See
[pricing evidence and later acceptance](RESEARCH-nebius-gpu-price-receipt.md).

## Native release packaging and explicit process identity (2026-10-07)

- Added `packages/harness-worker`: production multi-stage recipe, tracked Git
  export instructions, reviewed pinned bases and actual executable boundary tests.
  Built the unchanged locked Linux release worker from `39a1521` with thin LTO,
  one codegen unit and stripping. That initial build passed its no-request ABI
  probe. Its initial runtime lacked `/ion/work`; the updated recipe fixes this,
  uses private nonroot directories and removes inherited privilege bits.
- Found that OCI USER does not establish ConTree's process identity. Manifest
  version 2 now pins UID/GID 1000 and `/home/ion`; the instance body sends them
  explicitly with PATH/HOME. Before native catalog/authentication, the worker
  verifies effective UID/GID, HOME and `/ion/work`. Original private request
  witnesses include identity, and operator recovery rejects changed/missing UID,
  GID, HOME, cwd or input. Historical omitted execution fields keep exact legacy
  serialization and recovery semantics; new images require version 2.
- Added current native Amp and Cursor contract research. Amp's actual official
  catalog fixture preserves exact model/caps but its offline plugin runner rejects
  inference dispatch and the helper drops metering provenance. Cursor SDK 1.0.36
  supplies an explicit native account, toolset and retry controls but lacks a
  documented hard completion ceiling; unexpected effects and final usage still
  need executable acceptance. Neither Teams adapter is falsely marked complete.

Verification: **19 remote and two worker Rust tests passed**, including five
new identity-related test groups. Desktop/daemon `cargo check`, formatting and
whitespace checks passed; existing ts-rs/dead-code/proc-macro warnings remain.
The updated release recipe/executable is the next concrete acceptance check.
No real account/provider/cloud/GPU request, credentials, deployment, app bundle,
second watcher or visual test was used. The full goal remains active; Amp/Cursor,
other native account transports/bridges, full project/dataset/checkpoint transfer,
GPU agent launch, complete GPU image/device/IPC/egress/CUDA, billing/finality and
historical missing inventory/retention remain development.

### Updated native release image acceptance

The committed updated Linux worker (`ad21a87178555ef52cf6a6e5074a97039becee29`)
built successfully with the production release profile and unchanged lockfile,
offline from an exported tracked archive. Archive SHA256:
`8dd408103bcd9e5102c1b0323593133f8710fda948a01c03cf74d6c69d9ee57b`;
1,230 tracked members, with ignored source/build/user caches excluded.

**Ten actual-executable test groups passed** in an offline, read-only disposable
Linux container under user 1000:1000, all capabilities dropped and
no-new-privileges. A new context fixture proves wrong UID/GID/HOME/cwd prevents
native lookup, while the correct registered context reaches exactly the fake
`exec --help` probe and then fails before real login/inference. Input/argument/
cap/version/unsupported-harness errors produce empty stdout and bounded diagnostics
without private input echo. Runtime checks establish the intended HOME/cwd,
0700 owner directories, root-owned 0555 ELF, absent builder/source caches and
zero `/usr` setuid/setgid bits.

Local image ID:
`sha256:d65a15825be163e9b86757090d3ff9d601525c9b5183d9a75dc90a6eb49f4170`.
ELF SHA256: `6d404557a22f03e719b16d112a4269175ba5550003572e82448a36aee27c5d5c`
(7,565,808 bytes). Source, pinned runtime and verified builder are recorded in
[the package guide](../../packages/harness-worker/README.md). Docker's two
empty-default base-argument warnings are expected: bases must be supplied by the
builder. Existing Rust warnings remain; source formatting and whitespace gates
passed. Test containers were removed, verified local image/cache retained.

This closes the actual production release-build/boundary gate. The test image
has no native provider installation/login/host manifest and is not registered
as a cloud target. Live complete inference/usage and deployment image acceptance
remain separate, along with the broader development scope above. No user key,
live provider/cloud/GPU request, deployment, app bundle, second watcher or visual
test was used. The primary `ion-prod` checkout and its pre-existing edits were
preserved. The full goal remains active.

## Native normal-task Codex team bridge (2026-10-08)

Extracted the existing Nebius source-task team behavior into one shared module:
catalog, saved-team invocation with exact lead authorization, durable replay,
source/project status and document scope, bounded waits and cancellation cleanup.
Normal Codex New task now registers five native dynamic functions and translates
only current main-thread/turn calls into that same owner behavior. The normal
selected Codex transport remains intact; delegated assignments retain the saved
team's selected harness/model/environment and grants.

A bounded background worker isolates approval/database/wait work from native
provider reads. Separate Ion approval IDs prevent late replies from approving
native requests. Exact native thread/turn/call identity survives peer RPC changes;
changed replay, foreign calls, extra arguments and queue overflow cannot create
new work. Cancellation cleanup covers both successful and failed calls, including
approval/queue transaction races. No UI/render path performs this work.

The actual native Codex 0.162.0-alpha.2 fixture passed through Ion's real
process/reader/writer path with an empty private profile, fake key and localhost
Responses provider. It advertised the tools, approved/queued one run, returned the
actual durable run ID, confirmed process exit and reopened the native thread.
Persisted tools then read the same run without a new grant or invocation. Six
fixture model requests were served locally; native shell/subagent/app/plugin
capabilities were disabled in this isolated test. Initial fixture errors (event
variant and identical HOME/temporary paths) were corrected before the successful
run. Production normal Codex features are not replaced by fixture settings.

Verification: **five bridge regression groups**, **one actual native start/resume
fixture**, **three Nebius workspace tests**, **39 Codex driver tests**, **nine
source/workspace tests** and **one concurrent invocation test** passed. Two
existing authenticated Codex tests were intentionally excluded. Desktop/daemon
`cargo check`, `cargo fmt --check` and `git diff --check` passed; existing
Rust/ts-rs/proc-macro warnings remain. The native executable SHA256 was
`3553cd6e7df5a093d8cb8301cd8088a57e0971aba71ddbe0e67f7f44a15cdf68`.
See the adapter guide and release checklist for the exact reproduction command,
version, review items and native resume limitation: threads originally created
without dynamic tools cannot retrofit them through the verified resume interface.

Remaining development includes other external solo bridges, Amp/Cursor contracts,
unverified native account transports, full remote project/dataset/checkpoint
transfer, GPU agent submission/image/device/IPC/egress/CUDA acceptance, billing
finality and historical inventory/retention. The full goal remains active. No live
provider/cloud/GPU request, user credential, deployment, app bundle, second watcher
or visual test was used. The primary checkout's existing changes are preserved.

## Native normal-task Claude team bridge (2026-10-08)

Normal Ion Claude tasks now register five SDK MCP functions and route them through
shared saved-team discovery, invocation, source-scoped status/waits and accepted
document reads. The native main-session assistant must first emit the exact tool
identity and arguments. A subsequent MCP call must match its native
`claudecode/toolUseId`; peer control UUID/JSON-RPC IDs only address replies. The
session/tool-use identity supplies durable replay across bridge restart. Foreign
sessions, subagents, unobserved/changed identities, extra arguments/metadata and
canceled calls cannot enqueue work. A changed observation also revokes a grant
already waiting for the user.

Codex and Claude now use one bounded background approval worker. The existing
shared owner behavior still supplies authoritative task/project, saved lead
harness/model/environment, access and limits. Independent Ion approval IDs, a
five-minute approval deadline, four queued calls and cancellation cleanup preserve
native read/interrupt responsiveness. No render path performs these operations.

Native shutdown now has an asynchronous owner with a two-second stdin EOF grace,
then process termination/reaping. An owned Windows Job closes descendants;
Unix uses a private process group. The existing bounded Teams process helper
reuses the same Windows Job implementation. Drop signals shutdown and does not
wait. Native user Stop retains its interrupt control request.

**Actual Claude 2.1.289 passed** through Ion's production process/reader/writer
path with a private fake account and localhost Messages service. It found a team,
requested the exact lead grant, queued one durable run and returned its ID.
After confirmed process exit, reopening the native session read the same run
without another grant/invocation. A third real process closed while owner approval
was outstanding and exited without creating another run. Seven synthetic Messages
responses plus optional startup `HEAD /api/hello` preflights were served locally.
Native executable SHA256:
`bcc6d9117aec30ad9414490302a25414359c871f5647e32e49b055c92bf84e0b`.

The first native fixture exposed the startup HEAD route, which the fixture now
handles exactly without consuming its model responses; it also exposed a stalled
EOF shutdown, fixed by the process owner. Short three-second behavioral deadlines
were unreliable under concurrent compilation. They were raised to ten seconds;
all tests passed with sequential database fixtures. These changes do not loosen
production approval, model-call or process deadlines.

Verification: **ten source bridge regression groups**, **two actual native
Claude/Codex start/resume fixtures**, **19 existing Claude driver tests** and
**six bounded Claude Teams adapter/process tests** passed (**37** total).
The actual Codex fixture was rerun after the worker extraction. Desktop/daemon
`cargo check`, formatting and whitespace gates passed; existing Rust/ts-rs and
proc-macro warnings remain. Two authenticated Claude driver tests require the
user's account and were excluded. See the adapter guide/release checklist for
reproduction commands and live review.

Other solo bridges, Amp/Cursor, unverified native account routes and broader
remote project/command/GPU resources, transfers, device/egress/CUDA acceptance,
billing finality and historical inventory/retention remain development. The full
goal remains active. No live provider/cloud/GPU request, user credential,
deployment, app bundle, second watcher or visual test was used. The primary
checkout's pre-existing changes are preserved.

## Native command/owner integration increment (2026-10-08)

- Fixed actual shipped command-image preflight: the exact child socket-boundary
  field is required. A missing or changed manifest rejects before reservation.
- Linux root supervisor permissions are checked before user code starts; missing
  collection, identity or stop permissions cannot produce an uncollectable effect.
  Packaged script line endings remain LF when checked out on Windows.
- Eight actual installed CLI groups passed in a pinned local Linux image, covering
  binary transfers, hashes, private environment/result isolation, socket denial,
  nonzero exits, timeout, orphan cleanup, rejected outputs/requests and permission
  failure before effects. Seven host worker tests passed.
- Thirteen Rust command tests passed, including native exported receipt acceptance
  through the production owner run path: file bundle, preflight, reservation,
  one create/observe, atomic artifact, scoped paged readback and exact replay.
  Desktop and daemon compile checks, formatting and diff checks passed.
- Live ConTree/GPU/FUSE acceptance, broader project/resources, Amp/Cursor contracts
  and final cloud/account accounting remain development. See the
  [command integration evidence](23-remote-command-gpu-jobs.md#production-command-entrypoint-and-owner-acceptance-2026-10-08).

## Approved command child resources increment (2026-10-08)

- New commands require a versioned private owner resource policy and the shipped
  `linux-rlimit/v1` image marker. Missing approval stops before remote reads/reservation.
- The fixed nonroot guard lowers hard CPU/address-space/file/process/descriptor
  limits, disables core dumps, retains stricter inherited ceilings and removes
  setup data before exec. Native model arguments cannot alter these limits.
- Historical policy/body omission remains stable; original environment witnesses
  protect lost-acknowledgement binding and replay. CPU/memory limits are per
  process and process counts per real UID, not aggregate VM or billing ceilings.
- Fifteen command tests (including the actual native exported receipt), thirteen
  executed check-gate regressions, seven host worker tests, eleven installed Linux
  CLI groups and nine guard groups passed: 55 checks total. Desktop/daemon compile,
  formatting and diff checks passed.
- Large project/dataset transfers, aggregate host resources, GPU devices/volumes,
  final price/account billing and Amp/Cursor native contracts remain open. See
  [policy, runtime semantics and review](23-remote-command-gpu-jobs.md#pinned-linux-command-resource-limits-2026-10-08).

## 2026-10-08 — approved project snapshot command checkpoint

The owner now exposes `run_remote_project_command` through the shared Teams tool
runtime. It snapshots only the sealed isolated Git workspace under explicit
private owner limits, stores the exact archive before upload, confirms its
content-addressed provider identity and pins the injected file before compute.
The native worker verifies the full archive before exec and returns its source
hash. Report and parent acceptance compare all included project sources; changed
files outside argv also invalidate the check. Staging participates in existing
Stop/restart/pending-effect fences. Original inline/GPU wire contracts omit the
new optional fields and retain their saved semantics.

Actual credential-free Linux CLI acceptance passed 13 groups, including a 98-file
project and 512 KiB binary, private archive ownership, tampering/path rejection,
child resources, network denial and descendant cleanup. The same exported native
project receipt passed the actual owner path with loopback provider responses:
durable staging before upload, lost upload acknowledgement resolved by its hash,
one upload/compute POST, exact file map, stable replay/readback, no compute after
Stop and whole-project rejection at parent review. No live provider/account
execution or cloud billing acceptance is claimed.

Reviewed local immutable image:
`sha256:050fbc7a43e32a8b1f550ef1a6520615168fc5f40832057ebd72dac6fd8d6f55`.
Built offline from the preceding exact local resource image with only the current
worker and manifest replaced; no image push, cloud import or key was used.
The [project execution review checklist](23-remote-command-gpu-jobs.md#2026-10-08--project-snapshot-command-execution)
records approval values, bounds and unresolved namespace retention/live acceptance.

Verification: 16 command/job Rust checks (including the installed-worker receipt),
17 executed-check/review Rust checks (including the native project fixture), one
native tool-grant regression, seven host worker tests, 13 installed Linux CLI
checks and nine Linux guard checks passed: **63 checks**. Desktop/daemon compile,
formatting and diff checks passed. Fixtures use synthetic data and loopback
provider responses; live import/account/retention acceptance remains required.

Historical **implementation estimate before the adapter scope correction: ~75%**.
This is a scope-based estimate, not a measured test-coverage percentage or release
readiness claim. Amp/Cursor native limits/usage, additional solo-harness bridges,
broader local/remote checks, aggregate host/cloud accounting and GPU runtime
contracts remain development. The broader Ion release checklist additionally
tracks Inbox, artifact/usage, gateway and cloud speech work.

## 2026-10-08 — native OpenCode normal-task Teams checkpoint

Normal OpenCode tasks now discover and invoke saved teams, inspect their durable
run status, await results and read accepted generated documents through the
shared owner runtime. The managed native server receives a private import-free
plugin; existing native coding tools, account/model selection and user plugins
stay available. JSONC content is preserved. The daemon supplies source/project
identity and keeps the saved lead's harness, model, environment, limits and
permissions; privileged access still requests an exact source grant.

A callback must match the live native assistant message and running tool part
observed on that task's SSE stream. A private loopback credential alone cannot
queue work. Complete arguments and session/message/call IDs bind durable replay.
Changed, foreign, browser and stopped calls are rejected. Source Stop cancels
pending approvals and delivery; a queued prompt cannot reopen a later Stop.
Canceled observed identities remain tombstones. A closed driver's session can
be attached again immediately, and nested message/part session IDs now filter
traffic from other tasks on the shared workspace server. The event subscription
is ready before accepting a first prompt. HTTP handlers/queues are bounded across
the owner and never run on rendering.

Actual **OpenCode 1.18.15** passed through Ion's real server and driver: discovery,
source grant, one durable team invocation, status, immediate same-session resume,
shared-server isolation and Stop on another source's pending approval. The first
source remained attached while the other task emitted native tool events. Seven
localhost Responses requests used a private empty profile and fake key. The
fixture preserves native built-in tool registration while disabling external
model/package/account discovery and metadata agents. This is native transport
and owner acceptance, not a live account, billing or team-completion claim.

Verification: **10 OpenCode bridge checks** (including actual native acceptance),
**19 existing OpenCode driver checks** and **10 existing Claude/Codex source
checks** passed (**39 total**). Two authenticated OpenCode checks and the two
separate native Claude/Codex fixtures were excluded from those regression runs;
the preceding checkpoints retain their native fixture evidence. Desktop/daemon compile,
formatting and whitespace gates passed; existing unrelated Rust warnings remain.
The [OpenCode setup and review checklist](22-harness-adapters-remote-execution.md#opencode-normal-task-teams-bridge-2026-10-08)
records the native version, reproduction command and real configuration review.

Before the adapter scope correction, the rough implementation estimate was
**about 75% for Teams/harnesses/remote execution**, with **about 25% development
remaining**. Amp/Cursor completion and other unfinished normal-harness Teams
bridges are now deferred. Broader local/remote command and dataset handling,
aggregate cloud accounting and GPU runtime/device/network/volume contracts remain
in the plan. Live native account and Nebius resource acceptance is tracked
separately. The historical estimate is not a percentage for the reduced scope,
measured coverage or full Ion release readiness.

## 2026-10-08: executable remote project snapshot checkpoint

- Windows archives preserve tracked Git executable modes; Unix retains current
  filesystem permissions. Untracked Windows files stay non-executable.
- Tracked links, submodules and unresolved index entries reject before transfer.
  A permission-only edit changes the original archive hash and rejects stale
  success evidence at both result submission and parent review.
- Actual Windows owner export ran an executable script with the installed Linux
  worker; the owner validated the original request, archive identity and receipt.
  The existing full project upload/replay/artifact/review acceptance also passed.

Verification: **19 job/snapshot regressions**, **17 command-check regressions**,
**14 native Linux CLI groups**, **two native owner acceptance checks** passed.
The test fixture's tracked-link setup needed Git's explicit `--add`; the local
Docker mount needed `exec` to run approved project scripts. No provider key,
cloud provision, adapter addition or visual test was involved. Broader remote/GPU
and other release checklist features remain open.

## 2026-10-08 — Inbox state and original artifact evidence

Implemented independent daemon-owned Inbox read/acknowledge/archive/restore state,
current-notification view filters and persistent desktop task notification policy.
Native UI uses cached state and background generation-fenced requests; collection
identities are hashed at cache rebuild rather than per visible row. Existing
project/task/type/status controls remain. Read/acknowledge/archive never authorizes
a decision or changes the pinned agent/team definitions. Conflicts request Refresh;
exact retries and bounded-capacity existing-item updates survive restart.

New artifact metadata contains original byte count/SHA-256 and native task or team
producer identities. Client upload/metadata replacement is rejected; agent capture
and atomic team publication remain. Content is bounded and verified when read,
exported, submitted, accepted or handed to a later workflow stage. Legacy content
without evidence remains explicitly unverified. Corrupted prior-stage bytes block
new work, and restoring the original content permits the same stage to continue.

Verification: three Inbox owner tests, one native identity/view test, three new
artifact integration tests, three publication/read/concurrency tests, parent
revalidation, native task capture and thirteen workflow/routine regressions passed.
Desktop/daemon compilation, generated binding equality, shared and full phone
TypeScript, formatting and whitespace checks passed. The worktree has no installed
TypeScript/Expo dependencies: checks reused the primary checkout's compiler/types
and exact installed Expo/React export declarations through an ignored configuration.
The initial native compile exposed two verifier identifiers and three helper
visibility errors; those were fixed before final verification. Existing warnings
remain; no app launch, visual test, live account, provisioning or second watcher.

See [behavior and manual review](24-inbox-artifact-review.md). Complete owner
notification history/global unread/mobile Inbox, rich embedded artifact previews
and global usage accounting remain development. Other current-scope release gates
are still open; this checkpoint does not mark the full Ion implementation complete.
Unfinished adapters remain deferred under the user's current scope correction.

## 2026-10-08 — pinned workflow document requirements

Implemented per-stage required HTML/Markdown/text/CSV/JSON artifacts in the
workflow draft model, prompt generator, native editor and desktop/phone run
details. The owner pins an actual lead output contract when creating each stage's
Teams run. Submission uses the existing scoped document and contract validator;
missing formats or summary-only output cannot complete the stage. Preparation
replay rejects a changed root contract, and owner progression revalidates original
evidence even for the final stage. Saved edits do not alter existing runs.

Supported format lists are bounded, unique and validated before storage; empty
legacy lists retain original serialization. Existing execution grants, review,
budgets, deadlines, gates, outbox/restart and Stop behavior are preserved. This
increment does not add automatic retry, code integration, binary team publication
or semantic quality/deployment assertions.

Verification: **16 owner/workflow/routine tests passed**, including two new pinned
contract/invalid-format/legacy/final-stage cases and the actual native dispatcher
fixture now requiring Markdown from both stages. Desktop/daemon checking, exact
generated binding equality, shared/full phone TypeScript, formatting and whitespace
passed. An invalid optional Vec derive annotation was corrected to the existing
serde-driven binding pattern before final gates. No live account, visual/device
acceptance, cloud provision or second watcher. See the expanded
[workflow review](21-workflows-routines.md#required-stage-documents-2026-10-08).

The full planned release remains incomplete; deferred adapters stay out of scope.
Calendar/conditional/retry/code integration, historical/mobile Inbox and global
usage, registered owner/gateway and cloud speech, and broader execution/resource/
billing/GPU integrations remain the remaining implementation families.

## Calendar routines checkpoint — 2026-10-08

Implemented in `codex/ion-agent-teams`, protocol **32**:

- Daily, weekday and monthly calendar rules use a pinned bundled IANA database.
  UTC intervals retain their previous defaults/serialization. The owner validates
  rules, computes exact future slots and returns three zoned preview timestamps.
- Desktop reuses existing keyboard-operable form controls and background RPC.
  Draft edits invalidate previews and generation fences discard stale responses.
  Saved/phone history exposes the pinned calendar rule and run policies.
- Explicit Run latest/Skip missed and Skip/Allow overlap remain subject to shared
  agent concurrency, existing budgets, approvals and explicit host selection.
  Fire deduplication, trigger snapshot and next schedule commit atomically.
- DST gaps skip, folds choose the earlier occurrence once; missing monthly dates
  skip. Unknown/invalid schedules fail before save or pause only the corrupt rule.

Verification: seven calendar math/owner tests and all twenty automation regression
checks passed, including concurrent owner restart, immutable triggers, real shared
agent capacity and the existing two-stage dispatcher/artifact handoff. The native
bounded-field test passed. Native/daemon compilation, generated shared/phone
TypeScript, formatting and whitespace passed. Review items are in
[workflows and routines](21-workflows-routines.md). No visual/device test or live
provider/cloud effect was performed.

The full goal remains active. Conditional workflow graphs, historical/mobile
Inbox, global usage accounting and broader remote/GPU, gateway and speech
integrations remain development. Unsupported new harness adapters remain deferred.

## Conditional workflow checkpoint — 2026-10-08

Implemented in the isolated Teams worktree, protocol **33**:

- Forward typed Next/Go to/Finish and JSON document conditions; 24-stage bound,
  reachability, exact scalar/pointer validation and rejection of cycles/back-links.
- Manual/prompt-generated editable routes and route-aware native reorder/remove
  controls. The selected path retains each actual team's profile/grants and gates.
- Original accepted JSON document ID/hash and matched destination are durable.
  Missing/invalid/ambiguous/changed condition evidence blocks before routing;
  unselected stages have no tasks. Accepted choice survives a next-stage budget
  block while partial next-team preparation rolls back independently.
- Prior visited usage and accepted predecessor documents follow the actual path.
  Desktop/phone display pinned rules, chosen receipt and skipped stages.

Five graph acceptance tests and all twenty-five automation regressions passed,
including both paths through exact gates, immutable edits/restart, early finish,
invalid topology, document tampering/ambiguity and preserved shared model caps.
Native/daemon compilation, full shared/phone TypeScript, generated-binding equality,
formatting and whitespace passed. Review is in
[workflow routes](21-workflows-routines.md). No visual/device/provider acceptance
was performed. Parallel fan-out, loops and bounded stage retry remain separate
work; this does not complete historical/mobile Inbox, global usage or the broader
remote/GPU, gateway and speech goal. New harness adapters remain deferred.

## Historical and phone Inbox checkpoint — 2026-10-08

Implemented in the isolated Teams worktree, protocol **34**:

- Owner facts for persisted task lifecycle/deletion, proposed tasks, team work,
  decisions, workflows/routines and generated documents; same-transaction source
  capture, once-only current-state backfill and durable bounded task outbox import.
- Retained history after source resolution/removal, independent read/ack/archive
  state, exact retries, concurrent revision fences, indexed project/task/type/state
  paging and global unread/pending counts independent of the 32-row client page.
- Native cached background pages and source links. Phone Inbox with foreground
  polling, shared controls, actual decision/proposal review and stale owner/client
  fences. Accepted proposals fill the existing unsent New task flow, preserve an
  existing draft and require the user to Send; approval is separate from archive.

Verification: all **6 new history tests** passed; the broader Inbox filter ran
**10 tests** including the three prior compatibility cases. **129 team/source
bridge/workflow/budget/recovery tests passed**, with two pre-existing native
Claude/Codex account-fixture cases ignored because their binary environment was
not set. **54 persistence regressions passed**. Native/daemon compilation, exact
protocol binding equality, shared/full phone TypeScript, Rust formatting and
whitespace passed. The initial fixtures were corrected to create a started task
and resume the actual persisted decision continuation. No live provider, device,
visual acceptance, cloud deployment, Push service or second watcher was performed.

Manual acceptance is in [Inbox and artifacts](24-inbox-artifact-review.md).
Historical transition reconstruction before this upgrade and Push notifications
are not delivered. The full goal remains active: global usage accounting, bounded
workflow retries/code integration and broader remote/GPU, gateway and speech
integrations still require development. New harness adapters remain deferred.

## Usage ledger checkpoint — 2026-10-08

Implemented in the isolated Teams worktree, protocol **35**:

- Durable owner facts for existing Teams harness model reservations/receipts,
  ordinary Nebius and specialist calls, remote commands and GPU allowances/results.
  Original run/agent/model/actual host identity is pinned; accounting survives Stop.
- Metadata-only indexed project/task/agent/harness/model/environment/type/time
  paging. Filter totals are independent of page; currencies and estimate/finality
  classes stay separate. Missing prices/receipts are unknown, never fabricated zero.
- Local/remote Claude SDK cost totals are labeled estimated/provisional. The
  optional native worker estimate preserves legacy response bytes when absent.
  Remote terminal/process/token/cost evidence commits atomically; invalid metadata
  cannot partially release allowances. Late retries match the stored receipt.
- Background cached desktop Usage and foreground phone Usage with filters, source
  task links, paging and owner/filter generation fences. Existing desktop provider
  history remains independently available. Filters do not enable new adapters.

Verification: all **10 accounting cases** passed; **40 Nebius transport** and
**21 remote lifecycle** regressions passed, with two exact reservation/Stop tests
and two native worker contract checks. Desktop/daemon/native worker compilation,
exact shared protocol binding equality, shared/full phone TypeScript, formatting
and whitespace passed. No visual/device/live account/deployment or second watcher.
See [usage coverage and review](25-global-usage-review.md).

Ordinary non-Nebius CLI calls are not yet imported into the owner ledger; external
provider history has overlapping scope and must not be added to its totals.
Invoice/cloud billing reconciliation, bounded workflow retries/code integration,
registered-owner/gateway/cloud speech and broader GPU/resource integrations remain
implementation. The full goal stays active; new harness adapters remain deferred.

## 2026-10-09: ordinary native Claude/Codex usage

The existing normal-task adapters now publish validated metadata receipts into
the owner ledger already consumed by desktop and phone Usage. Claude message
identities survive copied native sessions; Codex observed-turn/cumulative
checkpoints exclude historical replay and context fills. Cached/reasoning counts
follow each provider's exact inclusion rules. Original task/project/time is
immutable; exact replay does not change ledger revision. No transcript or native
identifier is published, and no UI/event-pump I/O or new harness adapter was added.

Five focused receipt tests passed. See [coverage and review](25-global-usage-review.md).
Other existing solo-harness receipt capture, authoritative billing, workflow
retries/code integration and broader GPU/gateway/cloud speech remain development.
The full implementation goal remains active; live accounts/device review are
separate and have not been claimed.

Native usage integration verification: the five receipt tests and four existing
ledger regressions passed. Desktop, daemon and native worker compilation passed;
Rust formatting and whitespace checks passed. Protocol remains 35, with no shared
wire or phone source changes in this increment. No visual/live/device test was run.

## 2026-10-09: optional workflow retry implementation

The desktop stage editor and pinned native/phone details now expose an optional
retry policy, with legacy/default off. Durable owner waits preserve the chosen
delay across restart. Retry uses the same stage team/assignment/task and existing
bounded queue mutation, retaining harness/model/environment/grants, deadline and
all usage. Automatic recovery requires a confirmed failure before effects and
rejects unknown hosts, prior file/app/command/GPU/child effects and exhausted
attempt/call/token limits. Stop remains authoritative.

Protocol is now **36**; desktop, daemon, worker and phone clients must match.
See [retry behavior and user review](21-workflows-routines.md#optional-bounded-retries-2026-10-09).
Verification: 29 workflow regressions, six native usage receipts and the existing
exact retry fence passed. The four retry groups passed after the indexed query
correction. Desktop/daemon/worker, generated protocol 36 equality and shared/full
phone TypeScript passed. No visual/device/live/deployment or new watcher was used.
The full goal remains active;
code integration, other existing solo-harness usage receipts, billing, broader
remote/GPU, registered-owner/gateway and cloud speech still need implementation.
Deferred new harness adapters remain outside scope.

## 2026-10-09: shared desktop and phone speech streaming

Configured cloud microphone sessions now use NeMo's explicit transcription WSS
route on both clients. Native PCM is sent after format acknowledgment, partials
stay previews, and only final segments plus the commit acknowledgment become
editable text. No reconnect/replay/upload fallback follows a cloud failure.
Capture, queue, event/text and finalization limits are enforced. Existing local
speech and WAV import behavior remain available.

Phone cancellation fences task/daemon changes, backgrounding, disabled composers,
late sockets and late results. Desktop Stop retains the Cancel channel through
finalization; canceled results stay discarded. Local preview failure cannot change
its pinned destination because settings changed mid-capture. Network and local
speech process work remain on the existing worker, with preview cadence capped
at eight updates per second. Explicit per-connection crypto selection avoids
Rustls's ambiguous global provider when the app enables both Ring and AWS-LC.

See [shared speech implementation and review](26-shared-speech-streaming.md).
Protocol remains 36. No service has been deployed, and no credentials, billable
resources, native device acceptance or visual review are claimed. The full goal
stays active: code integration, other existing harness usage and billing, broader
remote/GPU and registered-owner/gateway work remain. New adapters stay deferred.

Verification for this checkpoint: **nine Rust speech boundary tests** and
**thirteen phone stream/WAV tests** passed. Desktop, daemon and native worker
compilation, full phone TypeScript, Rust formatting and whitespace checks passed.
The stream tests cover PCM/auth ordering, endpointed final text, canceled/late
sockets, malformed/oversized responses and buffers, TLS failure and redirects.
They use controlled local/fake transports and do not establish live service,
OS microphone, Android/iOS TLS/redirect or visual acceptance.

## Workflow code inheritance (2026-10-11)

Later stages now have an explicit **Inherit code** control. It is off for legacy
workflows and new stages; the first stage cannot enable it. Definitions, stage
instructions and phone/desktop run details retain the pinned setting. A started
stage records the actual completed predecessor, including skipped branch paths.
Prompt generation can propose the same editable flag.

The next team and code reservation commit together after its human gate and
shared budgets have been checked. Git/receipt/file verification runs outside the
dispatch transaction in background workspace preparation. An immutable,
bounded manifest pins the predecessor, repository/source binding, original base,
exact bytes and SHA-256 before copy effects. The owner installs it into a new
isolated worktree. Both the original source and predecessor workspace stay intact.
Claims are fenced until installation and the workspace are sealed.

Read-only receiving stages keep write access denied. Existing Local/Nebius
snapshot dispatch uses the installed workspace with the same selected harness,
model, grants, fallback and remaining limits. Subsequent receipts extend exact
prior bytes; final source review and generated change documents contain cumulative
original-before/final-after content and original edit IDs. Native source
application remains a separate human review action.

A restart marks an in-progress copy uncertain. Recovery adopts only the full
exact result, unchanged manifest, no unreceipted files and no private staging
files. Partial/unknown copies are preserved for manual review and are never
replayed. New source commits do not change the pinned code base; final source
application rejects the changed-base conflict. The review/recovery checklist is
in [code handoff between stages](27-workflow-code-handoff.md).

Verification: the full **24 workspace/source regression tests** passed,
including all five new workflow handoff groups. The final five groups passed
again after repository/manifest hash binding, metadata-tamper rejection, stricter
staging checks and exact post-restart adoption were added. Shared-client and full
phone TypeScript checks passed. All **29 coordinator workflow/calendar/retry/history regressions** passed. The
separate native dispatch fixture repeatedly reached its total-clock wait limit
while agents were still making progress through review. Workspace preparation
now reuses its existing read connection instead of adding schema initialization
on ordinary tasks. That fixture retains a 30-second no-progress guard plus a
finite overall bound, with original task, approval and completion assertions and
better failure diagnostics. The native dispatch fixture then passed with its
original exact task, approval and completion assertions. The five handoff groups
passed again against the final connection-reuse implementation. Desktop, daemon
and native worker compilation, generated protocol 37 equality, Rust formatting
and whitespace checks passed. This is 54 distinct coordinator/workspace/native
regression checks; reruns are not counted as additional coverage.

This checkpoint does not complete the full release. Binary document publication,
phone definition editing, additional app binding and notification delivery,
other existing solo-harness usage, authoritative billing, registered-owner/gateway
controls and broader GPU/resource integrations still have development or live
acceptance remaining. Deferred new harness adapters stay outside scope. No
visual test, live provider request, billable provisioning or extra dev watcher
has been performed.

## Generated PDF artifacts (2026-10-11)

Existing native Teams tools now publish bounded opaque PDFs with exact project,
task, assignment, producer and request provenance. Bytes and an agent-authored
summary commit atomically. Reads, exports, report/review, workflow requirements
and interruption recovery verify that original evidence. PDF model reads and
stage handoffs explicitly label the summary as agent-authored; they do not
interpret the PDF or send binary bytes into model context. Native artifact
export retains the exact file. Text excerpts preserve their original JSON
escaping budget through a shared helper.

Verification: nine new PDF/native/workflow/recovery/source/excerpt regressions,
three existing text-document tests, three artifact-integrity tests, two workflow
document tests and one original source scope regression passed (18 distinct
checks). Desktop/daemon/worker compilation, protocol 37 equality, formatting and
whitespace passed. Two fixture mistakes were corrected: Stop requires the real
revision, and a second run must wait for the same agent's capacity. No production
controls were loosened. See [PDF behavior and review](28-generated-pdf-artifacts.md).

This is a completed bounded PDF publication increment, not a completed full
release. Larger binary transfers/additional formats and previews, existing
solo-harness usage and authoritative billing, general Local/remote checks,
resource/GPU and registered-owner/gateway work remain. Cloud speech deployment
and live/native device acceptance still need the user's target and credentials.
Deferred new harness adapters remain out of scope. No visual test, live provider,
remote account mutation, billable provisioning or additional watcher was used.

## Ordinary OpenCode usage (2026-10-11)

The already implemented resident OpenCode reader now records witnessed live
provider steps in the owner ledger. Session and actual assistant model binding,
observed start/finish, bounded metadata, exact token breakdowns and durable hashed
receipt identities exclude copied history, unrelated tasks, cumulative/context
updates and client cost estimates. First ownership survives exact replay. The
receipt total resolves the verified older/newer reasoning-count difference;
ambiguous or malformed metadata remains unavailable. Render and the event pump
perform no new accounting I/O. Normal task/team invocation remains unchanged.

Verification: six new OpenCode receipt tests plus the six existing native
Claude/Codex accounting tests passed (12 total). Nineteen existing OpenCode
transport/permission/transcript tests passed; its two authenticated live tests
remain explicitly ignored. Desktop/daemon/worker compilation, protocol 37
binding equality, formatting and whitespace passed. See
[usage behavior and review](25-global-usage-review.md#native-opencode-live-step-receipts-2026-10-11).

Remaining implementation includes other existing solo receipts/projectless
accounting, authoritative billing, phone definition editing, general checks,
resource/GPU and registered-owner/gateway work. Live deployment and native/device
acceptance remain separate. Deferred new harness adapters stay out of scope.

## Phone workflow/routine authoring (2026-10-11)

Workflows & routines now creates/edits saved definitions directly on phone. Its
native form retains the existing Ion theme, normal task/team invocation and
owner-authoritative control RPCs. Stage teams, generated/manual instructions,
gates, formats, inheritance, retries, forward JSON branches and aggregate limits
are editable. Calendar routines choose team/workflow targets, preview exact owner
occurrences and save paused before explicit schedule enablement.

Owner/address-scoped bounded drafts retain definition UUID, original baseline and
expected revision. Ordered persistence precedes save; exact acknowledgement/read
reconciliation handles missing responses without replaying mutation. Unrelated
revisions can be refreshed without discarding edits, while changed/deleted
definitions require explicit review. Connection/foreground/dialog fences keep
old responses and confirmations from crossing scope. The authoring screen does
not poll full snapshots while typing. Native input APIs and picker FlatLists
preserve the existing theme, system switches, scalable text and keyboard insets.

Verification: 14 new editor/effect/recovery tests and all 11 existing automation
start/recovery tests passed (25 distinct checks). Full phone/shared TypeScript,
protocol 37 equality, mechanical UI detector and whitespace passed. See
[phone behavior and device review](29-phone-automation-editing.md). The isolated
worktree initially lacked package resolution for Bun/shared TypeScript; the
existing worktree alias preload and cached dependency paths resolved that check
environment. No production behavior was changed to make tests pass.

The full release remains active: general checks, other existing solo receipt
coverage/projectless accounting and authoritative billing, background notification
and registered-owner/gateway controls, and broader resource/GPU integration still
have implementation remaining. Live Nebius speech/provisioning and native device
acceptance require the user's later target, credentials and review. Deferred new
harness adapters remain outside scope. No visual test, live generation request,
billable provisioning, deployment or extra dev watcher was performed.

## Registered desktop gateway workflow increment (2026-10-11)

Added owner-specific metadata publication and native phone workflow/routine/run
views with owner/project filters. Explicit Read workflows and Control workflows
grants gate durable start, stage decision, stop and schedule controls. Owner and
resource witnesses remain bound through claim and daemon revision checks. A phone
persists its command/run identity before sending, reads lost receipts and fences
foreground/credential changes. Start acknowledgement recovery reads the exact
persisted run; other unknown effects remain unreplayed. Existing solo-task queues
are now publisher-scoped as well.

See [gateway implementation and unchecked live review](30-gateway-workflow-controls.md).
This increment uses the existing desktop background worker. Headless cloud owner
publication and broader historical gateway portfolio data remain development;
existing deferred harness adapters remain out of scope.

## Headless registered-owner workflow increment (2026-10-11)

Moved persisted-pairing workflow publication and phone controls into a dedicated
production daemon worker. A Linux service can use a private, owner-validated
pairing file without an OS credential vault; that source also serves gateway app
proposals. Same-project validation is shared by RPC and the outbound owner.
Durable claim, exact response witnesses, revision checks, post-claim identity/
grant checks and shutdown fences preserve effect ownership. Recovery reads exact
persisted start receipts after restart and never replays missing/unknown effects.
Temporary desktop pairing keeps its existing desktop bridge.

Verification: 13 new contract/owner/environment tests plus 30 existing automation
regressions passed (43 distinct). Locked desktop/daemon/selected-worker compilation,
protocol 37 binding equality, formatting and whitespace passed. Linux descriptor
acceptance is still unchecked: Ubuntu's registered WSL disk is missing and the
configured Docker engines are unavailable. No service restart, runtime repair,
live provider/cloud call, billable resource or visual test was performed.

See [implementation, setup and remaining owner review](31-headless-gateway-owner.md).
Broader historical gateway portfolio data, resource/accounting work and live
provider/device acceptance remain open. No deferred new harness adapter was added.

## Gateway Inbox and usage history increment (2026-10-11)

Added a durable typed read relay and independent phone Inbox/usage grants. The
headless owner queries its real historical Inbox/global ledger and all-project
directory. Cursor pages retain full counts/totals, unknown costs and accounting
quality; private host paths and billing references stay off the phone. Native
phone filters and paging are reachable from Personal gateway; focus, owner,
credential and filter changes fence late answers. Read previews never approve
or execute work. Memory-only desktop pairing retains its app-session bridge.

Verification: 49 gateway, 27 phone, 10 daemon-owner and 4 protocol cases passed
(90 distinct), plus locked desktop/daemon/worker compilation, full TypeScript,
binding equality, formatting and whitespace checks. Native visual/device and
live internet acceptance remain unchecked; no live provider/billable deployment
was performed. See [setup and review](32-gateway-inbox-usage-history.md).

Historical gateway workflow/evidence pages, notification writes, remaining
resource/accounting integration and cloud speech/device acceptance remain open.
Deferred new harness adapters were not added.

## Gateway workflow history and accepted documents (2026-10-11)

Implemented indexed chronological owner history for older workflow/routine runs,
phone project/definition/attention filters, pinned stage/branch/code-inheritance
metadata and a separate evidenceRead grant for accepted generated documents.
The registered daemon and memory-only desktop bridges read existing owner stores.
Phone source/export reads check exact stage/producer membership, immutable metadata,
16 KiB byte continuations and the complete original SHA-256 before showing or
exporting content. HTML stays inert source; text previews use virtualized rows.

Verification: 136 distinct focused tests passed (8 protocol, 11 owner, 30 existing
workflow/routine/branch/retry, 51 gateway and 36 phone helper tests). Locked Rust
checks passed for desktop, daemon bins and the native harness worker; complete
mobile/gateway/client TypeScript checks, protocol-37 binding freshness, formatting,
diff whitespace and the native source detector also passed. Existing Rust warnings
remain. These checks do not prove native folder-picker behavior, visuals, Linux
service descriptors, TLS/live gateway or live provider/device acceptance.

Review and setup: [workflow history and accepted documents](33-gateway-workflow-history-documents.md).
Gateway notification writes and broader accounting/resource/speech work remain;
deferred harness adapters stay outside scope. No watcher, live provider or billable
resource was started. The overall goal remains active.

## Gateway Inbox notification controls (2026-10-11)

Implemented native gateway-phone read/unread, acknowledge and archive/restore
controls. An independent Inbox read+control grant, pinned owner/phone/sequence/
revision/action/nonce/deadline and separate durable command queue protect the
notification operation. The phone saves intent before sending; owner state and
its exact receipt commit atomically. Lost gateway or daemon acknowledgments
recover that receipt after restart without reapplying it after another client's
change. Stale, missing and expired requests reject without changing owner work.
Both headless daemon and memory-only desktop bridges use the same owner receipt.

The new receipt RPC raises the direct wire version to 38. Desktop, daemon and
phone must upgrade together; gateway health advertises Inbox control support.
No existing phone grant acquires write access. No provider effect, task/decision
approval, workflow start or billable operation is representable in this command.

Verification: 19 new focused tests passed (5 owner DB receipt, 1 strict protocol,
4 gateway routing/durability and 9 phone persistence/recovery/transport tests).
Combined with the workflow-history increment, 155 distinct focused tests passed
this continuation. The full gateway suite (55), phone helper suite (45), complete
mobile/gateway/client TypeScript checks, locked desktop/daemon/worker Rust checks,
protocol-38 generated-binding freshness, formatting, diff whitespace and native
source detector passed. Existing Rust warnings remain. No visual/device test,
watcher, live provider, deployment or billable resource was started.

Review/setup: [gateway Inbox controls](34-gateway-inbox-controls.md).
Remaining work includes broader resource/monetary accounting, other existing
solo-harness receipts, larger artifact formats/interpretation/previews, background
notification delivery and shared speech/live device acceptance. Deferred harness
adapters remain outside scope. The overall goal remains active.

## Shared GPU budgets (2026-10-11)

GPU allowances now include one owner budget per exact Nebius project across runs
and routines. Each fresh launch reserves against both its original run allowance
and that shared cap in the same transaction as launch/resource/price facts. Exact
replay does not submit or reserve twice; unknown, canceled and terminal jobs retain
money. Legacy reservations are adopted on budget save and cannot be reset through
identity edits, cap decreases or deletion. Increasing a cap remains an explicit
owner action and cannot expand the original per-run grant.

Desktop Usage and directly connected phone Usage expose budget creation, editing
and enable/pause for new launches. Status shows integer held/available allowance,
limit and request count, not invoices. Native rows are cached/virtualized and
keyboard navigable. Phone saved-request identity, exact-state reconciliation and
owner/focus/revision fences prevent interrupted saves from creating another cap.
Gateway Usage readers receive redacted totals only, with no budget write route.
Direct protocol **39** requires desktop/daemon/worker/phone to upgrade together.

Verification: seven shared-cap owner tests, 62 existing GPU regressions, two
protocol tests, 56 full gateway tests and 31 shared-client/phone helper tests
passed (158 distinct focused tests). Complete mobile/gateway/client TypeScript,
locked desktop/daemon/worker checks, generated protocol-39 binding freshness,
formatting, whitespace and source/UTF-8 label audits passed. Existing Rust
warnings remain. No visual/native device acceptance, watcher, live provider,
deployment or billable resource was started.

Review/setup: [shared GPU budgets](35-shared-gpu-budgets.md).
Remaining development includes provider billing/finality, broader cloud/resource/
egress controls, other existing solo usage receipts, larger document interpretation/
previews, background notification delivery and shared speech/live device acceptance.
The GPU submission gate remains closed where its original prerequisites are unmet;
unimplemented harness adapters remain deferred. The overall goal remains active.

## Historical Inbox background notifications (2026-10-12)

Added an indexed ascending, content-free owner notification feed and publication
from both headless owner and temporary desktop pairing workers. First publication
establishes a silent baseline after task-history migration. New eligible Inbox
facts respect current unread/ack/archive state, recency and All/Issues only/Off;
suppressed facts still advance the durable cursor.

Gateway cursor and alert queue commit together, with exact replay after a lost
publication reply, bounded pending alert coalescing, expiration, phone/publisher
revocation and receipt/token-rotation fences. The native phone has a separate
explicit Inbox opt-in and grant, distinct Android channel and foreground/owner
checks around token registration. Old task subscriptions stay task-only, and the
new endpoint cannot change an older gateway's task setting. A generic alert opens
the current paired Inbox and never approves or marks anything read.

Automatic approval review rejected the initially proposed Expo payload containing
internal routing IDs. The implemented alternative sends only a fixed generic
Inbox message and scope; publisher/device/event identifiers stay in the owned
gateway. All external transport tests use fake fetch. Push is best effort: uncertain
sends do not repeat, so a missed alert remains visible in durable history. No real
phone notification or cloud call was sent.

Verification: 100 distinct focused tests passed (4 owner feed, 67 full gateway,
29 shared-client/phone helpers). Tests include legacy migration, separate grants
and opt-ins, atomic cursor replay/restart, uncertain sends, mute/revocation,
coalescing/expiration, rotated token failures, receipt window, malformed metadata,
current pairing navigation and older-gateway behavior. Complete mobile/gateway/
client TypeScript, locked desktop/daemon/worker checks, protocol-40 generated
binding freshness, formatting, whitespace and frontend source checks passed.
Existing Rust warnings remain. No visual test, watcher, live provider/deployment,
physical phone acceptance or billable resource was started.

Review/setup: [Inbox background notifications](36-inbox-background-notifications.md).
Update owner, phone and gateway together; direct wire protocol is **40**. EAS/native
notification credentials and real device acceptance are still required. Remaining
implementation includes provider billing/finality, broader remote/resource/egress
controls, other existing solo usage receipts, larger document formats/previews
and shared speech integration/acceptance. Deferred adapters stay out of scope.
The full goal remains active.

## 2026-10-12 — priorities, verified document reading and streaming readiness

The user prioritized billing, broader remote controls, document previews and
speech integration before more AI provider connections. Unfinished solo native
Pi/OMP/OpenCode2 accounting is preserved separately in Git stash. Deferred
adapters remain outside the active scope.

Desktop Artifacts/Inbox/team documents now have verified, cached native Preview
with Markdown reading/source and JSON formatted/source modes. Long blocks and
source rows are bounded, reads/parsing run on a worker, navigation is fenced and
links/images remain inert. Gateway workflow documents gain the same Markdown/
JSON reading/source choice on the phone after complete hash verification.
HTML/CSV/text remain inert source; PDF remains export-only. Rich HTML/PDF,
additional formats and direct-phone artifact-library parity are still development.

Desktop Test and phone Test and save now check inventory plus authenticated
streaming session/format acknowledgment without microphone audio. Phone checks
cancel on screen exit/backgrounding and reject late saves. Saved secrets can only
be reused on explicit Test for the exact saved origin. Deployment, platform TLS/
redirect behavior, real recognition and device acceptance remain unverified.

Verification: 37 distinct tests passed (4 native documents, 4 phone documents,
10 credential/streaming and 19 phone readiness/stream/WAV). Locked desktop/daemon/
worker compilation, full mobile TypeScript, formatting, whitespace and frontend
source checks passed. No watcher, visual test, live deployment/provider, paid
resource or microphone acceptance was run. Existing warnings remain.

[Priority scope, billing source evidence and owner review](37-priority-integrations.md).
Billing reconciliation, broader aggregate resource/egress and remote lifecycle,
rich document rendering and shared speech/live acceptance remain open. The goal
is active; this checkpoint does not mark the full implementation complete.

## 2026-10-12 — scoped native billing reports

Desktop Usage → Cloud billing now saves account/contract/currency/origin scopes
and explicitly syncs existing successful single-month native FOCUS 1.2 exports.
GET-only transport validates source identity, approved origin and native metadata;
IAM does not reach archive requests and signed URLs do not persist. Bounded
worker parsing preserves exact signed costs, credits/taxes/corrections, complete
account charges and original resource attribution. Atomic snapshots replace the
selected scope/month amount instead of summing repeated exports. Original
historical reports and paginated charges survive restart. Stale/partial reads and
changed replay content fail. Imported identity remains immutable and GPU holds
are unchanged. Contract/currency partitions and pre-release schema migration
preserve original definitions/revisions.

Desktop has virtualized monthly/history/charge views and keyboard controls.
Direct phone Usage → Cloud billing reads saved accounts, exports and charges with
owner/foreground and response identity fences; these reads do not call providers.
Protocol is **41** and generated bindings were updated. Further AI provider
connections remain deferred; previously stashed usage work was not resumed.

[Setup, supported bounds, automated evidence and later review](38-nebius-billing-reports.md).
Live reports, gateway billing redaction, owner credential refresh/discovery and
job/tariff/finality reconciliation remain open. Broader remote/resource/egress,
rich HTML/PDF and speech deployment/device acceptance still remain. The goal
remains active; no billing account or deployment resource was accessed.

Checkpoint verification: **22 distinct tests passed** (17 core billing/store/
transport boundaries and 5 phone reader helpers). Complete mobile, gateway and
shared-client TypeScript, locked desktop/daemon/worker checks, protocol-41 binding
freshness, formatting, whitespace and frontend source checks passed. Tests use
fake/local source data; these results do not establish live report or UI/device
acceptance. Existing Rust warnings remain.

## 2026-10-12 — cross-project remote execution inventory

Desktop and direct-phone Usage now include a bounded cross-project inventory of sandbox model calls, commands and GPU jobs, with project/type/unresolved filters and original-run control navigation. Reads never call providers. Missing operation handles stay unresolved; GPU cancellation remains distinct from cleanup and monetary settlement. Protocol is **42**, with generated types updated. Background loading, virtualized rows, keyboard controls and stale owner/page fences are included.

[Implementation and later review checklist](40-remote-execution-controls.md). The wider remote/GPU launch, aggregate resource/egress, rich HTML/PDF, billing reconciliation and speech deployment/device acceptance work remains open. Additional provider adapters remain deferred.

## 2026-10-12 — independent gateway billing branch merged

`codex/ion-gateway-billing` was implemented independently from the same baseline, committed as `3ac2e07`, then merged into `codex/ion-agent-teams` at `ccef8c2`. Gateway phone users with Usage read permission can now read saved account summaries, individual historical exports and paginated charges. Connection/account/contract/archive and cloud container/resource identities are redacted; exact signed decimals, export identities and report digests remain intact. All reads use the existing owner grant, receipt and stale-pairing fences, with no provider or billing write authority.

Review: [gateway billing](39-gateway-billing-history.md) and [remote executions](40-remote-execution-controls.md). The remote inventory also preserves a team stop request while a model operation is still unresolved, and shows original cancellation handles.

Merged verification: **105 distinct tests passed** (68 gateway regression, 29 mobile/shared-client readers and transport, 4 native billing projections and 4 native remote collection tests). Complete mobile, gateway and shared-client TypeScript passed. Additional compile/binding and formatting checks are recorded below after their final result. No live provider, account credential, paid resource, deployment, physical-phone or visual test was used. Additional provider adapters remain deferred.

Final merged gates passed: locked desktop/daemon/worker compile checks, current protocol-42 generated bindings, complete TypeScript checks, Rust/Prettier formatting, whitespace/UTF-8 audit and frontend source checks. Existing Rust warnings remain.

## 2026-10-12 — native document layout and PDF pages

Desktop Artifacts, Inbox and team documents now support static HTML layout in a separate document browser surface and native PDF page rendering with page/zoom/scroll controls. Complete original evidence is verified before worker preparation. Windows InPrivate/disabled-script and resource-request controls, macOS nonpersistent/disabled-script navigation controls, Source/Save, background PDF rendering, bounded pixels and generation fences are included. UI scroll state stays on the foreground entity.

**17 native tests passed** (11 document boundaries/rendering and 6 existing browser regressions). The real Windows PDF API runs all three zooms and verifies the fix for 125% DPI scaling. macOS compilation/rendering and browser/window visual acceptance are unverified on this Windows host. No watcher, UI screenshot, provider, paid resource or microphone was used.

[Implementation and owner review](42-native-document-rendering.md). Direct-phone artifact/preview parity, additional formats/limits, remaining billing settlement, wider remote/GPU enforcement and shared speech deployment/device acceptance remain open. Further provider adapters stay deferred.

Native document final gates passed: locked desktop/daemon/worker compilation, Rust/Markdown formatting, whitespace/UTF-8 audit and frontend source checks. Existing Rust warnings remain.

## 2026-10-12 — independent speech acceptance branch merged

`codex/ion-speech-acceptance` was implemented independently from `4351b7e`, with commits `06baaa3` and `ab03819`, and merged into this implementation branch at `89e5579`. The shared service now has executable readiness and explicit private-fixture recognition/concurrency acceptance tooling, bounded HTTP/WebSocket transport, cancellation/finalization checks and sanitized output. Launch preflight verifies owner-pinned GGUF SHA-256 bytes before serving; the supported stable API name `nemotron-3.5` matches the actual desktop/phone inventory predicate. A label alone does not prove model provenance.

**40 speech tests passed again in the merged checkout**, using real loopback HTTP/WebSocket/TLS transports and synthetic recognition. No real ASR/model, systemd/CUDA, deployment, microphone or phone was exercised. [Setup and later owner review](41-speech-service-acceptance.md). A VM/DNS/TLS target and real fixtures/device acceptance remain required.

## 2026-10-12 — native gateway remote inventory contract

The registered owner and temporary desktop bridge now dispatch bounded remote inventory reads through the existing coordinator/RPC and whitelist the phone projection. Cloud target/operation/job/cancellation and internal run/assignment/attempt/agent identities are omitted; opaque original-request/root-task/project identity, handle-availability flags and cleanup/stop state remain. Structural query and retained cursor checks enforce the same collection identity. Protocol is **43** and generated bindings are current.

**108 protocol tests and 5 real owner/core inventory tests pass**. The full protocol run also exposed two baseline assertions expecting the old 13-entry catalogue and Waku branding; expectations now verify the existing 14 provider IDs (including Nebius) and ion translations. No provider catalogue or adapter was added. Locked desktop/daemon/worker compilation and full mobile/gateway/client TypeScript checks pass. Gateway/mobile branch integration follows separately.

## 2026-10-12 — merged gateway remote inventory verified

The independent `codex/ion-gateway-remote-controls` branch, commit `8ce309b`, was merged at `c0f9109` into `codex/ion-agent-teams`, alongside the native protocol43 projection. Gateway phones with Usage read can page and filter redacted original remote operations. Task navigation additionally requires Task read and pins the published owner/project; existing controls retain separate Task control grants and fresh runtime/nonce/foreground checks. No new recovery, GPU cleanup or launch authority is exposed.

**268 distinct tests passed across this checkpoint:** 108 protocol, 5 owner/core remote collection, 17 native document/browser, 40 speech, 70 merged gateway and 28 merged mobile/shared-client tests. The last approval-review usage interruption was resolved after continuation; the merged gateway/mobile tests, full TypeScript checks, protocol43 freshness, formatting and frontend source checks were rerun successfully. Locked native compile gates passed before the TS-only gateway merge. No live cloud/provider/microphone/device or visual test was used.

[Gateway review](43-gateway-remote-controls.md), [document review](42-native-document-rendering.md), [speech setup/review](41-speech-service-acceptance.md). Remaining host/resource/egress, GPU launch/acceptance, billing settlement and direct-phone document parity development continues.

## 2026-10-12 — verified aggregate command sandbox caps

Private owner command-image resource policy can now opt into real cgroup2 aggregate CPU bandwidth, charged-memory/no-swap and process/thread caps across the whole sandbox, including supervisor and descendants. The pinned image marker is checked before reservation; supervisor and demoted guard verify root-owned finite kernel controls before arbitrary code. Caps cannot come from tool arguments, and omitted historical fields preserve original signatures/recovery. This is a read-only verifier for configured runtime caps, with no host provisioning or new provider changes.

**69 tests passed** across pure policy/source workers, installed Linux group/CLI/socket boundaries and native owner command paths. Native cases prove actual CPU throttling, fork ceiling, OOM-kill accounting, nonroot identity and refusal to change controls. One optional native owner-archive test and two Rust fixture-export tests remain unrun. No Nebius/live resource, credential, watcher or visual test was used. [Configuration and review](45-sandbox-resource-groups.md). Cross-run host/project admission, storage totals, wider egress/GPU/CUDA and billing finality remain open.

## 2026-10-12 — merged phone artifact library and sandbox verification

The independent `codex/ion-phone-artifacts` branch was committed at `9b52c5f` and merged into this implementation branch at `f2e6cc8`. Direct phones can now browse proven agent publications across projects/tasks/formats, verify the complete original bytes, read Markdown/JSON/source and export originals in bounded chunks. Private owner/address, foreground, route and generation fences cover the read, digest and file picker/write paths. Protocol is **44**; legacy default-only provenance does not become AI publication evidence.

The sandbox verifier now also checks that the verifying process belongs to the exact kernel group whose controls were read. **104 distinct tests pass for this increment:** 9 native artifact collection/integrity tests, 26 phone library/reading/export tests and 69 command policy/native Linux/owner tests. One optional owner executable-archive and two Rust fixture-export cases remain explicit unrun gates. The merged core tests, locked desktop/daemon/worker build, protocol44 binding freshness, full mobile/gateway/client TypeScript, formatting and source checks pass. Shared Cargo protocol cache was refreshed before compiling this checkout to avoid another worktree's old bindings.

[Phone review](44-phone-artifact-library.md) and [sandbox setup/review](45-sandbox-resource-groups.md). Native phone HTML/PDF rendering, macOS/native/device acceptance, wider cross-run host/project/storage/egress/GPU enforcement, billing reconciliation and actual shared speech deployment remain open. Additional providers stay deferred. No live provider, paid resource, watcher, microphone, physical device or visual test was used.

## 2026-10-12 — billing discovery and phone HTML/PDF integration

Verified phone PDF pages were committed at `91a03a9`. The independent structured HTML branch `84016e8` was merged at `fe47f6d`; the independent native billing discovery branch `c3ad30f` was merged at `72ba192`. All three capabilities are in `codex/ion-agent-teams`, with direct wire protocol **45** and freshly generated combined bindings.

Native billing Discover uses one bounded GET of existing exports in the saved contract, private expiring/cycle-fenced continuations, narrow ready monthly FOCUS metadata and separately explicit Select/Sync. It creates no export or compute resource. Phone HTML Reading uses bounded offline semantic native rows in direct Artifacts and gateway workflow evidence, with exact Source/Export and an explicit styling limitation. Direct-owner phone PDF pages use the shared Windows/macOS worker renderer, reverified original publication and derived PNG SHA, page/zoom controls, bounded pixels/chunks and owner/foreground/generation fences. The desktop retains its direct BGRA pixel path; no new mobile dependency is required.

**197 distinct tests passed for this combined checkpoint:** 110 complete protocol tests, 27 core billing, 10 core artifact/PDF, 5 shared renderer, 43 merged phone document and 2 native billing reply tests. The protocol/core suites were freshly rebuilt from the merged root checkout after narrowly refreshing the shared protocol cache. The two native billing reply tests passed in their independent branch; the combined desktop/daemon/worker build also passed. Full mobile/gateway/client TypeScript, protocol45 `export_types --check`, Rust/Prettier formatting, frontend source checks and whitespace/UTF-8 checks passed. Existing Rust warnings remain.

[Billing setup/review](46-billing-export-discovery.md), [HTML Reading review](47-phone-html-reading.md), [PDF review](48-phone-pdf-pages.md). No visual/physical-device test, watcher, live provider/account, microphone, deployment or paid resource was used. The full implementation remains in progress: owner IAM vault/refresh and job/account/tariff/finality reconciliation, wider host/project/storage/egress/GPU/CUDA controls, gateway PDF pages/larger formats, macOS/device acceptance and actual shared speech deployment remain open. Further provider adapters stay deferred.

## 2026-10-12 - combined protocol46 resource, gateway PDF and speech preparation

Merged speech preparation `5bfa432` at `71a3301`, command admission `ebdfeb4` at `b69e194`, and gateway PDF `1596955` at `e5a0a95` into the current implementation branch. Root adds a cached native Command resources view, project filtering and bounded phone-build preflight. These changes are committed on `codex/ion-agent-teams`; no merge into `ion-prod` is included.

Command admission reserves owner/project allocations atomically across runs and targets before submission/upload. Unknown historic quantities block checked admission, outstanding approvals intersect, and verified original results release compute while retained storage stays held. This is logical reservation policy, alongside existing verified kernel group limits, rather than a physical host quota or final provider bill.

Gateway PDF previews add page/zoom through immutable source-verified image transfers, a bounded expiring owner cache and an evidence-only read grant. Durable PDF receipt pruning retains bounded nonce/query tombstones, WAL/FULL and other history. The largest real HTTP fixture passed in its independent branch: two 12 MiB pages, 768 chunks, both complete digests and eight pending maximum, 70.092 seconds transfer/70.844 seconds including cleanup. Actual device/internet performance stays open.

The real installed Windows CPU speech binary/index/741,548,352-byte model passed metadata/checksum inspection. A credential-free source package of 26 files/189,961 bytes was generated and verified by its copied command. Preflight confirms declared Expo 57 dependencies and a nonempty Java executable; Android SDK is missing, local iOS compilation is unavailable on Windows, and phone observations remain pending.

**398 distinct merged tests pass:** 112 full protocol, 5 shared renderer, 21 admission SQL, 35 command/coordinator/HTTP, 6 native gateway/cache, 10 direct-owner artifact/PDF, 65 phone document/history, 73 complete gateway and 71 Node speech/preflight. Two existing exported native CLI fixture cases are ignored. One additional unchanged large HTTP test passed on the feature branch and was excluded from the concurrent merged rerun. Protocol 46 generation/freshness, locked desktop/daemon/core harness worker binaries, full TypeScript, formatting and UTF-8/whitespace checks pass. Native test binaries were rebuilt from this merged source after refreshing the shared protocol cache. Existing Rust warnings remain.

Review guides: [gateway PDF](49-gateway-pdf-pages.md), [speech/device tooling](50-speech-deployment-device-review.md), [command admission](51-command-resource-admission.md), [physical device setup](52-physical-device-review.md) and [native resource Usage](53-command-resource-usage.md).

The full goal remains active. No live cloud/provider, target deployment, real spoken ASR, microphone, physical-device/macOS or visual acceptance was performed. The owner has no Nebius/SSH/HTTPS target yet; native builds and actual observations for iPhone 15/Moto G32 are outstanding. Broader physical host/egress/GPU/storage cleanup and owner credential/billing settlement work remain open. Further harness adapters stay deferred and the historical stashes are untouched.

## 2026-10-13 - installed strict GPU runtime integration

Merged `codex/ion-gpu-runtime-boundary` (`fcff7b9`, `e214432`) and `codex/ion-gpu-file-confinement` (`f65446c`, `1318e4b`) into this implementation branch. New GPU approvals bind the canonical image profile; the supervisor and demoted guard verify the same runtime snapshot before strict IPC/mount/device syscall controls and Landlock. Original ordinary jobs and omitted historical recovery witnesses retain their contracts. Agent GPU launch remains gated pending actual CUDA/provider acceptance.

**162 distinct increment tests pass**, including 7 complete installed strict-worker cases, 33 snapshot and 25 file/device cases on actual Linux ABI7, 22 ordinary installed guard/CLI cases, 11 portable worker/policy cases and 64 fresh Rust GPU tests. One optional owner-generated Git archive case stays skipped. Locked desktop/daemon/core worker binary checks pass. The scanner now accepts literal bracket filenames while still refusing owner profile patterns.

Actual installed Windows CPU Nemotron recognition also passed single/two-concurrent clients with WER0 on fixed synthesized English audio, committed at `90ce62c`. This does not establish human microphone, phone, TLS or Nebius acceptance. [Recognition evidence](54-native-speech-recognition.md).

[Strict GPU setup, evidence and later review](57-strict-gpu-worker-integration.md). Actual CUDA image/driver/runtime and aarch64 acceptance, physical host/VRAM/storage/egress controls, provider-supported storage cleanup, owner IAM/billing finality and shared speech deployment/device review remain open. No actual cloud/GPU resource, paid allocation, visual test or watcher restart occurred. No `ion-prod` merge is included; additional harness adapters remain deferred. The full goal remains active.

## 2026-10-13 - GPU sandbox aggregate CPU/RAM/task caps

New GPU approvals bind `child_group` to the exact original launch environment and allocated shape. The root supervisor and demoted fixed guard independently verify actual private cgroup2 finite CPU/charged-memory/task controls with zero swap before user code. Limits are supplied by the runtime before startup; Ion reads them and does not modify host controls. Larger GPU policy bounds do not relax ordinary command bounds. Historical omissions retain their original serialized bodies for recovery; new preflight/pricing require approval.

**109 distinct current-source tests pass:** 66 fresh Rust GPU launch/recovery/result/approval cases,14 installed strict-worker cases,6 ordinary group,9 ordinary guard and14 portable worker/policy cases. Actual kernel counters prove CPU throttling and OOM enforcement; forks hit the task ceiling; mismatched limits refuse in both supervisor and demoted guard. Locked desktop/daemon/core worker binaries and Rust formatting pass. Prior162 case total corrects two duplicated runtime-test passes; current counts are deduplicated by full name.

[Setup, evidence and review](58-gpu-sandbox-resource-limits.md). Actual CUDA/aarch64/provider acceptance, VRAM/HugeTLB/physical capacity/persistent-storage/egress enforcement and owner billing/credential work remain open. No paid allocation, cloud deployment, visual test, watcher restart or `ion-prod` merge is included. Speech needs a deployment target/HTTPS endpoint and native iPhone 15/Moto G32 builds/device review. Additional harness adapters remain deferred.

## 2026-10-13 - dedicated GPU workspace storage ceilings

New GPU policies bind a versioned `child_storage` object to the exact allocated disk shape and launch environment. Supervisor and demoted guard verify a dedicated fixed `/tmp` filesystem, actual kernel type/device/security flags, unique backing with no nested mount, finite allocated-block/inode geometry and the same original capacity witness before user code. Work/home/temp stay on that backing under existing Landlock. The operator configures the filesystem before startup; the verifier does not modify host disks. Historical missing approvals preserve original recovery bodies; new preflight/pricing require exact storage approval.

**98 distinct cases pass:** 68 fresh current-source Rust GPU tests, 19 installed strict worker, 3 portable storage schema/platform, 7 existing worker cases and 1 actual unsuitable-mount test exercised in 4 separate geometries. Native proof includes real block/inode exhaustion, capacity-witness tamper refusal in the demoted guard, unchanged file/device/CPU/memory/task controls and valid checkpoint output after capacity denial. Final unsupported XFS rejection and complete GPU regressions are rerun; locked native binaries and Rust formatting pass. [Configuration/evidence/review](59-gpu-workspace-storage-limits.md).

Current native storage proof uses CPU tmpfs fixtures. Actual dedicated ext4/CUDA/aarch64/provider acceptance, XFS quota support, driver/VRAM/HugeTLB/host quotas, persistent collector/upload retention, supported cleanup and billing/credential work remain open. Speech still needs a deployment target/HTTPS hostname and native iPhone 15/Moto G32 review. No paid/cloud allocation, visual test, watcher restart or `ion-prod` merge is included; further harness adapters remain deferred.

## 2026-10-13 — manual artifact and collector retention

Implemented on `codex/ion-agent-teams` with **keep until explicitly cleaned** as the owner default.

- Native Artifacts → Manage storage: bounded owner pages, project/task filters, protected states, review/confirm cleanup and exact pending retry.
- Preserve original publication/evidence/source dedup/tool receipt identity; clean only Ion private content. No automatic expiry. Active/connected tasks, team reviews, workflow handoffs and original job recovery summaries are protected.
- Atomic BLOB cleanup and durable file claims; source reads/exports/PDF/model boundaries and old team publish requests refuse cleaned content. Native JSON automation statuses and historical status representations are covered.
- GPU collectors require explicit encoded-byte/count ceilings before new launch. All original receipts remain reserved across uncertainty, job cleanup and billing settlement; intersecting prefix caps cannot be loosened by a later allowance.
- Usage → Remote executions → GPU job → Collector storage exposes original sanitized reservations/observations and a virtualized scope list. Remote collector deletion and storage hold release remain unavailable.
- **123 distinct cases pass**: 51 retention-selector cases (25 artifact + 25 collector + one existing command-admission regression), 61 native GPU regressions, 10 artifact-library regressions, and one daemon provider-protection case. The final corruption-fixture correction only changes isolated test setup; related production behavior was unchanged during that correction.
- Protocol47 TypeScript generation/freshness and desktop/gateway/mobile typechecks pass. Locked desktop/daemon/core worker binary checks, Rust formatting and diff checks pass.

Owner review: [artifact retention](60-artifact-retention.md#owner-review-checklist) and [collector retention](61-gpu-collector-retention.md#setup-and-review-checklist).

Provider-supported remote collector/version/upload deletion, physical storage accounting and hold release remain open. Actual CUDA/ext4/macOS/phone acceptance, owner IAM/billing finality, and a speech deployment target/HTTPS hostname remain separate dependencies. Additional harness adapters remain deferred.

## 2026-10-13 — first-launch connection onboarding

- Fresh desktop installs show Welcome → AI providers → Apps → Speech → Personal gateway. All screens can be skipped; configured harnesses and the normal New task flow remain available.
- Missing, failed-test or session-only details keep Finish setup visible in the sidebar, collapsed-sidebar header and Settings. Settings → General can reopen setup even when all details are saved.
- Reuse masked vault-backed fields, existing Save/Test and gateway Pair actions. Native provider/app settings remain the place for harness authentication, individual app sign-in and agent target/budget configuration.
- Preserve only nonsecret version/screen/dismissed progress through existing settings; unfinished setup resumes, skipping stays dismissed, and existing active profiles keep their workspace.
- Credential/status work stays in background workers; rendering reads four cached states. Exclude pending credential actions from status refresh and keep native webviews hidden beneath the setup page.
- **27 client tests pass**, including 7 new onboarding/persistence cases. Final desktop/daemon binary checks, Rust formatting, protocol binding freshness and source diff checks pass. Protocol47 wire shapes are unchanged.

Owner [setup review checklist](62-first-launch-onboarding.md#owner-review-checklist) covers first-launch/restart, skip/later configuration, failed tests, session-only vault limitations, keyboard/themes and native webviews. No live key, endpoint, visual or device acceptance is claimed.

## 2026-10-14 — phone Office content reading

Direct Artifacts and gateway workflow evidence now extract bounded DOCX body paragraphs/table cells and XLSX sheet/cell/formula content after original SHA-256 verification. Native reading rows are virtualized, plain text remains inert, cached formula values and hidden sheets are labelled, and exact original export is preserved. ZIP/XML preparation yields and respects the existing owner/foreground/grant/expiry generation fences. Limits and unsupported Office fidelity are visible; native model replies continue to carry the original explicit summary.

Fifteen new Office regressions, full mobile TypeScript and a browser parser bundle pass. Existing document/export/HTML/image/PDF regression results and physical-device review are in [the release checklist](20-agent-teams-release-checklist.md) and [Office guide](67-phone-office-reading.md). No visual test, paid resource, watcher restart or ion-prod merge occurred. The wider goal remains active, including remaining HTML resource references and native harness contracts/acceptance.

## 2026-10-14 — inline SVG raster bundle references

The portable HTML tool now embeds pinned raster href/xlink:href references in inline SVG image and filter-image elements. Phone preparation preserves those embedded rasters while retaining the existing script/network isolation and same-document fragment behavior. Resource kind is validated before encoded-cache reuse, preventing a cached stylesheet/font from being accepted as an image.

Eleven native bundle regressions, fourteen phone HTML cases, mobile TypeScript, Rust formatting and locked desktop/daemon binary checks pass. [Resource scope and owner review](65-portable-html-bundles.md) retain external SVG symbol/sprite files and standalone SVG dependencies as unfinished. No visual/device acceptance, provider-account inference or ion-prod merge occurred; remaining harness contracts remain open.

## 2026-10-14 — native Windows ACP launch

The shared ACP launcher now uses the selected native executable on Windows, fixing its unconditional Unix env launch. New/load/resume retain the exact protocol workspace. Runtime and catalog/history share this path; existing native account/model negotiation is retained.

The real Windows SDK/stdio fixture verifies session creation, Unicode/special-character workspace propagation, streamed text, turn completion and shutdown. **21 ACP cases pass; 3 authenticated-provider cases are skipped.** Locked desktop/daemon binary checks, Rust formatting and diff checks pass. [Setup and owner review](68-windows-acp-startup.md) explicitly retain real provider startup configuration, resume and descendant-process Stop acceptance.

[Current harness coverage](64-remaining-harness-integrations.md#current-implementation-coverage) now records 12 bounded completion dispatches and 7 source-task Teams connections from current source. Amp/Cursor bounded completion contracts and seven further source-task connections remain development work. No live account/model request, visual test, watcher restart or ion-prod merge occurred. The full goal remains active.

## 2026-10-15 — DeepSeek normal-task Teams bridge

Additive temporary native tools now bind DeepSeek source callbacks to the owner's live main session, call ID and exact arguments. The shared worker retains approval/scope intersection, configured limits and durable replay. Stop, failed native results, turn end and host closure revoke callbacks; native account/model/profile remains selected.

**38 distinct cases pass**, including the actual pinned native SDK runtime plugin/catalog/approved invocation/stale-definition fixture. Locked desktop/daemon build and formatting results are recorded in the release checklist. The separate default-web round-trip still fails in that downloaded SDK executable because its session-title plugin dependency is absent; full native installation/owner acceptance is open. [Evidence and owner review](69-deepseek-source-team-bridge.md). Six source-task bridges, Amp/Cursor completion contracts, remaining document resources and physical-device/live-account review remain open. No ion-prod merge, paid inference or visual test occurred.

## 2026-10-15 — portable SVG image dependencies

Pinned static SVG images now embed their own local raster/vector/CSS/font dependencies in portable HTML snapshots. Strict XML preserves case/namespaces, local IDs, Unicode and named views; original files remain intact. Phone preparation validates embedded SVG image/responsive attributes in yielded bounded stages, including cached nested-depth checks, within existing owner/grant/foreground generations.

**34 distinct cases pass** (14 native bundle/publication/worker and 20 phone HTML/SVG), plus full mobile TypeScript, locked desktop/daemon binary checks and Rust/Prettier/diff checks. [Scope and owner review](70-portable-svg-images.md) records exact bounds, CSS image-context limits and original-versus-snapshot behavior. External SVG sprites, remaining native harness requirements and live account/device acceptance stay open. No visual test, paid resource, watcher restart or ion-prod merge occurred.

## 2026-10-15 — pinned SVG sprite and paint/path references

Portable HTML now inlines inspected SVG reference documents once and rewrites local symbol/use, text path, gradient/pattern and CSS paint/filter/mask/clip references. Private IDs/classes, original-value attribute snapshots, zero-specificity scope markers and font-family aliases retain referenced styles without changing source document nodes. XML/CSS/resource bounds, original files and exact publication/replay remain enforced.

**42 distinct checks pass**: 19 native bundle/publication/worker, one production fixture export, 21 phone HTML/SVG and one actual native-output-to-phone case. Locked desktop/daemon binaries, full mobile TypeScript, Rust/Prettier formatting and diff checks pass. [Review and fidelity limits](71-portable-svg-references.md) record physical-device shadow styling, wider global CSS names and remaining harness contracts. No visual test, paid resource, watcher restart or ion-prod merge occurred. The full goal remains active.

## 2026-10-15 — OpenCode 2 normal-task Teams bridge

- [x] Additive native project tools, memory-only callback binding and independently witnessed main-session/message/call/name/arguments.
- [x] Owner approval, durable replay, changed/foreign/failed/provider-hosted call refusal, Stop/terminal/disconnect fencing and mode-change restart.
- [x] Shared project-plugin leases keep files alive during driver replacement and remove only unchanged owned files after the final driver; existing owner plugins remain untouched.
- [x] Released native `/api/info` fallback retains process identity checks; strict inbox decoding supports native `time.created` and legacy `timeCreated`.
- [x] 102 distinct checks pass, including two actual Windows 2.0.29 driver/fake-model fixtures for approved invocation/result return and Stop. Native stale generation, owner plugin preservation, owned-file cleanup and adopted-service survival pass.
- [ ] Complete [authenticated model/account, native resume/hot-upgrade and platform review](72-opencode2-native-plugin-contract.md#owner-review). Configure the V2 executable explicitly; unverified native versions expose Teams-unavailable activity.
- [ ] Implement the five remaining source bridges (Cursor, Antigravity, Fx, Grok, Kimi), native Amp/Cursor completion contracts and the remaining document rendering/full-goal requirements. This checkpoint does not close the active goal.

## 2026-10-15 — portable document CSS names

- [x] Give direct/literal imported-SVG keyframes and custom counter styles a per-file identity across inline CSS, shared imports and nested rules; source HTML names remain native.
- [x] Preserve shorthand roles, case/escapes/quoted names, keyframe selectors, priorities, marker/content strings, invalid/reserved rules and predefined counter behavior.
- [x] 50 distinct native/phone checks pass, including two actual production-native fixture exports, trusted-worker publication/replay, phone preparation and independent CSS Tree 3.1.0 grammar with an invalid-value control.
- [x] Locked offline desktop/daemon checks, full mobile TypeScript, Rust formatting and diff checks pass.
- [ ] Complete [device/browser and CSS variable/scope acceptance](73-portable-css-names.md#owner-review). Variable-based animation/counter/font names, property registrations, layer/container/scope names and browser use-shadow behavior remain open.
- [ ] Remaining native harness bridges/contracts and full-goal provider/platform acceptance remain open; this increment does not close the active goal.
