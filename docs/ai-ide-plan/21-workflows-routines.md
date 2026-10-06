# Workflow and routine execution

Updated 2026-10-06. Development is on `codex/ion-agent-teams`; integration into
`ion-prod` and live acceptance remain separate. Protocol **27** requires matching
desktop, daemon and clients.

## Available behavior

- Generate editable workflow stages from a prompt with the existing Nebius
  generator, or create them manually. Assign a saved team to every stage. Each
  team retains its per-agent harness, model, environment and grants.
- **Run now** creates a durable automation run. Stages launch real Teams tasks
  sequentially. The run pins the workflow, teams and profiles; editing a saved
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
  paused. **Enable schedule** arms an elapsed interval; **Pause** prevents future
  firings without stopping an existing run. **Run now** is available separately.
- Intervals use UTC elapsed time, independent of daylight-saving changes. The
  interface shows the next firing in UTC. After downtime, missed intervals
  coalesce to the latest scheduled slot. An active or blocked previous run causes
  the new firing to be recorded as skipped; it never overlaps.
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

Calendar/timezone schedules, explicit catch-up/overlap policy choices, conditional
workflow graphs, bounded stage retries, required document-format gates, safe code
integration across stages, additional app binding controls, notification policies
and phone definition editing remain subsequent increments. Additional Teams
harnesses and a registered remote execution host are tracked in
[the release checklist](20-agent-teams-release-checklist.md). Executed test receipts
and monetary caps are still required. Presence of this workflow runner does not
claim those capabilities or a finished Ion release.

## Verification

Automated evidence is recorded in [implementation progress](19-agent-teams-progress.md)
when the checkpoint is committed. No live provider billing, app mutation, cloud
provisioning, visual test, device acceptance or second watcher is performed here.
