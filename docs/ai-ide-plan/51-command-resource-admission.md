# Cross-run command resource admission

## Scope

Credential-free ordinary and project commands can opt into durable resource
admission across all runs and command targets owned by one Ion coordinator database.
A second cap applies to each Ion project UUID. This is an owner allocation policy;
it is not an attestation of host capacity, cloud quota, actual spend, or GPU capacity.
GPU jobs retain their separate project/tenant resource and verified cleanup policy.

The charged compute quantities are the actual owner-approved sandbox cgroup
ceilings: CPU bandwidth in millis, cgroup-charged RAM bytes, and tasks (processes
and threads). The worker checks the kernel group before arbitrary code runs; see
[verified resource groups](45-sandbox-resource-groups.md). Admission requires that
approval and the native instance request's finite `resources_limits.max_layer_bytes`.
No guessed provider fields or agent-selected quota paths are introduced.

## Owner setup

In the private `ION_TEAM_COMMAND_IMAGES` registry, add `admission` beside
`resources` and `project_snapshot`. This example admits two commands globally and
one per Ion project, while reserving at most 512 MiB of aggregate command storage:

```json
"admission": {
  "version": 1,
  "owner_limit": {
    "jobs": 2,
    "cpu_millis": 1000,
    "memory_bytes": 268435456,
    "processes": 64,
    "storage_bytes": 536870912
  },
  "project_limit": {
    "jobs": 1,
    "cpu_millis": 500,
    "memory_bytes": 134217728,
    "processes": 32,
    "storage_bytes": 536870912
  }
}
```

Pair it with a verified `resources.group` of 500 CPU millis, 128 MiB and 32 tasks.
Set `max_layer_bytes` within the storage allowance, leaving room for project
archives. The example is not a deployment recommendation. The operator must
configure the runtime cgroup and import the exact approved worker image.

Limits must be finite and positive: jobs 1..128, CPU millis 100..4096000, memory
64 MiB..1 TiB, tasks 2..32768, storage bytes 1..1 TiB. Every project dimension must
fit its owner dimension; one command must fit both. Invalid versions, extra fields,
missing group/native layer witnesses, zero quantities and overflows are refused.

The optional field is omitted from historical serialization when absent. It is
owner policy, never accepted from model arguments. Original native execution bodies,
worker receipts, durable request IDs and replay signatures retain their existing
contracts. No new setup is required for unchanged policies that have no earlier
aggregate admission holds.

## Atomic reservation and cap changes

The original command, compute/storage allocation and project archive are committed
in one SQLite `IMMEDIATE` transaction before instance submission or project upload.
Concurrent runs cannot race past the cap. A lost acknowledgement, crash, restart,
Stop or lease expiration preserves the original reservation; none repeats compute.

Admission combines outstanding quantities across all command targets and runs in
the owner database, independent of provider target/image/namespace changes. Project
usage combines all targets and runs with the same Ion project UUID. Every dimension
uses the intersection of the new approval and all relevant outstanding approvals.
Editing a registry to increase a limit cannot enlarge an earlier outstanding cap.
A tighter cap may refuse new work until usage fits it. New work cannot omit
`admission` to bypass a prior retained approval.

Historical jobs are inspected from their original saved policy. Known group/native
layer bounds count even if the old policy had no aggregate admission object.
Unknown active compute or unknown retained layer bounds are not treated as zero;
new checked admission is refused until actual original evidence is resolved.
The usage summary reports unknown jobs separately. No invented estimate fills a
missing historical allocation. Checked admission and usage inspection are bounded to 2048 outstanding
or historical records and process witnesses one at a time on the owner worker.
Unconfigured legacy submissions use indexed retained-cap detection and keep their
original path when no aggregate approval remains; they do not gain this history cap.

## Compute release and retained storage

The owner persists separate compute and mutable-layer release flags. A validated
receipt from the original trusted worker releases only compute. The isolated CLI
calls `stop_tree` and the Linux subreaper's `reap_descendants` before collecting
outputs or producing its receipt. The owner accepts the full request-matching
receipt only after the original operation/image identity and successful supervisor
exit are verified. `finish_job` revalidates the full saved request before release.
Nonzero command exit or a command timeout can still have a trusted teardown receipt;
they do not claim successful task work.

