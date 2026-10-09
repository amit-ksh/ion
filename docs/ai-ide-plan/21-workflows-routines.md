# Workflow and routine execution

Updated 2026-10-09. Development is on `codex/ion-agent-teams`; integration into
`ion-prod` and live acceptance remain separate. Protocol **36** requires matching
desktop, daemon and clients.

## Available behavior

- Generate editable workflow stages from a prompt with the existing Nebius
  generator, or create them manually. Assign a saved team to every stage. Each
  team retains its per-agent harness, model, environment and grants.
- Each stage can require HTML, Markdown, text, CSV and/or JSON output. These
  requirements are edited on desktop, pinned for the run and visible on desktop
  and phone. A missing format prevents its team lead from submitting completion.
- **Run now** creates a durable automation run. Stages launch real Teams tasks
  along the selected forward path. The run pins the graph, teams and profiles; editing a saved
  definition affects later runs. The normal New task flow stays unchanged.
- A marked stage waits for an exact user decision before its team exists in the
  dispatch queue. **Open** the run to see its instructions and prior stage task
  links, then Approve stage or Decline. App mutations still require the exact
  action/account/arguments approval provided by the Teams adapter.
- Only a genuinely completed prior team permits the next stage. Its accepted
  UTF-8 reports are copied as bounded, immutable input data. Inputs do not grant
  access. Each coding stage retains its own isolated workspace; code changes are
  reviewed in its task rather than silently merged into the next stage.
- Workflow-wide call, token, assignment and time limits constrain later stages.
  Unknown usage keeps the original reservation. Stage limits also retain the
  selected team's smaller caps. Exhaustion blocks further dispatch.
- Create a routine with a saved team or same-project workflow. Saving keeps it
  paused. **Enable schedule** arms the selected interval or calendar rule; **Pause** prevents future
  firings without stopping an existing run. **Run now** is available separately.
- Intervals use UTC elapsed time. Calendar rules support daily, selected weekdays
  and selected month days at a 24-hour time in an IANA zone. Preview next runs
  asks the owner for the next three local-time/offset timestamps. The saved list
  shows the rule and next UTC firing. Missed and overlap policies are explicit;
  their defaults retain latest-slot catch-up and skip-overlap behavior.
- Firing identity, next schedule and automation creation commit together. Stage
  task IDs and the existing task outbox survive restart and lost acknowledgements.
  Unknown transport outcomes retain the same submission ID. A confirmed rejection
  and confirmed absence allow the user to correct and resubmit.
- Run history is paged. Dashboard, Inbox and Decisions receive the same cached
  run states; stage gates contribute to the decision count. Native actions use
  existing keyboard-operable controls and background, generation-fenced RPCs.
- On the phone, open task history → **Workflows & routines**. Browse saved
  definitions, their instructions and all-project or per-project run history.
  Run now, enable/pause an interval, inspect a run's stage instructions, approve
  or decline its exact gate, Stop run and open actual stage task transcripts.
  Definitions are created/edited on desktop in this increment.
- Phone start identity and definition revision are stored before sending. A
  disconnect, app restart or storage-cleanup failure retains the original ID;
  Run now retries that submission. Only a completed rejection plus confirmed
  absence clears it for a corrected request. Saved status cannot enable controls:
  a fresh response from this owner connection and foreground/focus generation is
  required. Hidden/background screens do not poll or navigate on old responses.
- Stop fences further stages before cancellation is propagated to the active
  team. Deadline cancellation is also replayable. Interrupted effects retain the
  existing Teams reconciliation rules. A final team that completed before its
  enforced deadline remains completed even if the owner observes it later; a
  late observation cannot launch another stage.

## Review checklist

- [ ] Create a two-stage workflow with a team per stage; gate the second stage.
- [ ] Run it and verify that every stage opens a real lead task with descendants.
- [ ] Confirm the second task does not exist before its gate is approved.
- [ ] Open its decision, inspect instructions and the first task's reports,
  approve, and verify the second team can read the pinned report inputs.
- [ ] Edit a profile/workflow while a run is active; verify only a later run uses
  the new definition. Decline a separate run's gate and verify no next stage starts.
- [ ] Set a small workflow call/token limit; verify exhaustion blocks the next
  stage and prior usage is retained. Stop and inspect the actual descendant state.
- [ ] Create a one-minute routine; verify saving does not start it. Enable it,
  inspect the next UTC firing and its run history, then Pause.
- [ ] Leave a routine's previous run waiting at a gate; verify later firings are
  recorded as skipped. Pausing its schedule must preserve that existing run.
- [ ] Restart the owning daemon after missed intervals; verify one latest firing,
  no backlog burst and no duplicate task IDs.
