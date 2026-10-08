# Ion remote command worker

This is the version 1 internal JSON worker for actual commands and selected file
transfers. It runs in a separate **credential-free Linux image** registered by
an operator. The existing model/harness image must not be reused: it holds model
credentials. `ION_JOB_ISOLATED=1` is an execution contract marker; the remote VM,
image, grants and network policy provide isolation.

## Prepare an image later

1. Review a Python Linux base and pin its `@sha256:` digest. Build this Dockerfile
   with `PYTHON_BASE` set to that digest, `LIBSECCOMP_VERSION` set to an exact signed
   Debian package version and `ACCEPTED_KERNEL_RELEASES` set to the comma-separated
   exact Linux kernel releases accepted for that image (one to eight). Add only
   reviewed command dependencies. Missing or unapproved kernel/library support
   stops the child before user code. Reaccept runtime changes and rebuild/import
   a new immutable image before changing an approval.
2. Scan the image/layers for credentials and private material. Keep the worker and
   `/ion/job-host.json`, fixed guard and root-owned kernel policy unchanged. Do not
   bake model/IAM tokens into it.
3. Import the image using the supported Nebius ConTree image procedure and retain
   its immutable image UUID. Docker build alone does not register a ConTree image.
4. Save a private JSON registry and set `ION_TEAM_COMMAND_IMAGES` to its path on
   the owner daemon. Each `target` is the agent's pinned **model sandbox UUID**;
   `image` is this separate command image. Use actual IDs in this template:

```json
[
  {
    "version": 1,
    "target": "MODEL-SANDBOX-UUID",
    "image": "CREDENTIAL-FREE-COMMAND-IMAGE-UUID",
    "credential_free": true,
    "max_commands": 8,
    "max_seconds": 60,
    "max_layer_bytes": 268435456,
    "uid": 1000,
    "gid": 1000,
    "executables": ["/usr/local/bin/python3"],
    "resources": {
      "version": 1,
      "cpu_seconds": 60,
      "address_space_bytes": 536870912,
      "max_file_bytes": 16777216,
      "processes": 128,
      "open_files": 128
    }
  }
]
```

Set the existing owner `ION_NEBIUS_SANDBOX_PROJECT` and sandbox IAM credential
privately. The inference key is separate. Select Nebius for the agent and grant
command plus workspace read access. A Local fallback attempt cannot run commands.
No resources are provisioned by installation; unregistered command images fail
before a POST. Registry policy and image stay pinned once used within a run.
Across mixed targets, the smallest configured command count is the run cap.
New command launches require an approved `resources` policy and the exact image
`resource_boundary: linux-rlimit/v1` marker. Missing approval rejects before image reads
or reservation. Legacy missing policies retain their original saved wire body
and remain recoverable; they cannot authorize new commands.
An executable allowlist permits that executable's full behavior; Python can run
arbitrary code in the disposable VM. It is not a source-code filter.

## Contract and boundaries

- `argv` is literal and bounded; no implicit shell. Explicit inputs/outputs only.
- At most 32 files in either direction; 64 KiB/file and 256 KiB aggregate. Private
  path patterns, traversal, links and oversized outputs are rejected.
- Child home/environment are fresh; owner credentials are never transferred.
- Network is disabled by the ConTree instance API. In isolated Linux execution,
  the root supervisor launches the fixed Python guard with `-I -S`, a sanitized
  environment, closed descriptors and the registered nonroot UID/GID with empty
  supplementary groups. The guard checks identity, empty capabilities, exact
  accepted kernel and trusted libseccomp, then installs the socket filter before
  exec of user argv. Child processes cannot signal/read the supervisor or alter
  its outer receipt stream. Descendants are reaped on timeout/exit; the VM is
  disposable and layer size is bounded.
- Logs drain while retaining 8 KiB/stream. Real command exit, timeout, selected
  output bytes and SHA256 hashes return in the receipt; the owner verifies hashes.
- Request reservation precedes submission. Stop/restart observes and cancels the
  same operation. A missing acknowledgement remains unresolved; never resubmit or
  switch to Local. Review recovery can bind a **command** acknowledgement only
  after verifying the original complete input and execution-policy witness.
  Model-call and command binding remain separate.
- Receipt and AI-generated summary artifact are committed together. Outputs can
  be read in 4 KiB pages with `read_remote_output`, scoped to the same assignment.
  They never write to the user's source checkout.
