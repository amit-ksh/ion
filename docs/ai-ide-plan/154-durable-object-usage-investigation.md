# Durable Object usage investigation and fix

21 October 2026. User-requested priority task. Preserve unrelated QA edits and
the separate redesign worktree. No implementation or live-usage attribution is
claimed by this queue entry.

## Report and current evidence

The user reports approximately **114.86k requests** to
`ion-cloud_AccountRelay`, approximately **213 kB stored**, and a meter near
**90%**. These are not independently verified telemetry. Identify the meter's
metric, unit, denominator, account/class/script filters, time range and timezone
before attributing its percentage to requests, SQL rows or another resource.

`crates/waku-core/src/gateway_owner.rs` sets a five-second regular interval and
calls automation, portfolio, Inbox-control and notification synchronization.
The paths independently perform capability/authorization checks, reads and
publications. The automation relay currently upserts projection and timestamp.
These are supported contributors to investigate, not proven attribution of the
live total. The daemon may run without the phone open.

A repeated `GatewayOwner.start()` on one object is rejected by its consumed
receiver. This does not rule out duplicate objects, processes or publishers.
Check these separately without stopping another session.

## Ordered tasks and acceptance

| ID | Task | Required evidence / initial status |
| --- | --- | --- |
| CF-01 | Identify the live meter and reporting window | Open. Use existing permitted telemetry or a redacted screenshot/export. Record metric/unit, period, scope and plan allowance. Do not expand scopes to bypass existing billing-read denial. |
| CF-02 | Measure unchanged idle and active cycles | Next. Instrument real sync, HTTP and SQL boundaries. Separate Worker requests, internal AccountRelay calls, SQL statements, actual cursor rows read/written, returned rows, local SQLite changed rows and stored bytes. Record a reproducible baseline before changes. Label local approximations. |
| CF-03 | Check duplicate owner activity | Queued. Inspect redacted publisher starts and relevant daemon/watch processes, multiple owner objects, paired publishers and restart behavior. A single object's start guard is insufficient proof of single-instance operation. |
| CF-04 | Consolidate redundant checks/reads | Queued after baseline. Consider one bounded authenticated cycle/batch. Preserve authoritative admission, pairing rotation, revocation, account isolation and operation grants, including checks after claims and before effects. |
| CF-05 | Separate liveness from content changes | Queued. Suppress equal projection rewrites and publish content less often when unchanged. Preserve necessary presence heartbeats, authoritative revisions, freshness requirements and notification cursors. |
| CF-06 | Adaptive idle/disconnected scheduling | Queued. Back off genuinely idle/error states, avoid unnecessary unpaired/disconnected work, and wake promptly on changes/reconnect. Consider events and batching rather than increasing every interval. Preserve responsive mobile controls, PDF transfer and recovery. |
| CF-07 | Safety and prolonged-idle regressions | Queued. Prove bounded idle requests/writes; changed tasks, commands and approvals; no duplicate dispatch, stale-account exposure or lost updates; uncertain-operation recovery, durable approvals/idempotency and cancellation. Use simulated time and actual sync boundaries where possible. |
| CF-08 | Before/after report, commit and live limits | Queued. Report measured idle and active request/read/write rates. Estimate daily usage for 10–50 users against current official Cloudflare limits, stating assumptions. Commit the completed fix and document remaining live verification and deployment provenance. |

Six requests every five seconds would equal 103,680 requests/day for one
continuously running daemon. This is an illustrative arithmetic scenario, not
a measured current rate or a billing calculation. Count duration/compute and
indexed SQL operations separately when applicable. Cloudflare SQL cursor
accounting may differ from local SQLite `changes`; returned rows are not proof
of rows scanned. Stored bytes do not establish request or row-operation usage.

## Constraints and integration

First local baseline: [155](155-relay-sync-local-baseline.md) records 340
handler calls, 4,422 SQL calls and 41 locally changed rows across 20 identical
source-derived idle passes. These are not live Cloudflare billed counters.
CF-02 remains partial. The owner batching/idle milestone and before/after
local measurements are recorded in [156](156-relay-sync-batching-progress.md).
Native credential HTTP tests passed; full native owner execution, exact
Cloudflare cursor metrics and the separate desktop task bridge remain open.
Do not use the owner-only result as whole-app live attribution.

- No Durable Object data deletion, paid-plan upgrade, expanded permissions,
  weakened security, unauthorized resources or spend.
- Preserve durable approvals, idempotency, no-replay handling of unknown
  outcomes, cancellation, account isolation and current data.
- Follow existing deployment/device-testing/budget authorization. Local fixture
  success is not live deployment or physical-device acceptance.
- Preserve unrelated QA edits and the separate redesign worktree.
- Latest native UI source is `051e31a`; compilation/package gates remain open.
  Prepare the next exact release snapshot after this performance work so an
  older package cannot be presented as containing the fix.
