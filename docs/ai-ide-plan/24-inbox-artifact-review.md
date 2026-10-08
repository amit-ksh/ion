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

Notification preferences and item state are bounded to 10,000 event identities.
Concurrent client edits use an independent expected revision. Retrying an already
applied action after a lost response is safe; a conflicting edit requires Refresh.
At capacity, existing entries and the notification policy remain editable.

The current notification catalog is still a projection of current task state,
control records, and loaded run/decision pages. State is durable, but this is not
an append-only historical notification feed: a replaced source event can leave
that projection. A complete owner notification index, global unread badge,
paged historical feed and phone Inbox UI remain development work. No Push
notification delivery service is introduced by this increment.

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

- [ ] Rebuild matching desktop/daemon clients and open Inbox. Confirm the default
  scope is All projects / All tasks, then combine project/task/type/status filters.
- [ ] Mark a notification read; acknowledge another; archive a third. Restart the
  app and daemon and confirm their state. Restore an archived notification and
  use Mark unread to reset its read/acknowledge state.
- [ ] Archive a pending approval/proposal. Confirm it is still pending in
  Decisions and its task; acknowledging it must not authorize or start work.
- [ ] Complete another turn in a task with an archived earlier notification.
  Confirm its new current notification is unread.
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

Three owner Inbox tests cover restart, exact retry, concurrent SQLite clients,
revision isolation from pending approvals, invalid event identities and capacity.
One native test covers stable event identity, new unread versions and view filters.
Three artifact integration tests cover changed/bounded bytes, original provenance,
client metadata rejection, lease/report fences and altered prior workflow outputs.
Three existing publication/read/concurrency tests, parent revalidation, task capture
and thirteen workflow/routine tests passed, including the real dispatcher handoff.

Desktop/daemon compilation, generated binding equality, shared-client TypeScript,
full phone TypeScript, formatting and whitespace checks passed. TypeScript reused
the primary checkout's existing compiler/dependencies through an ignored check
configuration; the worktree has no dependency installation. Native builds retain
existing warnings. No visual test, live account action, deployment or second
watcher was performed. Live account/device and UI acceptance remain unchecked.
