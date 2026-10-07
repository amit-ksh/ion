# Nebius GPU price receipts and billing identity

Checked 2026-10-07 against the public native API at commit
[`97201c925d3b03924ee4d1020a4ce31ab35f83e8`](https://github.com/nebius/api/commit/97201c925d3b03924ee4d1020a4ce31ab35f83e8).
The pinned protobuf files were fetched without authentication. REST routes were
verified in the earlier [launch-policy investigation](RESEARCH-nebius-gpu-launch-policy.md).
No account credential, authenticated estimate, provisioning request or billing
export was used. Local fixture results are recorded separately in the progress document.

## Contract findings

### Native v1 estimator

`POST https://api.nebius.cloud/billing/v1/calculator/estimate-batch` accepts typed
resource create/update requests inside `resourceSpecs[].spec`. The payload must
include the target type's billing routing annotations. The source gives Compute
instance requests as examples. The public schema does not enumerate every
supported type or establish that an AI Jobs create request is supported.
[Resource schema](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/billing/v1/calculator.proto),
[request/response schema](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/billing/v1/calculator_service.proto),
[REST reference](https://docs.nebius.com/rest-api/billing/v1/calculator/estimate-batch).

Ion sends its complete original `nebius.ai.v1.CreateJobRequest` with the native
Any type URL. This is a valid native message type, but calculator acceptance of
that type still requires provider verification. An unsupported estimate stops
before launch reservation or provisioning. Ion does not substitute a partial
Compute instance/disk request: a Compute instance also requires a network and
boot-disk specification, and a managed job's generated resources have not been
proven equivalent to an independently constructed Compute shape.
[Job request](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/ai/v1/job_service.proto),
[Compute specification](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/compute/v1/instance.proto).

The original request includes the approved injected command/input configuration.
The fixed native calculator receives that same body under the owner's existing
IAM authorization. The durable price witness stores its hash, not raw command
arguments, injected input bytes or credentials. This endpoint queries prices;
it is not the AI Jobs create endpoint. Its failure never triggers a create,
Compute approximation, account change or Local fallback.

The implementation requests contract prices, an explicit currency and hourly
aggregation. Response quantities and costs are decimal strings. Display-rounded
fields are not used for reservation arithmetic. Time-unit filtering can still
return other usage units; Ion rejects those additional or ambiguous units.
The older alpha calculator has a different payload and response and cannot
satisfy the v1 receipt. [Native calculator schema](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/billing/v1/calculator.proto),
[alpha schema](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/billing/v1alpha1/calculator_service.proto).

### Account identity and finality

The inspected v1 request/response has no explicit billing-account ID, selected
contract ID, quote ID, expiry, price revision or provider price-lock commitment.
`OFFER_TYPE_CONTRACT_PRICE` selects an offer class; the field alone does not prove
which active billing account or contract supplied the result. Ion's `quote_id`
is an owner allowance identity. It must not be presented as a provider quote.
[Estimator schema](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/billing/v1/calculator_service.proto),
[offer types](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/billing/v1/offer_type.proto).

A native project read identifies its parent tenant and region; active project or
tenant state is resource readiness, not active billing-account verification.
The inspected project/tenant messages do not supply a billing-account identity.
[Project](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/iam/v1/project.proto),
[tenant](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/iam/v1/tenant.proto).
No undocumented account endpoint or inferred contract identifier is implemented.

A price estimate does not settle actual usage. Startup, delayed cleanup, mounted
storage, retention, operations, transfer and taxes can create additional exposure.
Nebius budget alerts do not cap usage. Money remains held until an independent
billing reconciliation contract can prove the original account/currency/resource
charges and finality. [Pricing and quotas](https://docs.nebius.com/serverless/pricing-quotas),
[budgets](https://docs.nebius.com/signup-billing/budgets),
[remaining reconciliation investigation](RESEARCH-nebius-gpu-launch-policy.md#7-billing-reconciliation-uses-original-compute-resource-ids).

## Implemented Ion receipt

This is internal owner policy, not agent-supplied configuration or a new enabled
GPU submission tool. `GpuBudgetPolicy.pricing` optionally stores a version-1
`nativeJobV1` approval. New launches require it; historical original-launch replay
can retain an absent field without changing its saved signature.

The owner pins an exact SKU ID set, positive exact quantities and coverage of
GPU, vCPU, RAM and allocated disk. One approved bundled SKU can cover several
components. This coverage is an owner approval for the separately pinned resource
vector, not evidence that the provider includes every billable managed resource.
Result-volume launches additionally require a positive extra allowance; that
amount does not prove a storage/retention cap.

The verifier requires one matching resource and one matching hourly aggregate,
with exact project/name, currency, SKU identities/quantities, per-SKU cost equality
and exact sums. Changed metadata overrides, duplicate/incomplete SKUs, unsupported
cost types, non-time units, negative/invalid numbers and rate or total allowance
excess stop before reservation. Ordering and equivalent exact decimal formats
are accepted.

Amounts use checked integer arithmetic at 18 fractional digits. Coefficients,
exponents and serialized lengths are bounded; unsupported precision and overflow
are errors. These limits are Ion policy, not a documented provider precision
promise. Lifecycle reservation is computed as:

```text
horizon_seconds = approved_job_timeout + startup_seconds + cleanup_seconds
estimated_micros = ceil(exact_hourly_cost * horizon_seconds * 1_000_000 / 3600)
required_micros = estimated_micros + extra_reservation_micros
required_micros <= approved_reservation_micros
```

The separate hourly ceiling also rounds upward to monetary micro-units. No
provider amount passes through floating point. The complete approved monetary
allowance remains reserved, even when the estimate is smaller or the job ends.
Startup/cleanup seconds and extra spending are explicit owner assumptions;
they are not guarantees of runtime or invoice finality.

A sealed receipt binds request UUID, original body/policy/request hashes,
collection time, exact currency, SKU witness, rate and lifecycle calculation.
The atomic launch transaction checks a maximum 30-second age and exact policy/body
binding, then stores the sanitized receipt in `ion_team_gpu_prices` with the
resource-readiness witness and reservation before the one job create POST.
No raw provider response, rounded display fields, unknown response fields,
input bytes or IAM token are stored in this witness. Exact eligible replay uses
the original saved launch without a new estimate, readiness read or create POST.

## Remaining implementation and later acceptance

- [ ] Establish native calculator support for the complete AI Jobs request using
  the chosen account, including exact real metadata, omitted defaults, SKU units
  and all allocated VM/disk components. If unsupported, build a separately
  verified managed-resource estimation contract; do not silently approximate it.
- [ ] Bind the selected billing account/contract and obtain authoritative evidence
  of its currency, offer application, usage join and finality.
- [ ] Implement retained storage/transfer/tax policy and billing reconciliation.
  Delayed cleanup and output retention remain financial exposure.
- [ ] Accept changed prices, unsupported units, denied estimates and lost create
  acknowledgements without reservation release, repeated creation or fallback.
- [ ] Complete GPU image/device/IPC/egress/CUDA acceptance, missing historical
  inventory, dataset/checkpoint volumes and agent submission integrations.

Local fixture verification proves request fidelity, exact arithmetic, rejection
and transactional/replay behavior. It does not establish live Nebius support,
account identity, cloud spending caps or successful GPU execution.
