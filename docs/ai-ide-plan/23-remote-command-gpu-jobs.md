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
Command lost-acknowledgement binding is still outstanding; the existing UI binding
for model calls does not resolve command jobs. Recoverable native calls are
matched to their original arguments and committed artifacts without execution.

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
The native injected configuration must fit 64 KiB. These field names and lifecycle
routes were checked against the [official REST schema](https://api.nebius.cloud/openapi.json)
and [Job service definition](https://github.com/nebius/api/blob/main/nebius/ai/v1/job_service.proto).

**Agent GPU submission is not enabled.** Transport fixtures establish protocol
behavior only. The implementation still needs:

- [ ] Durable GPU resource records, one-time launch reservation and verified
  recovery/binding after an uncertain acknowledgement.
- [ ] Native asynchronous waits for long jobs with Stop/restart/deadline handling,
  retaining the selected harness, environment, grants and task identity.
- [ ] Price/currency snapshots, aggregate monetary reservations and reconciliation
  of compute, startup, disk/storage and unknown cost.
- [ ] Approved dataset/checkpoint volumes, bounded download/readback, real worker
  receipt/log retrieval and artifacts with actual command evidence.
- [ ] GPU-compatible image validation and resource/quota/readiness checks.
- [ ] Enforced egress/resource grants. `publicIp=false` is not a network-deny rule;
  registered subnet policy and access authorization are needed.
- [ ] Compute cleanup confirmation. A terminal job or cancellation acknowledgement
  alone does not prove every underlying resource has been deleted.
- [ ] Live GPU acceptance with your configured project/quota/resources.

The GPU transport is deliberately separate from the command path; registering a
ConTree image or an inference key does not configure GPU execution. Full remote
project files, large transfers, remote checks and spending enforcement remain in
[the release checklist](20-agent-teams-release-checklist.md).

## Verification

Final fixture and compile evidence is recorded in
[implementation progress](19-agent-teams-progress.md). No live cloud submission,
provider billing, deployment, app visual test or second watcher was performed.
