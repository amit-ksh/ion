# Manual artifact retention

## Default and owner flow

Owner choice: **keep until explicitly cleaned**. Ion has no automatic expiration, age-based sweeper, agent cleanup tool, or scheduled artifact deletion.

Desktop **Artifacts → Manage storage** lists bounded pages with project and task filters. Each publication is Retained, Protected, Cleanup pending, or Cleaned. **Review cleanup** shows the original filename, format, content size and SHA-256; **Clean document content** sends an explicit owner request for that publication. Pending cleanup offers **Review retry**, retaining the original request after reconnect or restart.

Cleanup removes Ion's private stored copy. A file originally edited in the project remains intact. Publication metadata, original producer/evidence, source deduplication records, team tool receipt identity and existing history links remain. Cleaned content cannot be read, previewed, exported, used as a workflow handoff or recreated by an old team publish request.

## Protection and execution boundary

The daemon supplies task/project protection; clients cannot supply an eligibility list. Busy tasks, queued work and connected normal provider sessions protect source/project documents. A connected provider is conservatively protected even when its last saved task projection is Idle. Close its task connection before cleaning that project's documents.

The same immediate SQLite transaction rechecks team work/reviews, unfinished assignments, live attempts, workflow/routine activity, publication identities and original result recovery references. GPU and remote command result summaries remain protected for original recovery. Historical publications with missing/unverifiable provenance are shown as Protected. Unknown or malformed protection/status representations fail closed; native JSON-encoded automation statuses and historical plain statuses are both handled.

Normal provider startup is serialized with owner cleanup. Daemon state/provider locks remain held until the cleanup operation returns. Team/workflow SQL state and content claims are serialized by the shared control database. A new reader that arrives after a claim receives an unavailable-content error.

## Storage and recovery

`ion_artifact_cleanup` preserves the exact Artifact, owner request UUID, original byte count, storage kind, claim/completion time, and pending/cleaned state. Requests bind to full publication metadata and content evidence; stale metadata, changed bytes, changed request identity or reuse for another publication are refused.

Team document BLOB cleanup commits its content removal and tombstone atomically, leaving the publication receipt row in place. File cleanup first verifies the private regular file and original hash, durably commits a pending claim, deletes the managed copy and then records completion. The pending claim blocks reads. Exact retry can finish a previous unlink after a restart; a missing file without a claim is refused.

Windows uses a pinned real directory and a verified DELETE-capable file handle without write/delete sharing. Unix uses a pinned directory descriptor, no-follow file open, entry identity verification and unlink relative to that directory. Managed directory/file links and reparse points are refused. Only the UUID-named private copy is a cleanup target; arbitrary project paths cannot enter the API. SQLite content removal does not promise secure erasure or immediate database-file shrinking.

Native render reads cached RPC results. Filesystem/hash/SQL work runs on daemon workers; client requests run on background workers. Pages are capped at 32, async updates are generation-checked, and controls use the existing keyboard/focus primitives.

## Wire and client compatibility

Protocol **47** adds owner `ListArtifactRetention`, `CleanupArtifact` and `LoadGpuCollectorUsage` commands, plus typed pages/receipts/snapshots. TypeScript bindings are generated from the Rust contract. Pair the desktop/daemon/direct phone client on the same version. Gateway/device cleanup authority was not added; owner cleanup runs through the native daemon.

The related collector allowance and its separate remote-deletion limitation are described in [61](61-gpu-collector-retention.md).

## Owner review checklist

- [ ] Generate an agent HTML/PDF/Markdown document, then open Artifacts → Manage storage.
- [ ] Check All projects/All tasks, filtered pages, Previous/Next and icon Refresh.
- [ ] Check Tab/visible focus, Enter/Space, arrows/Home/End/Page Up/Page Down and Escape.
- [ ] Confirm active/connected tasks, team reviews and workflow handoffs show Protected and cannot be cleaned.
- [ ] Close the source connection and finish relevant work; review the original publication and clean one eligible private copy.
- [ ] Confirm the project source file, filename, producer, SHA and history remain; Preview/Save/model reads report content unavailable.
- [ ] Confirm old team publication retries do not generate replacement content.
- [ ] Retry a pending cleanup with its original request after reconnect/restart.
- [ ] Check both themes, long filenames, empty/filter/error states and leaving the page while a request is pending.
- [ ] Review collector ceilings and retained storage using Usage → Remote executions → GPU job → Collector storage.

No visual test or device/cloud acceptance was performed for this increment. Linux cleanup behavior requires native platform review; automated Windows filesystem cases and source/build checks are recorded in the progress checkpoint after verification.
