# Direct phone artifact library

Later integration: [structured phone HTML Reading](47-phone-html-reading.md) and
[verified direct-owner PDF pages](48-phone-pdf-pages.md) are now implemented in
the combined branch at protocol **45**. The version44 evidence below describes
the original library checkpoint. Upgrade current phone/owner builds together;
gateway PDF rendering, browser CSS fidelity and actual device/macOS acceptance
remain open.

## Implemented scope

The native phone's task drawer now has **Artifacts** beside Inbox, Usage and
Workflows. It opens the active paired daemon's cross-project collection of
AI-published documents. The default view is all projects, tasks and formats;
searchable native filter sheets select a project, task or document format.
There is no user artifact upload, create or metadata-edit action.

The selected document opens a verified reader. Markdown and JSON have Reading
and Source controls; text, CSV and HTML/HTM use source. Links and images in
Markdown are inert text; HTML source never evaluates scripts or fetches images.
PDF/Word and documents beyond the 64 KiB text-preview ceiling show **Export
document**, preserving their complete original verified bytes. No truncated
preview is presented as a complete document.

Native phone HTML/PDF rendering is still **open**. The shipped mobile dependencies
do not provide the isolated HTML browser/native PDF render path used on desktop.
This increment does not add a webview, script-enabled authored content, external
URL viewer or simulated rendering. The user can export the verified original
and open it in the chosen document app.

## Wire contract and owner storage

Protocol version **44** adds these direct authenticated daemon commands:

- `listArtifactLibrary { query }`: project/task/format filters, an optional opaque
  cursor and page size 1..32. Reply contains bounded items, total and next cursor.
- `readArtifactDocument { query: { expected } }`: the exact immutable metadata
  selected from the library. Reply contains the same metadata and complete
  base64-encoded bytes after existing owner evidence verification.

An item carries only document ID/name/format, project/task IDs, creation time,
byte count, SHA-256 and producer origin. Private source paths, content location,
agent instructions and app credentials are absent. Existing direct pairing and
daemon authorization are unchanged. These commands do not grant publication or
modify any task, decision, artifact or cloud resource.

The owner keeps an indexed artifact metadata projection, updated transactionally
by control-record insert/update/delete triggers. Installation backfills current
records once. Project, task, format and chronological order have indexes. Each
page fetches at most 33 bounded metadata records; list rendering never reads
files or parses full documents. The original control library's 1000-record limit
and publisher limits remain in force.

The cursor records an append-sequence ceiling, the project/task/format/page-size
scope fingerprint, exact immutable anchor item and complete publisher-record
digest. It preserves created-time/ID ordering and excludes later publications even when their timestamps match or
sort earlier. Continuation tolerates unrelated workspace writes; changed/missing
anchor or different filters refuses the cursor and asks for refresh. Cursor
parsing has an explicit size/encoding bound. Paging does not silently reset to a
different collection. Deletions can reduce the remaining matching count.

## Actual publisher evidence

Default `AgentGenerated` labels on old imports cannot prove agent publication.
The new direct-phone collection requires persisted explicit publisher evidence:

- Agent edit: trusted turn, activity and source provenance already captured after
  successful native agent edit activities.
- Team assignment: committed run, assignment, agent and tool request provenance.
- Agent-generated document: a trusted native tool-call activity ID, recorded by
  the current Nebius `save_document` tool publication path.

The native tool supplies the activity ID from its actual tool call, independently
of model-provided document arguments. Its existing journal/replay behavior and
format/payload bounds are preserved. Historical default-only imports and records
without evidence are excluded from this new collection/read API. They are not
reclassified by a migration or filename/label heuristic. Existing desktop and
legacy `readWorkspaceArtifact` behavior is unchanged; ask an agent to republish
an old document when explicit publisher evidence is needed.

## Exact reads and exports

The owner validates format, bounded filename, document/project/task identities,
producer evidence and the complete selected metadata before opening the payload.
Existing `verified_document` checks bounded regular-file storage or atomic team
payload receipts and the original hash. The full bytes are verified again
against evidence before the reply. A metadata substitution, changed payload or
missing source fails closed.

The phone validates response shape, exact selected metadata and byte count,
decodes canonical base64 into a bounded fixed allocation, then checks the full
SHA-256 using native Expo Crypto before reading or export. Long decode work
periodically yields and rechecks scope so phone input and cancellation are not
blocked by a synchronous 16 MiB decode loop. Existing format-specific publisher
limits remain in force, including the 64 KiB team-document receipt limit. The
general original-file storage/export ceiling is 16 MiB; it does not increase any
publisher limit. Phone text reading is separately limited to complete UTF8
documents up to 64 KiB. Invalid UTF8 and larger original files remain exportable
as their exact verified bytes.

