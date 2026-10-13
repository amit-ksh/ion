# GPU workspace filesystem capacity limits

Checkpoint: 2026-10-13, `codex/ion-agent-teams`, protocol 46. This adds physical allocated-block and inode ceilings for the private GPU workspace. Actual managed GPU/CUDA deployment remains unaccepted.

## Owner approval

The GPU resource policy now binds `child_storage` to the original launch body:

```json
{
  "child_storage": {
    "version": 1,
    "filesystem": "ext4",
    "bytes": 17179869184,
    "inodes": 1048576
  }
}
```

The example is an upper bound for a dedicated 16 GiB disk filesystem, not recommended sizing or a provisioning command. The filesystem's usable capacity can be tighter. The approved byte ceiling must fit the original allocated disk shape. Version1 permits 16 MiB..1 TiB and 128..16,777,216 inodes.

New pricing/preflight require this approval alongside the original profile, group, image, network, resource and monetary approvals. The identical canonical JSON travels as `ION_JOB_GPU_STORAGE_LIMITS`. Missing/changed/oversized/malformed approval, wrong types and duplicate environment entries refuse before provider reads/create. Historical absent fields retain their original serialized policy/body for recovery; they do not authorize a new launch.

The operator must provision a dedicated `/tmp` filesystem before startup and include its actual topology/options in the fixed reviewed image profile. The current worker stages work, HOME and TMPDIR beneath this fixed mount; all three roots are private. No owner/model-selected filesystem path is accepted. This startup verifier reads kernel state and does not mount, resize, format or modify a host disk.

## Verification and enforcement

The root supervisor validates the profile and runtime snapshot, then checks the dedicated filesystem before child execution. The demoted guard repeats the filesystem check and requires the original capacity witness SHA before installing the existing Landlock/syscall boundary and executing user argv. The original storage policy and internal snapshot variables are removed before exec.

The check requires:

- Exactly one accepted mount at `/tmp`, rooted at `/`, with matching filesystem type, writable, `nosuid` and `nodev` options.
- No nested mount under `/tmp` and no second visible mount sharing its backing device identity.
- A real root-owned directory opened with `O_NOFOLLOW`; mountinfo device major/minor matches the pinned descriptor's `st_dev` and actual kernel filesystem magic.
- All three private writable roots on that same filesystem.
- Finite total allocated-block capacity (`f_blocks*f_frsize`) and finite inode capacity (`f_files`) no greater than the approved ceilings; actual kernel readonly/security flags agree.
- Stable mount metadata, filesystem identity and total capacities before/after verification, then identical supervisor/child witness. Free-space counters are deliberately excluded: they are not total capacity or a reservation.

Accepted fixed-capacity filesystem contracts are tmpfs and ext4. The tmpfs contract verifies actual finite size/inode limits; ext4 requires a dedicated bounded disk filesystem. Overlay/root directories, FUSE/network filesystems, ramfs, aliases and nested bypass mounts reject. Actual disk filesystem and managed CUDA acceptance remain separate gates.

