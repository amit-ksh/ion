# Managed Token Factory sandbox boundary

20 October 2026. Backend implementation and local acceptance. This is separate
from long-running app-hosting VMs and does not claim a live sandbox was launched.

## Actual access result

The saved operator inference key was tested against the documented read-only
`GET /sandboxes/v1/whoami`, with both supplied project IDs. Both requests returned
200, but `spawn`, `spawn_disposable`, `list`, `cancel`, `import` and
`set_image_tag` were all false. Limits were returned, including 50 concurrent
operations and 3600 seconds maximum timeout. Those limits do not grant access.

The inference key is accepted for introspection. A second key is not assumed to
be mandatory. The project/caller needs sandbox activation or permission before
running, listing or cancelling operations. See [Nebius access setup](https://docs.tokenfactory.nebius.com/sandboxes/start/set-up-access).
No image import, sandbox, VM, disk or controller resource was created for this check.
Evidence: `.waku-cache/ion-sandbox-access.json`.

## Implemented backend

- Authenticated, admitted requests are scoped to the verified Ion workspace.
  Clients cannot choose project credentials or access another account's operation
  or uploaded file. Provider tokens stay in the Worker secret store.
- Only reviewed immutable images, their exact manifests, fixed Ion worker
  executables, bounded stdin, time, output and writable layers are accepted.
  Command workers retain the existing non-networked child/resource boundary;
  arbitrary proxy endpoints, shell entrypoints, environment overrides, image
  imports and retained result images are rejected.
- The request UUID, exact payload and full spending hold are committed before
  the only spawn POST. A lost acknowledgement blocks resubmission and local
  fallback. A confirmed UUID can be read again after restart without launching
  another operation. Recovery requires the saved original payload and an account-
  stamped provider hostname; a foreign account cannot claim a lost operation by
  copying its logical request ID. Native recovery retains the original witness.
  Status, cancellation and alarms retain the original project even after the
  operator removes or replaces the launch policy.
- Per-account/global concurrency, request rates, upload bytes and retained
  history are bounded. Project uploads are checksummed and owned by the caller;
  uncertain upload acknowledgements require reconciliation instead of replay.
- Only an operator-supplied verified operation price ceiling enables dispatch.
  Uploads additionally require their own verified retention/price ceiling.
  Reservations use the combined Ion spending envelope. Unknown outcomes and raw
  provider `cost` fields never release a hold.
- The shared Token Factory permission gates new work. Durable Object alarms
  independently observe submitted operations and cancel them after owner stop or
  the runtime deadline. Cancellation acknowledgement is followed by actual
  terminal-state observation. Lost-client connections do not remove this duty.

No production `SANDBOX_POLICY_JSON` is installed: accessible reviewed images and
real pricing are still unverified. `/v1/sandbox/config` truthfully reports this
unavailable state. Enabling Token Factory inference cannot bypass those checks.

## Native integration

The existing registered harness and project-command clients can use the account-
bound gateway when no personal sandbox IAM environment is configured. Managed
requests refresh Ion login without receiving provider credentials. Image command
policies come from the reviewed backend registry; personal operator configuration
remains compatible. The existing coordinator still owns exact command/project
approvals, file snapshots, result receipts and recovery decisions.

Native chat also offers exact approved command execution through the reviewed managed
command registry. It uploads no project files. A real nonzero process exit is
preserved as a failed command, even when the sandbox operation itself completes.
Native compilation and account-scope results are recorded in [126](126-managed-native-execution.md).
No live image, installed-app or physical-device acceptance is claimed by fixtures.

## Verification

- Seven backend tests, 78 assertions: disabled permission, cross-account reads,
  spending reservation, restart, changed payloads, uncertain launch, owner stop,
  injected environment, retained images and owned file transfers.
- Seven local workerd integration tests pass. The sandbox test uses real Worker
  authentication and SQLite, restarts the object, prevents a duplicate dispatch,
  turns Token Factory OFF and waits for its actual alarm-driven cancellation.
- Full cloud backend suite: 43 tests, 444 assertions. Worker TypeScript passes.
- These use fixture sandbox responses. They are not live Nebius execution or
  invoice reconciliation.

## Required live acceptance

1. Enable sandbox permissions for the intended Token Factory project/caller.
2. Discover/import and inspect the reviewed immutable images and their manifests.
3. Confirm operation, upload/retention and billing units with Nebius; do not use
   raw operation `cost` as an invoice amount. Install a policy only within the
   existing USD 10 target / USD 15 cumulative authorization.
4. Run one short approved command, transfer a bounded fixture, verify output,
   cancel a second bounded operation, and confirm terminal states and retained
   billable resources. Do not infer shutdown from a client timeout.

References consulted: [limits and billing fields](https://docs.tokenfactory.nebius.com/sandboxes/operate/limits-retention-and-usage),
[spawn schema](https://docs.tokenfactory.nebius.com/api-reference/sandboxes/instances/spawn-a-new-container-instance),
[operation lifecycle](https://docs.tokenfactory.nebius.com/sandboxes/concepts/execution-lifecycle),
[file uploads](https://docs.tokenfactory.nebius.com/api-reference/sandboxes/files/upload-a-file-to-the-server-the-body-must-be-a-file-content).
