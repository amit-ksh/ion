# Resolved review notifications and worker verification

21 October 2026. Follow-up to plan 160 and commit bf2ce72. Native recovery tests
and refreshed client packages remain pending; this report covers backend source
and local worker acceptance only.

## Notification correction

A real HTTP/SQLite regression reproduced a notification for a late snapshot that
the relay correctly recorded as resolved. The `interaction.created` flag means a
new database record, not a pending action. The handler now queues a review notice
only when the record is both newly created and pending. The same regression
proves a genuinely new pending request still queues exactly one notice. Test
push requests use a local fetch stub; no actual phone notification was sent.

## Verification

- HTTP/task-relay tests: **19 passed, 148 assertions**. The new notification
  regression failed before the correction (one push job instead of zero).
- Both backend TypeScript projects passed. A fetch-mock signature error in the
  new test was corrected and the personal gateway check rerun successfully.
- Real workerd SQL/cursor test: **1 passed**, including complete worker shutdown
  and restart with the same disposable SQLite persistence. A previously resolved
  but unpublished request stayed resolved after restart; a new pending request
  still reached the phone fixture afterward.
- Fresh production bundle public-route suite: **8 passed**. These cover local
  JWT account isolation, pairing/restart durability, static publishing/storage,
  connected-app scoping and controlled sandbox/budget behavior. External services
  are stubbed; this does not prove real provider execution or device effects.
- Bundle SHA-256:
  `9d946b238a7702a242b419a12b473d86845a7071ab90c47d87acb06443c4e641`.
  All 61 source-map entries were compared against the preceding deployed bundle.
  Only `packages/personal-gateway/src/http.ts` and `task-relay.ts` changed; both
  mapped sources match current files exactly.

Raw local evidence is in `.waku-cache/ion-resolved-notice-{red,tests}.log`,
`ion-resolved-notice-types-fixed.log`, `ion-scoped-cloud-worker-types.log`,
`ion-scoped-relay-workerd-sql.{json,log}`, `ion-scoped-worker-runtime-tests.log`
and `ion-scoped-worker-bundle-comparison.json`.

## Actual workerd counters

These are simulated-clock HTTP cycles with one publisher and one phone, using
actual Durable Object SQLite cursor accounting. They are **not live billing** or
native worker-loop measurements. The earlier before/after comparison in plan 158
remains valid for idle traffic; the new table introduces no additional idle rows.

| Scenario | HTTP requests | SQL statements | Rows read | Rows written |
| --- | ---: | ---: | ---: | ---: |
| Idle, 100 seconds at 5-second fixture cadence | 40 | 880 | 600 | 9 |
| Idle, one hour at 10-second cadence | 720 | 15,840 | 10,800 | 360 |
| Changed task and snapshot | 2 | 34 | 17 | 6 |
| Equal task and snapshot replay | 2 | 24 | 15 | 0 |
| First resolution before publication | 1 | 14 | 7 | 2 |
| Twenty repeated resolutions | 20 | 280 | 140 | 0 |
| Late publication and phone read after restart | 2 | 26 | 17 | 3 |
| Identical late publication replay | 1 | 13 | 9 | 0 |

Two workerd row writes for the first resolution include SQLite index accounting;
they are not two application records. A request still incurs authentication and
database reads when its content is unchanged. Client coalescing suppresses
unnecessary sends; conditional writes alone do not reduce HTTP request counts.
Stored bytes are a separate metric and were not measured by this test.

The stop/permission paths, uncertain-command recovery, exact permissions and
missing-snapshot recovery also passed. Full measurements are saved alongside
this document in `161-relay-worker-sql-measurements.json`.

## Remaining deployment and client gates

- Production remains version `d948eb00-1607-497d-9dd8-9e7dc75dbcbb` until a new
  deployment receipt proves otherwise. Revalidate version, semantic routes,
  source, tested bundle and secret-name hashes before publishing. No credentials,
  service controls or permissions need to change.
- The first dry-run rejected an unsupported CLI flag; the second was blocked by
  profile filesystem access. The corrected dry-run passed through the permission
  system. Neither failed attempt uploaded a worker; logs were preserved.
- The bf2ce72 native follow-up waiter stopped as designed when this source changed,
  before starting another Cargo process. Preserve its receipt, then queue a
  separately pinned follow-up for this committed source. The original native
  owner job, session 32910, remains live and was not restarted.
- Native fingerprint/combined checks, generated bindings, authenticated mobile
  presence, new EXE/ZIP/APK provenance and upgrade acceptance remain required.
- Original live Cloudflare meter identity/window/plan and post-client-rollout
  attribution remain unverified. This does not expand the 10–50-user Free-plan
  request budget documented in plan 158, or authorize a paid plan.
