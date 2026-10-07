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
Required check labels remain declared observations. Pinned `requiredCommands`
now enforce actual successful receipts at assignment submission and parent
acceptance; see [executed command checks](#executed-command-checks).

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

## Executed command checks

A lead can pin up to 16 command checks in the child's `output_contract` at
`delegate` time. The complete output contract fits 32 KiB. Each `requiredCommands`
entry fixes a name, exact literal `argv`, ordered `inputPaths` and `outputPaths`,
and `timeoutSeconds`. All requirements remain unchanged through rework.
`requiredChecks` remains descriptive evidence, such as a source audit.

Example tool arguments inside a delegated contract:

```json
{
  "requiredCommands": [{
    "name": "Unit check",
    "argv": ["/usr/bin/python3", "check.py"],
    "inputPaths": ["check.py"],
    "outputPaths": ["test-report.json"],
    "timeoutSeconds": 30
  }]
}
```

The selected Nebius child runs that exact command through `run_remote_command`.
It includes the returned request ID in `report_result`:

```json
{
  "executed_checks": [{
    "name": "Unit check",
    "requestId": "<returned request UUID>"
  }]
}
```

The owner loads its original durable receipt for the same run, assignment and
current attempt. Submission and parent acceptance both require exit zero, no
timeout/cancellation/output errors and the exact command/file/time contract.
A label, another task's receipt, a previous attempt or a legacy receipt without
its original check context cannot satisfy this requirement.

Project input checks require the sealed isolated run workspace. The owner captures
a bounded edit epoch before bundling, rechecks it before reserving the command,
and verifies original input bytes against current scoped regular files at each
review gate. An owner edit or changed/missing input invalidates the receipt; the
child must rerun against the current source. Parent rework retains the pinned
requirements. This proves the explicit file bundle at the gate, not unlisted
files, later edits or an entire remote checkout. No receipt is copied from a
superseded child into a correction attempt.

Verification occurs on owner workers; UI frames do not read source files.
Handoffs include bounded complete receipt references and explicit omission counts.
Empty extension fields are omitted, preserving existing stored replay signatures.
Protocol version **31** requires the daemon and clients to use matching builds.

These gates currently support bounded remote command receipts. Local/fallback
shell checks, large project transfers, GPU metric/training checks, dedicated
CPU/RAM policy, verified prices/billing and Linux/live acceptance remain open.
A new command requirement grants no additional command, image, file or network
access.

Later review:

- [ ] Delegate a pinned test command to a configured Nebius-selected specialist;
  inspect the actual receipt, generated report and accepted child result.
- [ ] Submit only a matching check label or another assignment's receipt and confirm
  completion is refused.
- [ ] Run a failing command, timeout and command with a missing required output;
  verify no successful check is recorded.
- [ ] Change a bundled source file after execution, and again after child submission;
  verify both submission and parent acceptance reject the old receipt.
- [ ] Request rework and rerun against current files with a fresh command ID.
- [ ] Restart before parent review; verify the original settled receipt is reused
  without another command POST.

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
- [x] GPU recovery UI and verified binding of original create/cancel/delete
  operations from exact native witnesses. No automatic mutation retry or Local fallback.
- [x] Native asynchronous waits for approved bound jobs with Stop/restart/deadline handling,
  retaining the selected harness, environment, grants and task identity.
- [ ] Verified price snapshots and billing reconciliation for compute, startup,
  disk/storage and unknown cost. Aggregate reservations exist; unknown and
  terminal-job costs retain the full approved allowance until reconciliation.
- [x] Owner-approved result bucket/prefix, fixed-origin signed object reads, real
  worker receipt/log/file validation, atomic artifacts and scoped binary pages.
  The native adapter can await an approved bound job and read its retained outputs;
  agent submission stays gated by the remaining launch requirements.
- [ ] Approved large dataset/checkpoint volumes, retention/storage cost policy and
  full project/large-file transfer. Small receipt-contained outputs remain bounded.
- [x] Exact resource/quota/readiness approvals and cross-run quantity reservations.
- [x] Fixed child socket filter with Linux CLI evidence and image-bound approval.
- [ ] GPU-compatible image/device/runtime acceptance and complete egress grants.
  `publicIp=false` is not a network-deny rule; runtime devices, proxies, mounts
  and registered network policy still need review.
- [ ] Recovery of jobs whose VM/disk inventory was never captured. Missing native
  operation/input witnesses remain unresolved; no DELETE is automatically repeated.
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
Cleanup alone does **not** create a command receipt, clear spending, allow task
continuation or produce a user artifact. Runtime continuation requires both the
actual validated command receipt and verified cleanup. Monetary reservations
remain held after runtime settlement until separate billing reconciliation. Startup failures without complete inventory, resources
already removed before capture and lost deletion acknowledgements require the
remaining operator recovery work. Existing disks may be preserved by the provider;
the result-volume policy never authorizes deleting them separately or deleting
objects from the result bucket.

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

### GPU result retrieval

The official job API does not contain stdout or output files. Job logs are exposed
through the [console/CLI reader](https://docs.nebius.com/cli/reference/ai/logs);
the [CLI installer](https://docs.nebius.com/cli/install) currently supports
Linux/macOS. The owner result transport works on Windows without inventing a
job-log endpoint. It uses the
[documented native volume mount](https://github.com/nebius/api/blob/main/nebius/ai/v1/job.proto)
and [regional S3 endpoint](https://docs.nebius.com/object-storage/overview).

A private owner policy pins a regular bucket ID/name/project, region and prefix.
The owner verifies readiness and privacy before create. The worker's root
supervisor keeps `/ion` private from its nonroot child, then exclusively writes a
versioned execution receipt at `<prefix>/<request UUID>/receipt.json` after child
descendants stop. It preserves nonzero exit, timeout, bounded logs, requested
files and their hashes. Command success is determined from that receipt; the
job's terminal state and collector exit do not replace it.

Owner-only S3 credentials sign a fixed-origin HTTPS GetObject. Redirects, agent
URLs, unapproved buckets, malformed/oversized JSON, unexpected files and changed
SHA256 hashes are rejected. A 404 means no receipt yet, never cleanup proof.
Ion checks the original terminal job and input/output contract, then commits the
receipt and AI-generated summary artifact together. Exact replay reuses the
artifact without revision churn. Readback is restricted to the original
assignment attempt and returns 4 KiB binary pages without source-checkout writes.
The signing implementation matches independent
[AWS botocore S3SigV4Auth vectors](https://github.com/boto/botocore/blob/1.40.0/botocore/auth.py),
including a session-token header. Provider error bodies and raw secrets do not
enter receipt diagnostics.

GPU submission remains an internal owner transport. The native adapter can await
an already approved, observed job and read its verified outputs. Billing,
egress/resource validation, large datasets/checkpoints and missing historical
GPU inventory recovery remain development. The result collector can operate separately
from resource cleanup: missing result credentials cannot prevent Stop/cancel
attempts. Saved objects persist after job deletion; retention, trusted Linux
mount semantics and associated storage costs need explicit integration/acceptance.
Native waits must capture inventory while compute resources still exist.

Later setup/review is detailed in [the worker guide](../../packages/job-worker/README.md):

- [ ] Prepare a private regular result bucket/prefix in the same approved project.
- [ ] Configure distinct owner S3 read credentials; restrict IAM and bucket policy.
- [ ] Validate the pinned GPU image, root-owned private `/ion`, nonroot child,
  mount/exclusive-create/flush behavior and denied command access to the receipt.
- [ ] Verify authentic failed-command logs/files and the generated artifact.
- [ ] Try missing/invalid/oversized receipts, permissions and mismatched bucket or
  file hashes; verify no successful command/cleanup/cost evidence is manufactured.
- [ ] Restart and collect the original result once; confirm no create replay or
  duplicate artifact. Attempt reads from another task/attempt and verify denial.
- [ ] Delete the compute job and verify results remain accessible under the
  reviewed retention policy, independently of cleanup and unknown billing.

### Durable GPU waits and native continuation

`await_gpu_job` accepts only this assignment attempt's already approved, bound,
observed GPU request with its original result-volume and command/read/network
grants. An unknown create acknowledgement cannot be parked. It must be the last
action in the completion. There is no agent GPU submit tool yet.

Suspension persists the native journal and request before revoking the lease and
releasing the agent slot. The owner polls on independent background workers, at
most four at once, with a five-second minimum cadence per job. Ordinary polling
observes the original job and captures its VM/disk inventory; Stop or the original
deadline claims cancellation once. Resource cleanup uses the existing original
job deletion proof. Polling never submits a replacement or falls back to Local.

A real receipt plus verified cleanup settles the **runtime effect**. It does not
settle billing: the complete monetary reservation remains unchanged. The native
adapter resumes once with the original task, attempt, host, harness, model,
grants and budget history, under a fresh lease generation. It receives the real
exit/timeout/artifact context and can use `read_gpu_output` for the requested
binary pages. A failed command stays failed; analyzing that failure can complete
a task, but it cannot claim a passed training run or test.

Other unfinished file/app/CPU/GPU effects remain a fence. The active GPU job still
blocks additional work in its own run; releasing the agent slot allows other
independent runs to use it. This checkpoint does not enable parallel GPU work
within one team run. Stop/deadline keeps parked work uncertain until the original
resources are confirmed stopped. Missing receipts never trigger execution again.
Collection after a canceled run continues through persistent owner maintenance;
see [post-cancellation collection](#persistent-gpu-maintenance-after-task-cancellation).

Later review:

- [ ] Await an approved bound job; inspect Waiting, retained budget and agent slot.
- [ ] Restart the owner during the wait; verify original identities and no POST.
- [ ] Return a nonzero command receipt; confirm its exit/logs/artifact survive resume.
- [ ] Delay the receipt or cleanup proof independently; confirm no continuation.
- [ ] Stop or reach the original deadline while waiting; confirm original resources
  are canceled/verified without a replacement job or Local fallback.
- [ ] Inspect resumed task/attempt/host/model/grants and retained spending.

### Original GPU operation recovery

Protocol **30** adds creation, cancellation and deletion binding to the existing
native **Review recovery** panel. Supply the original Nebius operation ID (a
provider resource string, separate from the command worker's UUID), then use
**Verify and bind operation** on the relevant phase. The candidate is checked on
an owner background worker before an atomic database update.

The fixed-origin GET must return the original typed request, distinct operation
and job identities, and exactly the saved `x-idempotency-key`. Creation additionally
requires the saved metadata/specification intent and a SECRET read of that same
job. Operation history can redact sensitive input leaves; paths and environment
names must still match, and the immutable injected bytes and fixed environment
values must match the full job read. Known protobuf defaults are normalized;
unknown capabilities, dry runs, absent input declarations and ambiguous keys are
rejected. See [the API research note](RESEARCH-nebius-gpu-recovery.md).

Binding captures compute IDs observed during creation recovery, preserves the
original assignment/attempt/grants, and rechecks the run revision and mutation
claim after network access. Exact confirmation replay reads the saved outcome
without another provider call. A pending or failed operation can establish its
identity; it does not establish successful work, a stopped process or cleanup.
Deletion still needs successful typed completion and independent absence of the
original job and every captured VM/disk. Monetary reservations remain held.
Settled/cleaned jobs no longer produce stale lost-cancellation recovery notices.

Missing operation/key/input evidence stays unresolved. A deleted VM with missing
historical disk inventory is not proven clean by its name or a partial operation
listing. Agent GPU submission and monetary/resource/egress integrations remain
separate development work.

Later live review:

- [ ] Lose each create/cancel/delete acknowledgement and bind its exact original
  operation. Confirm no second mutation or Local fallback occurs.
- [ ] Try another task's operation, a changed key/type/path/input, a missing SECRET
  permission and an operation without the key header; confirm binding is denied.
- [ ] Change the run while verification is in flight; refresh and confirm the
  stale result is rejected without changing the original request or budget.
- [ ] Replay the exact saved confirmation and inspect unchanged identities,
  grants, spending reservation and provider request count.
- [ ] After deletion binding, delay job/VM/disk absence separately; verify cleanup
  remains unresolved until all original resources are confirmed absent.


### Persistent GPU maintenance after task cancellation

The owner now polls saved bound jobs even after canceled runs leave normal task
dispatch. Terminal/blocked assignments and inactive runs use a durable maintenance
queue. The queue retains the original request/attempt/job/operation identities;
unbound or unknown create requests are not resubmitted. Active GPU waits retain
their existing collector.

Maintenance shares the four-worker pool with active waits, excludes work already
being polled and checks for due work at most every five seconds. Poll claims have
a saved token and ten-minute expiry. Owner restart reuses the schedule; after an
owner crash, the unfinished claim becomes eligible when its lease expires. Missing
receipts and unavailable permissions/credentials retain exponential retry backoff
from five seconds to five minutes. Replaced callbacks cannot mark a newer claim
done or reset its retry schedule. Provider reads stay outside dispatcher/state locks.

A canceled model task does not prevent approved receipt collection or independent
resource cleanup. A valid late command receipt and artifact remain inspectable,
including real failure exit codes. Cleanup errors do not discard a retrieved
receipt; missing S3 access does not prevent cleanup. Runtime settlement requires
both receipt and cleanup proof. Canceled tasks stay canceled and all monetary
reservations remain held for independent billing reconciliation.

The provider job timeout now requires **3600–86400 seconds**: the documented
Nebius minimum plus Ion's existing 24-hour cap. The worker command can have its
own shorter limit. Ion never pads the original team deadline to fit a job; a run
must have enough remaining time for the entire provider job lifetime. The default
one-hour team limit may therefore need an explicit increase before GPU launch.
[Provider timeout requirements](https://docs.nebius.com/serverless/jobs/manage).
Agent submission still awaits the launch policy, price/resource/egress, volume
and live acceptance work in [GPU launch research](RESEARCH-nebius-gpu-launch-policy.md).

Later live review:

- [ ] Stop a disposable GPU task, confirm it leaves dispatch, then publish its
  approved receipt late; verify the same job produces one artifact without a new job.
- [ ] Restart the owner during a retry and after an interrupted poll; verify saved
  backoff/claim expiry and original IDs, with no model or Local fallback launch.
- [ ] Deny storage and cleanup permissions independently; verify the other operation
  continues, missing evidence stays pending and costs remain reserved.
- [ ] Hold several provider observations; verify at most four shared GPU workers,
  responsive task dispatch and no duplicate concurrent poll for the same work.
- [ ] Try a sub-hour GPU job or insufficient original team time; verify rejection
  before reservation/submission. Verify a short command deadline remains unchanged.


### Exact GPU resource approval and fresh readiness

New owner GPU launches now require an immutable versioned resource policy. It
pins tenant, project, region, network/subnet, image digest, platform/preset,
GPU count and memory in decimal GB, vCPU count, RAM in GiB, the declared disk in
bytes and shared memory. Separate project and tenant limits bound cumulative
outstanding jobs, GPUs, vCPUs, RAM and disk bytes. Both project and tenant GPU,
VM-count, disk-count and disk-byte quota identities/units are approved explicitly.
The policy cannot be supplied or changed by a model. Legacy reservations without
this policy remain recoverable; they cannot authorize a new production launch.

Before a new reservation, fixed-origin owner GETs verify the approved active
project, exact platform/preset quantities, ready subnet/network and applicable
active quotas with known usage and enough headroom. GPU quota names must match
the native platform's quota type. Capacity advice must identify the same tenant,
region and exact generic on-demand shape, have fresh HIGH/MEDIUM availability and
an observation no older than two minutes. Fabric-specific advice cannot satisfy
a job that has no approved fabric attachment. Pagination is limited to four
pages of 64 entries, with repeated/invalid tokens and ambiguous matches rejected.
See [provider contracts and limitations](RESEARCH-nebius-gpu-launch-policy.md).

The sealed receipt binds the exact request/injected bytes, launch body and policy.
Reservation requires a receipt checked within 30 seconds; the entire provider
read sequence is bounded to two minutes. Sanitized read paths/query parameters,
response hashes, quota IDs/headroom and capacity identity/time are saved in the
same database transaction as the original launch, before its one create POST.
IAM credentials and raw provider responses are not retained in that receipt.
Eligible exact launch replay checks the saved signature without freshness reads
or another POST; original-job recovery and maintenance retain their saved IDs.

Reservation aggregates approved quantities across runs atomically. The stricter
limits of the new and outstanding approvals apply. Unknown historical quantities
block new checked allocation until original cleanup is verified. Terminal/canceled
states, owner restart and lost acknowledgements do not free quantities. Verified
original deletion plus complete VM/disk absence releases resource capacity while
missing output and all monetary reservations remain independent.

Provider quota and matching capacity-advice headroom are conservatively reduced
by all outstanding owner reservations, even when provider usage may already
include them. Capacity age is checked again inside the reservation transaction. This can refuse an otherwise
available job; it does not subtract foreign usage or assume which old requests
the provider has counted. Readiness is a snapshot, not an allocation guarantee,
price lock, complete managed-job disk inventory or proof of an egress boundary.
No additional cloud permission is granted by this policy.

Agent GPU submission remains disabled. The fixed child socket boundary is now
implemented and locally kernel-tested. Native estimate receipts are now implemented
and fixture-tested; live estimator support, account/billing binding and complete
GPU image/device/IPC/egress acceptance, historical missing inventory, large
dataset/checkpoint volumes and retention policy remain development.

Later live review:

- [ ] Review the exact platform/preset vector, project/tenant quota IDs/units and
  network/subnet relationships against the chosen account; verify real response
  default/omission and capacity-unit semantics before enabling the target.
- [ ] Change project, shape, subnet/network or a quota after approval; verify no
  new reservation or create POST. Deny individual reads and inspect the failure.
- [ ] Provide stale/unknown, insufficient, fabric-specific or ambiguous capacity
  advice; verify rejection. Confirm a valid bounded paginated catalog is accepted.
- [ ] Hold independent runs near each resource limit; verify aggregate caps,
  conservative quota headroom and retained reservations after cancellation/restart.
- [ ] Delay VM/disk absence after deletion, then verify cleanup releases resource
  capacity while costs and missing command receipts stay pending.
- [ ] Replay an original eligible launch after availability changes; verify the
  saved job is returned without new readiness reads, submission or Local fallback.

### Fixed Linux child socket boundary

Production isolated command execution now goes through a fixed root-owned guard
before exec of user arguments. The nonroot guard uses an isolated Python startup,
checks empty capabilities and no_new_privs, accepts only image-pinned exact kernel
releases and trusted libseccomp, and closes inherited/launcher descriptors. The
filter denies socket families, io_uring, namespace creation, descriptor theft,
ptrace/BPF and compatibility ABIs. Ordinary threads/fork/exec inherit the filter.
The trusted root collector remains outside it. A guard failure is a real failed
receipt (exit 125), with no user code started or successful output-check evidence.

The image requires reviewed `PYTHON_BASE`, exact `LIBSECCOMP_VERSION` and
`ACCEPTED_KERNEL_RELEASES` build arguments. New owner preflight also requires
`"child_network":"seccompSocketDenyV1"` in the immutable resource policy before
any readiness read/reservation. This approval names the tested mechanism, not
an attestation of the provider runtime. Historical missing fields are omitted
from serialization: eligible original replay/recovery still uses the exact saved
signature and no reads or resubmission; that old policy cannot launch new work.

Nine guard CLI cases plus six worker cases passed in a pinned local Linux x86_64
image. Guard checks passed with and without Docker's outer seccomp filter, and
cover actual sockets/raw syscalls, inheritance, x32, descriptor closure, failure
before user code and private exclusive collector receipts. No GPU/cloud call was
made. See [research and remaining boundaries](RESEARCH-linux-gpu-child-network.md)
and [worker setup](../../packages/job-worker/README.md).

Later acceptance:

- [ ] Review and pin each actual provider kernel/image/library/runtime. A release
  string alone is not binary attestation. Reaccept changed runtimes and ARM64.
- [ ] Exclude child-accessible RDMA/NIC/VFIO devices, preconnected handles, network
  filesystems and IPC/file proxies. Socket denial cannot constrain those paths.
- [ ] Run a small offline GPU training/checkpoint with actual device/thread access;
  verify denial of sockets/metadata access and authentic collector result reads.
- [ ] Validate private mounted-result and exclusive-create/flush semantics on the
  provider filesystem, including failed commands and duplicate UUID refusal.
- [ ] Preserve explicit environment/harness/grants and all existing launch, money,
  recovery and live account gates. The agent GPU submit tool remains disabled.

### Exact native GPU estimates and atomic price receipts

New owner launches also require `GpuBudgetPolicy.pricing`: version 1,
`method: "nativeJobV1"`, the approved hourly ceiling in monetary micro-units,
startup/cleanup seconds, extra lifecycle/storage allowance, and an exact SKU
ID/quantity/component plan for the approved GPU/vCPU/RAM/disk vector. Bundled
compute SKUs are supported. The approval is immutable and cannot be changed by
the model. Historical missing pricing fields remain omitted from serialization;
eligible original launch replay/recovery keeps its saved signature.

After resource readiness, the owner queries the fixed native v1 calculator with
the complete original typed AI Jobs create request, contract offer class, exact
currency and hourly filter. A single matching resource and aggregate must have
exact quantities, matching per-SKU costs and exact sums. Changed identities,
unsupported units/cost types, duplicates, omissions, negative/invalid values,
overflow, unsupported precision and rate/allowance excess fail before reservation
or job creation. Display-rounded fields do not authorize spending.

Checked integer decimal arithmetic reserves the upward-rounded hourly amount
for the approved job lifetime plus startup and cleanup, with the additional
owner allowance. The private receipt binds request/body/policy hashes, currency,
SKU/rate witness and calculation. The launch transaction requires a receipt
checked within 30 seconds and saves it in `ion_team_gpu_prices` alongside the
resource witness and full original monetary reservation before its one create
POST. No raw input, IAM credentials or unknown response fields enter that witness.
Original launch replay needs no new estimate/readiness reads or POST.

**Provider limitation:** the generic native Any schema does not establish that
AI Jobs requests are supported by the calculator. That path is implemented and
fixture-tested; a denied/unsupported native estimate stops launch. It never uses
a Compute approximation. The schema also supplies no billing-account/contract
identity or price lock. Owner-approved SKU coverage and lifecycle allowances are
estimation assumptions, not final invoice caps. Result-volume extra allowance
does not establish a storage/retention cap. All money stays held after estimate,
terminal state, receipt and cleanup. See [API evidence and remaining work](RESEARCH-nebius-gpu-price-receipt.md).
Agent GPU submission remains disabled pending those integrations and the existing
image/device/egress, historical inventory, volume and live acceptance gates.

Later review:

- [ ] Review the actual account's complete native Job estimate, metadata/defaults,
  currency, SKU IDs/units and GPU/vCPU/RAM/disk coverage before approving pricing.
- [ ] Test changed/incomplete estimates, rate or allowance excess, unsupported units,
  denied/unsupported responses and redirects; verify no reservation or job create.
- [ ] Inspect the saved sanitized price/resource witnesses before the original
  create; replay after restart and confirm no new estimator/readiness/create call.
- [ ] Compare approved startup/cleanup/storage assumptions with real lifecycle and
  billing; retain money until authoritative account/resource charges are reconciled.
