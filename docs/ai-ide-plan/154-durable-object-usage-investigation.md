# Durable Object usage investigation and fix

21 October 2026. User-requested priority task. Preserve unrelated QA edits and
the separate redesign worktree. Local implementation and measurements are
recorded below; no live-usage attribution or deployment is claimed.

## Report and current evidence

The user reports approximately **114.86k requests** to
`ion-cloud_AccountRelay`, approximately **213 kB stored**, and a meter near
**90%**. These are not independently verified telemetry. Identify the meter's
metric, unit, denominator, account/class/script filters, time range and timezone
before attributing its percentage to requests, SQL rows or another resource.

The original five-second owner pass independently performed capability checks,
queue reads and publications; the separate desktop bridge added its own checks,
queue, reconciliation and heartbeat. Local replay confirmed amplification.
The two source milestones now batch reads, suppress unchanged content and adapt
idle/failure polling. These are supported contributors, not proven attribution
of the live total. A paired daemon can run without the phone open.

A repeated `GatewayOwner.start()` on one object is rejected by its consumed
receiver. This does not rule out duplicate objects, processes or publishers.
Check these separately without stopping another session.

## Ordered tasks and acceptance

| ID | Task | Required evidence / initial status |
| --- | --- | --- |
| CF-01 | Identify the live meter and reporting window | Open. Use existing permitted telemetry or a redacted screenshot/export. Record metric/unit, period, scope and plan allowance. Do not expand scopes to bypass existing billing-read denial. |
| CF-02 | Measure unchanged idle and active cycles | Local before/after HTTP + workerd SQL cursor measurements recorded in 158, including task publication, Stop and exact permission receipts. Native loops and live telemetry still need measurement. |
| CF-03 | Check duplicate owner activity | Process guard, weak multiwindow registry and cross-process publisher locks implemented; isolated tests pass. Live duplicate-window/process census remains open. |
| CF-04 | Consolidate redundant checks/reads | Owner and task batch handlers and native consumers implemented. Five native HTTP tests pass; no cached effect grant. Full native/public-route acceptance remains separate. |
| CF-05 | Separate liveness from content changes | Implemented: canonical projection digest, conditional task-set receipts, unchanged publication suppression, 30-second presence. Local workerd unchanged task replay writes zero rows. |
| CF-06 | Adaptive idle/disconnected scheduling | Implemented: healthy idle at most ten seconds, failure backoff at most 300 seconds, event wakeups, no unpaired network work; PDF failures respect backoff. Native cadence and device latency acceptance remain open. |
| CF-07 | Safety and prolonged-idle regressions | Local HTTP/durable tests and workerd cursor test pass. Native owner/bridge, reconnect, public account isolation and physical controls require remaining gates. Do not substitute fixtures for live effects. |
| CF-08 | Before/after report, commit and live limits | Owner milestone committed as 29021c9; desktop milestone/report in 158. Pilot forecast explicitly exceeds Free request limits for 10–50 always-on users. Fresh packages, live meter identity and deployed-version comparison remain open. |

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
The desktop milestone [158](158-desktop-relay-usage-fix.md) includes both
publishers and actual local workerd SQL cursor counters. Native credential HTTP
tests passed; full native owner/bridge execution and live metrics remain open.
Neither local result is whole-app live attribution.

- No Durable Object data deletion, paid-plan upgrade, expanded permissions,
  weakened security, unauthorized resources or spend.
- Preserve durable approvals, idempotency, no-replay handling of unknown
  outcomes, cancellation, account isolation and current data.
- Follow existing deployment/device-testing/budget authorization. Local fixture
  success is not live deployment or physical-device acceptance.
- Preserve unrelated QA edits and the separate redesign worktree.
- Redesign source is merged at `fc956df`; fresh compilation/package gates remain open.
  Prepare the next exact release snapshot after this performance work so an
  older package cannot be presented as containing the fix.