XFS is not accepted by this capacity contract: its kernel inode statistic can be computed from allocated inodes plus potential inodes derived from free blocks. Accepting that as a fixed inode limit would be incorrect. XFS/project-quota support needs explicit reviewed quota evidence and remains open. [Linux XFS statfs implementation](https://github.com/torvalds/linux/blob/master/fs/xfs/xfs_super.c).

Landlock prevents writes outside work/home/temp; the mount limits cover all allocated content within those roots, including files not named for export, caches and multiple regular files. Kernel capacity exhaustion causes allocation/create failure. The trusted host/runtime must keep geometry and controls fixed. [Tmpfs size/inode contract](https://docs.kernel.org/filesystems/tmpfs.html), [fstatvfs capacity fields](https://man7.org/linux/man-pages/man2/fstatvfs.2.html).

These limits count allocated filesystem blocks and inodes. They are not a ceiling on sparse files' apparent logical length, a limit on driver/VRAM allocations, or a complete physical host/account quota. Tmpfs consumes RAM and must fit actual cgroup/host sizing; no-swap group approval remains independently required for new GPU launches.

## Evidence

The full installed strict-worker suite passes 19 cases on the actual local Linux kernel, including 5 new storage cases. Native results prove successful approved private I/O and removal of storage environment, refusal of malformed/wrong-filesystem approvals, real block exhaustion across multiple files, real inode exhaustion from empty files, changed original capacity-SHA refusal in the demoted guard, and successful bounded checkpoint output after releasing space. Existing CPU/RAM/task, file/device, authentic failed-result, collector and original-identity checks remain green.

Four additional actual disposable fixtures refuse user execution: 32 MiB backing with 16 MiB approval, 256 inodes with 128 approval, ordinary `/tmp` directory without a private mount, and an unapproved nested tmpfs despite a matching captured profile.

Three portable schema/platform tests and seven existing worker tests pass on Windows. **98 distinct test cases pass for this increment:** 68 freshly compiled Rust GPU approval/launch/recovery/result cases, 19 installed strict-worker cases, 3 portable storage-policy cases, 7 existing worker cases and 1 committed actual refusal case exercised under 4 different native geometries. Repeated selectors/configurations are not added again. Final current-source Rust regressions are rerun after excluding XFS; locked desktop/daemon/core worker binary checks and Rust formatting pass. Protocol 46/client shapes are unchanged and existing Rust warnings remain. Test totals are deduplicated rather than added across overlapping selectors.

The acceptance uses cached CPU image `ion-job-worker-group-test:20261012`, x86_64 WSL2 kernel 6.18.40.1, Landlock ABI7, network=none, no image pull/credentials/host mounts/GPU. The positive workspace has 16 MiB allocated-byte capacity and 128 inodes; the separate collector has its own 8 MiB tmpfs. CPU 0.5/RAM 128 MiB/no-swap/tasks 32 are retained. Current root-owned files are copied into disposable containers; all owned containers are removed after each run. This is a CPU tmpfs/kernel proof, not ext4/CUDA/provider acceptance.

The refusal recipe runs the committed `test_linux_storage_refusal.py` in four explicitly prepared geometries, selected by `ION_JOB_STORAGE_REFUSAL_SCENARIO`. Positive enforcement runs the committed `test_linux_runtime.py`. These require disposable native fixtures and must not be run as host cleanup/provisioning commands.

Private ignored evidence: `.waku-cache/gpu-storage-installed-linux.log`, `gpu-storage-refusal-linux.log`, `gpu-storage-core-build.log`, `gpu-storage-final-schema.log`, the four `gpu-storage-final-*` Rust regression logs and `gpu-storage-final-binaries.log`.

## Later owner review

- [ ] Build/review a pinned CUDA worker image with current guard/runtime and accepted filesystem/profile/group approvals.
- [ ] Provision a dedicated bounded tmpfs or ext4 workspace before startup; review real disk and inode geometry, private backing and mount flags. Do not share a mutable filesystem between independent jobs.
- [ ] For large training/data workloads, use an appropriately sized dedicated disk filesystem and complete real ext4/CUDA acceptance. Local evidence currently covers tmpfs only.
- [ ] Repeat success, block/inode exhaustion, wrong ceiling, changed topology, missing/nested/shared backing and failed-output/restart cases on the actual managed runtime.
- [ ] Review charged-memory overhead and driver/VRAM/HugeTLB behavior, host quotas and collector retention independently.
- [ ] Obtain supported provider cleanup proof before treating retained uploads/results as cleaned or releasing any related existing hold. Command cancellation and GPU workspace limits do not establish remote upload deletion.

The collector and retained cloud archives/results are outside this workspace budget. Existing transfer/receipt bounds, monetary reservations and ordinary command upload holds remain. GPU collector retention/admission is separate unfinished work. Wider host/VRAM/storage/egress, billing/credentials and live speech/device acceptance remain open.

Related: [strict execution](57-strict-gpu-worker-integration.md), [CPU/RAM/task caps](58-gpu-sandbox-resource-limits.md), [remote upload cleanup contract](RESEARCH-command-storage-cleanup.md), [remaining priorities](37-priority-integrations.md).
