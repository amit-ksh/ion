# Pairing-scoped interaction recovery

21 October 2026. Follow-up to the relay usage fix in plans 158–159.
This is a source milestone; the new native client and resolution table have
not yet been released or deployed. The preceding deployed Worker remains
`d948eb00-1607-497d-9dd8-9e7dc75dbcbb`.

## Reproduced failures

1. The shared desktop publisher collected every window's updates in one map.
   A Settings/account refresh in one window sent `Reset`, clearing another
   window's queued permission. An extracted production collector reproduction
   failed with `window B refresh erased window A approval`.
2. The worker drained new events before detecting a pairing change, then
   cleared those events. Simply draining afterward would risk associating old
   events with a different account.
3. Resolving an interaction before its first publication returned success but
   left no durable record. A real SQLite restart test then recreated the
   request as **pending** from a stale window. Its first run failed as expected.

## Changes

- Every window has a consumer identity and a cached opaque pairing scope.
  The scope includes origin, publisher, workspace and token/binding identity;
  token rotation changes it. No bearer token is put in the UI queue.
- Scope calculation and credential loading run in existing background account
  refresh/pairing jobs. Streaming and render paths only copy cached state and
  enqueue messages. A linked account cannot finish verification without loading
  its pairing. Startup/re-pair republishes currently pending permissions and
  questions after the scope is installed.
- Refresh upserts current state; it does not clear another window's requests
  or outstanding resolutions. Rebinding retires only that consumer's old
  ownership. Equal requests coalesce across windows; resolution dominates a
  stale pending snapshot.
- The worker publishes only entries matching its current API scope, under the
  existing fresh authorization checks. An old API iteration cannot discard or
  send a newer pairing's events. Transient credential failures retain scoped
  entries without making network calls. Inactive account entries do not wake
  current-account polling.
- The old clear-all-at-1,001 policy is removed. The queue retains one value per
  distinct scoped interaction/target and removes delivered values; undelivered
  backlog memory therefore scales with distinct outstanding updates during an
  outage. It is not an independent durable store or a fixed memory bound.
- An additive `relay_interaction_resolutions` table records an owned, current
  runtime's request when resolution precedes publication. A later publication
  starts resolved, including after a restart. Unknown foreign publisher/runtime
  resolutions create no row. Retried resolutions use conditional updates or
  `INSERT OR IGNORE`; they do not rewrite equal rows.
- No existing Durable Object data was deleted, no permission or service control
  changed, and no paid workload was started.

## Evidence

All evidence paths below are local to `.waku-cache/`:

| Check | Result | Evidence |
| --- | --- | --- |
| Original production collector extraction | Expected failure, 0 passed / 1 failed | `ion-shared-interaction-red.log` |
| New production collector + actual queue source | 7 passed | `ion-scoped-collector-tests.log` |
| Resolution-before-publication across SQLite restart | Expected failure before fix | `ion-resolution-order-red.log` |
| SQLite task relay, HTTP auth/permissions, durable authority | 21 passed, 156 assertions | `ion-scoped-relay-durability-tests.log` |
| Repeated resolution / duplicate publication | Zero additional SQLite changed rows, asserted using `total_changes()` | Same 21-test run |
| Rust syntax / whitespace | Five changed Rust files parse; `git diff --check` passes | Operator command output |

The isolated collector harness uses the exact production enum, keying,
collector, test functions and queue file, with lightweight payload/UUID stand-ins.
It is not proof of full native type checking, credential fingerprint integration
or end-to-end network delivery. The new credential fingerprint regression is
checked in but still requires its native test run.

## Remaining gates

- Finish the already-running native owner recovery test job (session 32910,
  source 51c17b8); do not restart it because observation is quiet.
- Run fresh credential scope/transport tests and combined desktop/core/client
  type checking for this source, plus authenticated mobile presence and the
  generated protocol bindings check.
- Run the relevant real-workerd public auth/restart/SQLite cursor cases for the
  added table. Record actual request/row counters before updating plan 158's
  measurements; Bun's changed-row assertions are not Cloudflare billing data.
- Deploy only after bundle/source, version, route, secret-name and security
  checks. Reconfirm the existing deployment before writing; never blindly retry
  an uncertain deployment.
- Prepare a **new** release input snapshot and Windows/ARM64 package verifiers.
  Preserve the 51c17b8 draft and historical verifiers; they should reject this
  changed source. Native Android branding was already regenerated and pinned.
- Resource observation before heavier checks: about 0.73 GiB physical memory
  available, below the 2.5 GiB release guard. Existing owner test process remains
  live. No other session/process was stopped.
- Physical Android speech, native UI/accessibility acceptance, sandbox provider
  image/pricing/lifecycle acceptance and the final QA handoff remain open under
  the original goal. No fixture or source pass substitutes for those gates.

The latest public [sandbox limits and usage guidance](https://docs.tokenfactory.nebius.com/sandboxes/operate/limits-retention-and-usage)
still says raw operation `cost` requires confirmed units and billing records for
invoice reconciliation. It does not supply a numeric sandbox price ceiling.
This recheck does not authorize a provider import or paid execution; the existing
USD10 target / USD15 cumulative ceiling remains unchanged.
