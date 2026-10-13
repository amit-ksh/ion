# GPU collector retention and admission

## Owner choice

Keep collector receipts until explicitly cleaned. New launches with result storage require an owner-approved allowance; there is no TTL, automatic cleanup, or agent cleanup authority. The allowance is separate from the dedicated workspace filesystem limit in [59](59-gpu-workspace-storage-limits.md) and from compute/spending settlement.

## Approval contract

`GpuResultVolume` adds an optional `retention` field:

```json
{
  "version": 1,
  "bucket_id": "storagebucket-example",
  "bucket_name": "ion-results",
  "region": "eu-north1",
  "prefix": "ion/results",
  "retention": {
    "version": 1,
    "mode": "keepUntilExplicitlyCleaned",
    "max_bytes": 16777216,
    "max_receipts": 32
  }
}
```

These are example owner approvals, not automatically installed defaults. Bytes must be 512 KiB–2 GiB and receipt count 1–4096. Both ceilings apply independently. Missing historical retention remains omitted in serialized policy and leaves the original launch body/mount unchanged. Existing collection and exact replay remain readable. Missing approval cannot authorize a new native result-volume launch.

## What stays reserved

The supervisor produces one exclusive `receipt.json` per original request, bounded to 512 KiB of encoded UTF-8. Each collector-enabled original launch reserves that full allowance and one receipt before launch, in the same immediate transaction as the durable original GPU request. A small observed receipt does not reduce the reservation. States `submitting`, running, terminal, cleaned and settled are all retained. An uncertain creation response, compute cleanup, cost settlement, absent object or local document cleanup does not release collector capacity.

Reservations span all matching original requests in this owner database, across runs, targets and projects using the same bucket. Exact immutable bucket ID/name/region and original prefix/mount are verified; equivalent regional bucket names with changed IDs and mismatched original witnesses fail closed. Historical collectors without an approved ceiling still count their full receipt allowance.

Prefix comparisons respect path components. `ion/results` and `ion/results-two` have separate caps. An ancestor and descendant intersect conservatively. Every overlapping prior cap remains enforced: a later larger allowance cannot loosen an earlier one; an ancestor cap counts retained sibling descendants even when a new launch uses another child prefix. A launch under a broader prefix must also respect overlapping prior child caps. This deliberately errs toward retaining a hold when scopes overlap; it does not claim precise live object enumeration.

## Owner review

Desktop **Usage → Remote executions → GPU job → Collector storage** shows the original bucket/prefix reservations, observed encoded bytes, and all intersecting scope caps in a virtualized list. The owner RPC is read-only and takes the original GPU request ID.

`TeamCoordinator::gpu_collector_for_request` accepts the original non-nil GPU request ID, reads its original bounded policy, and resolves a consistent scoped snapshot. `gpu_collector_usage` is a worker/background primitive. Neither API performs network calls, returns payloads/credentials, or performs cleanup.

The snapshot contains bucket/region/prefix, manual retention mode, scoped/bucket reserved bytes and receipt counts, locally observed encoded receipt bytes/counts, historical unbounded receipt count, per-prefix original ceilings/headroom, and how many additional full receipts all intersecting caps and the bounded owner review admit. `admissible_receipts: null` means no cap was approved; it grants no launch permission. Observed bytes come from saved validated local receipts and are not measured remote bucket usage or invoices.

The query fetches compact policy/mount/request/provenance facts rather than output payloads or whole launch signatures. Each volume fact is bounded to 2 KiB, mount witness to 4 KiB, and retained original collection to 4096 per reviewed bucket. Unknown JSON or collector identity is included for verification rather than silently treated as another bucket. Bound exhaustion blocks admission without dropping original records. A separate known bucket does not consume this bucket collection bound.

## Verification

The focused regression suite covers strict bounds/manual default/historical omission, independent byte/count ceilings, retained uncertain/terminal/cleaned/settled originals, smaller observed receipt without release, larger-policy refusal, ancestor/sibling and child/parent constraints, component boundaries, old uncapped originals, distinct buckets/regions, alias/changed region refusal, original replay identity, changed policy/mount/request/provenance, unknown identities, oversized witness allocation, UTF-8 byte reporting, read-only exhaustion, noncollector history, nil lookup and retained collection bound. Run the owner core filter `gpu_collector_` together with the existing native GPU results/preflight/pricing tests after integration. Do not substitute a fixture result for live storage acceptance.

## Setup and review checklist

- [ ] Approve a private bucket ID/name/region and a project-specific prefix.
- [ ] Review manual retention byte/count ceilings and separate storage spending allowance.
- [ ] Check owner collector usage for the original request after successful and failed jobs.
- [ ] Confirm uncertain/terminal/cleaned/settled originals keep the collector reservation.
- [ ] Confirm prefix changes and larger later caps cannot bypass existing approvals.
- [ ] Confirm collector exhaustion prevents a new cloud launch without repeating the original job.
- [ ] Confirm local artifact cleanup preserves GPU original receipt/artifact identity and collector reservations.
- [ ] Perform live bucket/CUDA/provider acceptance once an approved Nebius target exists.

No remote deletion is implemented here. Safe manual collector cleanup and hold release require provider-supported deletion/version/retention verification tied to the original bucket, key and receipt; a user deleting an object in a console, an S3 404, a lifecycle claim or successful compute deletion alone is insufficient proof. Remote object versions, multipart uploads, unrelated writers, filesystem overhead and bucket-wide invoice storage are outside this encoded receipt allowance and need their own provider/storage controls. Wider VRAM/host/egress, billing reconciliation, cloud speech and phone acceptance remain separate open work.
