# Phone workflow and routine editing

Implemented 2026-10-11 in `codex/ion-agent-teams`. Protocol **37** is unchanged;
use matching owner and phone builds. No new harness connection is introduced.
The full Ion release still has development remaining.

## Use it

Open task history → **Workflows & routines**. Use **New workflow**, **New routine**
or **Edit** on a saved definition. Select the project and saved teams; each team
keeps its existing per-agent harness, environment and permissions. Saving a
definition does not launch a workflow. **Run now** uses the existing durable start
flow. New and edited routines save **paused**. Enable their schedules explicitly
from Routines after reviewing the owner preview.

Workflows support a prompt and editable generated/manual stages. Each stage
selects a team, instructions, an optional human gate, required HTML/Markdown/text/
CSV/JSON/PDF documents, optional bounded retries, code inheritance after the first
stage, and a forward destination or JSON document condition. Optional aggregate
workflow limits and child review are editable. Generation uses the owner's
configured Nebius model only after the user chooses Generate; missing credentials
leave the existing phone draft available. There is no automatic generation.

Removing a referenced stage is rejected until its authored routes are changed.
Other forward indexes are remapped; the first surviving stage cannot inherit code.
The owner remains authoritative for valid routes, stage reachability, calendars,
teams, limits and output contracts. Existing runs keep their pinned definitions.
Connected app permissions and each real action's decision remain in the existing
Teams/app approval flow; a workflow draft does not establish an app connection.

Routine editing selects a team or a workflow in the same project, instructions,
an elapsed interval or daily/weekly/monthly calendar, IANA zone and local time,
missed-run policy and overlap policy. Schedule preview uses the owner's pinned
calendar implementation and returns exact upcoming occurrences with zone labels.
The phone does not compute DST or invent a next firing. Saving pauses the timer
without stopping an already active run. Enable schedule calculates its next firing
through the existing owner action.

## Drafts, conflicts and acknowledgement recovery

A phone draft retains a stable definition ID, the original definition and its
expected revision. Draft keys include profile, exact owner address and definition
(or new-definition slot). Writes are serialized, debounced and flushed on ordinary
navigation; the exact submitted draft is persisted before a save request. Drafts
are bounded to 512 KiB of UTF-8 and validated before restoring. Native storage
failures send no mutation. Malformed/incompatible drafts remain stored until an
explicit local discard. This path uses no native TextEncoder/structuredClone
requirement. Incomplete invalid scalar/day text stays visible and prevents Save;
authoring records retain the last structurally valid field values.

Save uses the existing authenticated compare-and-save RPC. The acknowledgement
must contain the exact submitted authoring content. A lost/unexpected response
permits one read for reconciliation, with no automatic mutation replay or revision
advance. If the exact definition is present, its saved identity is acknowledged;
otherwise the original draft and revision remain. A cleanup failure after a
confirmed save can reconcile the same definition on retry.

**Refresh revision** retains edits and advances the expected revision only when
the saved definition still matches the original baseline. An unrelated workspace
edit therefore need not discard the phone draft. A changed/deleted definition
requires explicit **Load latest definition** and review. Timer-owned next-fire and
last-error fields alone do not create an authoring conflict. Routine submissions
are explicitly paused, rather than rewriting an active timer with a stale firing.

The editor refreshes when it becomes foreground/focused or reconnects and through
the refresh action; it does not poll full authoring/task snapshots while typing.
Mutations require a fresh current owner/client. Mounted/connection/foreground
fences prevent an old confirmation or asynchronous result from mutating or
replacing a different connection's draft. Picker collections use bounded native
FlatLists, search, semantic labels, system switches and theme colors. The page
preserves safe-area/keyboard insets and scalable text, with 48-point form targets.

## Automated verification

- Fourteen new phone tests passed for persisted identities, storage failures,
  lost/wrong acknowledgements, actual definition conflicts, exact retry after
  cleanup failure, concurrent taps, ordered writes, owner/address isolation,
  safe revision refresh, paused routine saves, invalid/oversized drafts and
  branch-remapping/default-omission behavior.
- All eleven existing phone automation start/recovery regressions passed.
- Full phone and shared-client TypeScript checks passed. Protocol 37 bindings
  remain current. The mechanical UI detector found no issues; whitespace checks
  passed. These checks do not establish native visual/device acceptance.

No live generation request, billable provisioning, native screenshot test,
app/device relaunch, deployment or additional dev watcher was performed.

## Your review checklist

- [ ] On the matching phone build, open Workflows & routines → New workflow.
  Select a project and saved teams, create two stages and save. Verify the same
  definition on desktop and start it through the existing Run now flow.
- [ ] Add a decision gate, PDF/JSON output, code inheritance and bounded retry.
  Verify the actual run shows its pinned settings and uses each selected team.
- [ ] Configure a named JSON condition and both destinations. Remove an
  unreferenced stage and check remapped destinations; verify referenced-stage
  removal is rejected until you change the route.
- [ ] Type an invalid condition scalar/monthly day list. Confirm visible recovery
  guidance and disabled Save; correct it without losing other edits.
- [ ] With a later Nebius key, use Generate stages, review the result and select
  teams before saving. Without a key, confirm the original draft is preserved.
- [ ] Create daily, weekly and monthly routines in Asia/Kolkata and a zone with
  DST. Compare the owner's preview with desktop, save paused, then explicitly
  enable the schedule. Check missed-run and overlap policy behavior.
- [ ] Edit an enabled routine while one run is active. Confirm saving pauses
  future occurrences, keeps the active run intact and resets no prior run history.
- [ ] Interrupt a save acknowledgement and reopen the app. Retry the same draft
  and verify one definition identity, no duplicate launch and exact reconciliation.
- [ ] Edit the same definition on desktop while a phone draft is open. Confirm
  Save cannot overwrite it and Refresh revision does not discard your edits.
  Load latest only after reviewing the replacement confirmation.
- [ ] Change another workspace record, then Refresh revision. Confirm your phone
  edits survive and a fresh save succeeds when the definition is unchanged.
- [ ] Switch owners, reconnect or background the app during generation, preview,
  saving and confirmation dialogs. Confirm old results/actions do not cross scope.
- [ ] Test storage failure/corrupt drafts. Confirm no unpersisted mutation and
  explicit local discard; check saved definitions before replacing an unknown save.
- [ ] Review keyboard, screen-reader labels, both themes, large text, long team
  names, phone/tablet widths, Back gestures and keyboard reachability on hardware.

## Still separate

Team/profile creation on phone, broader background notification delivery,
registered-owner/gateway workflow controls, new harness adapters (deferred),
authoritative billing, general Local/remote checks and broader resource/GPU work
are not completed by this editor. Device and live account review remain pending.
