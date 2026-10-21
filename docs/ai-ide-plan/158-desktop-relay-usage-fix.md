# Desktop relay batching and measured SQL usage

21 October 2026. Continuation of [154](154-durable-object-usage-investigation.md)
and the owner milestone [156](156-relay-sync-batching-progress.md). This is a
local source/verification milestone. Live namespace counters are now readable;
the dashboard's 90% meter and caller attribution remain unverified. No plan,
data, permission or paid resource changed.

## Implementation

- `GET /v1/device/tasks/sync` authenticates the publisher and combines its task
  command queue, presence heartbeat and task-snapshot receipt. Phones and foreign
  publishers cannot use it to read another publisher's queue. Claims and effect
  authorization remain separate requests; only HTTP 404 permits legacy fallback.
- Task presence updates at most every 30 seconds. Equal reconciliations no
  longer rewrite the receipt; a missing receipt invalidates the desktop snapshot
  cache and causes a bounded republication. Failed publications invalidate cached
  snapshots/events. Uncertain commands remain durable and are not replayed.
- Windows sharing one daemon supervisor share one weakly owned publisher/channel.
  Every window can publish interactions; the last window releases it. A distinct
  OS advisory lock fences task publishers across processes without colliding with
  the legitimate daemon workflow owner. These are two intentional publishers,
  not evidence of two accidentally launched daemon processes.
- Healthy idle polling adapts from five to ten seconds. Interaction messages wake
  it immediately; failure backoff reaches 300 seconds and local changes cannot
  bypass it. Pairing changes clear queued interactions/caches; unpaired operation
  makes no gateway requests. PDF-only work also respects failure backoff.
- Before an effect, the task bridge checks the current local pairing, remote
  publish grant and daemon runtime after the durable claim. Stop still attempts
  both computer-control cancellation and turn cancellation. Revocation or lost
  acknowledgements leave an operation uncertain rather than replaying it.

## Measurements

The new test worker runs real gateway handlers and `durableDatabase` against
**workerd Durable Object SQLite**, consumes every cursor, then reads its actual
`rowsRead` and `rowsWritten`. It repeats AccountRelay's flush/pending SQL after
each request. The clock is simulated; initialization is excluded. One publisher
and one phone are paired. The native loops, public Auth0 entry point, real
device effects, alarms and live billing are not exercised by this measurement.

Before source is pinned to `26e88a5`: the source-derived 17-request owner pass
plus four-request desktop pass. After uses the two new batch handlers. The
sanitized receipt is [158-relay-sql-measurements.json](158-relay-sql-measurements.json).

| Local scenario | Handler requests | SQL statements | Cursor rows read | Cursor rows written |
| --- | ---: | ---: | ---: | ---: |
| Before: 20 idle passes in 100 seconds | 420 | 5,382 | 3,461 | 61 |
| After: same 20 passes / 100 seconds | 40 | 880 | 600 | 9 |
| After: one idle hour at ten-second cadence | 720 | 15,840 | 10,800 | 360 |
| Changed task upload + reconciliation | 2 | 34 | 17 | 6 |
| Equal task upload + reconciliation | 2 | 24 | 15 | 0 |
| Stop queue/read/claim/fresh grant/settle/empty queue | 6 | 89 | 59 | 5 |
| Waiting task + exact permission publication/decision/receipt | 9 | 145 | 91 | 13 |

The same-cadence sample reduces requests by **90.5%** and row writes by **85.2%**.
The steady idle request scenario is 0.2 requests/second versus the old inventory's
4.2, a **95.2% reduction**. These do not prove the origin of reported live usage.
Stop and permission receipts above simulate local acknowledgements; they do not
prove a real daemon or provider performed an effect. The changed-task sample
excludes its preceding poll and transcript/event uploads.

### Pilot projection

Multiply the measured empty-queue idle hour, assuming one workflow owner and
one desktop task bridge per user, no additional windows/publishers, no phones
polling, no alarms or active work. Indexed cursor operations are included in
the local workerd measurement; a populated live database may scan more rows.

| Users / paired hours per day | Requests/day | Rows read/day | Rows written/day |
| --- | ---: | ---: | ---: |
| 1 / 24 | 17,280 | 259,200 | 8,640 |
| 10 / 24 | 172,800 | 2,592,000 | 86,400 |
| 50 / 24 | 864,000 | 12,960,000 | 432,000 |
| 10 / 8 | 57,600 | 864,000 | 28,800 |
| 50 / 8 | 288,000 | 4,320,000 | 144,000 |

