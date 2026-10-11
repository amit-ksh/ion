# Headless registered-owner gateway controls

## Implementation

The production daemon now owns outbound workflow/routine/run publication and
phone start, stop, stage decision and schedule controls. A registered Linux
service can serve this workflow view without an open desktop. Each owner still
has its own gateway device identity and the phone's Read workflows / Control
workflows grants remain separate from task grants.

The worker uses the existing coordinator directly, with the same transactional
revision checks and project validation as the direct daemon RPC. It opens no
additional listener, obtains no copied daemon token and performs no work on the
UI thread. Projection rows remain bounded and exclude private profile bodies,
workflow prompts, local paths, credentials and document contents. It does not
refresh individual-task heartbeats, so an offline desktop task mirror cannot
appear fresh just because its workflow publisher is online.

A durable relay claim precedes each effect. The owner rechecks its pairing and
publish grant after claiming. Shutdown fences new local mutations and drains an
in-flight database mutation without waiting on gateway network I/O. A database
or commit error keeps the claim unknown. A lost gateway settlement is reconciled
only against the exact persisted start UUID, definition identity and pinned
revision. Absent starts and unknown decisions/stops/schedule controls are not
replayed. Each pass processes at most 32 queued and 32 unknown receipts, with a
20-second processing budget and 5-second HTTP deadlines. The normal poll interval
is five seconds; these are bounds, not a guarantee of five-second network delivery.

Persisted desktop pairings use the daemon worker. A temporary memory-only
pairing keeps its existing desktop automation bridge because that capability
cannot cross into another process. Save in an available OS vault or register a
Linux service file before expecting headless operation.

## Linux owner setup

1. Keep the dedicated owner-managed daemon and persistent project/provider state
   from [personal cloud setup](14-phone-asr-cloud-execution.md#personal-cloud-execution).
   Deploy matching reviewed desktop/daemon versions when upgrading a desktop.
2. In the gateway owner console, create a new invite with **Publish desktop
   state** for this service. Use a distinct name such as `Cloud owner`. Add
   **Propose app actions** only if this host's agents need gateway app proposals.
   The human owner's token must stay on the gateway and is never this file.
3. Consume the invite once through the existing `POST /v1/pair` route, supplying
   `invite` and `name`, using a trusted HTTPS client. Keep its response local.
   Store these exact fields in `/etc/ion-daemon/gateway-pairing.json`:

   ```json
   {
     "baseUrl": "https://your-gateway.example/",
     "deviceId": "the-response-id",
     "token": "the-response-device-token",
     "expiresAt": 0
   }
   ```

   Replace all placeholder values: `deviceId` is the response's `id`, and
   `expiresAt` is the response's actual Unix **milliseconds**, not zero or
   seconds. Do not include `grants` or other response fields. Keep this file
   outside workspaces, task databases, logs and source control. Set owner to
   `ion-runner`, mode `0600`, and place it in a protected secrets directory.
4. Uncomment the optional `ION_GATEWAY_PAIRING_FILE` line in the included service
   example. The path must be absolute. Restart the owner-managed service after
   changing the path; rotating contents at that path is read on later requests.
   Use a newly written private regular file and atomic rename for rotation.
5. Pair the phone with **Read workflows** and optionally **Control workflows**.
   Open **Personal gateway → Workflows & routines**, select this owner and
   inspect its project and current stage before taking an action. No inbound
   daemon port or full-control daemon token is needed for this workflow relay.

The startup validator checks the opened descriptor with `O_NOFOLLOW` and
`O_NONBLOCK`. It requires a regular file owned by the service user, no group or
other permissions, one link, bounded JSON, a registered UUID, a bounded URL-safe
device token, unexpired millisecond timestamp and an HTTPS origin (loopback HTTP
is allowed for local checks). Malformed/unavailable configured files fail closed.
The source applies to gateway app proposals as well. Vault pairing/deletion is
refused while a file-managed source is configured, preventing a silent identity
change. Settings can report the managed-file storage state.

## Automated verification

- Nine new daemon owner tests passed: durable start/restart receipts, missing
  receipt/non-start non-replay, grants/support/claim identity, post-claim revocation,
  shutdown fencing/drain, stale revisions/missing projects, real gate/stop/routine
  mutations and database failure classification.
- Three new pairing contract tests passed for exact file schema, origin/device/
  token/expiry, capability rotation and prevention of silent vault replacement.
- One shell environment regression passed: a profile cannot re-import the
  service pairing-file path.
- Thirty existing workflow/routine/calendar/branch/document/retry regressions
  passed against the same current core test binary.
- Locked desktop, daemon and selected-harness worker Cargo checks passed.
  Protocol 37 binding equality, Rust formatting and whitespace checks passed.

These are **43 distinct passed tests**; the initial eight-test owner run is a
subset of the final nine and is not counted twice. Tests use private temporary
state and simulated gateway transport failures. They do not establish actual
Linux service deployment, real gateway HTTP/TLS/device acceptance or provider
execution. No real saved pairing or billable resource was used.

The Linux-only opened-descriptor test is written but **not run** here. The
registered Ubuntu WSL disk is missing (`ERROR_FILE_NOT_FOUND`), and both configured
Docker endpoints are unavailable. No runtime/service was started or repaired.
Run that test on the owner's Linux build host before deployment. No visual test
or extra development watcher was started.

## Owner review checklist

- [ ] Run the Linux descriptor-validation test in `waku-credentials`, including
  public permissions, symlink, FIFO, hard link, oversized file and atomic rotation.
- [ ] Deploy a service with its own publishing device and persistent project,
  selected harness and credentials. Keep the desktop off; verify a fresh phone
  workflow list, one start UUID and the expected owner/environment.
- [ ] Approve/decline a reviewed stage, stop a run, and pause/enable a routine.
  Verify selected execution, child cancellation and no later unauthorized stage.
- [ ] Rotate/revoke the publisher and phone. Verify old capabilities stop working,
  configured-source errors do not fall back, and another owner cannot claim work.
- [ ] Drop settlement responses and restart the daemon. Verify exact-start read
  recovery, and no replay of unknown gates/stops/schedules or absent starts.
- [ ] Verify expired millisecond timestamps, missing source, relative path,
  wrong ownership/permissions and malformed JSON fail without secret log output.
- [ ] Confirm app-action proposals use this service identity and still require
  the configured exact external-action decision before execution.
- [ ] Check real TLS, gateway/device reconnect and expected service resource usage.

## Remaining scope

This is outbound workflow metadata and controls. Broader historical gateway
Inbox/usage projections, paged workflow/evidence feeds, full document transfer
and background delivery remain development work. Gateway individual-task
mirroring still uses the desktop; the direct headless daemon connection already
provides native phone tasks, transcripts, decisions, artifacts and authoring.
Live cloud/provider/speech deployment and installed-device acceptance still
require the owner's later target, credentials and review. Deferred new harness
adapters remain out of scope.