- These caps do not provide a monetary reservation, dedicated CPU/RAM preset,
  large dataset filesystem or GPU workload. Required output-contract commands now
  verify real successful receipts against the exact bundle and current source.

## Verification

```powershell
python packages/job-worker/test_worker.py
cargo test -p waku-core team_job_ --lib --locked
```

Windows fixtures execute actual Python child processes. Production `main`
requires an isolated Linux root supervisor with explicit nonroot child UID/GID
and rejects Windows. On 2026-10-07, nine fixed-guard CLI tests and six worker tests
passed on Docker's Linux x86_64 kernel `6.18.40.1-microsoft-standard-WSL2`, with a
pinned Python image and signed `libseccomp2=2.6.0-2`. Guard checks also passed with
Docker's outer seccomp disabled, so its filter could not mask a missing child
filter. These tests use fake requests and no GPU or provider credentials.

Run `python packages/job-worker/test_linux_guard.py` inside the built image as
root, alongside `test_worker.py`. The guard suite checks socket families,
thread/fork/exec inheritance, raw syscall and x32 rejection, descriptor/environment
closure, missing library/unapproved kernel, actual failures and private exclusive
result receipts. The Windows suite does not prove Linux isolation. Imported
ConTree/GPU images and live resources still need separate acceptance.

### Production CLI and owner-path acceptance (2026-10-08)

`test_linux_cli.py` invokes the **installed entrypoint**, including its real root
supervision and fixed nonroot guard. Eleven groups cover binary input/output and
hashes, real nonzero exits, private environment/collector isolation, socket denial,
timeout/log bounds, detached descendants, unsafe outputs, invalid requests,
exclusive result creation/replay, CLI flags, missing supervisor capabilities and
actual CPU/address-space/file/process/descriptor limits.
The root collector requires CHOWN, KILL, SETUID, SETGID and either DAC_OVERRIDE or
DAC_READ_SEARCH. It checks effective permissions before user code; the child
still receives no capabilities. Linux script line endings are pinned by the
package's `.gitattributes`, including Windows checkouts.

Run the suite in a fresh disposable **reviewed** image. The image root must be
writable for the private result collector; use no account/model credentials.
For example, on Linux with `ION_REVIEWED_JOB_IMAGE` set to its verified image ID:

```sh
fixture_dir=$(mktemp -d)
docker run --rm --pull=never --network=none --cap-drop=ALL --cap-add=CHOWN --cap-add=KILL --cap-add=SETUID --cap-add=SETGID --cap-add=DAC_OVERRIDE --security-opt=no-new-privileges --pids-limit=64 --memory=256m --cpus=1 --tmpfs=/tmp:rw,exec,nosuid,nodev,size=64m --env=ION_JOB_CLI_ACCEPTANCE=1 --mount "type=bind,source=$PWD/packages/job-worker,target=/acceptance,readonly" --mount "type=bind,source=$fixture_dir,target=/out" --entrypoint=/usr/local/bin/python3 "$ION_REVIEWED_JOB_IMAGE" -I -B -S /acceptance/test_linux_cli.py --fixture-dir /out
ION_NATIVE_JOB_WORKER_FIXTURE="$fixture_dir" cargo test -p waku-core team_job_native_linux_receipt_full_run_transfer_replay_and_artifact --lib --locked -- --include-ignored --test-threads=1
```

The output directory must permit only the trusted root fixture to write. Exports
use exclusive creation and happen only after every CLI group passes. The Rust
acceptance test feeds that actual receipt and manifest through the production
owner path with a localhost ConTree transport: exact file bundle, preflight,
durable reservation, single create/observe, generated summary artifact, full
paged readback and identical replay without another POST. No owner input files
or user credentials are discovered automatically. This is local Linux/owner
integration evidence; imported ConTree/GPU/FUSE images still need live acceptance.

## GPU result volume (owner integration)

The same version 1 worker can run in a reviewed GPU-compatible Linux image.
Serverless AI Jobs are separate from ConTree; GPU agent launch is still gated
until the native waits, resource/egress policy and accounting are integrated.
The private owner launch policy can pin one regular Nebius Object Storage bucket:

```json
{
  "version": 1,
  "bucket_id": "APPROVED-STORAGE-BUCKET-ID",
  "bucket_name": "ion-results",
  "region": "eu-north1",
  "prefix": "ion/results"
}
```

