# Ion remote command worker

This is the version 1 internal JSON worker for actual commands and selected file
transfers. It runs in a separate **credential-free Linux image** registered by
an operator. The existing model/harness image must not be reused: it holds model
credentials. `ION_JOB_ISOLATED=1` is an execution contract marker; the remote VM,
image, grants and network policy provide isolation.

## Prepare an image later

1. Review a Python Linux base and pin its `@sha256:` digest. Build this Dockerfile
   with `PYTHON_BASE` set to that digest; add only reviewed command dependencies.
2. Scan the image/layers for credentials and private material. Keep the worker and
   `/ion/job-host.json` unchanged. Do not bake model/IAM tokens into it.
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
    "executables": ["/usr/local/bin/python3"]
  }
]
```

Set the existing owner `ION_NEBIUS_SANDBOX_PROJECT` and sandbox IAM credential
privately. The inference key is separate. Select Nebius for the agent and grant
command plus workspace read access. A Local fallback attempt cannot run commands.
No resources are provisioned by installation; unregistered command images fail
before a POST. Registry policy and image stay pinned once used within a run.
Across mixed targets, the smallest configured command count is the run cap.
An executable allowlist permits that executable's full behavior; Python can run
arbitrary code in the disposable VM. It is not a source-code filter.

## Contract and boundaries

- `argv` is literal and bounded; no implicit shell. Explicit inputs/outputs only.
- At most 32 files in either direction; 64 KiB/file and 256 KiB aggregate. Private
  path patterns, traversal, links and oversized outputs are rejected.
- Child home/environment are fresh; owner credentials are never transferred.
- Network is disabled by the ConTree instance API. The trusted root supervisor launches the command with the registered nonroot
  UID/GID and empty supplementary groups. Child processes cannot signal/read the
  supervisor or alter its outer receipt stream. The nonroot process group is
  reaped on timeout/exit; the VM is disposable and layer size is bounded.
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
  large dataset filesystem, GPU workload or executed output-contract gate.

## Verification

```powershell
python packages/job-worker/test_worker.py
cargo test -p waku-core team_job_ --lib --locked
```

Windows fixtures execute actual Python child processes. Production `main` requires an isolated Linux root supervisor with explicit
nonroot child UID/GID and rejects Windows. Linux process-group cleanup, symlink defenses and the imported image need
Linux/live acceptance later. Build/import and all live requests require your
configured account/resources; none are performed by these tests.

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
supports assignment-scoped 4 KiB pages; the GPU tool/async wait integration remains
development. Storage reads do not write to the source checkout.

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
