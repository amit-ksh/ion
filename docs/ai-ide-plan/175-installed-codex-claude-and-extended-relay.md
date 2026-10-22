# Installed Codex/Claude transport acceptance and longer relay sample

22 October 2026. `ion-prod`, reviewed code `fd9a36a`. Tests used the existing
installed daemon; no watcher, desktop or daemon was restarted.

## Installed binary identity

Read hashes of the installed executables match the verified release inputs:

- `waku-daemon.exe`: `dd0d15975119bf621a7df8f30d810dcef2dbe664fb42eac56fcbc248d7e145d5`.
- Installed `ion.exe` / Cargo output `waku.exe`:
  `69205c4fd643a88b77d47a89f013ace0685e918b342d9ee378a20c5238a8c491`.

The package intentionally maps the Cargo output to the Ion executable name.
Local exposure remained disabled; the existing daemon listened on loopback.
The test used its existing token in memory, without logging or persisting it.

## Actual supported harness checks

The authenticated protocol handshake and settings read passed. Both providers
were already enabled; no settings were changed. Native probes found:

| Harness | CLI version | Models | Actual bounded prompt |
| --- | --- | ---: | --- |
| Codex | 0.163.0-alpha.11 | 7 | Started, streamed expected exact reply, successful turn |
| Claude Code | 2.1.289 | 12 | Started, streamed expected exact reply, successful turn |

Each received one short prompt requesting only its fixed test marker, in an
isolated cache folder, Ask mode and computer use disabled. No permission or
user-input answer was sent. Existing project and chat IDs matched before/after;
the test did not write app task state or change settings. These were transient
native runtimes, not a GUI task creation/persistence or Teams acceptance test.

After completion, Cancel and Close acknowledgements passed for each owned
runtime, and Attach reported no remaining runtime. Cancel was sent **after**
completion; this does not verify cancellation of an actively streaming turn.
The subsequent daemon child census found only `conhost.exe`, not a direct
Codex/Claude harness child. No unrelated process was stopped.

The original reported Codex app-server and Claude streaming-input launch errors
did not reproduce through these installed adapters. Codex emitted one error
event naming GitLab/MCP, yet streamed the expected marker and finished
successfully. This is further real evidence that the known integration failure
does not mean Codex failed to start. No global MCP configuration was changed.

### Original diagnostic-helper failure retained

The initial read-only probe helper failed before either probe request because
it indexed an absent `provider_binary_overrides` map. Rust intentionally omits
that map when empty; production mobile consumers already normalize/default it.
The corrected helper uses optional lookup and both real probes passed. The
original failed receipt was preserved; this was not an app-source fix or a
reason to relabel the failed helper as successful.

Evidence in `.waku-cache/`:

- `ion-installed-daemon-harness-probe.json` (original failed helper).
- `ion-installed-daemon-harness-probe-complete.json` (corrected real probes).
- `ion-installed-harness-smoke.json` (both real bounded prompts and cleanup).

## Ten-minute deployed relay observation

The fixed **12:14–12:24 IST** window precedes the actual harness prompt test.
All ten minute buckets are present; analytics returned HTTP 200 without errors.

| Namespace | DO requests | Rows read | Rows written | Duration GB-sec |
| --- | ---: | ---: | ---: | ---: |
| AccountRelay | 110 | 3,445 | 58 | 0.047143680 |
| Platform | 30 | 740 | 0 | 0.005469696 |
| Total | 140 | 4,185 | 58 | 0.052613376 |

This is consistent with the initial sample in plan 174 and extends the observed
post-reconnect period. No SQL stored-byte sample was returned. Namespace totals
remain shared-service observations; user activity and other clients were not
controlled. This is not exclusive attribution, an invoice, a prolonged idle
load test or active command/approval/reconnect correctness acceptance.

If this combined rate represented one user's background operation, arithmetic
would give **20,160 requests, 602,640 rows read and 8,352 rows written per
connected day**. Multiplying by ten or fifty always-connected users still
exceeds the published Free request/read limits. Actual provider plan, the
original 90% meter, pilot load and cost reconciliation remain open. No paid
plan, expanded permission or data deletion was used.

Evidence: `.waku-cache/ion-cloudflare-ten-minute-window-20261022.json`.
Requests, SQL rows, duration and stored bytes remain separate metrics; outer
Worker tail counts from plan 174 must not be added to DO requests.

## Remaining scope

Real Teams/routine/workflow execution and active cancellation/recovery still
need their applicable checks. GUI account switching, native layout/keyboard,
TalkBack and physical dictation remain unverified. Sandbox image/pricing and
provider billing gates persist. No full completion or final QA handoff is claimed.