No receipt, a malformed receipt, `FAILED`/`CANCELLED` status alone, Stop, a cancellation
acknowledgement, or a lost spawn acknowledgement releases compute. The reservation
remains visible even when the task outcome is recorded as failed. A definitive
rejection before instance creation can release compute and the uncreated layer.
Recovery never issues another create to resolve an uncertain original request.

Storage charge consists of the full native mutable-layer ceiling plus twice each
staged archive's byte length: the retained local database copy and a conservative
uploaded-copy allowance. Content-addressed deduplication does not reduce this charge.
Even a staging rejection keeps the archive/upload allowance because upload may have
succeeded before an acknowledgement was lost. Terminal compute does not prove remote
layer or uploaded-file deletion. Neither a valid receipt nor task completion releases
these storage charges or the related stricter cap.

This step adds no force-release button, guessed deletion endpoint, or automatic
retention deletion. A future cleanup path must prove removal of the exact original
remote objects and apply a separately authorized local retention policy. Until then,
retained storage can intentionally exhaust the admission allowance. This charge is
a conservative logical allocation, not an invoice or complete host filesystem limit.
Stored request/receipt JSON, published artifacts, logs, caches and unrelated files
are outside this layer-and-archive allowance and need separate retention controls.
Provider enforcement of the requested mutable layer limit still needs live runtime
acceptance; uploaded storage, total owner host storage, network egress and GPU VRAM
are separate controls.

## Native status

`TeamCoordinator::command_resource_usage(None)` returns owner totals;
`Some(project_uuid)` returns project totals. The transactional reply contains used
quantities, the intersected outstanding limit when configured, unknown jobs, held
compute count and retained local archive bytes. It contains no commands, host paths,
credentials, provider handles or release authority. The desktop reads it on a worker;
render uses the cached summary. A missing limit means no retained aggregate approval,
not unlimited verified host headroom. Zero known usage with unknown jobs is not proof
of available capacity.

## Automated evidence and review checklist

Fresh locked `waku-core` test compilation, including `waku-protocol` and
`waku-client`, passed after switching source worktrees and narrowly rebuilding
protocol artifacts. **56 tests passed**: 21 admission SQL tests and 35 ordinary/
project command driver/coordinator/HTTP tests. This includes 26 new admission
cases. Two existing tests that require exported real Linux CLI fixtures were
explicitly ignored; they are not counted as passed. Formatting and whitespace
checks passed. No paid cloud, device, visual, GPU, or live runtime test ran.
The test cases cover actual production SQLite schema and transactions, cross-run
and project isolation, target changes, each vector dimension, cap expansion/tightening,
policy omission, historical unknown/known groups, checked overflow, atomic archive
rollback, restart/lost acknowledgement, concurrent admission and release distinctions.
Coordinator/HTTP cases verify actual command submission is refused before POST,
project upload is refused before POST, replay keeps one hold, invalid receipts keep
compute, valid original receipts release only compute, and precreate rejection frees
only its proven allocation.

Owner acceptance:

- [ ] Configure reviewed group limits on a real imported command image and runtime.
- [ ] Verify the actual native mutable layer ceiling is enforced under a full layer.
- [ ] Run concurrent real ordinary and project commands across two Ion projects;
      verify owner totals, project totals and refusal at each approved dimension.
- [ ] Stop/restart during upload and spawn acknowledgement loss; observe the original
      operation and confirm held capacity remains until original proof arrives.
- [ ] Exercise malformed/missing worker receipts, OOM and escaped-grandchild teardown;
      confirm no guessed terminal cleanup or repeated launch releases reservations.
- [ ] Review retained storage exhaustion; provision explicit verified cleanup before
      claiming long-running unattended storage recycling.
- [ ] Validate desktop status, keyboard/legibility and stale-response behavior in the
      watcher-managed app. No visual test was run for this implementation.
- [ ] Keep GPU device/CUDA acceptance, GPU/project/tenant reservations, cloud billing,
      wider host/network controls and speech/device deployment as separate gates.
