# Strict GPU worker integration and owner review

Implementation checkpoint: 2026-10-13 local / 2026-10-12 UTC, on `codex/ion-agent-teams`, protocol 46. Independent runtime and file/device branches are merged. This is CPU/Linux proof of the installed execution boundary; a usable CUDA image and real Nebius acceptance remain open.

## Implemented

New GPU preflight and pricing require an owner-approved `resources.child_runtime` alongside the existing immutable image, network, project, resource and pricing approvals:

```json
{
  "child_runtime": {
    "version": 1,
    "profile_sha256": "<64 lowercase hex characters of the canonical validated profile>"
  }
}
```

The launch body carries the identical `ION_JOB_RUNTIME_PROFILE_SHA256`. Missing, malformed, changed or duplicate environment witnesses reject before provider reads/create. The digest identifies the canonical validated contents of the fixed root-owned `/usr/local/libexec/ion-job-runtime.json`. It does not attest an image or provider by itself. Historical omitted approvals retain their original serialized policy/body for recovery, while new preflight requires the approval.

The installed supervisor stages and owns three separate per-job directories: work, home and temporary files. It loads only trusted image helpers, validates the reviewed profile and runtime snapshot before child launch, and sends the original snapshot digest to the demoted fixed guard. The guard repeats the snapshot and verifies equality, closes all inherited descriptors above stderr, installs socket/IPC/mount/device-creation syscall restrictions, and installs Landlock file/device confinement before executing the literal argv. Internal profile/snapshot/collector variables are removed before user code.

Only explicit image trees and exact devices receive approved rights. Writable grants cover the three fresh private roots. The separate collector stays inaccessible to the child. The strict result-volume contract requires an actual unique mount behind root-only `/ion`; a matching path on an ordinary directory or aliased backing is insufficient. Actual nonzero command output remains collectable in its original receipt; duplicate submissions preserve that receipt.

The default CPU Dockerfile includes the helpers but no accepted GPU runtime profile. The exact reviewed CUDA image must embed its real profile. There is no successful strict execution when the profile, kernel feature, library or confinement setup is unavailable. Agent GPU launch remains gated pending actual runtime acceptance.

## Verification

**162 distinct tests pass for this increment**, counted independently of earlier protocol46 totals:

| Suite                                   | Passed | Evidence                                                                                                                                   |
| --------------------------------------- | -----: | ------------------------------------------------------------------------------------------------------------------------------------------ |
| Rust GPU driver/coordinator regressions |     62 | Fresh root core executable; original identity, recovery, pricing and result paths                                                          |
| Rust runtime approvals                  |      2 | Missing/changed policy rejects before provider reads; historical omission preserved                                                        |
| Runtime snapshot                        |     33 | Actual Linux filesystem/device/race/bounds and root/child snapshot equality                                                                |
| Landlock                                |     25 | Actual ABI7 tests, exact ioctl rights, denied outside paths/FD aliases, inheritance                                                        |
| Installed strict worker                 |      7 | Complete guard execution, literal bracket input/output, authentic failed output, mounted collector, refused changed/missing profile/device |
| Installed ordinary guard                |      9 | Existing socket, identity, privilege, descendant and receipt contracts                                                                     |
| Installed ordinary CLI                  |     13 | Existing resources, project archives, logs, receipts and invalid-input refusal                                                             |
| Portable worker/resource policy         |     11 | 7 source worker and 4 policy cases on Windows                                                                                              |

Count correction on 2026-10-13: the earlier broad GPU selector already included the two runtime approval cases. They are now counted once, giving 162 distinct cases for this historical checkpoint.

The optional actual owner-generated Git archive case in the ordinary CLI suite remains skipped. Mocked kernel failure tests are included in the helper suites; the opted-in Linux cases genuinely install and exercise the kernel boundary.

