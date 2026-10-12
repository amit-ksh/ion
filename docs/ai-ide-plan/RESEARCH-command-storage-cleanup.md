# ConTree command storage cleanup contract

Researched 2026-10-12 against the public Nebius sources below. Local source
checkpoint: `e8adb14` on `codex/ion-agent-teams`. This note covers ordinary and
project commands in Token Factory Sandboxes, not Serverless AI GPU Jobs.

## Decision

**The documented API does not support implementing verified deletion of an
original command's remote writable layer or uploaded project archive.** Keep
those conservative storage holds until a provider contract supplies the missing
proof. The documentation explicitly asks customers to confirm deletion and
retention guarantees with the Sandboxes team. Disposable execution establishes
the absence of a saved continuation image; it does not establish deletion of
uploads or service records. See
[security and data handling](https://docs.tokenfactory.nebius.com/sandboxes/operate/security-and-data-handling).

There are useful independent implementation opportunities: separate local
archive retention from the remote upload allowance, retain explicit persistence
witnesses, and show unresolved remote storage accurately. These do not complete
remote cleanup.

## Primary evidence and reproducibility

The [documentation index](https://docs.tokenfactory.nebius.com/llms.txt) links
the ConTree schema at
[eu-north.nebius.computer/static/api.yaml](https://eu-north.nebius.computer/static/api.yaml).
A credential-free HTTPS GET returned 200 and 119,648 bytes, with SHA256:

```text
23d748a2918b1124b3072d54d17d8eb43aa00d0b969bc1fa718f34ac884ee887
```

The schema declares OpenAPI 3.0.0, API version 1.0.0, and the default server
`https://api.tokenfactory.nebius.com/sandboxes/v1`. All 21 path objects were
enumerated. The only DELETE methods are tag removal, operation cancellation and
subprocess signaling. There is no file/image/instance deletion route, expiration
route, TTL request field or cleanup-operation response. These are observations
about this captured public schema, not a claim about undisclosed provider
internals. [ConTree OpenAPI](https://eu-north.nebius.computer/static/api.yaml)

No account API request, credential inspection, resource creation, provider
deletion, installation, build or device interaction was performed.

## Documented route meanings

| Route                              | Contract                                                                                                         | Cleanup implication                                                                                                                                                                                                   |
| ---------------------------------- | ---------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `POST /files`                      | Raw bytes; 201 returns UUID, SHA256 and size.                                                                    | Acknowledges an upload, not exclusive ownership or removal. [Upload reference](https://docs.tokenfactory.nebius.com/api-reference/sandboxes/files/upload-a-file-to-the-server-the-body-must-be-a-file-content)        |
| `GET /files/{sha256}`              | Current namespace's file metadata: UUID, SHA256, size, first-upload and last-upload timestamps; 200/401/403/404. | Positive metadata can identify the staged archive. There is no deletion receipt. [File metadata](https://docs.tokenfactory.nebius.com/api-reference/sandboxes/files/get-file-info-by-sha256)                          |
| `HEAD /files/{sha256}`             | 200 if uploaded in the current namespace; otherwise 404.                                                         | Tests namespace availability, not global physical erasure. [Existence reference](https://docs.tokenfactory.nebius.com/api-reference/sandboxes/files/check-if-a-file-exists-for-the-current-namespace)                 |
| `DELETE /operations/{operationId}` | Cancels; 202 accepts cancellation, 409 is a conflict.                                                            | Does not delete uploaded inputs, saved images or history. [Cancel reference](https://docs.tokenfactory.nebius.com/api-reference/sandboxes/operations/cancel-an-operation)                                             |
| `DELETE /images/{imageUUID}/tag`   | Removes one/all tags; the image remains accessible by UUID. A public tag may become visible again.               | Tag removal is not image deletion. Never use it as a storage-release witness. [Untag reference](https://docs.tokenfactory.nebius.com/api-reference/sandboxes/images/remove-tags-from-an-image)                        |
| `GET /inspect/{image_uuid}/`       | Returns image metadata, including UUID and nullable originating operation UUID.                                  | A read success establishes availability. A failed read does not identify a successful cleanup mutation. [Inspection reference](https://docs.tokenfactory.nebius.com/api-reference/sandboxes/inspect/inspect-an-image) |

The existing source-image UUID is a reusable owner-selected command image.
Cleanup must never delete or untag that shared base as a substitute for disposing
of one command's temporary layer.

### Retention is conditional

The current
[limits, retention and usage guide](https://docs.tokenfactory.nebius.com/sandboxes/operate/limits-retention-and-usage)
describes checkpoint retention of 180 days, after which untagged and unreferenced
images may be deleted. It supplies no guaranteed deletion deadline or uploaded
file lifetime. It asks customers to confirm required deletion behavior,
cancellation retention, and tagged/referenced image treatment.

**Inference for Ion:** elapsed time alone cannot release the original remote
storage reservation. Untagging and waiting 180 days is not a verified cleanup
implementation.

### Disposable execution is a persistence choice

`disposable:true` discards the command's filesystem changes instead of saving a
continuation image. Missing result-image UUIDs can also arise from unchanged
filesystems and interrupted/failed outcomes, so a nullable result-image field is
not a deletion certificate.
[Images, checkpoints and branches](https://docs.tokenfactory.nebius.com/sandboxes/concepts/images-checkpoints-and-branches)

Operation finalization ends runtime compute; saved images have a separate
lifetime. Cancellation acknowledgement, client disconnect and final runtime
status are distinct observations.
[Execution lifecycle](https://docs.tokenfactory.nebius.com/sandboxes/concepts/execution-lifecycle)

A future policy could distinguish temporary working allocation from retained
continuation-image allocation, using an exact original disposable request plus
validated terminal result. That would be a change to Ion's logical accounting
contract. It must not be presented as proof of physical storage deletion, uploaded
archive cleanup or invoice finality.

## Identity and structured absence

The REST authentication contract requires bearer authentication plus the
`Project` header. The selected project scopes the request. Access setup creates
credentials in that project.
[Access setup](https://docs.tokenfactory.nebius.com/sandboxes/start/set-up-access),
[upload authentication](https://docs.tokenfactory.nebius.com/api-reference/sandboxes/files/upload-a-file-to-the-server-the-body-must-be-a-file-content)

File lookup and timestamps are namespace-relative. Upload deduplication uses
content hashes; the same content can be reused for subsequent work.
[File metadata](https://docs.tokenfactory.nebius.com/api-reference/sandboxes/files/get-file-info-by-sha256),
[files and artifacts](https://docs.tokenfactory.nebius.com/sandboxes/guides/work-with-files-and-artifacts)

`GET /whoami` exposes token UUID, expiration, permission flags, optional limits
and reserved operation statistics. It does not expose a stable namespace UUID,
account/project echo, ownership generation or object deletion authority.
A token UUID is not documented as a namespace identifier.
[Token introspection](https://docs.tokenfactory.nebius.com/api-reference/sandboxes/auth/get-current-token-information)

For GET file lookup, the documented JSON error requires only `error`, whose
value can be a string, object or array; `status` and `traceback` are optional.
There is no mandatory machine-readable not-found reason, original object UUID,
namespace echo, deletion generation or tombstone. HEAD deliberately has no JSON
absence witness.
[File metadata and error schema](https://docs.tokenfactory.nebius.com/api-reference/sandboxes/files/get-file-info-by-sha256),
[HEAD existence schema](https://docs.tokenfactory.nebius.com/api-reference/sandboxes/files/check-if-a-file-exists-for-the-current-namespace)

**Inference:** an HTML 404, changed project, changed origin, inaccessible object,
expired credential or revoked permission must not become original-object cleanup
proof. Even a correctly scoped HEAD 404 proves only the documented namespace
availability fact. Public documentation does not prove that a rotated principal
maps to the exact original namespace or that namespace absence means all remote
copies were removed.

## Existing Ion witnesses and gaps

Read together with
[ordinary/project execution](23-remote-command-gpu-jobs.md),
[kernel resource groups](45-sandbox-resource-groups.md), and
[aggregate admission](51-command-resource-admission.md).

- [Command body](../../crates/waku-core/src/driver/team_jobs.rs) already sends
  `disposable:true`, `preserve_env:false`, an immutable image UUID and a
  finite `max_layer_bytes`. It requests no saved result image.
- [Original reservation](../../crates/waku-core/src/teams/jobs.rs) persists
  request/run/work/attempt identities, provider project, image, complete policy,
  source/check context and original request before effects. Project facts include
  archive SHA256, byte length, file count and source size.
- [Project upload](../../crates/waku-core/src/driver/team_jobs/project.rs)
  checks returned UUID/hash/size and recovers a lost upload acknowledgement through
  one hash lookup without repeating POST. It can reuse a preexisting file.
- [Transport](../../crates/waku-core/src/driver/team_remote.rs) fixes the origin,
  prevents redirects and sends the configured project. It has no stable original
  namespace witness or file/image cleanup transport.
- [Admission](../../crates/waku-core/src/teams/jobs/admission.rs) separately
  retains compute and layer flags. It charges the full native layer ceiling plus
  two archive copies. Compute release does not clear either archive allowance.
- [Project table](../../crates/waku-core/src/teams/storage.rs) keeps original
  archive bytes and optional provider file UUID. Admission rederives original
  archive size from the live database blob. Simply clearing the blob would
  invalidate the stored allocation comparison; it is not a correct cleanup patch.

These are source observations at the checkpoint above. Existing upload lookup
uses a bare HTTP 404 as absence for deciding whether to make the first upload.
That is not currently a reservation-release path and must not be reused as one.

## Lost acknowledgement: what can and cannot be recovered

| Situation                                             | Supported next observation                                                                      | Result                                                                                                           |
| ----------------------------------------------------- | ----------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| Upload POST acknowledgement lost                      | Original fixed origin/project and saved archive hash; GET and validate complete UUID/hash/size. | Can confirm an available original-content upload before launch. Does not prove sole ownership or later deletion. |
| Operation cancellation acknowledgement lost           | GET the saved operation; validate operation/image/full request witness and actual result.       | Can observe the original execution. A cancellation/terminal state cannot certify upload or layer removal.        |
| Tag-removal acknowledgement lost                      | Inspect by immutable UUID and the selected tag separately.                                      | Can inspect tag state. Original image is still a stored object.                                                  |
| Hypothetical file/image deletion acknowledgement lost | No documented cleanup operation, lookup-by-delete-request or tombstone exists.                  | Remains unresolved. Do not invent a route or repeat a mutation.                                                  |
| Retention boundary passes                             | Original UUID/hash may be queried if authorization remains valid.                               | Availability observation only; no guaranteed deletion cause, deadline or namespace continuity proof.             |

The last two rows are gaps, not implementation promises.

## Concrete implementation recommendation

### Work that can proceed without a remote deletion contract

1. Keep remote archive and layer allowances held. Add a sanitized diagnostic
   reason distinguishing unsupported cleanup from an in-flight or malformed
   original effect. Do not add a force-release control.
2. Introduce separate durable original archive facts, local-retention state and
   remote-upload state. Keep replay signatures, original request facts and
   verified receipts immutable. Historical uncertainty keeps its allowance.
3. If an owner explicitly approves local archive retention, prune only settled
   records past that retention period after checking all recovery/review/input
   consumers. Revalidate original blob length and SHA256, then atomically drop
   the local blob and reduce **only the local-copy logical allowance**.
   Never reduce the remote upload allowance in that transaction.
4. Preserve an immutable tombstone with original byte length/hash, request/run/
   attempt, prior blob witness, retention-policy revision and cleanup time.
   Migration must derive original allocation from those fixed facts rather than
   the current blob length. Replays may return existing receipts and must never
   reupload or resubmit to reconstruct pruned bytes.
5. Do not call database blob removal secure erasure or immediate filesystem
   reclamation: SQLite pages/WAL, backups and exported copies require a separate
   storage policy and maintenance path.
6. Test concurrent admission/pruning, lost acknowledgement, replay after pruning,
   still-active/reviewable records, mismatched hashes, old unknown allocations,
   rollback, stricter prior caps, and unchanged remote charges.

This is a proposal, not an implemented feature or an assertion that existing
authorization permits deleting retained audit inputs.

### Exact provider contract required for remote cleanup

Obtain a published, versioned answer from Nebius covering:

- A cleanup route or retention mechanism for the exact disposable writable
  layer and namespace upload object, with allowed methods, arguments and
  response schemas.
- Stable authenticated namespace/project identity, including token rotation and
  permission changes.
- Whether removal affects only an association or all stored copies; treatment
  of content-addressed reuse, reference counts, in-flight consumers and retained
  images.
- Mutation acknowledgement, idempotency scope/lifetime and original cleanup
  lookup after acknowledgement loss.
- An authoritative absence/deletion receipt that binds the original object,
  namespace and cleanup identity, with a defined consistency boundary.
- Cancellation/failure retention, fixed expiry guarantees and which operation
  records, logs, backups or replicas remain.
- Billing/storage quota release timing, distinct from data availability.

The official contact for confirming these requirements is listed in the
[limits and retention guide](https://docs.tokenfactory.nebius.com/sandboxes/operate/limits-retention-and-usage).
No message has been sent.

After that contract exists, implement a durable owner claim before a single
cleanup mutation; bind only the original acknowledgement or documented recovery
witness; verify the specified original-object absence; release that dimension
atomically. Unknown acknowledgement retains its hold and blocks replacement
cleanup. Reference-sharing checks must precede any destructive action.
