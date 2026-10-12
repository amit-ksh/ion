# GPU sandbox CPU, charged-memory and task limits

Checkpoint: 2026-10-13, `codex/ion-agent-teams`, protocol 46. This extends the installed strict GPU worker with owner-bound aggregate limits. Actual CUDA/provider deployment is still pending.

## Owner approval and launch

Add an exact `child_group` object in the owner GPU resource policy:

```json
{
  "child_group": {
    "version": 1,
    "cpu_millis": 8000,
    "memory_bytes": 34359738368,
    "processes": 256
  }
}
```

This example permits up to eight CPUs of aggregate bandwidth, 32 GiB of cgroup-charged memory and 256 tasks across the supervisor, command and descendants. Choose limits for the actual runtime and workload; the example is not a hardware recommendation. Approval includes supervisor and output-collection overhead.

Native validation requires version1, 100..65,536,000 CPU millis, 64 MiB..262,144 GiB charged-memory ceiling and 2..4096 tasks. CPU and memory must also be no greater than the original approved job shape (`vcpus*1000` and `memory_gib*2^30`). GPU-sized ceilings do not enlarge the ordinary command profile's existing bounds.

The complete object is serialized into `ION_JOB_GPU_GROUP_LIMITS` in the exact original launch body. New preflight/pricing refuse missing approval, changed limits, shape overflow, invalid types/version or duplicate environment witnesses before provider reads/create. Historical omitted fields preserve their serialized policy/body for existing recovery; old requests are not rewritten to authorize a new launch. There is no new agent environment-selection mode.

The worker requires the strict runtime-profile approval when GPU group limits are present. It loads only its root-owned fixed guard, rejects unknown/duplicate JSON fields, non-integer values and bodies over 1024 characters, and verifies the actual sandbox group before child staging/execution. The demoted guard repeats the check before Landlock/user exec, removes the internal group variable, then retains all existing socket/IPC/file/device controls. The child receives no cgroup path or setup instruction.

## Actual enforcement

The operator must configure a private cgroup2 domain before this worker starts. Ion reads the actual kernel controls; it does not create/remount a hierarchy, patch a VM after user code starts, or configure an unrelated host/container.

Both checks require membership `0::/`, real cgroup2 filesystem, root-owned non-writable controls, the checking process in that group, finite `cpu.max`, `memory.max`, `pids.max`, and `memory.swap.max=0`. Tighter actual limits are accepted. Fractional CPU comparisons use integer cross-multiplication. Missing/unlimited/relaxed/unsupported controls refuse execution; no successful receipt or new launch follows from failure.

The kernel group covers child processes and threads. The unprivileged strict child cannot open the protected kernel controls, migrate, mount another hierarchy or change limits. The reviewed privileged runtime must keep caps unchanged during execution. [Kernel cgroup2 design, membership and controller contract](https://docs.kernel.org/admin-guide/cgroup-v2.html).

This controls CPU bandwidth and cgroup-charged RAM/tasks. It does not establish accelerator VRAM allocation, all driver memory accounting, HugeTLB accounting, physical host capacity, per-project physical quota, aggregate persistent storage or a final provider bill. No GPU per-process address-space rlimit is added: virtual mappings and charged RAM are different quantities. These broader gates remain open.

OOM can terminate a command or its trusted supervisor. A supervisor loss remains an uncertain original job; it never authorizes a repeated create, local fallback or financial/storage release. Actual failed command results remain authentic under the existing collector contract.

## Verified evidence

The complete installed strict-worker suite passes **14 cases** on actual Linux, including seven added GPU-group cases:

- Real finite group settings, nonroot command and removed private environment.
- Refusal of tighter-than-runtime owner caps, malformed/oversized/duplicate approval before the user marker.
- Independent demoted-guard refusal of actual cap mismatch, before user code.
- Refusal of GPU group approval without the strict runtime profile.
- Forks hitting the actual aggregate task cap while strict file restrictions remain.
- Busy child causing actual `cpu.stat:nr_throttled` growth.
- Child allocations causing actual `memory.events:oom_kill` growth.

The other seven installed cases preserve approved private I/O, literal bracket-named inputs/outputs, original failed-job logs/files, private mounted collector identity, duplicate refusal and changed/missing profile/device denial.

The six existing ordinary resource-group Linux tests also pass, including actual throttling, OOM accounting and forks. Nine existing installed socket/identity/receipt guard cases pass. Seven portable worker and seven pure group-policy cases pass on Windows; the latter include three new GPU cases proving enlarged GPU bounds do not change ordinary command limits, duplicate parsing refusal and actual-control comparisons.

Native fixtures use the cached CPU image `ion-job-worker-group-test:20261012`, x86_64 kernel `6.18.40.1-microsoft-standard-WSL2`, Landlock ABI7. The strict suite is given only 0.5 CPU, 128 MiB RAM, no swap and 32 tasks; root-owned current files are copied into private disposable containers. Network is disabled, no image is pulled, no credentials/host mounts/GPU are present. Outer seccomp is disabled to expose inner restrictions; no-new-privileges is enabled. The 7 existing fixture cases mask `/dev/core` only in the disposable test setup. Containers are removed afterward. Actual CUDA compatibility is not inferred from these CPU tests.

**109 distinct checks pass in this current-source increment:** 66 freshly compiled native Rust GPU tests (61 matching `team_gpu_`, 2 runtime, 2 new group and 1 global-usage regression),14 installed strict worker,6 ordinary group,9 ordinary guard and14 portable worker/policy tests. The selectors are deduplicated by full test name. The earlier strict-worker checklist had counted its two runtime tests twice; that historical total is corrected to 162.

Locked desktop/daemon/core worker binary compilation and Rust formatting checks pass. Protocol46/client shapes are unchanged, so no new wire binding or mobile dependency was introduced. Existing Rust warnings remain. No visual test, watcher restart, real CUDA/provider allocation or speech deployment occurred.

Private ignored evidence:

- `.waku-cache/gpu-resource-installed-linux.log`
- `.waku-cache/gpu-resource-original-group-linux.log`
- `.waku-cache/gpu-resource-core-build.log`
- `.waku-cache/gpu-resource-gpu_group.log`
- `.waku-cache/gpu-resource-gpu_runtime.log`
- `.waku-cache/gpu-resource-team_gpu.log`
- `.waku-cache/gpu-resource-extra-regressions.log`
- `.waku-cache/gpu-resource-binaries.log`

## Later owner review

- [ ] Build/review a new immutable worker image containing these current files and the accepted fixed runtime profile.
- [ ] Configure actual private cgroup CPU/RAM/no-swap/task limits before startup; include collector/supervisor overhead and realistic CUDA thread counts.
- [ ] Add original shape-bound `child_group` approval alongside image/profile/network/resource/pricing evidence.
- [ ] Verify group topology, actual process membership and permissions on the exact managed GPU runtime; unsupported runtime stays refused.
- [ ] Run offline CUDA/training with successful output plus real cap/missing-control failure, Stop, descendant cleanup and restart recovery cases.
- [ ] Verify driver/VRAM/HugeTLB/host-memory behavior and complete wider physical storage/egress enforcement separately.
- [ ] Reconcile original provider resources/costs; retain storage reservations until supported cleanup evidence exists.

Related: [strict worker integration](57-strict-gpu-worker-integration.md), [ordinary resource groups](45-sandbox-resource-groups.md), [GPU resource/price contract](RESEARCH-nebius-gpu-launch-policy.md), [remaining priority scope](37-priority-integrations.md). Cloud speech deployment and iPhone 15/Moto G32 native acceptance also remain pending the required infrastructure/builds.
