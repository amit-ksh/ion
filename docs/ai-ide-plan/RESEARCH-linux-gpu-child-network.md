# Linux GPU child network boundary

Research checked 2026-10-07. Scope: credential-free, offline, single-process GPU training in a reviewed single-host image. This is a recommendation, not a runtime acceptance result. No cloud calls, credential inspection, installs or implementation were performed.

## Launch path at the start of this investigation

`packages/job-worker/ion_job_worker.py` initially ran a root supervisor, sets no_new_privs/subreaper/non-dumpable state, launches the literal user argv with UID/GID demotion and no supplementary groups, captures pipes, kills descendants, then writes the receipt. It had no child network syscall filter. The initial `Dockerfile` removed setuid/setgid bits but supplied no libseccomp dependency or launcher. `crates/waku-core/src/driver/team_gpu.rs` fixes the worker entrypoint and credential-free environment and sets publicIp=false. These are local source observations.

Nebius's current JobSpec has subnet/public-IP/ports/volumes, but no capability, seccomp or security-group attachment field. UID 0 inside the container does not prove CAP_SYS_ADMIN availability. Private addressing still permits internet egress through the default gateway; security groups exclude 169.254.0.0/16. IMDS can return attached service-account tokens. Managed-job metadata reachability and capability sets were not observed. [JobSpec](https://raw.githubusercontent.com/nebius/api/main/nebius/ai/v1/job.proto), [routing](https://docs.nebius.com/vpc/routing/overview), [security groups](https://docs.nebius.com/vpc/security-groups/overview), [IMDS](https://docs.nebius.com/compute/virtual-machines/instance-metadata).

## Smallest suitable design

