# Gateway workflow history and accepted documents

## Implemented

The native phone can browse older registered-owner workflow/routine runs through
**Personal gateway → Workflows → Workflow history**. Filter all projects or one
project, an exact definition ID, and runs needing attention. Next/Previous uses
the owner's durable cursor. Runs sort by creation time descending, with ID as the
tie breaker; indexed owner queries preserve that order beyond the current feed.

Run details show every pinned stage's status, branch outcome, code inheritance
source and recorded document digest. Only the current waiting gate includes its
bounded review instructions. The existing current workflow screen remains the
place to send revision-bound controls. A historical preview does not approve work.

**Accepted documents** lists generated documents reported by completed assignments
in this exact run's started stages. Project, pinned team, stage task, producer and
accepted result membership are checked on the owner. Unaccepted, superseded,
foreign-team and foreign-project documents cannot enter this read projection.
Host paths, workspace profiles, credentials and unrelated task content are omitted.

The phone transfers immutable document bytes in 16 KiB chunks, checks the selected
metadata and continuation on every response, and verifies the complete SHA-256
before preview or export. Text/HTML/Markdown/JSON source is displayed as inert
selectable text in a virtualized list. PDF bytes can be exported with the native
folder picker. Export never evaluates document scripts in Ion. The existing
publication bounds remain 64 KiB for text and 24 KiB for generated PDF; a 16 MiB
transfer ceiling does not add larger binary publication.

## Setup and permissions

Upgrade the gateway, desktop/daemon owner and phone together. History reads were introduced without changing direct daemon protocol 37.
The later [Inbox controls](34-gateway-inbox-controls.md) require app/daemon/phone
protocol 38; upgrade the components together.
A legacy owner can reject unknown reads until upgraded.

- **Read workflows** (`automationRead`) permits history and stage metadata.
- **Read workflow documents** (`evidenceRead`) is a separate explicit invite grant
  for document catalogues and bytes. Existing task/workflow grants do not inherit it.
- The project/owner directory accepts any of the workflow, evidence, Inbox or usage
  read grants; directory access does not imply another kind of content access.
- Gateway reads recheck both the phone and registered publisher grants at enqueue,
  claim, response and result retrieval. Revocation stops subsequent chunks.

A persisted/private Linux pairing runs outbound in the daemon without the desktop
or an inbound port. A memory-only desktop pairing uses the existing app-session
bridge. Owner/run/filter/pairing changes, navigation away and phone backgrounding
fence pending results and document assembly. Scope changes during the folder
picker also prevent export. Reads queue only read queries and cannot start agents,
resolve decisions, call apps or provision resources.

## Verification

The focused checks cover query bounds and private-field rejection; chronological
history and cursors; exact accepted stage/producer membership; binary chunk and
full-digest verification; scope changes at every asynchronous boundary; independent
read grants, revoked grants and unchanged owner state. Full mobile/gateway/client
TypeScript checks and locked Rust checks cover the integration. Exact counts and
commands are recorded in the progress log after the final checks complete.

## Your review checklist

- [ ] Pair with workflow metadata only, document reads only, and both. Existing
      task-only invites must not expose either content type. Revoke each grant.
- [ ] Create more than one page of runs with different creation times. Browse all
      projects, project/definition filters and attention-only runs; compare the
      ordering and counts with the desktop/daemon.
- [ ] Inspect sequential and branching stages, skipped stages and inherited code.
      Confirm only the current pending gate shows review instructions.
- [ ] Open accepted Markdown/HTML/JSON source and export a PDF. Compare exported
      bytes/digest with the owner's original. HTML source must stay inert.
- [ ] Confirm failed/in-review/superseded assignment results, foreign projects
      and a document substituted under another workflow cannot be read.
- [ ] Change owner/run/pairing, background the phone or revoke grants during a
      multi-chunk transfer. No partial document may be previewed or exported.
- [ ] Cancel/change scope in the Android/iOS folder picker. Review large text,
      keyboard/screen reader operation, readable light/dark text and scrolling.
- [ ] Stop/restart a headless Linux owner and test the live gateway through TLS.

Native device/visual checks and live providers remain pending. No visual test,
second watcher, billable resource or provider deployment was started. Deferred
new harness adapters stay outside scope.

## Remaining work

Gateway Inbox notification writes are implemented in [the control increment](34-gateway-inbox-controls.md).
Broader resource/monetary accounting, other
existing solo-harness usage receipts, larger binary formats/previews and shared
speech deployment/device acceptance remain tracked separately. Background push
notification delivery is distinct from reading or changing Inbox state.
