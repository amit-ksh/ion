# Gateway Inbox and usage history

## Implemented

The native phone can read a registered owner's historical Inbox and global usage
ledger through **Personal gateway → Inbox & usage**. The outbound daemon owner
reads its existing durable stores. It can serve this screen without an open
desktop or an inbound daemon port. A process-only desktop pairing retains its
app-session bridge; persisted and private Linux file pairings use the daemon.

The phone starts with all projects and all tasks. Select an owner and project,
then use **More filters** for notification view/type/action status or harness,
environment, usage type, time range and exact model. Task choices identify tasks
on the current page; a full task ID filter also reaches older tasks not on that
page. Agent IDs and removed project IDs can be filtered explicitly. The project
directory pages through all current projects instead of silently limiting the
picker to a recent snapshot. **Next / Previous** follows the owner's indexed
history cursor. Filtered totals describe the whole matching ledger, not only
the transferred page.

Inbox rows preserve notification state, need-for-action status and historical
facts. Detail previews are limited to 500 Unicode characters and labeled when
shortened. They do not approve decisions, acknowledge notifications, change
preferences or start agents. Use the full source on the owner and the existing
exact gateway approvals/workflow controls for actions.

Usage preserves reported/estimated and final/provisional costs, currency,
unknown token/cost values, reserved tokens and separately labeled approved
spending allowances. It excludes host paths and private billing references.
An unavailable cost is never shown as zero.

## Setup

1. Upgrade gateway and owner daemon from the same reviewed worktree version.
   Health advertises `portfolioRelay: 1`; older gateways do not serve these reads.
2. Register the owner with **Publish desktop state**. For headless Linux setup,
   use [the private service pairing guide](31-headless-gateway-owner.md).
3. Create a new phone invite in `/owner` with **Read owner Inbox history**,
   **Read owner usage**, or both. Existing task/workflow grants do not inherit
   these permissions. A revoked or expired capability stops reads.
4. Pair the phone and open **Personal gateway → Inbox & usage**. Start the
   registered owner and refresh if it is offline.

Neither Nebius inference credentials nor a paid cloud deployment are needed for
local gateway contract checks. Real internet/phone acceptance needs the owner's
HTTPS gateway and separately configured owner service. Never put device tokens,
owner secrets or provider keys in source control or chat.

## Read boundary and lifecycle

Only typed Inbox, usage and project-directory queries are representable. Each
query pins the phone, publisher, nonce and exact filters/cursor. The server
checks the grant when submitting, retrieving, delivering and answering a query;
revoked publishers are hidden. Requests cannot contain shell, app or task effects.

SQLite durably commits the read request before returning it. The first answer
is immutable; a lost acknowledgement reads the same nonce. Re-reading owner
state after a lost answer causes no external effect. Job lifetime is two minutes;
expired bodies are withheld immediately and cleared on relay activity, and
receipts are pruned after ten further minutes. Responses use `no-store` and
mobile results stay in memory; no new transcript/usage disk cache is introduced.

The phone polls serially only while focused and foregrounded, with a 60-second
wait budget and existing 20-second HTTP deadlines. Pairing, owner, filter and
foreground changes fence old answers and subsequent polls. Owner requests have
5-second HTTP deadlines and the existing per-pass processing budget. These are
bounds, not network latency guarantees. The owner heartbeat is separate from
individual task mirroring, so owner availability does not refresh stale tasks.

Transfers are limited to 32 rows and 60 KB of UTF-8 JSON. Whole rows are trimmed
when needed and the cursor advances from the last retained row; totals remain
unchanged. The phone labels a shortened transfer. Project pages are sorted by
stable UUID and contain labels only. Native validation works without TextEncoder.

## Verification

- 49 gateway tests passed, including 9 read-relay and HTTP permission/persistence
  cases plus the existing task, approval, workflow and Composio regressions.
- 27 phone client/helper tests passed, including 12 history cases for lost
  acknowledgements, scope changes, expiry, cross-owner replies, bounded waits,
  exact time filters, unknown costs and native Unicode validation.
- 10 daemon owner tests passed, including a real-store Inbox/ledger query that
  does not start work; 4 pure protocol projection tests passed.
- Locked desktop, daemon and selected worker compilation passed. Full mobile,
  gateway and shared-client TypeScript checks, protocol binding equality,
  formatting and whitespace checks passed.

These checks do not prove visual, native microphone, Linux descriptor or live
internet acceptance. No second dev watcher, visual test, cloud provider call,
billable resource or deployment was started.

## Your review checklist

- [ ] Pair a phone with Inbox only; usage must require its separate read grant.
      Repeat with usage only, then both. Task/workflow-only invites cannot read Inbox or usage content.
- [ ] Select two owners with different projects. Change owner while loading;
      the previous owner's records must never appear under the new owner.
- [ ] Browse more than 32 projects and several Inbox/usage pages. Compare counts
      and filtered totals with the direct desktop/daemon views.
- [ ] Exercise unread, archived, acknowledged, notification type and action
      filters. Use full project/task/agent IDs for older or removed sources.
- [ ] Verify provisional/estimated and unknown costs, currencies and approved
      allowances. Compare usage time filters with the owner view.
- [ ] Check a long Unicode notification preview and a shortened transfer;
      continuation must preserve row order and totals without skipped rows.
- [ ] Background the phone or change filters/pairing during a request. Old
      results must stay fenced. Return and refresh to read current state.
- [ ] Stop the owner, then restart it. Revoke the phone and publisher separately;
      reads must stop and no agent/app effect may run from a history request.
- [ ] Review light/dark text, large text, screen-reader controls and physical
      Android/iOS behavior. Visual/device review remains for you.

## Remaining objective

Paged workflow runs and accepted generated-document reads are implemented in
[the history increment](33-gateway-workflow-history-documents.md). Gateway
notification writes are implemented in [the control increment](34-gateway-inbox-controls.md). Other existing solo-harness receipts, authoritative billing,
broader host/remote/GPU resource enforcement and shared speech deployment/device
acceptance also remain tracked in the release checklist. Deferred new harness
adapters remain outside the current scope.
