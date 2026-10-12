# Verified gateway PDF page previews

Status: implemented on `codex/ion-gateway-pdf`; merge and protocol46 regeneration are coordinated by the main Teams branch. Physical iPhone 15/Moto G32, macOS and actual internet pairing acceptance remain open. No Nebius resource or speech hostname is configured. Additional provider adapters remain deferred.

## User flow

In a paired phone's workflow evidence, choose Preview on an accepted generated PDF. The complete original first passes the existing accepted-document read and SHA-256 checks. The owner then renders one static page. Previous/Next, Fit, 150% and 200%, horizontal/vertical scroll, retry and verified original Export share the current direct-phone document controls. Neither phone PDF plugins nor a cloud GPU are required; the owner must be reachable and support Windows Data.Pdf or macOS CoreGraphics. Unsupported/larger originals keep Export and an explicit preview error.

## Source and transfer contract

`workflowPdfPage` is an `evidenceRead` portfolio read. It accepts the exact original workflow/run revision and complete selected accepted document metadata, zero-based page and 800/1200/1600 width. Query/source structs reject unknown fields and arbitrary paths/effects/render options. Existing acceptance verifies a pinned started non-skipped workflow stage, matching team/project/task identity and completed assignment-reported document, versioned publisher provenance and immutable full original digest. A new daemon read command gives temporary process-only desktop pairing the same owner implementation used by headless persisted owners.

The owner checks accepted membership, full original bytes/SHA and a private digest over the complete run, team snapshots and publisher record on every image chunk, and again before returning. The native render is admitted only after original verification; a second accepted-source check precedes cache publication. Replacements, deletion, run revision/status/stage changes, accepted result/provenance changes or missing binary publication receipts refuse old transfers. Private paths, prompts, source bodies, provider resource IDs and credential material never enter the result.

A process-local coordinator cache retains at most two immutable PNG pages / 24 MiB with a fixed ten-minute expiry. Space is reserved before raster work; concurrent starts cannot exceed admission. One nonblocking process-wide render/encode permit is shared with direct-owner phone PDF requests and recovers from mutex poisoning. Expired/evicted pages require Retry. Continuations carry an opaque transfer UUID, exact source/revision/page/zoom, aligned offset and fixed 32 KiB maximum chunk. The response pins full PNG size, SHA, dimensions/page count, transfer expiry and exact next offset. An out-of-order parallel chunk or exact read retry cannot mutate the underlying page or trigger another render.

Original preview remains limited to 64 KiB, 32 pages, 3.2 million pixels and a 12 MiB derived PNG, using existing Windows DPI/decode/deadline bounds. Publisher limits do not increase. Every result including metadata/base64 remains below the existing 60,000 UTF-8 byte portfolio limit; gateway HTTP body/grant/owner/nonce checks remain intact.

## Phone integrity and bounded work

The phone validates exact query/result schemas, metadata and transfer identity before allocation, then assembles into one prebounded output buffer. At most eight 32 KiB reads are in flight, with exact disjoint offsets and an immutable transfer seal. Any failed read fences the rest of that window. Full PNG SHA and the shared static RGBA PNG signature/dimension/chunk checks must pass before a data-image URI reaches native Image. Encoding yields every 48 KiB and checks the current scope; Buffer, atob and btoa are not required.

Owner, pairing/token/grants, run revision, selected evidence, foreground, mounted and request generation remain pinned. Navigation, backgrounding, refresh, owner/pairing change and late decode/digest results cannot restore an old image. Status and error text is explicit; controls use the incumbent accessible native targets and theme tokens. There is one loaded page and no image bytes in a shared query cache.

## Active transfer scheduling and persistence

The owner and temporary desktop bridge retain their existing five-second cadence for regular task, automation, Inbox and usage work. Only PDF queries are serviced at 500 ms while a verified transfer was accessed in the last fifteen seconds. PDF fast passes do not hydrate transcripts or rerun ordinary sync. The phone polls these read receipts at 250 ms with at most 240 polls and the unchanged 60-second deadline; the existing bounded read deadline/recovery still applies. No GPUI render, streaming event pump or animation cadence is changed. `docs/performance.md` was read before this worker scheduling change.

