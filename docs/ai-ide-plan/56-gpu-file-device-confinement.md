# GPU file and device confinement

Implemented **2026-10-12** for the separate strict offline GPU worker profile.
The helper is `packages/job-worker/ion_job_landlock.py`; tests are
`packages/job-worker/test_landlock.py`. Historical ordinary jobs do not opt into
this contract.

## Contract

`install(read_paths, writable_roots, devices)` returns the **queried kernel ABI**
only after every rule and the restrictive domain have loaded. It requires Linux
native little-endian x86_64 or aarch64, 64-bit pointers/longs, Landlock ABI **5 or
newer**, a demoted positive UID/GID, and no kernel fallback.

Inputs come from the trusted runtime verifier:

- Up to 128 canonical absolute read/execute roots. Root-owned regular files and
  directories must not be writable by group or others; their ancestors receive
  the same ownership/mode check. Protected collector, process/kernel, runtime,
  global scratch and device trees cannot receive blanket grants.
- Exactly three disjoint child-owned directories, mode 0700: fresh work, home
  and private temporary storage. Writable roots cannot cover protected or image
  trees. The caller establishes freshness and private mount origins.
- Up to 64 exact character devices with
  `path/type/major/minor/read/write/ioctl`. Integer identities and strict Boolean
  rights are required; reopened object type, root ownership and actual
  `st_rdev` are rechecked. No wildcard device grant exists.
- Paths are bounded to 4096 UTF-8 bytes and 64 components. Dot, parent, duplicate,
  control-character and trailing-slash forms are rejected. All grant roles must
  be disjoint: additive ancestor rules cannot broaden exact device rights.

Each component is pinned through an `O_PATH|O_NOFOLLOW|O_CLOEXEC` directory FD.
Root symlinks are refused. Setup descriptors close on success and failure.