- [ ] Interrupt a Run now acknowledgement and retry; verify one automation run.
- [ ] On the matching phone build, open task history → Workflows & routines.
  Run a saved workflow, open the actual stage task and review/resolve its gate.
  Enable/pause a saved routine and inspect its recorded runs.
- [ ] Interrupt a phone start acknowledgement, close/reopen the app and use Run
  now again; confirm the same automation ID and original definition revision.
- [ ] Switch owner profiles or background the phone while a read/action is in
  flight. Confirm cached projections cannot approve, start, stop or alter schedules,
  and old responses do not navigate to another owner's task. Refresh on return.
- [ ] Check Inbox project/task/type/status filters, global Decisions, keyboard
  controls, light/dark contrast and long histories on the freshly rebuilt app.

## Remaining development

Binary document publication, safe code
integration across stages, additional app binding controls, background notification delivery
and phone definition editing remain subsequent increments. Broader remote execution and registered-owner integrations are tracked in
[the release checklist](20-agent-teams-release-checklist.md). Executed test receipts
and monetary caps are still required. Presence of this workflow runner does not
claim those capabilities or a finished Ion release.

## Verification

Automated evidence is recorded in [implementation progress](19-agent-teams-progress.md)
when the checkpoint is committed. No live provider billing, app mutation, cloud
provisioning, visual test, device acceptance or second watcher is performed here.

## Required stage documents (2026-10-08)

In the desktop workflow editor, use each stage's required-document menu. Select
one or more formats; an empty selection keeps the existing evidence behavior.
The prompt generator may suggest those same formats in an editable draft. It
cannot activate the workflow or claim connected-app access.

Run now copies every stage's requirement into the immutable automation run and
its actual lead assignment output contract, in the same transaction that creates
the task. At least one original generated artifact of every selected format must
be referenced in its completed result. A text summary or declared check cannot
replace a required document. Accepted specialist documents can satisfy the lead's
contract under the existing assignment scope and parent review rules.

Requirements are rechecked when the owner advances the workflow, including its
final stage. Original hashes, source/task ownership and stored metadata must
remain valid. Missing/changed bytes block progression; they do not start another
attempt or repeat a side effect. Restore original accepted evidence when possible,
or stop the run and begin a corrected workflow. Existing call/token/time limits
and human/app approvals remain authoritative.

Editing a saved stage changes future runs only. Empty/missing requiredFormats
retains the original legacy serialization and behavior. Duplicate, unsupported
or oversized format lists fail before the definition revision changes. Supported
formats match current team text publication; PDF/Word/binary publication and rich
preview/interpretation remain separate work. A format/hash gate proves original
stored output, not semantic document quality or successful deployment. Review
content and any required executed checks separately.

### Your review

- [ ] Create a two-stage workflow, with HTML and Markdown required from stage one
  and JSON required from the final stage. Choose a saved team for each stage.
- [ ] Run it and inspect the required formats in the actual lead task and desktop
  run detail. Verify the phone shows the same pinned requirements before approving.
- [ ] Ask for only a summary or the wrong format. Confirm completion is rejected
  and the next stage stays absent. Supply original documents of all required
  formats and confirm the run advances within the same limits and approvals.
- [ ] Edit or clear saved requirements during the run. Confirm its existing stages
  retain the original contract and a newly started run receives the edit.
- [ ] Remove/change a generated document in a disposable run after its team completes
  but before the owner observes it. Confirm the workflow blocks, including when
  it is the final stage. Restoring original bytes must not create another team.
- [ ] Open an older workflow with no requirements, then save/restart it. Confirm
  it retains its existing behavior; selected formats and human gates survive restart.
- [ ] Verify keyboard operation and both themes on desktop, then the actual phone
  screen. No visual/device acceptance was performed during implementation.

Verification: sixteen owner/workflow/routine tests passed, including two new
contract/legacy/final-stage tests and an actual two-stage dispatcher run requiring
Markdown from both leads. Desktop/daemon compilation, regenerated binding equality,
shared/full phone TypeScript, formatting and whitespace passed. No live model key,
app account effect, deployment, visual test or second watcher was used.

## Calendar schedules and run policies (2026-10-08)

In the desktop routine editor, choose Every few minutes, Daily, Days of the week
or Days of the month. Calendar rules take an IANA zone such as `Asia/Kolkata`,
a 24-hour `HH:MM` time and selected days. All rules are saved paused. The owner
validates the zone and rule before changing the definition revision. Calendar
calculation and preview RPC run on workers; row rendering reads cached data.

**Preview next runs** returns exactly three future fires from the owning daemon.
Editing a schedule clears its preview; a delayed response cannot replace the
preview for another draft, form or owner. These timestamps are a preview, not
reservations. Enabling computes the next future time at the owner. Pausing clears
that next time while preserving active runs. Run now starts immediately.