Use one fixed trusted child launcher, invoked through the existing Popen demotion, which installs a native-ABI libseccomp filter before exec of any user executable. Keep the root supervisor/result collector outside that filter. Installation needs no capability when no_new_privs is set; descendants inherit filters through fork/clone/exec, and adding another filter cannot relax the existing one. Seccomp is a syscall restriction, not a complete sandbox. [Kernel seccomp contract](https://docs.kernel.org/userspace-api/seccomp_filter.html).

A private network namespace would isolate devices, routes and abstract Unix sockets, but unshare(CLONE_NEWNET) requires CAP_SYS_ADMIN in the owning user namespace. No public Nebius JobSpec knob grants it. Creating a user namespace first depends on runtime/kernel policy and changes privilege/device semantics; it is not a dependable default. Network namespaces also leave filesystem Unix sockets and existing descriptors to handle. Use an empty network namespace as additional protection only after proving support, never fall back to unrestricted execution when it fails. [unshare](https://man7.org/linux/man-pages/man2/unshare.2.html), [network namespaces](https://man7.org/linux/man-pages/man7/network_namespaces.7.html).

For the narrowly accepted pinned image/kernel, a default-allow filter with these unconditional denials is the least disruptive initial policy:

- All socket domains: socket, socketpair, connect, bind, listen, accept/accept4, sendto/sendmsg/sendmmsg, recvfrom/recvmsg/recvmmsg, shutdown; optionally socket-option/name calls too. No AF_UNIX exception.
- Alternate submission: io_uring_setup, io_uring_enter, io_uring_register. Kernel io_uring has SOCKET/CONNECT/SEND/RECV operations; blocking ordinary socket syscalls alone does not cover this path. [Kernel operation table](https://raw.githubusercontent.com/torvalds/linux/master/io_uring/opdef.c), [io_uring setup](https://man7.org/linux/man-pages/man2/io_uring_setup.2.html).
- External-process/descriptor access: pidfd_getfd, ptrace, process_vm_readv/process_vm_writev; also bpf, setns and unshare. pidfd_getfd duplicates another process's open descriptor. [pidfd_getfd](https://man7.org/linux/man-pages/man2/pidfd_getfd.2.html).
- Preserve the absence of namespace capabilities: reject CLONE_NEW* bits in native clone flags; reject clone3 because its flags sit behind a pointer that classic seccomp cannot inspect. ENOSYS for clone3 permits runtimes that implement fallback to legacy clone; validate the pinned libc/framework. Otherwise explicitly require the outer runtime to forbid namespace creation. A new user namespace grants namespace capabilities even to a previously demoted task; this alone does not remove the inherited filter. [clone](https://man7.org/linux/man-pages/man2/clone.2.html), [user namespaces](https://man7.org/linux/man-pages/man7/user_namespaces.7.html).

Default allow is an explicit compatibility tradeoff. It must be tied to a reviewed kernel/syscall surface and reaccepted on changes; a syscall allowlist is stronger against future alternate mechanisms. Linux recommends allowlists for precisely this reason. [seccomp caveats](https://man7.org/linux/man-pages/man2/seccomp.2.html).

## Launcher ordering and libseccomp details

Launch an absolute root-owned Python interpreter with `-I -S` and a root-owned fixed script; preserve literal user argv only as data until the final exec. The launcher, interpreter, standard library, ELF loader, loaded native libraries and their parent directories must be unwritable by the child. Sanitize the environment before launching it: no LD_* interposition or PYTHON* injection. -I excludes unsafe import paths/user site and ignores PYTHON*; -S prevents site/.pth startup. User ELF/script loaders execute after the filter is loaded and inherit it. Avoid a ctypes preexec_fn; Python warns that callbacks before exec can deadlock in threaded parents. [Python startup](https://docs.python.org/3/using/cmdline.html), [subprocess](https://docs.python.org/3/library/subprocess.html), [ELF exec](https://man7.org/linux/man-pages/man2/execve.2.html).

Use explicit close_fds=True, pass_fds=(), stdin=/dev/null and stdout/stderr pipes, then close any launcher-created descriptors above 2 before user exec. Retaining a connected socket/ring would let read/write/mmap bypass the socket-denial premise. Do not pass result-volume, metadata, credential, network or IPC handles. AF_UNIX supports descriptor transfer through SCM_RIGHTS; denying its socket creation/connection and sendmsg/recvmsg closes that path. [Python FD contract](https://docs.python.org/3/library/subprocess.html), [Unix descriptor passing](https://man7.org/linux/man-pages/man7/unix.7.html).

Require fixed nonzero UID/GID and empty supplementary groups; verify effective/permitted/ambient capabilities are empty. Keep no_new_privs set, since it prevents setuid/file-capability gains during exec but does not remove existing capabilities or prevent every non-exec privilege change. [capabilities](https://man7.org/linux/man-pages/man7/capabilities.7.html), [kernel no_new_privs](https://docs.kernel.org/userspace-api/no_new_privs.html).

seccomp_init adds only the native architecture. Do not add x86/x32 compatibility architectures; explicitly set ACT_BADARCH to KILL_PROCESS. Current libseccomp generation rejects x32 calls in a native x86_64 filter. Require Linux >=5.4, or separately reject legacy syscall numbers 512–547, because older kernels had confused x32 handling. Resolve names with seccomp_syscall_resolve_name and use rule_add's architecture rewriting rather than handwritten syscall numbers. A negative pseudo syscall ID can be valid; only __NR_SCMP_ERROR means resolution error. Check every allocation, attr, rule and load result and fail before user exec on any error; set ctypes signatures exactly. [architectures](https://raw.githubusercontent.com/seccomp/libseccomp/main/doc/man/man3/seccomp_arch_add.3), [attributes](https://raw.githubusercontent.com/seccomp/libseccomp/main/doc/man/man3/seccomp_attr_set.3), [x32 generator](https://raw.githubusercontent.com/seccomp/libseccomp/main/src/gen_bpf.c), [name resolution](https://raw.githubusercontent.com/seccomp/libseccomp/main/doc/man/man3/seccomp_syscall_resolve_name.3), [rule rewriting](https://raw.githubusercontent.com/seccomp/libseccomp/main/doc/man/man3/seccomp_rule_add.3), [kernel ABI caveats](https://man7.org/linux/man-pages/man2/seccomp.2.html).

## Acceptance limits

This leaves ordinary device open/ioctl/mmap, memory, files, pipes, threads and exec available for local CUDA. Compatibility is an inference, requiring actual training and artifact collection in each pinned GPU image/runtime. No NCCL/distributed rendezvous, MPS/local service, multiprocessing socket transport, package download or telemetry compatibility is promised; AF_UNIX and io_uring remain denied.

Crucially, reject a runtime exposing child-accessible RDMA/network devices or handles: InfiniBand userspace verbs use device writes and mapped hardware registers, including a fast path without a syscall. A socket filter cannot stop that. Limit devices to the reviewed GPU/basic device set; exclude RDMA, NIC/VFIO and equivalent network-capable access. [Kernel RDMA interface](https://docs.kernel.org/infiniband/user_verbs.html).

Also exclude accessible external FIFOs/shared memory/daemon command files and network/FUSE mounts capable of acting as proxies. Ordinary allowed file I/O can ask an unfiltered daemon to do network work. Keep the result mount behind root-only /ion, and stage any future datasets locally before launching the child. A seccomp filter cannot inspect pathname contents or enforce information flow through allowed IPC/files. [Kernel filter limits](https://docs.kernel.org/userspace-api/seccomp_filter.html), [FUSE daemon model](https://docs.kernel.org/filesystems/fuse/fuse.html).

The supervisor must be a bounded output sink/collector, never an RPC proxy for child-selected URLs or commands; it may upload only its authorized receipt/artifacts after all descendants stop. Acceptance should prove network and Unix-socket attempts fail for user code and fork/exec descendants; raw x32/i386, io_uring and descriptor theft fail; no dangerous inherited handles/accessible devices/proxies exist; local CUDA training and trusted result persistence still succeed. Until those checks pass on the exact provider image/runtime, the mechanism is designed but remote network-denial acceptance remains unproven.


## Fixed ctypes API and dependency floor

For this policy, require a reviewed libseccomp runtime >=2.5.0 (use a maintained patched package in the pinned image), seccomp_api_get() >=3, native x86_64/aarch64, Linux >=5.4, and successful filter loading. Version 2.5.0's syscall table contains clone3, all three io_uring calls and pidfd_getfd. The minimum describes API/name coverage, not a recommendation to deploy the unpatched 2020 release. [v2.5.0 syscall table](https://raw.githubusercontent.com/seccomp/libseccomp/v2.5.0/src/syscalls.csv).

Header identifiers: SCMP_FLTATR_ACT_BADARCH=2; CTL_NNP=3; CTL_TSYNC=4; SCMP_CMP_MASKED_EQ=7. Actions: ALLOW=0x7fff0000; KILL_PROCESS=0x80000000; ERRNO(e)=0x00050000 | (e & 0xffff). seccomp_api_get returns unsigned int and API level 3 includes KILL_PROCESS support. scmp_arg_cmp fields are unsigned int arg, enum/int op, uint64 datum_a, uint64 datum_b: ctypes c_uint/c_int/c_uint64/c_uint64, size 24 on the accepted native 64-bit ABIs. [Current public header](https://raw.githubusercontent.com/seccomp/libseccomp/main/include/seccomp.h.in).

Prefer the non-variadic seccomp_rule_add_array: arguments are void* context, uint32 action, int syscall, unsigned int argument count, pointer to scmp_arg_cmp; return int. For a forbidden clone bit, masked equality uses datum_a=bit and datum_b=bit on syscall argument 0; add a separate rule per bit. Combining all forbidden bits into one equality would reject only when all those bits are set together. Initialize through seccomp_init(uint32)->void*, set attrs with seccomp_attr_set(void*, int, uint32)->int, resolve names with seccomp_syscall_resolve_name(char*)->int, load through seccomp_load(void*)->int, release through seccomp_release(void*)->void. NULL init or any nonzero attr/rule/load result aborts; negative returns carry library error codes rather than relying on ctypes errno. [Rule API](https://raw.githubusercontent.com/seccomp/libseccomp/main/doc/man/man3/seccomp_rule_add.3), [load result contract](https://raw.githubusercontent.com/seccomp/libseccomp/main/doc/man/man3/seccomp_load.3).

ENETDOWN for the denied network calls and EPERM for the process/namespace/BPF calls provide explicit failure without relaxing the policy. Keep BADARCH as KILL_PROCESS and clone3 as ENOSYS. TSYNC is unnecessary in the fixed single-threaded pre-exec launcher; if it ever starts threads before loading the filter, require successful TSYNC instead. [Filter attributes](https://raw.githubusercontent.com/seccomp/libseccomp/main/doc/man/man3/seccomp_attr_set.3).

## Subsequent implementation and local acceptance (2026-10-07)

After the source investigation, the parent implementation added the fixed
`ion_job_guard.py`, pinned libseccomp and exact accepted kernel releases in the
worker image, routed isolated Linux children through it after demotion and
required image-bound socket approval before new GPU preflight. The internal
version 1 command/result transport is unchanged; historical GPU policies omit
the optional approval field so exact replay/recovery retains old signatures.

Nine guard CLI cases and six worker cases passed on the local Docker Linux x86_64
kernel `6.18.40.1-microsoft-standard-WSL2`, with a pinned Python image and signed
`libseccomp2=2.6.0-2`. The guard cases also passed with outer Docker seccomp disabled.
They prove the actual fixed child filter and private collector ordering on that
kernel, not a Nebius/GPU acceptance result. Exact kernel strings narrow accepted
runtime changes but are not cryptographic kernel binary attestation. The device,
IPC/file-proxy, network-filesystem and CUDA compatibility limits above remain.
No cloud credentials or provider resource were used for these tests.
