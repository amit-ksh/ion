# Verified sandbox resource groups

## Scope and configuration

Credential-free ordinary/project command images can opt into a kernel-enforced
aggregate CPU, charged-memory and process/thread ceiling for the entire sandbox.
The existing per-process rlimits remain. Add `group` inside the private owner
command-image registry's `resources` object before using that target in a run:

```json
"group": {
  "version": 1,
  "cpu_millis": 500,
  "memory_bytes": 134217728,
  "processes": 32
}
```

This requests at most half a CPU of execution bandwidth, 128 MiB of cgroup-charged
memory and 32 tasks across the supervisor, command and all descendants. Owner
bounds are 100..32000 CPU millis, 64 MiB..8 GiB memory and 2..256 tasks; tasks must
also fit the configured UID process rlimit. Swap must be disabled. A tighter
actual runtime limit is accepted. These numbers are examples, not a recommended
VM shape or a promise that a requested runtime allocation exists.

The pinned image manifest must declare
`resource_group_boundary: linux-cgroup2-sandbox/v1`. The owner checks that exact
contract before persisting/submitting the command. Complete group policy stays
inside the existing original policy/environment witness; arguments cannot select
a cgroup path or enlarge a cap. Historical policies omit the optional field and
retain their original signatures, replay and recovery behavior.

## What enforces the cap

The owner does not write guest kernel settings or invent unverified ConTree API
fields. The operator/runtime must configure the sandbox's real cgroup limits.
The fixed root supervisor and demoted guard independently verify the actual
cgroup before arbitrary code can run. They require a private unified cgroup root,
real cgroup2 filesystem, domain type, root-owned non-writable controls, the checking process's membership in that exact
kernel group, finite CPU/memory/task caps no greater than the approval and zero swap. CPU quota uses
integer cross-multiplication. Unlimited, malformed, missing or incompatible
controls refuse execution. The guard removes the approval environment before
user exec, then keeps the existing nonroot/no-new-privileges/socket boundary.

The check is read-only; it never creates/remounts a hierarchy or changes daemon,
host or unrelated-container resource settings. Descendants inherit the existing
kernel group, and the unprivileged child cannot migrate or change its controls.
The reviewed image/runtime must keep other privileged processes from changing
the verified group during the command. Both checks run on remote worker processes;
no work is added to the app UI render path.

A group failure is explicit. A supervisor failure has no accepted command receipt;
a guard failure yields exit125. Neither proves a successful check. Original remote
uncertainty, Stop, receipt/recovery and no-resubmission rules remain intact.
An OOM kill of the supervisor can leave the provider result uncertain; it is not
an authorization to repeat the command or release a monetary reservation.

## Local automated evidence

The source-only derivative test image uses the verified installed base
`sha256:050fbc7a43e32a8b1f550ef1a6520615168fc5f40832057ebd72dac6fd8d6f55`.
No dependency install or cloud resource is part of this test. Native Docker checks
use private cgroup/PID scopes, no network, explicit CPU/RAM/swap/task caps and a
bounded temporary filesystem. The six group tests run with a read-only root.

- Pure policy tests reject extra/missing fields, boolean/float values, unsupported
  platforms, unlimited controls, relaxed caps and invalid integer ratios.
- Native Linux tests inspect actual controls, run the production installed worker,
  verify nonroot identity/removed environment and reject relaxed runtime ceilings
  before command code.
- Excessive forks hit the shared task limit; child writes to limit/migration
  controls fail. Actual memory OOM-kill and CPU throttling counters show enforcement
  across forked/busy user code.
- **69 tests passed:** 4 pure policy, 7 source worker, 6 installed native group,
  13 installed native CLI, 9 installed socket-guard and 30 native owner/command tests.
  One optional owner-generated executable archive case and two Rust fixture-export
  cases were not run; they are explicit separate acceptance gates. Existing Rust
  warnings remain. After the phone artifact branch merge, locked desktop/daemon/worker
  compilation, freshly built merged core tests, protocol44 binding freshness and
  complete mobile/gateway/client TypeScript checks passed.

Current locally tested derivative image ID: `sha256:2200d363e2a9be95997968a5aecdc823a29d57a301b2593b2cecb76677bf4cca`. It copies current
worker/guard/manifest/test files onto the verified local base without installing
packages or using cloud credentials. Native group tests use no-new-privileges and
read-only root; legacy CLI/guard tests use a writable disposable image because
those cases deliberately modify kernel-policy and private project/result fixtures.
Their resource/network/cgroup and temporary-filesystem caps remain explicit.

## Later owner setup and review

- [ ] Rebuild/review/import a new immutable credential-free command image with the
      group marker and current worker/guard, accepted kernel and signed libseccomp.
- [ ] Confirm the selected managed runtime exposes the required private cgroup2
      domain and finite limits before enabling group approval. An image label alone
      is not proof; incompatible runtimes stay refused.
- [ ] Review CPU/memory/task ceilings for actual dependencies, including supervisor
      overhead, output collection, threads and concurrent processes. No swap is allowed.
- [ ] Add the exact group approval to the private registry before the target policy
      is pinned by a run. Do not change a used policy to continue an old command.
- [ ] On the exact Nebius image/runtime, test successful work, cap mismatch, fork/
      thread pressure, CPU throttling, OOM, Stop, lost acknowledgment and original
      recovery. Confirm failed/uncertain work cannot become successful check evidence.
- [ ] Verify network/device/IPC/filesystem restrictions separately, including
      metadata, same-network and proxy escapes; run the existing socket guard guide.

## Remaining scope

This caps one command sandbox's aggregate process tree, including its supervisor.
It does not implement cross-run owner/project admission or a host-wide scheduler,
combined storage quotas, GPU/VRAM/device/FUSE/CUDA support, bill finality, arbitrary
network access or new provider adapters. Existing image-layer and file limits,
network-disabled request and socket guard remain. Other local/fallback/harness
and GPU execution paths retain their existing contracts and enablement gates.

The native phone and desktop resource UI, real Nebius compatibility and operator
configuration are review/deployment gates; broader resource/egress development
remains in progress. These changes stay on `codex/ion-agent-teams`.

Kernel semantics were checked against the authoritative
[cgroup v2 documentation](https://docs.kernel.org/admin-guide/cgroup-v2.html).
