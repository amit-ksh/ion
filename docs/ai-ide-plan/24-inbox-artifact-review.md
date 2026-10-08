# Inbox state and artifact integrity

Updated 2026-10-08. Implemented in the `codex/ion-agent-teams` worktree.
These changes have not been integrated into `ion-prod`.

## Available behavior

Inbox keeps read, acknowledge, archive and restore state in the owning daemon's
SQLite store. It has its own revision; organizing a notification never approves
a decision, accepts a proposed task, starts work or changes team definitions.
The default view includes all projects and tasks. Project/task/type/status filters
remain available, alongside Inbox, Unread, Acknowledged, Archived and All
notifications views. A changed source event starts unread.

The Inbox notification menu offers All task notifications, Issues only and
Notifications off. These preferences govern the existing desktop system task
completion/background notifications; Issues only retains failed completion
notifications. They do not mute pending work in Inbox or Decisions. Preferences
load before system task notifications are sent and survive owner/app restart.

Current builds use protocol **35**. The owner now keeps a paged historical
journal of observed persisted task lifecycle changes, proposals, team assignments,
team planning decisions, workflow/routine states and agent document publication.
Each source write and its journal fact share a transaction. A resolved or removed
source clears its pending flag while retaining previous facts and read state.
Metadata-only task saves and replayed source writes do not create duplicate facts.
Existing stores backfill the current facts once; transitions from before this
upgrade are not reconstructed. Task facts use a durable bounded outbox across the
task/control databases; counts can increase while “Importing task history” is shown.

Desktop and phone show 32-row pages and counts across all projects. Filters use
indexed bound SQL on the owner, and both clients render cached/virtualized rows.
Historical item controls use independent expected revisions with safe identical
retries. Organizing history has no 10,000-item cap; the older system-notification
preference catalog retains its compatibility limit. Notification delivery policy
does not hide pending actions or change execution authorization.

Phone adds Inbox to the task drawer, foreground-only polling, persistent controls,
and owner source review for planning decisions and proposed tasks. Decision
approval uses the actual decision revision. Accepting a proposed task records its
verdict; “Prepare New task” loads it into the existing unsent composer, where the
user chooses execution and presses Send. An existing draft is preserved and must
be cleared/saved before loading the proposal. Profile/connection and foreground
changes disable stale controls; deleted sources retain history but cannot be
approved. Workflow gate review uses the existing run screen. Phone system Push
delivery and rich embedded document previews remain development.

Artifacts are published by agents. Native task document capture retains its
turn/activity/source path; team documents retain their run, assignment, agent and
publication request. New documents include original byte count and SHA-256.
Clients cannot upload artifacts or overwrite their original metadata/evidence
through workspace-record commands. Existing agent publication routes remain.

Document bytes are checked against their original evidence on read/export,
assignment submission, parent acceptance, normal-task team readback and workflow
handoff. Private files and SQLite blobs are bounded before payload allocation.
Changed bytes stop the read or transition; they are never handed forward as the
original report. Existing documents without evidence remain readable within the
original size limits and are labeled Legacy document / integrity unavailable.
They are not silently upgraded to verified provenance.

Captured agent files support HTML, PDF, Markdown, text, Word, CSV and JSON within
the existing 16 MiB bound. Team text publication supports HTML, Markdown, text,
CSV and JSON within 64 KiB. This increment does not interpret image/PDF inputs,
add embedded rich previews or implement the cross-project cost ledger.

## Your review checklist

- [ ] Rebuild matching protocol-34 desktop/daemon/phone clients and open Inbox. Confirm the default
  scope is All projects / All tasks, then combine project/task/type/status filters.
- [ ] Mark a notification read; acknowledge another; archive a third. Restart the
  app and daemon and confirm their state. Restore an archived notification and
  use Mark unread to reset its read/acknowledge state.
- [ ] Archive a pending approval/proposal. Confirm it is still pending in
  Decisions and its task; acknowledging it must not authorize or start work.
- [ ] Complete another turn in a task with an archived earlier notification.
  Confirm its new notification is unread and the earlier archived fact remains.
- [ ] Page across more than 32 notifications, combine project/task/type/status
  filters and confirm the global unread count remains independent of the page.
- [ ] Resolve a decision, complete a workflow and delete a disposable task.
  Confirm their earlier facts remain and only current unresolved work needs action.
- [ ] Connect the phone to the same owner. Read/archive there, refresh desktop and
  confirm shared state. Review a team decision and approve/decline its exact source.
- [ ] Accept a phone proposed task and use Prepare New task. Confirm it fills an
  unsent composer, preserves an existing draft and requires Send to start work.
- [ ] Background/reconnect/switch the phone owner during a delayed read or action.
  Confirm stale responses cannot enable controls, navigate or alter the new owner.
- [ ] Open a removed source from history and confirm its unavailable message;
  archived unresolved work must still appear in Decisions.
- [ ] Change a notification on another connected client. Confirm a stale changed
  action requests Refresh and cannot overwrite it; an identical retry is safe.
- [ ] Choose Notifications off, background the desktop and complete/fail a task.
  Confirm system task notifications are muted but Inbox still shows its state.
- [ ] Choose Issues only and then All task notifications. Confirm failed completion
  remains announced under Issues only and ordinary completion is announced under
  All, while the app is in the background. Confirm the preference survives restart.
- [ ] Ask a normal agent to create a report, then a team to publish a document.
  Confirm Artifacts shows their format, byte count and appropriate producer label;
  open/export each and follow its task link. No user-upload control should appear.
- [ ] In a disposable run, change stored document bytes after publication. Confirm
  read/export and result review reject it. For a completed workflow stage, confirm
  the next stage blocks until the original accepted evidence is restored.
- [ ] Open a pre-existing document without evidence and confirm its legacy label.
- [ ] Verify keyboard operation, readable text in both themes and long Inbox lists
  in the freshly rebuilt app managed by the existing development watcher.

## Automated evidence

Six history owner/integration tests cover immutable past facts after resolution
and deletion, task outbox restart/replay and bounded backlog, indexed paging and
project/task/type/state filters, global counts, independent concurrent client
controls, actual workflow gates/team decisions/documents and stale-lease rollback.
The three prior Inbox compatibility tests remain relevant; read/ack/archive never
approves execution. Workflow/routine and team decision regressions, matching
native/daemon/shared-client/phone builds and exporter checks are recorded in the
progress document when complete.

Prior artifact evidence tests and their bounds remain unchanged. No visual test,
phone device acceptance, live provider, cloud deployment, Push service or second
development watcher was performed. The manual review boxes remain unchecked.
