# Relay usage fix: owner-worker milestone

21 October 2026. Task 154 remains open. This is a source/fixture milestone,
not a deployed fix or attribution of the live dashboard's reported 90% meter.

## Implemented

- Authenticated `GET /v1/device/sync` batches owner capabilities, four queue
  reads and notification publication state. No effect authorization lease.
- Native owner consumes that batch; claims, effect-time authorization,
  settlements and requester revocation checks remain independent and current.
- A canonical projection digest suppresses equal content uploads. Separate
  presence timestamps update at most every 30 seconds, inside existing 60s
  freshness fences. Notification cursor **and policy** must both match before
  skipping an empty publication; mute still cancels queued alerts.
- Healthy owner idle cadence adapts from 5 to at most 10 seconds. Activity and
  local projection changes wake normal polling; failures back off to at most
  300 seconds. Pairing rotation resets state. Unpaired work makes no network
  requests. PDF transfer keeps its short active cadence.
- A process guard and OS advisory publisher lock fence duplicate owner loops.
  The lock is released on exit/crash; no data deletion or stale-file cleanup.

## Local measurements

Actual runtime HTTP handlers and the production Durable Object SQL adapter,
backed by isolated Bun SQLite, with AccountRelay flush/pending checks replayed.
Initialization/pairing excluded. Simulated time; no live provider/device calls.

| Phase | Before requests / SQL calls / changed rows | After requests / SQL calls / changed rows |
| --- | ---: | ---: |
| First regular pass | 17 / 223 / 3 | 4 / 75 / 3 |
| 20 equal passes over 100 seconds | 340 / 4,422 / 41 | 20 / 600 / 6 |
| One changed-projection pass | 17 / 221 / 2 | 2 / 44 / 1 |
| One hour idle at 10-second steady cadence | Not replayed at old cadence | 360 / 10,800 / 240 |

The baseline is retained in 155. The after receipt is in
`.waku-cache/ion-relay-sync-after.json`; the reproducible regression is
`packages/ion-cloud-worker/tests/relay-sync.local.test.ts` (set
`ION_RELAY_COUNTER_REPORT=1` to save counters). Local changed rows exclude index
accounting and are **not** Cloudflare `rowsWritten`. Returned query rows are
**not** scanned rows or `rowsRead`. Handler calls are not measured live billed
requests. Stored bytes are a separate dimension.

The owner-only idle scenario projects 8,640 handler requests and 5,760 locally
changed heartbeat rows/day per continuously paired daemon: 86,400 / 57,600 for
10 and 432,000 / 288,000 for 50. These exclude desktop task publishing, phones,
push alarms, platform services, active commands and SQL index operations.
They are not a whole-app capacity or billing estimate.

[Cloudflare's current pricing](https://developers.cloudflare.com/durable-objects/platform/pricing/)
lists Free allowances of 100,000 requests/day, 5 million rows read/day,
100,000 rows written/day and 5 GB stored. Paid allocations use a monthly window
(1 million requests, 25 billion rows read, 50 million rows written); duration,
Worker invocations and other products are separate. No plan was upgraded.
Even this owner-only 50-user always-on scenario exceeds Free request allowance.

## Verified and open gates

- Three real-handler/durable-adapter regressions passed (91 assertions).
- 45 relay/grant/recovery/retention/push tests passed; one existing HTML HTTP
  case remains skipped and is not a pass.
- Worker TypeScript check passed.
- Actual cadence and OS-lock modules passed two isolated Rust tests. First
  harness invocation used a mismatched 1.97 compiler/1.98 dependency artifact;
  rerun with the matching installed stable compiler passed.
- Three native credential HTTP regressions passed with the matching installed
  stable toolchain (`cargo test --locked --offline --release -j 1 -p
  waku-credentials gateway_sync --lib`): no cached effect grant, strict
  authorization/foreign-batch rejection and changed-projection delivery.
  Full owner/core/package acceptance is still pending.

## Remaining investigation and work

1. **Live metric identity/window:** saved Wrangler login expired at
   `2026-10-21T13:49:31.869Z`; checked at approximately 14:45 UTC. Recorded scopes
   contain no explicit analytics/billing scope. No scopes expanded, login
   refreshed, dashboard meter inferred or live counter attributed. The dashboard
   docs say namespace charts default to 24h, but the user's actual selection
   and 90% denominator are still unknown.
2. **Separate desktop task bridge found:** `src/gateway_bridge.rs` intentionally
   mirrors solo tasks while the daemon handles persisted workflow owners. It
   skips duplicate workflow ownership except memory-only pairings, but still
   has a session check, queue read, reconcile and heartbeat in a typical empty
   five-second pass. This is an additional contributor, not evidence of a
   duplicate process. Measure and optimize it before calling task 154 complete
   or presenting a whole-app pilot forecast.
3. Finish native credential/core safety checks and before/after active task,
   approval and command measurements; verify account switching/reconnect and
   physical mobile latency under the existing device authorization.
4. Fresh package refresh and deployed-version provenance remain pending.
   Preserve the separate redesign worktree and unrelated QA changes.

No Durable Object data was deleted, permissions expanded, security disabled,
paid resources created or native UI/device acceptance invented.
