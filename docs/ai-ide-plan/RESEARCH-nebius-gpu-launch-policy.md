# Nebius GPU launch policy: pricing, resources and egress

Checked **2026-10-07** against current official Nebius documentation and the
[Nebius API source at commit `97201c925d3b03924ee4d1020a4ce31ab35f83e8`](https://github.com/nebius/api/commit/97201c925d3b03924ee4d1020a4ce31ab35f83e8).
The public [OpenAPI schema](https://api.nebius.cloud/openapi.json) was fetched
without authentication during this research: 1,517,701 bytes, SHA256
`6ea2db2000afcfb40f1d9a55e47fa292bbb25d68f1c0fdf8def38ef82453293f`.
It verified the REST routes below. Raw `.md` guides and generated REST references
were checked separately. No credentials, authenticated account requests, billing
exports, dry runs, deployments or other cloud mutations were performed.

This is a contract investigation, not live acceptance or completion of Ion's
harness/remote execution work. Recommendations are identified as such.

## 1. Current Ion boundary

The inspected implementation is in
[`driver/team_gpu.rs`](../../crates/waku-core/src/driver/team_gpu.rs) and
[`teams/gpu_jobs.rs`](../../crates/waku-core/src/teams/gpu_jobs.rs), with separate
result, resource inventory, recovery and wait modules. Agent submission remains
gated. The trusted owner transport pins the request, image digest, platform,
preset, subnet, disk size, shared memory, on-demand mode, no public IP, zero
restarts and timeout. It reserves owner-supplied money before one create POST,
keeps reservations through unknown acknowledgements and terminal states, and
verifies original result/cleanup independently. This is implementation evidence,
not proof of provider prices or an egress boundary.

At the initial research checkpoint, `GpuBudgetPolicy` had a target, currency,
owner quote identifier, expiry, reservation amount, run budget, job/concurrency
limits and result volume. The subsequent implementation adds exact tenant/project/
region/network/preset quantities and quota identities, fresh quota/capacity reads,
a durable sanitized receipt and atomic project/tenant resource reservations.
See [the implementation/review guide](23-remote-command-gpu-jobs.md#exact-gpu-resource-approval-and-fresh-readiness).
It still does **not** bind a billing contract/account, trusted provider price
estimate or enforced child egress mechanism. `publicIp=false` is not an egress
restriction. Resource availability remains advisory; see sections 5–7.

## 2. What is charged and in which units

Serverless AI uses Compute quotas and charges for its allocated VM and disk.
Mounted buckets/filesystems have separate storage charges, including when the
workload is not running. Jobs cannot be stopped and resumed like Devlabs.
[Serverless pricing and quotas](https://docs.nebius.com/serverless/pricing-quotas).

Compute GPU/vCPU/RAM usage has a one-second billing increment; displayed rates
are hourly. Disks are priced per GiB per **730 hours**, with byte-second metering.
USD applies except for Israeli companies, which use ILS. Published prices exclude
taxes and rounding can differ from final charges. Rates for several GPU platforms
changed **2026-10-01**. A previous price table or a `720 hours` convention must not
be reused as the current disk tariff.
[Compute pricing](https://docs.nebius.com/compute/resources/pricing).

**Recommendation:** use exact decimal arithmetic, explicit units and ceiling
conversion to Ion's integer monetary units. Reserve GPU, vCPU, RAM and allocated
disk components together; do not assume one GPU-hour price includes every CPU/RAM
component. Maintain separate bounds for mounted storage, its operations,
retention, transfer and taxes if the displayed budget includes them. Never mix
USD and ILS using an implicit conversion.

## 3. Price estimation before provisioning

### Current v1 calculator

`POST https://api.nebius.cloud/billing/v1/calculator/estimate-batch` estimates
resource costs; it is not a compute creation endpoint. Request:

- `resourceSpecs[]`: each entry has `spec`, a typed `google.protobuf.Any`.
- `offerTypes[]`: `OFFER_TYPE_CONTRACT_PRICE` selects special contract SKU prices.
- `currency`: documented examples `usd`/`ils`; default `usd`.
- `filterAggregationUnit.filterAggregationUnitValues[]`: time-unit filter such
  as `FILTER_AGGREGATION_UNIT_HOUR`; non-time costs are still returned.

[Current REST calculator](https://docs.nebius.com/rest-api/billing/v1/calculator/estimate-batch),
[calculator request proto](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/billing/v1/calculator_service.proto),
[offer type proto](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/billing/v1/offer_type.proto).

The Any payload must contain a target resource create/update request with billing
routing annotations. The source examples are
`type.googleapis.com/nebius.compute.v1.CreateInstanceRequest` and
`type.googleapis.com/nebius.compute.v1.UpdateInstanceRequest`. The response has:

- `currency`.
- `resourceCosts[]`: `metadata`, optional `overrideMetadata`, `aggregationUnit`
  (`unit`, `unitType`), and `general`, `fixedInstance` or `autoscale` cost.
- `totalCosts[]`: aggregation unit and a general total or min/max range.
- Cost breakdown: exact decimal `cost`, display `costRounded`, and
  `skuCosts[] {skuId, quantity, quantityRounded, cost, costRounded}`.

[Calculator resource/cost proto](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/billing/v1/calculator.proto).

**Contract gap:** accepting an arbitrary Any does not prove that
`nebius.ai.v1.CreateJobRequest` is supported for estimation, or that the estimator
covers managed job startup/cleanup. No inspected example establishes this.
For a Compute-equivalent estimate, use the actual project and the selected
`resources.platform`/`resources.preset`, plus a separate CreateDiskRequest for the
exact disk type/bytes. A model must not invent a different project or resource
shape. Prove that these estimates correspond to the generated job VM/disk before
claiming they bound a Serverless job.
[Compute instance request](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/compute/v1/instance_service.proto),
[Compute disk request](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/compute/v1/disk_service.proto).

The older `/billing/v1alpha1/calculators/estimate-batch` uses an explicit oneof of
Compute instance/disk/filesystem requests. Its response differs and lacks v1's
currency field. Its monthly aggregation comment uses 720 hours. Do not decode
alpha responses as v1 or confuse aggregation with the disk's 730-hour tariff.
[Alpha calculator](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/billing/v1alpha1/calculator.proto),
[alpha service](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/billing/v1alpha1/calculator_service.proto).

### An estimate does not lock a bill

No inspected v1 estimate response supplies a quote identity, expiry, invoice,
price revision, billing-account identity or provider commitment to honor the
estimate. An owner-generated `quote_id` remains an Ion allowance identity.
[Calculator response definition](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/billing/v1/calculator_service.proto).

On-demand job pricing is an empty `onDemand` specification and has no maximum
price field. A preemptible job can select `spotPricingPolicy.id`; that policy is a
per-GPU hourly maximum in USD, scoped to a project/platform, with readiness and
scheduling state. It is not a total job/storage/invoice cap, and switching to it
would change Ion's current on-demand execution choice.
[Job pricing model](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/ai/v1/pricing.proto),
[pricing policy](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/billing/v1/pricing_policy.proto).

Nebius budgets send delayed alerts; they do not stop or cap usage.
[Budgets](https://docs.nebius.com/signup-billing/budgets).

**Recommendation:** pin the full sanitized estimate request/response, receipt
hash, collection time, approved currency, exact resource vector and owner rate
ceiling. Recheck immediately before a new create; reject changed or incomplete
estimates. Describe the amount as a reservation/estimate, not a guaranteed invoice
ceiling. Unbounded cleanup delay and storage retention remain financial exposure
until their resources are confirmed absent. Do not release money on job state.

## 4. Project, platform, quotas and capacity preflight

These reads require owner authorization but do not provision resources. Pin the
same project selected for `Job.metadata.parentId` throughout the sequence.

| Check | Fixed-origin REST request | Relevant evidence |
| --- | --- | --- |
| Project | `GET /iam/v2/projects/{project-id}` | `metadata.id`, tenant `metadata.parentId`, `spec.region`, `status.projectState` |
| Platform | `GET /compute/v1/platforms/get-by-name?parentId=<project>&name=<platform>` | Same parent/name, `spec.presets`, `gpuCountQuotaType` |
| Quota | `GET /quotas/v1/quota-allowances/get-by-name?parentId=<tenant-or-project>&name=<quota>&region=<region>` | Container/name/region, present limit, active known usage/unit |
| Capacity | `GET /capacity/v1/resource-advice?parentId=<tenant>&pageSize=<bounded>&pageToken=<token>` | Exact region/platform/preset and `status.onDemand` snapshot |

[Project proto](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/iam/v2/project.proto),
[platform REST](https://docs.nebius.com/rest-api/compute/v1/platforms/get-by-name),
[quota REST](https://docs.nebius.com/rest-api/quotas/v1/quota-allowances/get-by-name),
[capacity REST](https://docs.nebius.com/rest-api/capacity/v1/resource-advice/list).

A preset resolves to `vcpuCount`, `memoryGibibytes` and `gpuCount`; GPU memory is
on the platform as `gpuMemoryGigabytes`. Bind the quantities as well as the
preset name. The API's deprecated GPU-memory GiB fields must not silently replace
the current decimal GB field.
[Platform/preset proto](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/compute/v1/platform.proto).

Quota lookup includes tenant/project, name and region. `spec.limit` is optional;
missing is not zero/unlimited. Verify `status.state=STATE_ACTIVE`, `status.usage`,
`status.unit`, and a known `usageState` (`USED`/`NOT_USED`). Unknown usage means
retry later, not permission to launch. Enumerate bounded pages where necessary.
[Quota schema](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/quotas/v1/quota_allowance.proto),
[quota service](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/quotas/v1/quota_allowance_service.proto).

GPU count, VM count, disk count and disk capacity need their own allowance checks;
a GPU availability answer alone is insufficient. The platform exposes a GPU quota
type; discover the remaining account quota names rather than infer them from UI
labels. Compute quota usage remains held from VM creation through deletion,
including stopped VMs.
[Compute quotas](https://docs.nebius.com/compute/resources/quotas-limits).

Capacity advice is tenant-scoped, not a project allowance. Its exact
`spec.region`, optional fabric and `computeInstance.platform/preset` must match.
`status.onDemand` supplies `available`, `limit`, `dataState`, `effectiveAt` and
`availabilityLevel`. Require fresh, recent, positive compatible evidence;
unknown/stale/limit-reached states are not ready. A high availability snapshot
still does not guarantee later allocation. The service includes Serverless VMs.
[Capacity advisor guide](https://docs.nebius.com/compute/virtual-machines/capacity-advisor),
[capacity schema](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/capacity/v1/resource_advice.proto).

**Recommendation:** keep separate owner-side reservations for each resolved
resource quantity across all runs, while checking both relevant project and
tenant allowances. Recheck external usage before create. These checks reduce
avoidable failures, but only a confirmed provider create proves allocation.
Do not automatically recreate an uncertain operation or fall back to Local.

## 5. Job timeout and dry-run gaps

The current Jobs guide specifies `spec.timeout` **3600–604800 seconds**, default
86400 seconds. The inspected Ion validator initially permitted 1–86400 seconds.
The subsequent implementation correction requires 3600–86400 seconds, retaining
Ion's existing 24-hour cap and shorter independent child-command deadline. It also
rejects a job lifetime beyond the original team deadline without extending it.
The minimum timeout is not evidence of a one-hour minimum charge; Compute metering is per second.
[Jobs configuration guide](https://docs.nebius.com/serverless/jobs/manage).

`CreateJobRequest.dryRun` exists. The inspected API says only that it is a dry run
of creation; it does not enumerate which IAM/quota/capacity/network checks are
performed, promise price locking, or bind a later create atomically. Its operation
must remain separate from the actual create and must never satisfy original-job
recovery. No dry run was performed in this research.
[Job service schema](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/ai/v1/job_service.proto).

## 6. Egress is not disabled by a private address

The default route table sends public traffic to Nebius's default egress gateway.
Even a resource without a public address uses a shared dynamic public IP through
that gateway. Traffic between resources inside the same network always stays
inside it, ignoring route tables; routing cannot disable that traffic.
[Routing behavior](https://docs.nebius.com/vpc/routing/overview).

VPC security groups filter VM-interface ingress/egress. Default groups allow all
traffic. Explicit groups combine rules ordered by effective priority, with deny
winning ties and implicit deny when nothing matches. Traffic to/from
**169.254.0.0/16 is not evaluated** by security groups.
[Security group behavior](https://docs.nebius.com/vpc/security-groups/overview).

The current `JobSpec` has `subnetId`, `publicIp`, exposed ports and storage mounts.
It has **no security-group attachment, firewall policy, privilege/capability,
network namespace or service-account selection field**. Its subnet and network
specs also do not attach security groups. Compute's VM NIC separately has
`securityGroups`; reading a group does not prove a managed job uses it.
[Job schema](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/ai/v1/job.proto),
[subnet schema](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/vpc/v1/subnet.proto),
[network schema](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/vpc/v1/network.proto),
[current public API schema](https://api.nebius.cloud/openapi.json).

VM metadata exposes attached service-account tokens through
`http://metadata.nebius.internal/v1/iam/sa/token` with `Metadata: true`. A header
requirement is not protection against arbitrary code that can send that header.
The actual managed-job service-account attachment and metadata reachability were
not observed here; neither absence nor accessibility is proven.
[VM metadata guide](https://docs.nebius.com/compute/virtual-machines/instance-metadata).

### Reads available to inspect an existing network

- `GET /vpc/v1/subnets/{id}`: require approved project, `READY`, expected
  `spec.networkId`, pool/route declarations and `status.routeTable.id`.
- `GET /vpc/v1/route-tables/{id}` and
  `GET /vpc/v1/routes?parentId=<table>&pageSize=<bounded>&pageToken=<token>`:
  verify network/project, ready state and complete destination/next-hop set.
- `GET /vpc/v1/security-groups/list-by-network?networkId=<network>&pageSize=<bounded>&pageToken=<token>`;
  then `GET /vpc/v1/security-rules?parentId=<group>&pageSize=<bounded>&pageToken=<token>`:
  verify group/rule identities, `READY`, network, access, protocol, egress/ingress
  CIDRs/ports and effective priorities.

[Subnet REST](https://docs.nebius.com/rest-api/vpc/v1/subnets/get),
[route-table schema](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/vpc/v1/route_table.proto),
[route schema](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/vpc/v1/route.proto),
[security-group schema](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/vpc/v1/security_group.proto),
[security-rule schema](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/vpc/v1/security_rule.proto).

**Unresolved boundary:** no inspected Serverless contract pins an explicit
restrictive security group to the job NIC before user code starts. Patching the
VM after observing its ID leaves a startup window and is not a substitute.
A custom subnet/route hash is useful evidence but does not deny same-network or
metadata traffic. Network config can also change after preflight.

**Recommendation:** implement and verify a trusted supervisor boundary before
executing the arbitrary child. For a network-denied child, use a verified isolated
network namespace or an equivalent fail-closed kernel policy, retain GPU-device
access, drop privileges/capabilities and prevent escape. Keep any storage uploader
outside that child boundary. For network-enabled tasks, explicitly bound metadata,
private/project resources and allowed external destinations. The current Job API
does not promise the container capabilities needed to create that boundary;
actual Linux/container and live managed-job acceptance must prove it. An image
manifest or `ION_JOB_ISOLATED=1` alone is not proof. No such boundary is implemented
by Ion's existing job worker merely through UID/GID separation.

## 7. Billing reconciliation uses original Compute resource IDs

One-time reports can be generated without provisioning compute, but creation is
still a billing-export API mutation and is outside this research's actions:

- `POST /billing/v1alpha1/one-time-exports` with
  `metadata.parentId=<contract-id>`, `spec.startPeriod`, `spec.endPeriod`,
  `spec.format=EXPORT_FORMAT_FOCUS_1_2_CSV`.
- `GET /billing/v1alpha1/one-time-exports?parentId=<contract>&pageSize=<bounded>&pageToken=<token>`.
- `GET /billing/v1alpha1/one-time-exports/{id}`.

The resource parent is a **billing contract**, not a project or an inference API
key. State `ONE_TIME_EXPORT_STATE_SUCCESS` is the enum's successful export state;
`downloadUrl` is sensitive and `expiresAt` is present. One proto comment calls
success “DONE” even though no such enum exists: decode the enum, not the comment.
[Export schema](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/billing/v1alpha1/one_time_export.proto),
[export service](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/billing/v1alpha1/one_time_export_service.proto),
[export REST list](https://docs.nebius.com/rest-api/billing/v1alpha1/one-time-exports/list).

Each successful GET generates a fresh presigned URL. The guide documents a
15-minute link lifetime and seven-day export-file availability. Periodic exports
run hourly into an approved bucket; there is one active periodic export per
contract. Their storage/operations are charged separately.
[Export guide](https://docs.nebius.com/signup-billing/usage/export).

Relevant FOCUS CSV identity and cost fields:

| Purpose | Fields |
| --- | --- |
| Account/currency | `BillingAccountId`, `BillingAccountType`, `BillingCurrency`, `PricingCurrency` |
| Project/region | `x_ContainerId`, `RegionId` |
| Original resource | `ResourceId`, `ResourceType`; names are display data |
| Tariff | `SkuId`, `SkuPriceId`, `PricingUnit`, `PricingQuantity`, unit prices |
| Period | `BillingPeriodStart/End`, `ChargePeriodStart/End` |
| Accounting | `BilledCost`, `ContractedCost`, `EffectiveCost`, `ChargeCategory`, `ChargeClass` |

`x_ContainerId` carries hierarchy identity such as the project. Resource identity,
region and tariff fields can be nullable. No exported field listed here is an
Ion request ID or a per-job final invoice marker.
[FOCUS field reference](https://docs.nebius.com/signup-billing/usage/focus-fields).

Cost labels are supported on **Compute billable resources**; labels on other
resource types do not appear in billing. No inspected source promises propagation
of `Job.metadata.labels` to generated VM/disk billing. Therefore `ion-request`,
job name or a matching label alone cannot join costs to an Ion job. Keep original
VM/disk IDs captured by the existing inventory collector; require exact scoped
identity matches. Bucket retention costs need an explicit allocation policy.
[Cost allocation labels](https://docs.nebius.com/signup-billing/usage/cost-allocation-labels).

Exports include corrections and adjustments. Commitment discounts have documented
reconciliation gaps: some adjustment rows lack SKU/commitment identifiers and
spend-based commitment usage can have zero `BilledCost`. `ConsumedQuantity` can
include minimum increments; the guide recommends `PricingQuantity` for cost
analysis. A zero billed row does not mean no resource usage/cost exposure.
[FOCUS conformance](https://docs.nebius.com/signup-billing/usage/focus-conformance).

**Recommendation:** persist a separate immutable billing-source receipt with
archive hash, export/contract identity, retrieval time, period and schema version.
Read presigned download URLs only from the trusted export service; validate the
approved storage origin, use bounded downloads/decompression/CSV parsing, disable
redirects and never forward IAM/S3 credentials to that URL. Match approved account,
currency, project, region and the complete original VM/disk inventory before
attribution. Use exact decimal parsing; quarantine ambiguous/missing identities.
Retain import revision/row provenance and update a period atomically rather than
adding repeated exports twice. Treat snapshots as provisional; no observed API
provides automatic per-job billing finality. Keep unknown reservations held.

## 8. Requirements before enabling the agent submit tool

The following are implementation recommendations, not claims that setup is ready:

1. **Owner target policy:** bind version, target, tenant/project/region,
   contract/account identity, currency, image digest, exact resolved resource
   vector, disk/shm, job/command deadlines, grants and storage retention.
2. **Trusted price receipt:** use v1 estimation, require complete matching
   currency/project/resource/SKU/unit evidence, conservative integer reservations
   and a short owner freshness window. Prove Job-to-Compute estimate equivalence.
3. **Resource receipt:** verify project/platform, both applicable quota scopes,
   exact capacity snapshot and independent quantity reservations before claiming
   a new mutation. Preserve failures and original identities.
4. **Actual egress enforcement:** establish the child boundary before arbitrary
   code; test external, same-network, metadata, IPv4/IPv6 and inherited descriptor
   escapes. Pin evidence to the immutable worker image and execution mode.
5. **Financial ledger:** keep command result, runtime cleanup, provisional usage
   and settled accounting separate. Never use `COMPLETED`, elapsed workload time,
   cleanup absence or an empty export to claim final cost.
6. **Agent contract:** accept only bounded arguments and selected file/volume
   references under that approved policy. Keep the existing durable one-create,
   original-operation recovery, wait, Stop and no-duplicate semantics.
7. **Acceptance:** verify actual managed-job startup, Linux isolation/GPU access,
   estimate correspondence, receipt collection, original resource inventory,
   quota/capacity failure, lost acknowledgement, cleanup and billing imports.

Price, quota, resource shape and existing network configuration can be read and
pinned without launching compute. A future allocation, invoice ceiling,
restrictive managed-job NIC attachment and child network enforcement cannot be
proven solely by those reads. Credentials or an account-ready status are not the
missing implementation. None of these unresolved requirements changes the user's
explicit agent environment/harness choice or authorizes cloud provisioning.

## Subsequent child socket boundary checkpoint (2026-10-07)

The fixed pre-exec Linux guard and new image-bound socket-mechanism approval are
now implemented. Nine CLI guard cases passed on a local pinned Docker Linux
image, also with outer seccomp disabled; six worker cases passed. This advances
item 4 above but does not complete managed-job egress acceptance: devices such
as RDMA, network mounts and IPC/file proxies remain separate runtime constraints.
GPU/CUDA/provider startup still needs acceptance, as do verified price/account/
billing and the agent submission contract. See [the boundary investigation and
local implementation evidence](RESEARCH-linux-gpu-child-network.md).
