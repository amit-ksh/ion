# Remote command and GPU jobs

Updated 2026-10-07. Worktree: `codex/ion-agent-teams`. This is an implementation
checkpoint, not a claim that the full remote execution or release plan is done.

## Available command implementation

A Nebius-selected Teams assignment can use `run_remote_command` when it has
command plus workspace-read grants and the owner has registered a separate,
credential-free command image. This does not change New task, team invocation,
per-agent harness/model/host selection or pre-execution fallback. A Local or
fallback attempt cannot submit a cloud command.

The model selects literal arguments, explicit relative input/output paths and a
bounded timeout. The owner checks the actual attempt and pinned policy, selects
only the named workspace bytes and reserves the request before POST. The remote
worker creates a private workspace/home and actually executes the command. It
returns the command's exit status, timeout flag, bounded logs and requested output
bytes. The owner verifies output sizes and SHA256 hashes. Nonzero exit remains a
failed command; a model cannot convert it into successful test evidence.

The receipt and AI-generated JSON summary artifact commit in one transaction.
`read_remote_output` returns 4 KiB base64 pages from those saved outputs, restricted
to the original assignment. Files are not written to the original checkout.
Required check labels are still declared evidence: wiring real command receipts
into enforceable output-contract/check gates remains development.

Limits: 32 files/direction, 64 KiB/file, 256 KiB aggregate/direction, 8 KiB retained
per log stream, 120 seconds/command maximum, configured count and image-layer cap.
Network is disabled in the ConTree request; shell expansion is disabled. Only
registered executable paths can be selected. Linux production uses a trusted root supervisor, a separate nonroot command
UID/GID with empty supplementary groups, inherited no-new-privileges and a child
subreaper; escaped process groups are collected before
output reads. The Linux privilege controls were checked against the
[kernel interface](https://github.com/torvalds/linux/blob/master/include/uapi/linux/prctl.h).
The VM is disposable. Windows fixtures do not verify Linux behavior.

Used target policies cannot change within the run. Across mixed targets, the
smallest registered count remains the aggregate command cap. These resource caps
are not a monetary reservation or dedicated CPU/RAM allocation.

Stop/restart observes or cancels the original bound operation. Cancellation
acknowledgement is not terminal evidence. Unknown acknowledgement keeps its
reservation and prevents continuation; no duplicate POST or Local retry occurs.
Review recovery now lists commands separately from model completions. For a lost
acknowledgement, Verify and bind checks the original operation's saved hostname,
image, literal command, files, environment, network, resource/time limits and
complete input witness before attaching it. A changed or missing witness cannot
be bound. Confirmation replay uses its committed receipt; subsequent GET returns
the real exit/log/output receipt. Prepare continuation and Continue retain the
original attempt, journal, harness and budget, with no repeated command POST.
Private commands/files are not included in the public recovery projection.

## Setup and later review

Image packaging, exact registry fields and tests are in
[the worker guide](../../packages/job-worker/README.md). Prepare resources when
available; no VM/image/cloud account changes are performed by local tests.

- [ ] Build/import a reviewed credential-free Linux command image, with the exact
  worker/manifest, nonroot UID/GID and reviewed executable dependencies.
- [ ] Set the owner registry path and project/IAM credentials privately. Do not
  reuse a model image or copy credentials into the command image.
- [ ] Select Nebius and intersect command/read grants for the lead/specialist.
- [ ] Transfer a small source/check script, return an output document and inspect
  its actual exit, bytes/hash and generated artifact.
- [ ] Run a deliberately failing script and timeout; verify real outcomes remain.
- [ ] Try private/traversal/link/oversized files and an unregistered executable.
- [ ] Stop during a command; confirm the original operation becomes terminal.
- [ ] Restart after an acknowledged launch; verify GET/DELETE only, no second POST.
- [ ] In a disposable command run, interrupt the create acknowledgement. Use
  Review recovery to bind the original operation. Verify modified command/input/
  limit witnesses are rejected and repeated confirmations do not submit again.
  Prepare continuation, then Continue; inspect the real command receipt/artifact.
- [ ] Test Linux subreaper/process cleanup, including detached descendants and
  symlink/hardlink outputs, in the disposable image.
- [ ] Verify source checkout remains intact and output reads cannot cross tasks.

## GPU implementation started

GPU workloads use Nebius **Serverless AI Jobs**, a separate API from Token Factory
ConTree. The current transport implements the official create, creation-operation
poll, bound job observation and cancellation requests. It separates creation,
job and cancellation identities; verifies project/request/image/resource/input
provenance; bounds responses; and never retries a create POST. It pins an image
digest and explicit platform, preset, subnet, disk, timeout and no-restart policy.
Create and cancel carry their persisted UUID idempotency keys, following the
[official API contract](https://github.com/nebius/api). This implementation still
makes one create/cancel submission; acknowledgement loss does not retry either.
The native injected configuration must fit 64 KiB. These field names and lifecycle
routes were checked against the [official REST schema](https://api.nebius.cloud/openapi.json)
and [Job service definition](https://github.com/nebius/api/blob/main/nebius/ai/v1/job_service.proto).

**Agent GPU submission is not enabled.** Transport fixtures establish protocol
behavior only. The implementation still needs:

- [x] Durable GPU launch/body/operation/job records and a one-time launch
  reservation. Exact replay reads saved state and never repeats create.
- [x] Private owner-approved currency/quote/expiry/reservation/run-budget/job-count
  and project-concurrency limits, checked before launch. The saved target must
  match the actual Nebius assignment with command/read/network grants. These are
  launch allowances, not provider invoice totals or a verified billing ceiling.
- [x] Stop/restart observation and one-time cancellation claim before POST, with a
  separately persisted cancellation identity. Lost acknowledgement remains held;
  observed compute IDs accumulate, even if terminal responses omit them. Job
  terminal/cancel acknowledgement cannot release money, resources or continuation.
- [x] Durable VM/disk inventory, single deletion claim and typed completed deletion
  plus structured absence checks for the original job, every captured VM and disk.
  Verified cleanup releases compute capacity only; outputs and unknown costs stay
  reserved independently.
- [ ] GPU recovery UI and verified binding of the original job after lost create/
  cancellation acknowledgement. No automatic create retry or Local fallback.
- [ ] Native asynchronous waits for long jobs with Stop/restart/deadline handling,
  retaining the selected harness, environment, grants and task identity.
- [ ] Verified price snapshots and billing reconciliation for compute, startup,
  disk/storage and unknown cost. Aggregate reservations exist; unknown and
  terminal-job costs retain the full approved allowance until reconciliation.
- [ ] Approved dataset/checkpoint volumes, bounded download/readback, real worker
  receipt/log retrieval and artifacts with actual command evidence.
- [ ] GPU-compatible image validation and resource/quota/readiness checks.
- [ ] Enforced egress/resource grants. `publicIp=false` is not a network-deny rule;
  registered subnet policy and access authorization are needed.
- [ ] Recovery of jobs whose VM/disk inventory was never captured, and verified
  operator binding after a lost deletion acknowledgement. These remain unresolved;
  no missing witness is promoted into proof and no DELETE is automatically repeated.
- [ ] Live GPU acceptance with your configured project/quota/resources.

### GPU resource cleanup behavior

Stop/restart retains VM IDs returned by the job, matches each VM's approved project,
platform and preset, and records the IDs from its disk attachment status before
requesting deletion. Missing IDs, incomplete attachments, changed complete
inventories and unexpected filesystems keep the cleanup pending. This follows the
[compute attachment contract](https://github.com/nebius/api/blob/main/nebius/compute/v1/instance.proto).

Ion deletes only the original AI job. It never directly deletes a VM or disk. The
claim and inventory snapshot commit before the single DELETE. The acknowledgement
is a third operation identity, separate from creation and cancellation. Recovery
requires a successful operation carrying the exact typed DeleteJobRequest, then
structured resource-not-found responses for the job and every captured VM/disk.
HTTP errors, an HTML 404, a pending operation or a remaining disk cannot close it.
The request is defined in the
[Job service](https://github.com/nebius/api/blob/main/nebius/ai/v1/job_service.proto).

Verified cleanup releases the project compute slot and supplies Stop evidence.
It does **not** create a command receipt, clear spending, allow task continuation
or produce a user artifact. Those gates remain pending until output/accounting
integration is complete. Startup failures without complete inventory, resources
already removed before capture and lost deletion acknowledgements require the
remaining operator recovery work. Existing disks may be preserved by the provider;
the current no-volume policy never authorizes deleting them separately.

Later live review, after the GPU integrations and your resources are available:

- [ ] Capture the original VM and complete disk inventory while the job is active.
- [ ] Stop the job and inspect its creation, cancellation and deletion identities.
- [ ] Leave a captured disk present; verify cleanup and compute capacity stay held.
- [ ] Verify cleanup after the job, VM and captured disks are absent. Confirm money
  and output reservations remain pending independently.
- [ ] Restart during deletion; confirm GET checks only, with no second DELETE.
- [ ] Lose the deletion acknowledgement in a disposable run; confirm the saved
  claim stays unresolved, with no retry, replacement job or Local fallback.
- [ ] Test missing permissions/inventory and check that Stop attempts other jobs.

### Result retrieval contract investigation

The official job API does not return command stdout or output files. Nebius
provides job logs through its console and the
[resource-specific CLI reader](https://docs.nebius.com/cli/reference/ai/logs).
The [official installer](https://docs.nebius.com/cli/install) currently supports
Linux/macOS. A production Windows result reader still needs an authenticated transport with verified resource selection and
bounded complete receipts; the current transport does not invent a job-logs REST
route or accept model text as an execution receipt. Native waits must capture
inventory while resources still exist and retrieve outputs before job deletion.

The GPU transport is deliberately separate from the command path; registering a
ConTree image or an inference key does not configure GPU execution. Full remote
project files, large transfers, remote checks and spending enforcement remain in
[the release checklist](20-agent-teams-release-checklist.md).

## Verification

Final fixture and compile evidence is recorded in
[implementation progress](19-agent-teams-progress.md). No live cloud submission,
provider billing, deployment, app visual test or second watcher was performed.
