# Offline GPU runtime boundary

Research checked **2026-10-12**, against checkout **e8adb14**. Scope: a reviewed,
credential-free Linux image executing an offline CUDA child on one host. This
note proposes the next implementation; it is **not** managed-job, GPU or CUDA
acceptance. Research used source reads and public primary sources only. No Cargo,
package install, credential read, cloud allocation or phone interaction occurred.

## 1. What the current source proves

The following are local source observations, rather than provider guarantees.

| Existing seam                                                                                                                                                  | Current behavior                                                                                                                                                                                            | Remaining gap                                                                                                                                 |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| [Fixed guard](../../packages/job-worker/ion_job_guard.py), `prepare`                                                                                           | Checks exact approved kernel, native ABI, child UID/GID, empty permitted/effective/ambient capabilities, no-new-privileges and standard descriptors; loads socket/escape/io_uring restrictions before exec. | No mount inventory, device inventory or file access confinement. System V IPC and POSIX queues are not denied.                                |
| [Worker](../../packages/job-worker/ion_job_worker.py), `run`                                                                                                   | Stages bounded files into a fresh workspace; demotes through Popen with closed descriptors, sanitized environment and the fixed guard.                                                                      | Child can still open ordinary globally accessible paths and devices. Setting HOME/TMPDIR does not restrict path access.                       |
| Worker `secure_result_parent` and `write_result`                                                                                                               | Makes /ion root-owned 0700 before GPU result mode; writes one exclusive regular receipt after descendant termination.                                                                                       | No check for alternative bind mounts exposing the same backing result filesystem. FUSE behavior is not established by a directory mode alone. |
| [Dockerfile](../../packages/job-worker/Dockerfile), [manifest](../../packages/job-worker/job-host.json)                                                        | Pins reviewed base/libseccomp/kernel inputs and removes setuid/setgid bits; advertises socket, rlimit, project-transfer and optional cgroup mechanisms.                                                     | This Python base is not an accepted CUDA image. It declares no GPU runtime profile, device allowlist or mount topology.                       |
| [GPU launch body](../../crates/waku-core/src/driver/team_gpu.rs)                                                                                               | Pins image digest, platform/preset/subnet/disk/shm, fixed entrypoint, nonpublic address, zero restarts and three fixed worker environment fields; optionally mounts results.                                | No runtime-boundary approval or child runtime receipt. Nonpublic address is not offline execution.                                            |
| [GPU resource approval](../../crates/waku-core/src/teams/gpu_jobs/resource_policy.rs) and [preflight](../../crates/waku-core/src/driver/team_gpu/preflight.rs) | Requires image-bound seccomp socket approval before new readiness reads; resolves exact resource quantities and maintains aggregate reservations.                                                           | `child_network=seccompSocketDenyV1` is approval of one mechanism, not device, filesystem or IPC isolation.                                    |

Historical policies must retain exact omitted fields, signatures and recovery.
Do not silently reinterpret the current socket marker as a stronger guarantee.

## 2. Why another runtime check is necessary