A maximum page needs 384 chunks; persisting all image bodies would fill the old 512-row receipt catalogue after one page. At each admission the gateway trims completed PDF body receipts to the latest 64 per owner (eight phone windows), preserving pending reads and all other history. Up to 64 pending reads can subsequently complete before another trim; the overall existing 512 stored receipt bound still applies. Before dropping older image bodies it writes compact device/nonce/publisher/canonical-query-SHA tombstones. Exact read retries can be requeued; continuation queries retain the same immutable transfer and an initial render read can create a new verified transfer; reusing the nonce for another source, page, zoom or owner is rejected even after restart. Tombstones expire after ten minutes and are bounded to 2,048 per owner; general 64 pending/512 stored admission remains. Flooding/backpressure remains an explicit read failure.

The measured Windows fixture showed durable DELETE-journal write cost dominating maximum page transfer. The gateway portfolio connection now enables SQLite WAL during initialization, keeps `synchronous=FULL` and `busy_timeout=5000`, and indexes completed PDF receipt trimming. WAL is persistent per database, so existing gateway connections use the same journal mode; initialization runs before the server serves requests and must succeed. There is no durability downgrade, manual checkpoint or background rewrite. SQLite's normal WAL checkpoint/recovery applies; deploy the gateway database and its WAL/SHM siblings together, use SQLite's backup interface for a live copy, and close all connections before copying a standalone database. Read/restart and rejected-write regressions verify existing semantics. Actual deployment storage latency and internet throughput are still acceptance gates.

## Automated verification

- Six owner tests cover bounded/expiring cache, immutable/out-of-order/replayed chunk identity, real accepted publication, actual native raster continuation chunks, invalid source and changed acceptance.
- Nineteen portfolio protocol tests, including two strict PDF query tests, and five shared native renderer tests pass.
- Ten direct-owner artifact regressions pass after sharing the native render permit.
- Phone tests cover largest PNG chunk/window bounds, full digest and headers, transfer mixing, source/grant/revision/expiry/offset substitution, background and late digest cancellation.
- Gateway tests cover independent evidence grants, wrong publishers and revoked reader completion, bounded two-maximum-page receipts, tombstone identity/restart and WAL/FULL durability.
- The real loopback HTTP fixture transfers two 12 MiB synthetic image-boundary payloads (768 chunks), checks both complete digests, maximum eight pending requests and the 500 ms active service cadence. Native PDF rendering is independently exercised with an actual accepted publication; the largest transport fixture does not claim native Image decoding or real device/network performance.

Final feature gates pass **149 distinct tests**: six gateway owner/cache, ten direct-owner artifact, nineteen portfolio protocol, five shared renderer, seventy-three complete gateway, and thirty-six phone PDF/history/transport tests. The passing two-maximum-page loopback HTTP gate measured **70,092 ms for 768 chunks**, with at most eight pending, two initial renders and both complete image digests verified; its cleanup completed and the whole test passed in 70,844 ms. Before WAL, the durable two-page receipt fixture took 62 seconds; it takes about 16–18 seconds with WAL/FULL. These are Windows development-machine synthetic transfer measurements, not an internet or physical-device latency claim.

Locked desktop/daemon/all native binary compilation, complete client/gateway/mobile TypeScript, Rust/TypeScript formatting, whitespace/UTF-8 checks and the mechanical UI detector pass. Branch protocol45 bindings were regenerated and checked against Rust before the final formatting audit; the main branch owns authoritative combined protocol46 regeneration/check after merge. No watcher, UI screenshot, physical device, live speech, paid provider or cloud deployment is run by these source checks.

## Owner review checklist

- [ ] Upgrade desktop/daemon/phone and gateway together after protocol46 merge.
- [ ] Pair with Read workflow documents; Preview an accepted generated PDF; compare every page/zoom with original Export and direct desktop preview.
- [ ] Review light/dark, portrait/landscape, large text, VoiceOver on iPhone 15 and TalkBack on Moto G32; horizontal/vertical scroll and page boundaries.
- [ ] Background, navigate, refresh, switch owner, expire/revoke the grant or replace source while rendering/transferring/digesting. No stale image returns.
- [ ] Concurrent page starts give bounded retry; cache eviction/expiry is explicit; a rejected or incomplete page preserves verified original export.
- [ ] Test actual internet/TLS latency and largest supported pages on the intended host; measure loaded-page memory and owner CPU/storage costs.
- [ ] Compile/run macOS CoreGraphics and verify orientation/color/rotation. Windows native tests do not establish macOS acceptance.

Gateway PDF page implementation and its final feature automated gates are complete; the full Ion goal also requires the separate GPU/resource and deployment/device work.