Native fixtures use the cached `ion-job-worker-group-test:20261012` image (`sha256:2200d363e2a9be95997968a5aecdc823a29d57a301b2593b2cecb76677bf4cca`), no image pull, no network, no credentials, no host bind mounts and no GPU. Root-reviewed files are copied into disposable containers. The strict collector test uses an owned 8MiB tmpfs; each container is removed after its run. Kernel: `6.18.40.1-microsoft-standard-WSL2`, x86_64, Landlock ABI7. The outer seccomp filter is disabled in these fixtures so the inner guard supplies the syscall denial. No-new-privileges is enabled. `/dev/core` is removed only in the explicitly owned snapshot fixture; production still rejects that unapproved alias. Production supervisor capabilities and outer runtime filters require separate review.

Integration exposed an incorrect rejection of literal filesystem names containing brackets. The runtime scanner now distinguishes trusted FD-enumerated literal names from owner profile patterns. Both a real image-file/alias test and complete installed `[input].txt` / `result[1].txt` transfer prove the fix. Profile patterns remain refused.

Locked `cargo check -p waku -p waku-core --bins` passes for the desktop/daemon/core worker binaries; the core test executable was freshly compiled before Rust verification. Existing Rust warnings remain. Protocol and TypeScript shapes did not change. No visual test, watcher restart, production deployment or paid allocation was performed.

Private ignored evidence in the implementation checkout:

- `.waku-cache/gpu-runtime-core-build.log`
- `.waku-cache/gpu-runtime-gpu-regressions.log`
- `.waku-cache/gpu-runtime-runtime-approval.log`
- `.waku-cache/gpu-test_runtime_boundary.py.log`
- `.waku-cache/gpu-test_landlock.py.log`
- `.waku-cache/gpu-runtime-installed-linux.log`
- `.waku-cache/gpu-test_linux_guard.py.log`
- `.waku-cache/gpu-test_linux_cli.py.log`
- `.waku-cache/gpu-runtime-final-binaries.log`

## Owner setup and review later

- [ ] Supply a real Nebius target, owner IAM configuration and approved resource/pricing limits.
- [ ] Select a pinned CUDA image and verify its upstream image, driver and library provenance.
- [ ] Review actual mounts, aliases and every exposed character device. Exclude network-capable/RDMA devices and filesystem proxies that can bypass the approved offline workload.
- [ ] Embed the reviewed root-owned runtime profile and record its canonical SHA alongside the immutable image approval. Do not auto-approve an inventory generated from an unknown host.
- [ ] Verify actual provider kernel release, Landlock ABI>=5, trusted seccomp library, outer policy, exact native architecture and supervisor permissions. Actual aarch64 remains untested.
- [ ] Accept offline CUDA allocation, a kernel/training loop and realistic memory/process limits through this installed worker. Exact driver proc/sys metadata needs actual image evidence; documented paths alone do not establish CUDA compatibility.
- [ ] Repeat refusal checks for unapproved device/alias, changed profile/mount/driver, missing Landlock/library and inherited handles on that runtime.
- [ ] Verify private actual provider result mount, failed-job checkpoint readback, descendant cleanup, duplicate/restart recovery and original owner receipts.
- [ ] Complete remaining physical host/VRAM/storage/egress controls and billing finality; this profile does not provide host capacity or final invoice proof.
- [ ] Obtain provider-supported deletion/retention evidence before releasing uploaded-storage reservations. Cancellation, tag deletion or namespace 404 are insufficient.

Shared speech already has actual installed CPU single/concurrent recognition evidence using fixed synthesized English audio. Cloud TLS deployment, human microphone/phone observations and native builds for iPhone 15/Moto G32 remain pending. No Nebius/SSH/HTTPS deployment target has been supplied.

Related: [runtime snapshot](55-gpu-runtime-snapshot.md), [file/device boundary](56-gpu-file-device-confinement.md), [storage cleanup contract](RESEARCH-command-storage-cleanup.md), [actual speech evidence](54-native-speech-recognition.md), [physical-device guide](52-physical-device-review.md), [priority scope](37-priority-integrations.md).