Read routes carry the selected pairing ID/address and exact document metadata,
including SHA. Changing pairing, address, client connection, route, visibility,
foreground state or selection fences all pending results. The private document
screen does not put payloads in the shared query cache. It discards loaded text
and bytes on blur/background/scope changes, and waits for a fresh current-owner
read on return. No wrong-owner saved route starts a payload request.

Export reuses the native folder picker/web download primitive with its narrow
ID/name/format contract. Scope is checked before/after the picker and around the
actual file write. Native exports use the shipped Expo FileHandle API with
explicit write-only mode for Android SAF files, at most 64 KiB per bridge write,
and an event-loop yield/scope fence after each chunk. Final offset and file size
must equal the complete original byte count. Handles close before cleanup;
failed/abandoned output is removed where folder permissions permit. PDF, Word,
HTML and CSV exports use their correct MIME types. File names are
bounded and sanitized; exported bytes remain the verified original.

## UI and performance

Collection and reading rows use FlatList virtualization and cached document
preview rows. JSON formatting estimates depth/whitespace growth before expansion
and falls back to the complete original source when formatting would exceed the
64 KiB reading bound. Source chunks and individual giant Markdown blocks retain the
existing bounded-row fallback. Formatting/Markdown parsing is performed once per
verified document/mode; row render reads prepared text/nodes. Reading works
without browser-only TextEncoder/TextDecoder support in the native JS runtime.

Filters have native search fields, modal dismissal, selected states and bounded
visible result sets. Buttons use existing 44-point accessibility targets and
native/web keyboard-operable Pressables. Text remains selectable in source.
Static errors, unavailable previews and connection changes have explicit copy.
No decorative motion or custom keyboard convention was introduced.

## Verification and review

Automated checks cover native indexed paging/provenance/filter scope, stable
cursor across unrelated writes/new documents, changed/missing anchor, strict
metadata/full payload integrity and historical backfill. Phone helper tests cover
foreign/oversized/duplicate/incorrectly ordered responses, exact saved routes,
canonical byte decoding, digest failures, large transfer cancellation and stale
read boundaries. Export tests cover picker/write fences, exact bytes in bounded chunks, yielded
cancellation, partial/short writes, handle-close failures and cleanup.
Existing Markdown/JSON/inert-source tests also run with native TextEncoder absent.

Completed automated checks on `codex/ion-phone-artifacts`:

- `cargo test -p waku-core --lib control::artifact_library --locked`: **9 passed**.
- Protocol `export_types --check`: **version 44 bindings current**.
- `artifact-library.test.ts`, `artifact-export.test.ts` and
  `document-preview.test.ts`: **26 passed, 102 assertions**.
- Full mobile and shared-client TypeScript checks passed. Frontend static detector
  returned no findings; formatting, diff whitespace and UTF8/LF checks passed.

The independent checkout reused the installed primary checkout's JS dependencies
through local path mappings and a test resolver preload, without copying/installing
packages or adding symlinks. Native Cargo checks used the serialized shared target
with existing low-optimization per-package overrides. The protocol package cache
was invalidated once because another worktree's compiled version 43 was stale for
this branch; the rerun compiled this checkout's version 44 source and passed.

Live native device/visual and paired-owner review remain open. No watcher,
billable resource, deployment, microphone capture or new provider was started.

- [ ] Publish agent Markdown, JSON, HTML and text documents in two projects; open
      Artifacts on a paired native phone and compare names/order/counts with desktop.
- [ ] Use more than 32 documents; browse next/previous with project/task/format
      filters. Publish another document and make unrelated workspace changes between
      pages. Confirm the original page sequence stays stable until Refresh.
- [ ] Change/delete the cursor's anchor and confirm the phone requires a refresh.
      Reset page and verify the current collection. Check default-only old imports
      do not appear as proven agent publications.
- [ ] Open Markdown/JSON Reading and Source, long blocks, links and images. Confirm
      no authored link/image request, script execution or unsolicited navigation.
- [ ] Export original HTML, PDF/Word and a text document beyond 64 KiB. Compare
      bytes/SHA with the owner. Verify correct filename/MIME handling on Android/iOS.
- [ ] Switch pairing/address, reconnect, navigate away or background during read,
      digest, picker and export. No late content, wrong destination or stale write.
- [ ] Deny/cancel folder access or simulate a failed write; verify no partial file
      is presented as successful. Refresh a missing/modified source and inspect error.
- [ ] Review native light/dark contrast, long names, Dynamic Type, VoiceOver/
      TalkBack, keyboard controls and scrolling. Live UI testing was not requested.
- [ ] Native isolated phone HTML/PDF rendering remains a separate open parity task.