Nonexistent local times during a daylight-saving gap are skipped. A repeated
local time fires at its earlier offset once. Monthly dates absent from a month
are skipped, including February 29 in a non-leap year. The pinned Jiff bundled
IANA database supplies these calculations without a network/system-zone lookup;
see [Jiff's time zone API](https://docs.rs/jiff/latest/jiff/tz/struct.TimeZone.html)
and [ambiguity handling](https://docs.rs/jiff/latest/jiff/tz/enum.Disambiguation.html).
Zone data updates require updating the bundled dependency and matching builds.

- **Missed run: run latest** coalesces downtime into the latest due slot, with no
  backlog burst. **Missed run: skip** records a skipped firing when that slot is
  more than 60 seconds late; the one-minute grace permits ordinary scheduler delay.
- **Overlap: skip** records a skipped run if the previous one is still active,
  blocked or waiting for a decision. **Overlap: allow** creates another run under
  existing shared agent capacity and each team's budget. It does not increase
  agent concurrency, grants, permitted environments or spending limits.
- A run pins the exact interval/calendar and both policies. Future edits do not
  change that run's history. Manual Run now ignores missed-time handling and still
  follows the selected overlap policy. Phone run detail displays the pinned trigger.
- Fire identity, schedule advancement and run creation share one SQLite transaction.
  Concurrent owners/restarts do not create another task for the same slot. An
  invalid stored rule pauses only its own routine and records the error.
- Legacy interval records omit the new default fields. Matching protocol **32**
  desktop/daemon/phone builds are required so calendar rules cannot be mistaken
  for intervals by an older client.

### Your calendar review

- [ ] Save a daily `09:00` routine in `Asia/Kolkata`; preview three runs and confirm
  their `+05:30` offset. Change time/zone and verify the old preview disappears.
- [ ] Select weekdays and monthly days `1, 31`; preview and verify unavailable dates
  are skipped. Confirm keyboard selection, field labels and text in both themes.
- [ ] Save, enable and pause; confirm saving starts nothing, the next stored firing
  is correct and pausing preserves any active run. Run now remains separate.
- [ ] Restart after downtime with Run latest and then Skip; confirm one traceable
  latest-slot run or one skipped record, without a backlog or duplicate team tasks.
- [ ] Allow overlap with agent parallelism one; verify a second task waits for agent
  capacity. Switch back to Skip and inspect the skipped reason in history.
- [ ] Edit the saved zone/policies while a run is active; desktop/phone run detail
  must retain the original trigger while a later run uses the new definition.
- [ ] Use `America/New_York` at `02:30` across a spring gap and `01:30` across a fall
  fold; the preview should skip the missing time and list the repeated time once.
- [ ] Try an unknown zone or invalid day/time; save must fail without changing the
  revision. Confirm an invalid routine does not stop other valid schedules.

Automated owner/math checks passed for IANA offsets, exact three-fire previews,
week/month dates, leap years, DST gaps/folds, bounded long-downtime catch-up,
concurrent restart deduplication, immutable triggers, missed-run grace, overlap
capacity, legacy defaults and invalid-rule isolation. Native/device review remains
unchecked; no visual test or live provider call was performed.

## Conditional workflow routes (2026-10-08)

Each stage now chooses an **After completion** route in the native editor:

- **Next stage** retains the linear default and legacy serialization.
- **Go to** selects a later stage or finishes the workflow.
- **JSON condition** names a generated JSON document, a JSON pointer such as
  `/release/ready`, a JSON scalar to compare and a destination for each result.
  For example, `release.json` at `/ready` equal to `true` can select deployment
  review, while `false` selects an investigation team. No fixed release/deploy
  workflow types are introduced. Prompt generation can propose the same editable
  typed routes; saving/activation still require valid teams and grants.

Graphs are bounded to 24 stages, forward only and rooted at stage one. Every
stage must be reachable by at least one possible path. Cycles, missing/backward
links, invalid pointers and non-scalar conditions reject before saving. Multiple
paths can join at a later stage. This runner follows one path at a time; parallel
fan-out and loops remain separate work. Optional retries before effects were
added in the 2026-10-09 checkpoint below.

The branch stage must publish the exact named JSON document and reference its
artifact ID in its accepted result. JSON becomes an implicit required format.
The owner first verifies completion, required documents, source scope and original
hashes; it then reads bounded original bytes and compares the scalar without type
coercion. Missing, invalid, ambiguous or changed documents block for review, rather
than selecting another path. A plain message or unaccepted file cannot choose a
transition. This verifies original evidence and a configured value, not whether
that evidence is factually correct or a deployment is appropriate.

The chosen destination, matched result, document ID and SHA-256 commit with stage
completion and run advancement. Restart/late acknowledgement retains that choice.
Editing a saved graph affects future runs only. Unselected stages are marked
Skipped by workflow route and have no Teams task. A selected decision gate still
waits before its team exists. Source/account/action approvals remain independent.
The selected predecessor's accepted documents are the next team's immutable input;
shared calls/tokens/assignment limits count the actual completed path, including
all earlier visited stages. A branch cannot reset the budget, grants or deadline.

Move up/down remaps links to preserve stage identity and rejects a move that would
create an invalid graph. Change incoming links before removing a linked stage.
Desktop and phone run details show the pinned rule, chosen destination, matched
result and skipped stages. Matching protocol **36** clients are required.

### Your branch review

- [ ] Generate or manually create four stages: decision document, ready-path team,
  investigation team and a shared final team. Link both alternatives to the final
  stage and choose explicit teams. Save and inspect the graph routes.
- [ ] Configure `route.json` at `/release/ready` equal to `true`; run once with true
  and once with false. Verify only the selected task exists; the other is skipped.
- [ ] Require user decisions on both alternatives. Confirm only the selected gate
  appears and no team starts before its exact approval. Decline/Stop a separate run.
- [ ] Inspect the chosen document ID/hash and result on desktop/phone. Restart or
  edit the saved graph after the document stage completes; the run's choice stays.
- [ ] Omit the named document/pointer, use a wrong type, submit duplicate names or
  change original bytes in a disposable fixture. Confirm the run blocks, without
  guessing a path or creating another stage task.
- [ ] Choose Finish workflow from one condition. Verify later unselected stages
  remain skipped and the run completes without dispatching them.
- [ ] Reorder alternatives and confirm links follow them. Try a backward/cyclic
  link, unreachable stage or removal of a linked stage; correct the shown error.
- [ ] Set small workflow-wide caps, branch around an unselected stage and verify
  actual prior usage still constrains the selected path and final team.
- [ ] Verify keyboard menus, labels in both themes and the actual mobile build.
  Live generation/provider/device/visual acceptance remains your later review.

## Optional bounded retries (2026-10-09)

Each desktop stage has a **Retries: off** menu. Choose one or two retries before
effects, with a 30-second delay. The owner supports at most three attempts and
a validated 1–300 second delay; stricter pinned team/workflow attempt limits still
apply. Older saved workflows and runs keep retries off. Phone definition/run
details show the original pinned policy; definitions edited during a run affect
future runs only.

Retry requeues the original assignment in its existing team and task. It keeps
its harness, model, environment, access, human stage approval, output contract,
original deadline and all call/token usage. The owner persists the failed attempt
and wait before returning. Repeated ticks and owner restart preserve the delay
and do not create another task or reset a budget. A new claim fences the old lease.
All attempts remain in task history, and usage is deducted before a later stage
can dispatch.

Only a confirmed failure before effects is eligible. Blocked/uncertain outcomes,
prior file/app/command/GPU effects, child delegation, unknown native remote hosts,
unmetered/excess usage, exhausted attempts/budgets and Stop remain blocked for
review. An automatic retry never repeats a successful or uncertain external action
or replaces recovery of a stopped checkpoint. Missing documents or failed semantic
checks are not reclassified as failures before effects.

### Retry review additions

- [ ] Confirm an old workflow and a newly created stage default to Retries: off.
- [ ] Enable a retry, induce a confirmed harness failure before effects and restart
  the owner during the delay. Confirm one original task/assignment is requeued once.
- [ ] Edit the saved retry policy during that run. Confirm its pinned policy and
  grants remain unchanged; new runs receive the edit.
- [ ] Exhaust the attempt and call/token limits. Confirm the workflow blocks and
  cannot create a new stage with a fresh allowance.
- [ ] Test an uncertain host or prior file/app/command/child effect. Confirm the
  original task requires review and the action is not replayed.
- [ ] Stop the run during its delay. Confirm it stays canceled after restart/ticks.
- [ ] Confirm native keyboard controls and matching phone policy display. No
  visual or device acceptance is claimed by compilation.

Code integration, parallel fan-out/loops, binary document publication, broader
GPU/gateway/cloud speech and authoritative billing still have development remaining.
Deferred new harness adapters remain outside scope.

Retry verification: **29 workflow/calendar/branch/history regressions passed**,
including all four new retry groups. The retry groups passed again after scoping
child-effect lookup to the original run; SQLite EXPLAIN changed from a history
scan to an indexed run/parent search. The existing exact manual retry/old-lease
fence passed. Desktop, daemon and native worker compilation, generated protocol
36 equality, shared/full phone TypeScript, Rust formatting and whitespace passed.
Six native usage receipt regressions also passed, including post-context-fill
Codex calls. No live provider, deployment, device/visual test or additional
dev watcher was performed.