[Current Cloudflare pricing](https://developers.cloudflare.com/durable-objects/platform/pricing/)
lists Free daily limits of 100,000 requests, 5 million rows read and 100,000
rows written, with 5 GB stored. Paid allowances use monthly windows: 1 million
requests, 25 billion rows read and 50 million rows written. Duration, Workers
requests, alarms, R2 and other products are additional dimensions. No plan was
upgraded. The 10–50-user always-on scenario **does not fit the Free request
allowance**. These figures are a capacity warning, not a complete invoice or
evidence that the existing USD10/USD15 ceilings can support that load.

The current approach combines bounded batches, content suppression and event
wakeups. A hibernating WebSocket notification path could reduce idle requests
further, but requires reconnect, revocation and uncertain-operation acceptance
before replacing this polling path; it is not claimed implemented here.

### Distinct metrics

- **Requests:** fixture handler calls. Live Durable Object request metrics and
  outer Worker invocations must be queried separately.
- **SQL statements:** executions, not billable rows.
- **Cursor rows read/written:** actual local workerd SQLite counters, distinct
  from Bun SQLite `changes()` and rows returned. They are not live telemetry.
- **Stored bytes:** database size, not request/row-operation usage. The reported
  approximately 213 kB does not identify the 90% meter's denominator.

## Verification and open gates

- Native credential HTTP regressions: **5 passed**, including strict 404-only
  fallback, foreign-publisher rejection, changed projection delivery, and fresh
  effect grants after batched reads. Actual cadence, OS lock and weak registry:
  **3 isolated Rust tests passed**.
- Actual local Workers SQL counter test: **passed**, including one-hour idle,
  unchanged task replay, changed task, stop and exact permission snapshots.
- Worker TypeScript check and changed Rust syntax parsing: **passed**.
- **41** relay/durability/push regressions passed (306 assertions).
- **Eight** fresh-bundle workerd integration tests passed, including public JWT
  workspace selection, forged-hint/device-token rejection, both batch endpoints,
  revocation, restart durability, Apps, static hosting and service/budget controls.
- Core/client Rust release typecheck **passed**. Full desktop typecheck is
  running; native owner/bridge execution and fresh installer/APK acceptance
  remain separate gates. A typecheck is not a native interaction pass.
- Duplicate native windows/processes need a live verification after build; the
  registry/OS-lock tests prove those mechanisms, not a census of other machines.
- The standard CLI refreshed the existing OAuth login successfully; existing
  analytics reads work. Billing history returns HTTP 403 / code 10000, so invoice
  reconciliation and the account's billing allowance remain unverified. The
  browser bridge still fails to initialize (`failed to write kernel assets`,
  OS error 3). No credential or additional permission was exposed or requested.

### Live pre-deployment observation

At `2026-10-21T16:24:22.148Z`, an explicit rolling 24-hour query for namespace
`29d1ef8b45224dc3b2467659f00c6e23` (`ion-cloud_AccountRelay`) returned:

- 115,278 HTTP and 9 successful JavaScript RPC requests: **115,287 total**.
- Periodic sums: **964,342 rows read**, **26,043 rows written**.
- SQL storage maximum: **212,992 bytes**.

This confirms substantial namespace traffic alongside a small stored database.
Adaptive/periodic analytics are not individual request logs. The query window
was `2026-10-20T16:24:22.148Z` through `2026-10-21T16:24:22.148Z`; it is not proof
of the user's dashboard selection, a calendar-day allowance, the 90% denominator
or which desktop/phone operation caused traffic. The deployed version was still
`b20ab52f-1155-424b-af51-eedc9957cd0f`. No after-deployment reduction is claimed.

Reproduce the cursor regression:

```powershell
node --test packages/ion-cloud-worker/tests/relay-sql-counters.test.mjs
```

`ION_RELAY_SQL_REPORT` optionally saves its sanitized counters to a chosen local
path. The fixture's counter/time routes exist only in the test worker and are
never exported by the production Worker entry point.

## Required live verification before closing task 154

1. Read the actual metric label/unit/denominator, namespace/script/object filter,
   timezone and selected period. [Cloudflare analytics](https://developers.cloudflare.com/durable-objects/observability/metrics-and-analytics/)
   documents a default 24-hour namespace view; that is not proof of the user's
   selected window or their 90% meter.
2. After native/recovery checks, record the deployed Worker version and exact
   desktop build. Observe matched idle and active windows with one authorized
   publisher, then multiple windows, reconnect and account changes.
3. Verify changed tasks, durable approvals, Stop and lost acknowledgements on the
   actual native path. Compare requests, cursor rows, alarms, duration and outer
   Worker requests independently. Retain uncertain receipts and existing data.
4. Reconcile measured pilot use against the existing plan/budget. Do not upgrade
   a plan, weaken security, remove data or claim capacity from stored bytes.
