# Native publisher reconnect and live relay counters

22 October 2026. Code `fd9a36a`; installed Windows package and Moto upgrade
are verified in plans 172–173. This follow-up records actual native/account and
deployed telemetry checks, without claiming full interactive acceptance.

## Expired grant and native recovery

A read-only, integrity-checked observation of the known Ion vault transaction
found a stored session for the expected origin/issuer, no personal pairing and
an expired managed device grant. No token, identity, header or credential blob
was saved. This means the quiet initial post-upgrade period cannot establish
healthy paired idle behavior.

The existing opt-in native `live_native_desktop_publish_grant` test passed using
the compiled credentials binary SHA-256
`a19c2a754aeb8a1aa8dddda462e595cda82b530327817c31ad1f5944bb4d548e`.
It verified the current native account, revoked the previous device grant and
registered/verified the same publish-only desktop permission. Its sanitized
receipt reports `newRegistration: true`, `publishGrantVerified: true` and zero
paid calls. No service switch, microphone or screenshot was used.

The same binary's `live_managed_gateway_catalog_and_stop_controls` test passed:
actual account-scoped connected-app catalog, spending/control reads and the
applicable disabled-service denials. No provider tool effect or paid inference
was executed. These are production native transport checks, not GUI login,
account-switch or phone-pairing acceptance.

Local process censuses found one installed `ion.exe` and one installed
`waku-daemon.exe`, started at 12:00 IST. This excludes duplicate processes in
that observation, not publishers on another machine or every multiwindow race.

## Actual deployed counters

After reconnect, the 12:14–12:17 IST analytics window returned all three minute
buckets under Worker version `8c731253-d440-4129-bf00-0c6da5ef6c2c`:

| Namespace | DO requests | SQL rows read | SQL rows written | Duration GB-sec |
| --- | ---: | ---: | ---: | ---: |
| AccountRelay | 32 | 1,282 | 18 | 0.014788736 |
| Platform | 9 | 222 | 0 | 0.001564288 |
| Total | 41 | 1,504 | 18 | 0.016353024 |

No SQL stored-byte sample was returned. Missing storage samples are not zero
stored bytes. The earlier 112-request reconnect observation is a startup sample,
not the settled rate. Namespace totals are shared-service observations and are
not exclusive attribution to this desktop or account.

A separate bounded 60-second Worker tail at 12:16:56–12:17:56 IST retained only
allowlisted route counters: six public task-sync calls, five public owner-sync
calls, two controls reads, two usage reads and three other HTTP calls, all `ok`.
Raw URLs, IDs, request bodies, headers, console messages and credentials were
discarded. The observer stopped its own process at its limit; no app/daemon was
stopped. The two batch routes are consistent with ten-second polling. This route
sample is not the same time window or metric as DO analytics and must not be
added to the DO request total.

No test task or command was dispatched during these windows. User/other-client
activity was not controlled, and the samples are short; prolonged idle, changed
tasks, approvals, cancel/reconnect and duplicate-window native acceptance remain.
The local before/after workerd measurements in plan 158 retain their own scope.

## Conditional pilot forecast

Scaling the combined three-minute observed rate as if it represented one user,
with identical data and no extra active work, gives the following planning
figures. These are conditional arithmetic, not a measured 10–50-user load test.

| Users / connected hours per day | DO requests/day | Rows read/day | Rows written/day |
| --- | ---: | ---: | ---: |
| 1 / 24 | 19,680 | 721,920 | 8,640 |
| 10 / 24 | 196,800 | 7,219,200 | 86,400 |
| 50 / 24 | 984,000 | 36,096,000 | 432,000 |
| 10 / 8 | 65,600 | 2,406,400 | 28,800 |
| 50 / 8 | 328,000 | 12,032,000 | 144,000 |

[Cloudflare's published Free limits](https://developers.cloudflare.com/durable-objects/platform/pricing/)
are 100,000 requests, five million rows read and 100,000 rows written per day.
The always-connected pilot projections exceed request/read limits; 50 users
also exceed writes. This account's plan/invoice and the original 90% meter are
still unverified. No paid upgrade, permission expansion or data deletion occurred.

## Sandbox resource check and remaining gate

Three read-only, status-filtered operation-list requests for owner-confirmed
`aiproject-e00h49rtswkv9bz46s` each returned HTTP 200 and an empty complete page:
`PENDING`, `ASSIGNED`, `EXECUTING`. No active operation was observed and none was
created/cancelled. This is not a compute-VM inventory, invoice or historical
zero-cost claim. Provider-compatible worker registration and project-confirmed
operation/upload/retention pricing remain necessary before paid execution.
The current [provider usage guidance](https://docs.tokenfactory.nebius.com/sandboxes/operate/limits-retention-and-usage)
still requires confirmed raw resource units and actual billing records; it does
not establish this project's numeric sandbox price ceiling.

## Evidence

Local sanitized reports in `.waku-cache/`:

- `ion-postupgrade-local-publisher-observation.json`
- `ion-postupgrade-native-publisher-job.json` and `ion-postupgrade-native-publisher-grant.json`
- `ion-postupgrade-managed-reads.json`
- `ion-cloudflare-postreconnect-20261022.json`
- `ion-cloudflare-settled-window-20261022.json`
- `ion-postreconnect-route-counters.json`
- `ion-sandbox-active-operations-20261022.json`

No full goal completion or final QA handoff is claimed. Current device/GUI,
provider billing and real paid-sandbox lifecycle gates remain open.
