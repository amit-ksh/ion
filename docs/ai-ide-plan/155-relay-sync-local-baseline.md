# Relay synchronization: local baseline

21 October 2026. First measurement milestone for task 154. The production
implementation is unchanged; no optimization or live billing attribution is
claimed. Baseline source is `3216cb7`.

## Method and limitations

The local fixture invokes the real `createGatewayRuntime` handlers and the
production `durableDatabase` adapter, backed by an isolated in-memory Bun SQLite
database. A publisher is paired with fixture credentials. Each request is
followed by the AccountRelay flush/pending checks. No external HTTP request,
provider effect, real credential, phone or live Durable Object is used.

The request inventory follows the empty regular pass in `gateway_owner.rs` and
its automation/portfolio/Inbox/notification modules. It has 17 handler calls:
five session checks, four health checks, three publications, four queue or
uncertain reads, and one notification cursor read. This is a **source-derived
replay**, not execution or timing of the native owner thread. PDF-only passes,
real commands, task publishers and overlapping processes can change totals.

A simulated clock advances five seconds per pass. Initialization and pairing
are excluded from the measured phases. Twenty identical idle passes represent
100 simulated seconds. The changed-projection phase increments the automation
revision; it does not represent a complete active task or approval lifecycle.

## Observed counters

| Metric | First pass | 20 identical idle passes | One changed-projection pass |
| --- | ---: | ---: | ---: |
| Handler requests | 17 | 340 | 17 |
| SQL calls through the production adapter | 223 | 4,422 | 221 |
| SELECT calls | 158 | 3,141 | 157 |
| Mutation statements attempted | 65 | 1,281 | 64 |
| Rows returned by queries | 118 | 2,381 | 119 |
| Local SQLite changed rows | 3 | 41 | 2 |

The 41 idle changed rows are 20 automation-owner upserts, 20 portfolio-owner
upserts, and one notification publisher cursor/signature update. Most attempted
pruning statements change zero rows in this empty fixture. The production
adapter's mutation wrapper also reads `changes()`/`last_insert_rowid()`, adding
SQL calls absent from a plain local SQLite adapter.

The fixture's allocated SQLite pages total **208,896 bytes**. This is a local
allocated-page measurement, not Cloudflare's stored bytes. Its similarity to
the reported live storage size does not establish common causes or attribution.

## What these numbers do not prove

- Handler calls are not measured billed Worker or Durable Object request totals.
- Returned rows are not rows scanned or Cloudflare `rowsRead`.
- SQLite changed rows exclude index accounting and are not Cloudflare
  `rowsWritten`. Mutation statement counts are not changed-row counts.
- The live 90% meter's metric, denominator and time range remain unknown.
- A continuously running 17-call/five-second inventory would arithmetically
  yield 293,760 calls/day/daemon. This is a scenario, not observed live traffic
  or the final 10–50-user quota/billing forecast.

The sanitized [counter receipt](155-relay-sync-local-baseline.json) is preserved.
The ignored fixture is `.waku-cache/measure-ion-relay-baseline.ts`; its normal
invocation passed. Setting `ION_ASSERT_IDLE_WRITE_BOUND=1` produced the intended
red signal: **41 changed rows in 20 unchanged cycles** exceeds its provisional
four-row idle budget. This target is a diagnostic signal, not a finalized
liveness contract or a passing acceptance test. The before report is retained
separately from future after-fix results.

## Hypotheses to test next

1. Redundant session/health/queue requests dominate idle request counts. A bounded
   authenticated batch should reduce them while preserving effect-time checks.
2. Unchanged owner publications rewrite content/liveness rows. Separating changed
   projections from bounded presence refresh should reduce writes without making
   an online owner stale or allowing revoked credentials to remain usable.
3. Repeated empty flush/prune operations contribute SQL work even with no writes.
   Triggering them only when due should reduce calls without losing cleanup.
4. Multiple publisher objects or processes could multiply traffic. A local exact
   process-name probe found no `ion`, `waku` or `waku-daemon` matches; that does
   not exclude renamed/debug, WSL, remote or previously running instances.

No loops or processes were stopped. The memory probe observed about 0.49 GiB
physical / 1.40 GiB virtual available; native compilation and package builds
remain below their resource guards. Fresh native-loop, actual SQL-cursor and
live telemetry measurements, safety regressions, the fix and after-report remain.
