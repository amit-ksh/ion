# Nebius GPU operation recovery contract

Checked 2026-10-07. Read-only primary-source research for Ion's Serverless AI Jobs
recovery. No credentials, account API requests, deployments or resource changes.
This note does not establish completion of the harness/remote execution goal.

The official API was inspected at commit
[`97201c925d3b03924ee4d1020a4ce31ab35f83e8`](https://github.com/nebius/api/commit/97201c925d3b03924ee4d1020a4ce31ab35f83e8)
(2026-10-07 09:12:14 UTC), with current first-party REST documentation checked
separately. The public `/openapi.json` request returned HTTP 403; a previously
saved public schema located the routes, which were then verified in the current
REST documentation. No account authorization was attempted.

## 1. Operation endpoints and exact types

Read the original operation using
`GET https://api.nebius.cloud/ai/v1/jobs/operations/{id}`. Resource-scoped listing
is `GET /ai/v1/jobs/operations?resourceId=<job-id>&pageSize=<size>&pageToken=<token>`.
Listing requires a resource ID; no documented query searches by an idempotency
key. Keep operation and job identities separate. An operator-supplied ID is a
candidate for verification, not proof. [Operation GET](https://docs.nebius.com/rest-api/ai/v1/jobs/operations/get),
[operation LIST](https://docs.nebius.com/rest-api/ai/v1/jobs/operations/list),
[operation service proto](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/common/v1/operation_service.proto).

| Mutation | REST route | Fully qualified request type | Request fields |
| --- | --- | --- | --- |
| Create | `POST /ai/v1/jobs` | `nebius.ai.v1.CreateJobRequest` | `metadata`, `spec`, optional `dryRun` |
| Cancel | `POST /ai/v1/jobs/cancel` | `nebius.ai.v1.CancelJobRequest` | `id` |
| Delete | `DELETE /ai/v1/jobs/{id}` | `nebius.ai.v1.DeleteJobRequest` | `id` |

All return `nebius.common.v1.Operation`. [Job service proto](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/ai/v1/job_service.proto),
[cancel reference](https://docs.nebius.com/rest-api/ai/v1/jobs/cancel),
[job lifecycle guide](https://docs.nebius.com/serverless/jobs/manage).

## 2. Returned witnesses and terminal semantics

`Operation.request` is a typed `google.protobuf.Any`. Normal JSON uses
`@type: "type.googleapis.com/nebius.ai.v1.CreateJobRequest"` alongside the embedded
request fields, with corresponding Cancel/Delete names. Ordinary message Any
payloads do not use a `value` wrapper. A type URL prefix is not the message's
namespace; the exact fully qualified message name is the final component.
[ProtoJSON Any format](https://protobuf.dev/programming-guides/json/#any).

`requestHeaders` maps lower-case names to `{values: [...]}`. `resourceId`
identifies the affected resource but can be empty for operations affecting
multiple/no resources. The current v1 operation has no retained resource snapshot;
`progressData` is service-specific and must disappear on completion.
[Operation schema](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/common/v1/operation.proto).

Missing/null `status` means pending. A present empty status means success because
its code defaults to zero; a present nonzero code means failure/cancellation.
Neither HTTP 200 nor an operation identity proves success. A successful create
operation does not establish a successful command/training outcome.
[Operation status schema](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/common/v1/operation.proto).

### Schema-derived examples, not account responses

```json
{"id":"operation-create","resourceId":"aijob-original"}
```

This is pending. A successful delete can have this schema-derived shape:

```json
{"id":"operation-delete","resourceId":"aijob-original",
 "request":{"@type":"type.googleapis.com/nebius.ai.v1.DeleteJobRequest",
            "id":"aijob-original"},
 "requestHeaders":{"x-idempotency-key":{"values":["saved-delete-uuid"]}},
 "status":{}}
```

With `status: {"code":7}`, it is a completed permission failure. The official REST
page's generic response contains placeholders and `code:123`; that sample is not
a real success or a guarantee that a particular header is returned.
[REST response example](https://docs.nebius.com/rest-api/ai/v1/jobs/operations/get),
[default/presence rules](https://protobuf.dev/programming-guides/json/#presence-and-default-values).

Nebius documents `X-Idempotency-Key` for modifying calls and says read calls ignore
it. Ongoing operations are retained; completed records have service-specific
retention. Concurrent mutations of one resource are discouraged. The published
contract does not explicitly promise that the idempotency header is always
returned in operation history, nor specify its retention duration.
[Official operation guidance](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/README.md#operations),
[idempotency guidance](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/README.md#idempotency).

## 3. Sensitive history and original injected input

The job schema marks these leaves sensitive:

- `environmentVariables[].value`;
- `injectedFiles[].content`;
- `registryCredentials.password`;
- `volumes[].s3Config.credentials.{accessKeyId,secretAccessKey,sessionToken}`.

Sensitive data may be masked/removed on persistent layers. Paths, environment
names, registry username, secret references and volume structure are not those
sensitive leaves. No inspected primary source demonstrates the actual operation
history mask string or promises that the whole `injectedFiles` list disappears.
[Job field annotations](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/ai/v1/job.proto),
[sensitive annotation contract](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/annotations.proto).

`GET /ai/v1/jobs/{id}?view=SECRET` returns injected content and unmasked inline S3
credentials and requires `resource.aijob.getSensitiveData`. An ordinary job read
cannot verify input bytes. `GetByName` has no SECRET selector: use it for candidate
discovery, then GET the candidate's exact ID with SECRET.
[Job GET reference](https://docs.nebius.com/rest-api/ai/v1/jobs/get).

The inspected JobService has no Update RPC or UpdateJobRequest. Restart accepts
only `id`. More strongly, `injected_files` is explicitly `IMMUTABLE`; changing it
requires recreation. A SECRET read of that same original job therefore supplies
an immutable input witness despite redacted history. Environment values are not
separately immutable-annotated. Do not extend this conclusion to future APIs or
arbitrary mutable secret/environment configurations.
[Job service definition](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/ai/v1/job_service.proto),
[injected file immutability](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/ai/v1/job.proto),
[IMMUTABLE semantics](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/annotations.proto).

**Ion's inspected body:** `Launch::body` supplies immutable
`/ion/job-request.json` and fixed environment values `ION_JOB_ISOLATED=1`,
`ION_JOB_CHILD_UID=1000`, `ION_JOB_CHILD_GID=1000`. These values are not account
credentials but remain API-sensitive leaves. Preserve their names and structure
when comparing history, then pin their exact values in the full SECRET read.
Implementation source: `crates/waku-core/src/driver/team_gpu.rs`, `Launch::body`.

## 4. Normalization boundaries

Normalize schema-known defaults only. ProtoJSON omits implicit zero/false values
and empty collections; int64 values serialize as decimal strings, bytes as
base64 and durations as seconds strings. Compare exact integers, decoded bytes
and precise durations. Preserve message presence and oneofs: `status:{}` and
`pricingModel.onDemand:{}` are meaningful. Do not globally strip empty objects or
ignore unknown capabilities. [ProtoJSON encoding](https://protobuf.dev/programming-guides/json/).

Read metadata adds an ID, timestamps and a spec version. Original parent, name
and request labels must still match. Ignore only documented generated metadata
when comparing caller intent. [Metadata definition](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/common/v1/metadata.proto).

Provider business defaults differ from protobuf zero defaults: the guide lists
16 GiB default shared memory and 250 GiB disk size. Do not interpret an actual
nonzero allocation as equal to a saved approved zero solely because the request
omitted a field. [Job creation defaults](https://docs.nebius.com/serverless/jobs/manage).

## 5. Proposed Ion acceptance rules

These are implementation recommendations derived from the sources above.

1. Read the persisted original mutation claim, UUID, run/work/attempt, policy and
   body. Use candidate IDs only with fixed-origin bounded GETs.
2. Require the returned `x-idempotency-key.values` to contain exactly the saved
   UUID, the exact typed request and original distinct operation/job identities.
   Missing, mismatched or ambiguous key witnesses remain unresolved.
3. For create, compare original metadata and non-sensitive spec intent. Ignore
   only the known sensitive leaves in that history comparison; absent whole
   injected/environment intent has no verified redaction contract. Reject dry
   runs. Fetch the same job with SECRET and verify the entire original body,
   fixed environment and immutable injected bytes.
4. For cancel/delete, require the matching typed request's exact `{id}`. Binding
   restores the original observation path; pending/failed records never prove
   cancellation, deletion, cleanup or successful execution.
5. Persist a verified binding before further observation. Repeated confirmation
   uses its saved binding. Unknown acknowledgement never creates a replacement,
   repeats the mutation or falls back to Local after uncertain execution.
6. Keep binding, mutation success, job terminal state, complete resource cleanup,
   command receipt and billing separate. No identity/name/header alone releases
   capacity, clears spending or resumes work.

A lost-create record with redacted content is defensible only through the full
chain: original idempotency witness + typed original request intent + same job
identity + immutable injected input + full SECRET body. If any link is absent,
keep it unresolved. The header completeness and concrete redaction shape still
need authentic disposable-account acceptance evidence.

## 6. Recovering missing VM/disk inventory

Compute has operation GET/LIST per resource, plus paginated parent history:
`/compute/v1/instances/list-operations-by-parent?parentId=<project-id>` and
`/compute/v1/disks/list-operations-by-parent?parentId=<project-id>`.
Resource reads use `/compute/v1/instances/operations/{id}` and the equivalent disk
route. Historical typed CreateInstanceRequest/CreateDiskRequest records can
corroborate project and creation intent, subject to permissions/retention. The
schema does not promise a job-to-VM backlink or a complete historical attachment
snapshot. [Instance parent history](https://docs.nebius.com/rest-api/compute/v1/instances/list-operations-by-parent),
[disk parent history](https://docs.nebius.com/rest-api/compute/v1/disks/list-operations-by-parent),
[instance service](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/compute/v1/instance_service.proto),
[disk service](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/compute/v1/disk_service.proto).

Managed disk declarations contain desired names/specs. Allocated IDs appear in
VM `status.diskAttachments`; they can be empty while attachment is pending.
Existing disks survive instance deletion, managed disks are deleted with it.
A readable disk's `status.managedBy` identifies its lifecycle-owning instance.
[Attachment schema](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/compute/v1/instance.proto),
[disk ownership schema](https://github.com/nebius/api/blob/97201c925d3b03924ee4d1020a4ce31ab35f83e8/nebius/compute/v1/disk.proto).

**Inference for Ion:** history can recover candidates and corroborate previously
witnessed IDs. A live disk's `managedBy` proves that particular ownership; it does
not prove that there were no other disks. Name matches alone are insufficient
because names can be reused and creation intent lacks allocated attachment IDs.
Missing/expired history is not proof of zero resources, and a failed creation can
leave uncertainty about partial allocation. When authoritative complete disk IDs
cannot be reconstructed, cleanup/capacity must stay gated. Do not guess inventory,
directly delete guessed VMs/disks or release unknown costs.

Cancel stops the container VM and deletes its container disk; deleting a running
job cancels it and removes the record. Mounted volumes remain. These lifecycle
statements do not enumerate resource IDs or authorize deletion of result
buckets/filesystems/preserved disks. [Lifecycle behavior](https://docs.nebius.com/serverless/jobs/manage).

## Later acceptance evidence

- Capture authentic create/cancel/delete history with original idempotency UUIDs.
- Verify actual leaf redaction and full SECRET immutable input comparison.
- Exercise missing sensitive-data/history permissions, pending/failed operations,
  retention expiry, lost acknowledgement and repeated confirmation.
- Capture VM/disk IDs before cancellation; independently prove original resources
  absent and mounted results retained.
- Test startup failure without captured inventory. Recovery remains unresolved
  whenever official witnesses cannot prove the complete allocated resource set.
