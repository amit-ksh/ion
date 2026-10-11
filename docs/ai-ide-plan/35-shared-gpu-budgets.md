# Shared GPU budgets

Updated 2026-10-11. Implemented in `codex/ion-agent-teams`; not integrated into `ion-prod`.

## Delivered

Usage → GPU budgets on desktop and the directly connected phone lets the owner
create and edit a named allowance, exact Nebius project ID, approved sandbox target
names, currency and cap. New budgets start paused. Enable/pause applies to **new
GPU launches**; it does not cancel an existing job. Limits use integer micro-units
with six decimal places, without rounding input or combining currencies.

Each Nebius project can have one shared budget in this owner database. Every new
GPU launch checks its original per-run resource/price allowance and the current
shared cap. The shared hold commits in the same SQLite transaction as the launch,
preflight/resource reservations and price evidence, before a provider create.
Independent runs compete for the same cap. An exact saved launch returns its
original receipt without reserving or submitting again.

The budget status shows held allowance, available allowance, configured limit and
request count. These are **reserved allowances, not provider invoices**. Job Stop,
terminal state, owner restart, cancellation acknowledgment or verified resource
cleanup cannot release a monetary hold. A budget with holds cannot be deleted,
change project/currency or be lowered below those holds. Renaming, explicit cap
increases and future target changes preserve existing holds and per-run limits.
Legacy GPU reservations in the same project are adopted atomically when its
budget is saved; an insufficient cap or mismatched historical currency rejects
that save without resetting accounting.

The desktop caches bounded pages on a worker and virtualizes rows. The budget list
supports arrows, Home/End, Page Up/Down and Enter/Space to edit, with visible focus.
The phone saves a valid budget request and its exact identity before transport,
reconciles an interrupted response against the saved owner record and refuses
writes after a focus/connection change or stale workspace revision. Reopen the
latest saved definition after reviewing a conflict; pending drafts are retained.

Gateway history → GPU budgets exposes redacted totals through the existing
`usageRead` grant. This route cannot create, edit, activate or delete a budget.
Cloud project IDs, target aliases and launch/price evidence are omitted. Reads
retain publisher, pairing, foreground and request fences and bounded UUID pages.

## Scope and remaining integration

This cap covers Ion GPU launches in **one owner database**. It does not cap other
owners, direct provider usage, account invoices, ordinary sandbox/model spending
or untracked egress. It has no automatic period reset or billing-based release.
Account/contract binding, authoritative charges and billing finality still need a
verified provider integration; no quote is represented as a guaranteed bill.

The existing GPU submission/resource/readiness/price gates remain authoritative.
Creating a budget does not enable a previously gated training tool, provision a
VM, configure a sandbox or deploy speech. Broader resource/egress and live GPU
acceptance remain in [the execution guide](23-remote-command-gpu-jobs.md).
Unfinished harness adapters remain deferred.

## Setup and review checklist

- [ ] Upgrade desktop, daemon/worker and phone together to wire protocol **39**.
  Update the gateway service for the new read query kind when using gateway history.
- [ ] When the Nebius deployment exists, confirm its real project ID, sandbox
  target alias, currency, native price/resource policy and account permissions.
  Enter credentials only in the existing local setup, never in a document/chat.
- [ ] Desktop Usage → GPU budgets: create a paused budget; confirm no resource is
  created. Check name/project/target labels and readability in both themes.
- [ ] Check Tab navigation, visible focus, list arrows/Home/End/Page Up/Page Down,
  Enter/Space editing, refresh icon and keyboard save/cancel.
- [ ] Enter `0.000001` and six-place decimals. Reject zero, negative/exponent input,
  excess precision, duplicate target aliases, malformed currency and duplicate
  budgets for the same project.
- [ ] Enable/pause and reopen the saved record from desktop/phone. Confirm a stale
  simultaneous edit rejects and preserves the draft until the latest is reviewed.
- [ ] Interrupt a phone save or switch owners/background the phone. Return, refresh
  and reconcile the original budget identity; confirm it was not duplicated.
- [ ] After GPU execution is enabled and the deployment is available, run two jobs
  from separate teams/routines against one cap. A losing reservation must not POST
  a job or leave a launch/resource reservation behind.
- [ ] Restart the owner, retry an exact launch and Stop/reconcile a job. Confirm the
  shared hold remains once, even after terminal state and cleanup.
- [ ] Lower below held allowance, change held project/currency or delete a held
  budget: each must reject. Explicitly increasing a cap preserves request history.
- [ ] Gateway with `usageRead`: view totals, paginate, refresh and switch publishers.
  Verify Inbox-only/workflow-only readers cannot read these totals and no budget
  write is available. Revocation/owner-offline states must not show fresh totals.
- [ ] Complete live GPU, billing and speech acceptance after the required deployment
  resources are supplied; their absence does not make automated checks live proof.

## Automated evidence

Focused checks cover exact monetary parsing, cross-run concurrent cap contention,
launch rollback, replay after pause/exhaustion, immutable held identity, legacy
adoption and restart/paging. Phone helpers cover persisted identity, lost receipt,
unknown results, stale revisions and owner/background fences. Gateway checks cover
separate read grants, routing and rejection of writes/private metadata in replies.
The existing GPU lifecycle/resource/price/recovery suite remains part of verification.
No visual test, watcher, live provider, deployment or billable resource was started.
