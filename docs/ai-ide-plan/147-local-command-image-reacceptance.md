# Current command worker: local image reacceptance

21 October 2026. Partial progress on SBX-02 in plan 141. No Nebius import,
provider request, VM, sandbox, paid workload or physical-device action occurred.
This is ordinary CPU command-worker acceptance, not CUDA/GPU training or a
configured remote harness.

## Image inspection findings

Read-only Docker inspection located the previously documented command image
`sha256:050fbc7a43e32a8b1f550ef1a6520615168fc5f40832057ebd72dac6fd8d6f55`
and harness image
`sha256:d65a15825be163e9b86757090d3ff9d601525c9b5183d9a75dc90a6eb49f4170`.
OCI manifest/index-to-config and compressed-layer digest chains were verified
before reading selected files. Temporary export archives were removed.

The old command worker, guard and manifest differ from current committed source;
its runtime and Landlock helpers are absent. The harness ELF matches its
historical recorded hash, but `/ion/host.json` is absent. Neither old image is
accepted as a ready current managed target. No credential-named entries or
sensitive environment names were found by the narrow metadata scan; that scan
is not a comprehensive secret scan or proof of working harness authentication.

Evidence: `.waku-cache/ion-local-worker-image-observation-20261021.json` and
`.waku-cache/ion-local-worker-image-contents-20261021.json`.

## Current-source candidate and reproduced refusal

Five exact tracked worker/helper/manifest files were exported from commit
`8fb3d9c408a29fad40856595004d9be67290818e` into a minimal allowlisted local build
context, without the checkout, user home, credentials or QA files. The existing
cached command image was used by its immutable digest; no dependency installation
or networked build step was needed. New files are root-owned and 0555, with the
manifest 0444. An installed-file witness verifies hashes and ownership/modes.

The first candidate built as
`sha256:27cd0fb8e551adbdb680178eb3b63289d39765fd46d2f80737036dabfd5c0259`.
Its CLI suite correctly failed closed: eight command cases returned exit 125
before user effects. The inherited exact kernel was
`6.18.40.1-microsoft-standard-WSL2`, but Docker now reports
`6.18.54.1-microsoft-standard-WSL2`. A read-only boundary probe confirmed the
mismatch while trusted libseccomp 2.6.0/API 7 checks passed. The failed result is
preserved, not relabeled as a passed test.

Evidence: `.waku-cache/ion-command-candidate-build-20261021.json`,
`ion-command-candidate-cli-20261021.json`, its `.log`, and
`ion-command-candidate-boundary-20261021.json`.

## Exact-local-kernel candidate

A separate image was built with only the newly observed local kernel in its
root-owned 0444 exact-release policy. The original image and rejected policy
remain preserved. No predicate, syscall filter, UID/GID, resource limit or
unavailable-runtime fallback was weakened.

- Local image:
  `sha256:4eccf2796b95ac2a5228e2bcaa62e632fac230dae696bfee72269012f0d0dd07`.
- Local tag: `ion-job-worker:pilot-local-kernel-8fb3d9c408a2`.
- Kernel policy SHA-256:
  `d804d926d815c823d0888afcc803f6ec3b43118649f98da3368fc242767bf0a2`.
- Installed CLI suite: **13 passed, one optional owner-generated executable
  archive test skipped** (14 discovered). Tests covered real command exit/logs,
  binary transfers, checksummed outputs, socket denial, CPU/memory/file/descriptor
  limits, timeout, detached descendants, invalid inputs, archive tampering and
  private result collection/replay protection.
- One additional installed-worker negative test passed: substituting an
  unapproved kernel refuses execution before effects; the original exact-local
  policy is restored and its hash rechecked.
- Containers used no network, one CPU and 256 MiB; only explicit disposable
  test fixture inputs/outputs were mounted. No user project or account was used.

Evidence: `.waku-cache/ion-command-local-kernel-build-20261021.json`,
`ion-command-local-kernel-cli-20261021.json` and
`ion-command-local-kernel-refusal-20261021.json`, with logs and source/hash-bound
request/receipt/archive exports. The helper scripts beside these reports refuse
to overwrite prior observations. This does not replace real managed/native
gateway lifecycle acceptance or optional strict GPU/cgroup/device tests.

## Resource closeout and remaining gates

At **18:12 IST**, successful Docker list calls confirmed no containers owned by
these tests remain. The boundary observation container is also gone. Prior
images and two named local candidates remain cached; they are not running
compute. Temporary inspection archives are gone. No cloud resources were created.
Evidence: `.waku-cache/ion-command-candidate-resource-reconciliation-20261021.json`.

SBX-02 still needs a provider-compatible reviewed kernel policy and immutable
registered image UUID, plus a real supported harness runtime, host manifest and
approved private authentication/bootstrap. Do not reuse the WSL kernel string
as a Nebius acceptance claim. SBX-03 pricing/retention bounds remain unverified;
no import/upload/execution is authorized solely by local image tests. Keep the
managed registry unavailable until these gates pass.

After cleanup, physical/virtual available memory was 1.29/2.55 GiB, below the
2.5/4 GiB release guards. No Cargo/Rust/Java build was running and no release
build was started. User applications and other sessions were preserved.