RDMA userspace verbs use /dev/infiniband/uverbsN device writes, and their fast
path can operate through mapped hardware registers without a syscall. A socket
filter consequently cannot prevent every network-capable device operation.
**Recommendation:** accept an exact reviewed compute/basic device set, rather
than only rejecting filenames containing “infiniband.”
[Linux RDMA interface](https://docs.kernel.org/infiniband/user_verbs.html).

FUSE forwards filesystem operations to a userspace daemon. Default FUSE
permission behavior can be delegated to that filesystem; allow_other also
changes who may access it. A read-only network mount still performs external
work when read. **Recommendation:** exclude child-accessible network/FUSE mounts
and filesystem-backed service channels; the collector-only result mount must
remain unreachable through every alias.
[Linux FUSE model and permissions](https://docs.kernel.org/filesystems/fuse/fuse.html).

IPC namespaces isolate System V objects and POSIX message queues. Members of the
same namespace can see the same objects. An IPC namespace inode identifies the
current namespace; without a trusted comparison, that number does not prove it
is private to this job. **Recommendation:** deny these IPC syscall families in
the narrow offline child profile instead of trusting an empty startup inventory.
[Linux IPC namespace contract](https://man7.org/linux/man-pages/man7/ipc_namespaces.7.html).

Cgroup2 device access is enforced using attached BPF programs, with no
devices.allow/devices.deny files. The existing CPU/memory/pids group checks do
not prove a device allowlist, and reading cgroup.controllers cannot supply that
proof. Do not invent a cgroup2 device-file verifier.
[Linux cgroup2 device controller](https://docs.kernel.org/admin-guide/cgroup-v2.html#device-controller).

NVIDIA's container toolkit separately selects visible GPUs and mounted driver
capabilities; compute supports CUDA, utility supports NVML/nvidia-smi, and other
features expose additional interfaces. Runtime variables are configuration, not
evidence that the selected devices are the only ones actually accessible.
[Official NVIDIA container configuration](https://docs.nvidia.com/datacenter/cloud-native/container-toolkit/latest/docker-specialized.html).

Nebius's inspected JobSpec supports platform/preset, image, subnet, disk, shm,
files and volumes, but supplies no explicit seccomp, read-only root, device
allowlist, namespace or Landlock switch. Its volume schema does not establish the
actual Linux filesystem implementation. Keep enforcement inside the reviewed
worker or a separately accepted runtime; do not add guessed provider fields.
[Current native JobSpec](https://raw.githubusercontent.com/nebius/api/main/nebius/ai/v1/job.proto).

## 3. Manageable first implementation: validate a strict runtime profile

This section is a **proposed Ion-owned contract**, not an existing Nebius API.

Add a separate optional owner approval, for example
`child_runtime=offlineCudaRuntimeV1`, scoped to the same image digest and original
launch witness. Keep agent submission disabled. A new strict launch requires this
approval as well as the existing socket approval. Historical omission remains
omission; it cannot authorize a new strict launch.

Embed a small, root-owned, non-child-writable profile such as
`/usr/local/libexec/ion-job-runtime.json` in the reviewed GPU image. Suggested
version-1 data:

- Exact contract/version, accepted kernel and reviewed runtime/driver profile.
- Explicit allowed character-device paths, required type, exact accepted
  major/minor identities and allowed read/write/ioctl purpose; no generic
  “/dev/nvidia*” wildcard and no model-supplied device path.
- Exact accepted mount roles/topology: image filesystem, approved driver files,
  process/kernel views, dedicated scratch and collector-only results.
- Fixed IPC-denial mechanism and whether the stronger file-access mechanism
  below is mandatory.
- Separate bounds for policy bytes, mount records, path length, device records,
  traversal depth and total inspected entries. Bounds must fail, not truncate
  silently.

Exact device numbers can vary by runtime/driver, particularly dynamically
registered devices. Reaccept changed identities; do not synthesize them from
assumed NVIDIA major numbers. No device numbers were observed in this research.

### A. Mount topology

Read `/proc/self/mountinfo` using a bounded FD read. A suitable initial parser
budget is 256 KiB, 512 records, 4 KiB per line and 4 KiB decoded path. Reject
duplicates, ambiguous stacked mountpoints, malformed IDs/options, invalid escapes,
unsupported filesystem types, extra child-visible mounts and propagated mount
trees outside the accepted profile. Parse per-mount and per-superblock options
separately.

The kernel exposes mount IDs, parents, filesystem major/minor, root, mountpoint,
propagation fields and filesystem type in mountinfo. Parent IDs outside the
visible root can legitimately be missing. Mountinfo's major/minor is
`st_dev`, rather than a character device's `st_rdev`.
[Linux mountinfo format](https://man7.org/linux/man-pages/man5/proc_pid_mountinfo.5.html).

Comparison should match approved topology and filesystem roles, not unstable
mount IDs. Preserve IDs in the bounded local witness to check changes between
supervisor and guard. Refuse unknown propagation tags for this strict profile,
while recognizing that this intentionally differs from a general-purpose parser.

Allow FUSE/network backing only for the trusted result role behind /ion. Reject
any second mount of that backing filesystem that could expose its content outside
the private collector subtree. Check containment by parsed path components,
never a string prefix such as /ion/results-other.

A readonly bind of a host directory is still a bind of host data; readonly is not
an origin/privacy proof. Image provenance plus the accepted runtime configuration
must establish which mounts contain reviewed code and which are local scratch.
An unexpected mount fails even when its filesystem type looks local.

### B. Devices and special filesystem objects

For each approved device, resolve through directory FDs without following
unapproved symlinks, verify type/owner/mode and compare `st_rdev` major/minor.
Device identity is carried by st_rdev; names alone do not identify the driver.
[Linux stat device identities](https://man7.org/linux/man-pages/man2/stat.2.html).

Require every child-accessible character/block device to belong to the profile.
Inspect approved read/execute trees as well as /dev: an alias under /usr or a bind
outside /dev must not evade verification. Reject unexpected block devices,
RDMA/VFIO/TUN/FUSE devices, FIFOs, Unix socket nodes and shared service directories
in child-accessible trees. This is a bounded worker startup operation, never work
reachable from GPUI rendering.

A default initial traversal budget can be 100,000 entries, depth 64 and 4 KiB paths,
with no symlink-following or cross-mount traversal. Overflow or a traversal error
is unsupported runtime, not permission to continue. Approved readonly aliases
of image/driver files need explicit resolution rules; blanket rejection of /lib
symlinks would break normal Linux images.

Use directory-relative O_PATH/O_NOFOLLOW and fstat checks. If openat2 is chosen,
check real syscall support and use the necessary RESOLVE constraints; a kernel
version string alone is insufficient. A fallback must preserve the same path
semantics or refuse.
[Linux openat2 path resolution](https://man7.org/linux/man-pages/man2/openat2.2.html).

Run the final accessibility checks after demotion so UID/GID and empty
supplementary groups match user code. The trusted root supervisor first verifies
root-only collector ancestors; the demoted guard verifies those paths cannot be
opened. Repeat the compact topology/device witness immediately before exec.

This validates one runtime snapshot. It is not continuous enforcement against
a privileged runtime changing mounts/devices after the check. The host/kernel and
the image's trusted supervisor remain part of the trusted computing base.

### C. IPC and namespace operations

Extend only the new strict profile's filter with unconditional denials:

- System V shared-memory, semaphore and message operations: shmget/shmat/shmdt/
  shmctl; semget/semop/semtimedop/semctl; msgget/msgsnd/msgrcv/msgctl.
- POSIX queue operations: mq_open/mq_unlink/mq_timedsend/mq_timedreceive/mq_notify/
  mq_getsetattr.
- Filesystem/namespace creation and reconfiguration entrypoints not already
  covered: mount/umount2/pivot_root, fsopen/fsconfig/fsmount/fspick, open_tree/
  move_mount/mount_setattr, mknod/mknodat. Validate syscall identity availability
  for each accepted ABI/libseccomp build.

Keep ordinary unnamed pipes, private anonymous memory and threads available.
No AF_UNIX, shared external shm, MPS, NCCL rendezvous or multiprocessing socket
transport is promised. Resolve names through libseccomp; missing policy syscall
coverage fails before user code. The library's maintained syscall table includes
the names, but availability must still be checked in the actual pinned package.
[Official libseccomp syscall table](https://raw.githubusercontent.com/seccomp/libseccomp/main/src/syscalls.csv).

## 4. Stronger enforcement: restrict files before exec

The snapshot verifier should not be presented as comprehensive confinement.
A feasible additional mechanism is a fixed native Landlock helper, called from
the existing single-threaded guard before user exec:

- Deny file/device access by default; allow reviewed image/library trees
  read/execute, explicit basic/GPU nodes with their minimal rights, and only the
  fresh work/home/private tmp trees for writing.
- Handle creation, deletion, refer and truncate rights; permit only required
  regular-file/directory operations in fresh writable trees. Do not grant writes
  to /tmp globally, /run, /dev/shm or the collector.
- Require ABI >=5 to handle device ioctl rights; these rights allow device ioctl
  access as a class, not a specific NVIDIA ioctl command allowlist. Query support
  at runtime and
  fail on missing/disabled support or any create/add/restrict error. ABI >=6 can
  additionally constrain signals; keep the existing socket restrictions.
- Close all setup FDs before exec. Do not keep an already-open device, directory
  or IPC descriptor to bypass the new open policy.

Landlock is unprivileged, inherited and monotonic. Device ioctl mediation was
added in ABI 5, but existing open FDs and some file operations remain outside its
coverage; it also has bind/overlay semantics that must be tested. Consequently,
mount validation and inherited-FD closure remain necessary. **These recommendations
need implementation and acceptance; Landlock support on Nebius is unknown.**
[Kernel Landlock contract](https://docs.kernel.org/userspace-api/landlock.html),
[ABI/version and caveats](https://man7.org/linux/man-pages/man7/landlock.7.html).

CUDA compatibility is an inference until real execution. MPS itself uses a control
daemon, server and client runtime, so rejecting shared service channels may
intentionally make MPS unavailable. Keep MPS and distributed operation outside
this first profile rather than relaxing the offline contract.
[NVIDIA MPS architecture](https://docs.nvidia.com/deploy/mps/latest/index.html).

## 5. Exact implementation seams and verification checkpoints

| Change                                                    | Existing seam                                                                               | Required verification                                                                                                                                                                                                           |
| --------------------------------------------------------- | ------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Pure bounded runtime-policy/mount parsers and comparisons | New helper beside ion_job_guard.py                                                          | Byte/record/path bounds, strict keys, exact device IDs, escapes, stacks, missing-root parents, component containment and alias rejection. Windows pure fixtures are useful here only.                                           |
| Trusted runtime profile and image marker                  | Worker Dockerfile + job-host.json                                                           | Root ownership/modes, unchanged guard location, exact policy digest, unsupported/missing profile refuses before exec. GPU dependencies use a separately reviewed digest.                                                        |
| Supervisor/child checks and fresh scratch                 | ion_job_worker.py run; ion_job_guard.py prepare                                             | Real installed Linux CLI creates no user marker on bad mounts/devices/policy; collector remains reachable only to root; internal commands keep literal argv.                                                                    |
| Strict IPC/mount syscall rules                            | Fixed guard install_filter                                                                  | Raw native syscall denial, thread/fork/exec inheritance, no weaker fallback and authentic exit-125 failure receipt. Existing socket tests still pass with outer seccomp disabled.                                               |
| Optional Landlock enforcement                             | Fixed helper invoked by guard                                                               | Missing ABI, disabled LSM, rule failure, read/write/truncate/link/symlink/device/IPC attempts, inherited descriptors and descendant inheritance, with real installed helper.                                                    |
| Owner policy binding                                      | GpuResourcePolicy, require_child_network, verify_preflight and original launch body witness | Missing/changed runtime approval fails before GET/reservation/POST; image/profile digest changes invalidate approval; exact historic omitted-policy replay retains its body.                                                    |
| Runtime witness receipt                                   | Original worker receipt and owner result parser                                             | Versioned bounded extension is emitted only by the trusted supervisor. Bind request/image/policy/snapshot hashes; never accept child stdout as boundary evidence. A witness does not settle billing or prove resource deletion. |

Keep output receipts backward-compatible by using a separately versioned optional
extension only when the original request requires it; do not accept an unsolicited
extension as authorization. The exact wire design is an implementation choice,
and needs the existing durable original-request/recovery tests.

### Acceptance still requiring real Linux GPU/provider execution

1. Inspect the exact runtime device and mount topology, trusted image layers,
   injected driver paths, collector backing and IPC/process environment.
2. Run a minimal CUDA kernel, device-memory allocation, synchronization and a
   small offline training loop after every required guard has loaded. A successful
   nvidia-smi check alone is insufficient.
3. Exercise device aliases, accessible RDMA/VFIO/host nodes, mounted file proxies,
   result-mount aliases, SysV/queue channels and descriptor/namespace escapes.
4. Verify failure leaves user code unexecuted and produces an authentic failed
   original receipt; upload/readback and exclusive receipt behavior still work.
5. Verify command timeout reaps descendants and resource reservations remain
   held until their existing authoritative release condition.
6. Repeat on the actual managed-job image/runtime and architecture after local
   acceptance. No test on this Windows checkout proves this gate.

## Decision

Implement the bounded profile parser and supervisor/guard verifier first, with
an explicit **runtime snapshot validation** name. Add kernel file/device
confinement and strict IPC denial before claiming the stronger offline runtime
boundary. Unsupported runtimes fail closed; neither a JSON marker nor successful
mount/device inspection authorizes GPU agent submission. Preserve the existing
environment choice, accounting/cleanup gates and one-create recovery semantics.

Related prior findings:
[Linux socket boundary](RESEARCH-linux-gpu-child-network.md),
[Nebius launch policy](RESEARCH-nebius-gpu-launch-policy.md).