The fixed rights cover all filesystem operations through ABI 5, including
device ioctl. Read grants permit read/execute; writable roots additionally permit
regular-file/directory creation, deletion, refer and truncation. Device, FIFO,
socket and symlink creation remain denied, as do device ioctls unless the exact
device grants that right. The ioctl right covers a driver class of operations,
not individually reviewed CUDA ioctl numbers; some common kernel ioctls are
outside that right. [Linux Landlock UAPI](https://raw.githubusercontent.com/torvalds/linux/v6.12/include/uapi/linux/landlock.h).

The helper uses native syscall identities 444/445/446, with the packed 12-byte
path-rule structure and an 8-byte filesystem ruleset prefix. Sources:
[x86_64 table](https://raw.githubusercontent.com/torvalds/linux/v6.12/arch/x86/entry/syscalls/syscall_64.tbl),
[common table used by native arm64](https://raw.githubusercontent.com/torvalds/linux/v6.12/scripts/syscall.tbl).


## Explicit kernel metadata reads

The pure `read_path()` contract allows owner-reviewed static metadata without a
blanket `/proc` or `/sys` grant. The runtime profile parser must use the same accepted
categories and reject unsupported roots before startup.

Allowed categories are exact static proc files (`cpuinfo`, `meminfo`, `devices`,
`version`, `uptime`, `stat`); the named NVIDIA proc driver trees; CPU/NUMA and
named NVIDIA module metadata trees in sysfs; exact `/sys/devices/soc0/family`;
and named information leaves under canonical PCI device paths. PCI BAR/resource
and config files are excluded. No numeric PID, `self`, `thread-self`, `fd`,
`fdinfo`, `map_files` or `mem` proc hierarchy is allowed. Kernel metadata grants
receive read permissions with execution removed.

NVIDIA documents runtime driver/GPU information below `/proc/driver/nvidia`.
Its current kernel procfs implementation exposes version, parameter and GPU
information reads. This supports an explicit reviewed metadata exception.
[NVIDIA proc interface](https://http.download.nvidia.com/XFree86/Linux-x86_64/550.78/README/procinterface.html),
[current NVIDIA procfs source](https://raw.githubusercontent.com/NVIDIA/open-gpu-kernel-modules/main/kernel-open/nvidia/nv-procfs.c).

NVIDIA's modprobe helper reads driver parameters and `/proc/devices`, checks
named module `initstate` metadata, and reads `/sys/devices/soc0/family`. Module
loading, device creation and sysfs writes remain unavailable to the child.
[NVIDIA modprobe source](https://raw.githubusercontent.com/NVIDIA/nvidia-modprobe/main/modprobe-utils/nvidia-modprobe-utils.c).

These sources establish real driver metadata uses. Exact closed CUDA library
read requirements still require a reviewed GPU image and real kernel/training
execution; the category policy is an Ion implementation choice.

A native test reads only explicitly granted `/proc/cpuinfo` and
`/sys/devices/system/cpu/online`, while unapproved metadata and process views
remain denied. Docker's sysfs mount is itself readonly, so that write assertion
accepts the VFS `EROFS` result and does not attribute it solely to Landlock.

## Required launcher ordering

1. Verify the root-owned profile, original approval hash, mounts, exact devices,
   private scratch origins and collector alias exclusion.
2. Demote and repeat the compact runtime witness.
3. Load the strict socket/IPC/mount/escape seccomp rules.
4. Close every inherited FD above 2; verify the standard descriptor contract.
5. Install Landlock and close its setup FDs; then exec user code.

Landlock preserves important rights associated with previously opened FDs.
The test intentionally demonstrates that a prior readable collector FD remains
readable after installation. Descriptor closure is therefore a required guard
premise. Restrictions inherit across descendants; already-running sibling
threads require separate treatment, which is why the fixed launcher remains
single-threaded.
[Kernel contract and FD/inheritance caveats](https://docs.kernel.org/userspace-api/landlock.html).

## Verification evidence

Windows pure/failure suite: **17 passed**, with eight Linux tests explicitly
skipped. Real disposable Linux suite: **25 passed**, no skips.

The real run used cached `python:3.12-slim` image
`sha256:05cda9777409a9c3ffddd94a4c476b79f0769a0b4857f0c7ed9226b6800b0d6f`,
kernel `6.18.40.1-microsoft-standard-WSL2`, native x86_64 and actually queried
Landlock ABI **7**. Reviewed files were copied into the disposable container;
there was no host filesystem bind. Networking was disabled and the outer
seccomp policy was disabled to isolate the helper's enforcement.

Real tests cover:

- Permitted regular reads/writes, truncation, private hardlinks/renames and
  approved executable startup.
- Collector, unapproved image file and global scratch denial; forbidden special
  object creation.
- Device ioctl denial without approval and actual driver reachability with the
  exact ioctl grant.
- Thread, fork and exec inheritance.
- Execute-by-FD permission checks, including denial for a preopened executable
  outside the approved tree.
- Prior-FD limitations and required guard closure.
- Rejected root and ancestor symlinks without leaking setup descriptors.
- Explicit proc/sys metadata read grants with unapproved views still denied.

Latest evidence log: `.waku-cache/gpu-landlock-metadata-linux.log`.
Initial evidence log: `.waku-cache/gpu-landlock-linux.log`. Pure tests additionally
exercise old/disabled/missing ABI, exact UAPI layouts, rule/NNP/restrict failure,
bounds, owner/mode/device mismatch, overlap rejection and descriptor cleanup.

To run pure tests:

```text
python -m unittest discover -s packages/job-worker -p test_landlock.py -v
```

The eight native tests require `ION_TEST_LANDLOCK=1` in a disposable Linux image
with actual ABI >=5 and root-owned reviewed test files. An unsupported opted-in
runtime fails the suite; it is not silently skipped.

## Remaining acceptance

- The fixed installed worker must call this helper in the required order and
  emit its trusted original-request receipt; helper tests alone do not prove
  that integration.
- Actual aarch64 execution remains untested.
- A reviewed CUDA image, exact driver/device/mount topology and local CUDA
  kernel/training test remain unaccepted. This Python image supplies no GPU.
- Bind/overlay behavior, hostile aliases and the actual managed-job runtime need
  acceptance alongside the snapshot verifier. Landlock does not establish mount
  origin or isolate a privileged host that changes the runtime.
- No Nebius allocation, cloud deployment, billing settlement or phone review
  occurred during these checks. GPU agent submission remains disabled.

Related: [offline runtime research](RESEARCH-gpu-runtime-boundary.md).