Ion verifies the bucket's project, ID, name, region, readiness and lack of anonymous
access before create. It mounts that approved prefix at `/ion/results` using the
[native JobSpec volume contract](https://github.com/nebius/api/blob/main/nebius/ai/v1/job.proto).
The exact entrypoint arguments are:

```text
--request-file /ion/job-request.json --result-volume /ion/results
```

The image must keep `/ion` owned by the root supervisor. Before child execution,
the worker makes `/ion` private (0700) and creates a fresh result UUID directory.
The nonroot command cannot reach that mount through `/ion` or alter its receipt.
After descendants stop, the supervisor exclusively writes and flushes
`<prefix>/<request UUID>/receipt.json`. It never overwrites an earlier receipt or
repeats execution after a partial write. GPU stdout reports only that writing
finished; Ion obtains real command outcomes from the authenticated object.
The CPU stdin/stdout protocol is unchanged. A nonzero command exit is preserved
in the receipt even when the collector itself exits successfully.

Configure owner read credentials privately, separately from GPU IAM/model keys:
`ION_NEBIUS_GPU_S3_ACCESS_KEY`, `ION_NEBIUS_GPU_S3_SECRET_KEY`, and optional
`ION_NEBIUS_GPU_S3_SESSION_TOKEN`. They are never passed to the image or command.
The reader uses a fixed HTTPS regional Nebius endpoint and signed S3 GetObject,
without redirects or an agent-provided URL. Receipt JSON is bounded to 512 KiB;
logs/files retain the existing worker limits and every output hash is checked.
Receipt and generated summary artifact commit together. Private owner readback
supports assignment-scoped 4 KiB pages. The native adapter can await an already
approved, observed job and resume once after the real receipt and verified
cleanup; full spending reservations remain held. Agent GPU submission is still
gated on verified launch policy. Storage reads do not write to the source checkout.

Missing objects, malformed receipts, access errors or invalid hashes remain
unresolved; none becomes successful command, cleanup or billing evidence.
A saved receipt survives restart and an exact replay reuses its artifact.
Result collection is separate from Stop: missing S3 keys cannot prevent attempts
to cancel/delete the original compute job. Mounted objects persist independently
of job cleanup and need reviewed retention/storage cost policy later.

Later acceptance must verify the actual mount, private parent, exclusive-create
and flush behavior in the approved Linux GPU image. The bucket prefix must be
mountable and writable by the trusted collector; unsupported filesystem semantics
must fail closed. Review the IAM/bucket policy, shared-prefix isolation, image
metadata/network credential exposure and retention policy before enabling launch.
Windows fixture tests do not establish Linux/FUSE/GPU/cloud acceptance.

## Fixed child socket boundary

`job-host.json` names `linux-seccomp-socket-deny/v1`. The fixed guard requires
Linux >=5.4, native x86_64/aarch64, libseccomp >=2.5.0 and API >=3; each accepted
kernel release is embedded in the immutable image. Only native x86_64 has been
tested here. The filter denies socket operations across all domains, io_uring,
namespace creation and descriptor/ptrace/BPF escape paths. It rejects compatibility
ABIs, persists through thread/fork/exec and sets no_new_privs. The trusted result
collector remains outside the child filter. Guard failure yields an authentic
failed command receipt (exit 125), never successful execution evidence.

This is a socket boundary, not complete GPU runtime isolation. Image/runtime
acceptance must exclude child-accessible RDMA/NIC/VFIO devices, connected handles,
network filesystems and IPC/file proxies that can perform external work. Stage
explicit inputs locally. The nonroot child cannot reach the collector's private
`/ion` result mount. CUDA/framework/thread compatibility needs actual GPU tests;
NCCL, MPS, AF_UNIX multiprocessing, package downloads and telemetry are not promised.
See [the boundary research](../../docs/ai-ide-plan/RESEARCH-linux-gpu-child-network.md).

New GPU owner preflight requires the pinned image's explicit resource-policy
approval `"child_network":"seccompSocketDenyV1"` before any readiness GET or
reservation. The marker records approval of the mechanism; it does not attest
provider devices or billing. Historical policies omit the field and retain their
original signatures/replay/recovery, but cannot authorize a new launch. Agent
GPU submission remains gated on the other launch requirements.

## Child resource approval (2026-10-08)

The owner passes only its pinned registry's `resources` JSON in the trusted
`ION_JOB_RESOURCE_LIMITS` environment field. Model tool arguments cannot supply or
change it. The root supervisor forwards that one bounded field to the fixed guard;
the guard removes it before user exec. Linux hard and soft limits are lowered
before user code, with tighter inherited limits retained. An unprivileged child
cannot raise a hard limit. Invalid/unsupported policy produces exit 125 before
user code and never successful check evidence. Core dumps are disabled when a
resource policy is supplied.

Allowed owner values: CPU seconds 1..registered max_seconds (maximum 120), address
space 64 MiB..8 GiB, file size 64 KiB..registered max_layer_bytes (maximum 1 GiB),
processes 1..256 and open files 16..256. CPU/address space apply per process, file
size per file, descriptors per process, and process/thread count to the real UID.
They are **not** aggregate VM CPU/RAM, disk totals, GPU limits, or a billing cap.
UID sharing can make the process limit stricter. Imported VM/runtime acceptance
must confirm those semantics; the native JSON request/receipt version remains 1.

The registry and complete environment field are stored with the original command
reservation. Lost-acknowledgement binding requires the same environment witness.
Historical missing fields are omitted, preserving original policy/body signatures.
GPU launches retain their separate owner policies and existing enablement gates.

## Project snapshot setup

The owner tool `run_remote_project_command` accepts `argv`, `output_paths` and
`timeout_seconds`. It has no model-selected upload root; only the sealed isolated
team workspace can be snapshotted. Add this optional approval to the private
command registry **before the run uses that target policy**:

```json
"project_snapshot": {
  "version": 1,
  "max_bytes": 33554432,
  "max_file_bytes": 8388608,
  "max_files": 4096,
  "max_run_bytes": 67108864
}
```

`max_bytes` counts source content; `max_run_bytes` counts stored archive bytes,
including index/framing and stopped/failed jobs. The image layer cap must fit both
archive and staged source. New snapshots require the exact additional manifest
field `project_snapshot: ion-project-archive/v1`. Original eight-field manifests
remain usable for original inline command contracts.

The supervisor CLI accepts only `--project-file /ion/job-project.ion` for this
mode. That file must be root-owned, regular, unlinked and mode 0400. Stdin retains
bounded version-1 JSON with an optional `project_snapshot` descriptor. The worker
validates the archive/hash/index/path/file bounds before staging, preserves
approved executable flags, applies the same child network/resource boundary and
adds `project_sha256` to its trusted receipt. Ordinary/GPU requests without the
extension keep their original wire fields. GPU datasets/result volumes are a
separate contract; the new CLI mode does not authorize them.

Review the full [project transfer and retention checklist](../../docs/ai-ide-plan/23-remote-command-gpu-jobs.md#2026-10-08--project-snapshot-command-execution)
before registering/importing it. Namespace blob lifetime/deletion and live
provider execution have not been accepted here.


## 2026-10-08: Windows executable project snapshots

Windows owners preserve the tracked Git executable bit in project archives.
Unix owners retain current filesystem permissions, including unstaged mode changes.
Untracked Windows files are non-executable. Tracked links, submodules and unresolved
merge entries reject the snapshot before upload; ignored/private files stay excluded.
Changing an executable bit changes the archive hash and invalidates a previously
successful command check at both report submission and parent review.

The local acceptance recipe explicitly mounts the private `/tmp` workspace with
`exec,nosuid,nodev`. Docker otherwise mounts it `noexec`, which prevents direct
execution of approved project scripts. This requirement does not alter the worker's
nonroot identity, socket-denial boundary or hard resource limits. Verify the chosen
registered host's actual workspace mount before live project execution.

Verification used the unchanged installed worker image
`sha256:050fbc7a43e32a8b1f550ef1a6520615168fc5f40832057ebd72dac6fd8d6f55`,
with no external network or credentials. All **14 installed Linux CLI groups** passed,
including a real Windows owner-generated executable script archive. The native Rust
owner validated the exact original archive/request/receipt and the full existing
project upload, replay, artifact and parent-review path.

To include the owner fixture in this acceptance run, export a fresh directory with
`ION_PROJECT_EXECUTABLE_EXPORT` and the ignored Rust test
`team_job_project_export_host_executable_archive`. Mount that directory read-only at
`/owner` and set `ION_HOST_EXECUTABLE_FIXTURE=/owner` for `test_linux_cli.py`.
The successful fixture export includes `host-executable-request.json` and
`host-executable-receipt.json`; validate them with
`team_job_project_native_executable_receipt_matches_owner_archive`, setting
`ION_NATIVE_JOB_WORKER_FIXTURE` to the Linux export directory.

Git mode source: [ls-files stage metadata](https://git-scm.com/docs/git-ls-files)
and [update-index executable bits](https://git-scm.com/docs/git-update-index).
