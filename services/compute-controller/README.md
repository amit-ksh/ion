# Operator compute controller

This **Linux-only** Node 22+ process runs outside the disposable app VM. It has
no public listener. It polls the authenticated Worker for one pinned instance's
desired action and uses the official `@nebius/js-sdk` **0.2.81** gRPC client.

An operator-owned Linux machine is sufficient for the pilot. It must stay
online for wake/stop decisions. This does not create another paid host or promise
24/7 availability. If the controller itself loses power, it cannot guarantee a
provider stop; deploy it on an appropriately supervised operator host before
accepting paid runtime. Linux shutdown alone does not stop Nebius billing.

## Prepare

1. Provision the dedicated app VM using the existing hosting runbook. Confirm
   project/region, actual upper-bound hourly cost, retained disk/IP costs, image
   digests, gVisor and quota-aware XFS. The controller never creates a VM or picks
   a chargeable preset automatically.
2. On the **separate Linux operator host**, install Node 22+ and workspace
   dependencies. The pinned SDK imports the native `fs-ext` module, so build that
   module for the operator's Node ABI (`npm rebuild fs-ext`, with Python, make and
   a C++ compiler installed). `bun install --ignore-scripts` alone is insufficient.
   Windows SDK import was checked and fails without this native dependency; do
   not package this controller in the Windows/Android application.
3. Create a scoped Nebius service-account credentials file; allow only required
   instance observation/start/stop operations in the approved project. Do not
   associate this credential with the app VM or app containers. Keep the file and
   environment owner-readable only (0600), and the state directory 0700.
4. Fill `operator.env.example` privately. Use the same random controller token in
   the Worker `COMPUTE_CONTROLLER_TOKEN` secret. Set the exact same instance and
   project in `COMPUTE_POLICY_JSON`. Never paste credentials into chat or commit
   them. The token is separate from `HOSTING_WORKER_TOKEN`.
5. Review/install the example systemd unit with real paths and a dedicated user.
   Start **after** Worker deployment. The native app's owner can then enable Cloud.

From the repo root, with the private environment loaded:

```sh
node --experimental-strip-types services/compute-controller/src/main.ts
```

## Failure rules

- Intent is persisted before every mutation; the returned Nebius operation ID is
  persisted and reconciled across restart. Mutation retries in the SDK are zero.
- Observations verify the exact VM/project. Competing controller IDs are rejected
  while a live controller lease exists; local state also has a single-process lock.
- VM starts reserve the maximum configured runtime plus stop allowance first.
  Idle sessions stop; hard runtime/startup failures latch Cloud until the owner
  explicitly switches Cloud OFF then ON. Token Factory changes cannot clear this.
- When the gateway is unavailable, the locally persisted deadline triggers a
  provider stop attempt. Failed stop requests are retried after observation and
  a 30-second bound. A successful request is not a confirmed stop.
- Only an observed provider STOPPED state with a settled operation releases a
  known-started session's hold. An ambiguous start that never becomes observable
  keeps its full hold and requires operator reconciliation. There is no automatic
  refund or silent new start after an uncertain outcome.
- Retained disks/IPs may still cost money while stopped. The policy leaves USD 3
  of the requested monthly/lifetime envelopes outside runtime reservations; an
  operator must verify that allowance against actual retained resource charges.

Local verification uses fixture observations and checks official SDK types; the
actual Linux native SDK, credentials, provider operations and billing must still
be accepted on the operator host. See plan 117 for the complete handover.
