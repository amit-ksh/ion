# Publication-bound HTML resource transport

Updated 2026-10-17. Implementation branch: `codex/ion-agent-teams`.

## Available implementation

The retained originals from chapter 100 now have direct daemon commands and personal-gateway workflow queries, shared client validators and direct/gateway phone readers. The readers return a complete verified resource set for the upcoming native resource-document loader. The current document screen continues using its existing snapshot preview; the resource loader and screen integration remain open.

## Publication commitment

New `save_html_bundle` artifacts begin with an inert HTML comment containing a source-manifest SHA-256. That comment is part of the published original and its existing artifact digest. It has no visible document content. The commitment hashes canonical JSON consisting of version, entry path, total source bytes and ordered descriptor tuples: source index, relative path, media type, byte count and SHA-256.

The commitment excludes the parent artifact digest, avoiding a circular hash. Response wrappers separately retain the parent artifact ID/digest and the selected library or workflow metadata. A relay cannot replace descriptor hashes, recompute a valid manifest and still match the commitment in the already verified original.

This changes the bytes and artifact hash of new bundle publications. Existing completed request replay returns its exact original artifact. Legacy receipts and retained-source v1 receipts remain readable; they provide no public resource commitment and do not get fabricated remote source history. The local trusted coordinator read still supports retained-source v1 archives. Older HTML with no commitment uses its existing snapshot path.

## Transfer and access

Direct commands are `ReadArtifactHtmlManifest` and `ReadArtifactHtmlSource`. They require the exact selected artifact-library metadata. The chunk query carries source index, source digest, manifest digest, offset and fixed limit; it accepts no filesystem path or URL.

Gateway kinds are `workflowHtmlManifest` and `workflowHtmlSource`. Both require `evidenceRead`. Headless owner and desktop bridge routes use the same coordinator implementation, which rechecks current accepted-result membership in the selected workflow and the exact parent digest before reading sources. Existing relay authorization checks apply at submission, completion and polling, including expiry/revocation.

Each source chunk is at most 16 KiB with aligned offsets and an exact terminal cursor. The reader uses at most eight in-flight chunks per batch and exposes none of their data before verification. The existing 33-source, 1 MiB HTML entry, 2 MiB individual resource and 9 MiB aggregate-original bounds apply. Empty CSS can be inspected and published as a pinned resource; other empty document/font inputs retain their regular-file checks. An empty CSS source needs no chunk request and still receives the SHA-256 check.

Before each admission, the relay compacts completed document-transfer replies to a 64-reply history per publisher and keeps compact nonce/query identities for evicted replies. Existing in-flight reads remain available and can finish before the next compaction; the existing 64-pending and 512-total-row caps still apply. This applies to original documents, PDF pages and HTML resources, allowing a maximum original plus resource set to fit the bounded catalogue. Pending reads and other history retain their existing lifecycle. If a completed transfer reply disappears before polling, the phone can readmit the exact nonce/query up to twice within its existing deadline, with current scope and read-grant checks. Repeated eviction fails explicitly.

Chunk reads check current artifact/receipt/commitment metadata and retirement state in one read transaction, then select only the requested bounded BLOB slice. They do not reload or rehash the entire resource archive for every fragment. A fragment is transport data: full source hashes must be verified before any resource is used. Same-size stored-data or response substitution is detected by the phone's final source digest check.

## Phone verification and lifecycle

`readArtifactHtmlResources` and `readGatewayHtmlResources` use the shared `readHtmlResourceSet` module. It verifies the complete original hash, the original's commitment, the manifest's canonical hash, every selected metadata field, chunk boundaries/order and every complete source hash before returning any resource set.

Owner/route/foreground generation checks surround every request and digest and the final return. Gateway readers also check the current read grant and expiry at each checkpoint. Resource preparation uses portable UTF-8/base64 code and does not require native Buffer/TextEncoder/TextDecoder globals. No connection credential, host resource URL or model context is added to the returned source data.

Parent-artifact cleanup and source corruption continue to use the retained-source lifecycle from chapter 100. Script/file/network policy and native resource-origin isolation must be enforced again by the loader when it is connected; receiving verified bytes is not browser execution authority.

## Verification

- [x] **141 native cases pass**: 24 publication/source cases, 27 retention cases, 76 HTML/SVG cases, one production native worker bundle flow and 13 gateway-owner cases. These include actual direct source handlers, accepted workflow membership, Unicode entry paths, empty CSS inspection, multi-chunk transfer, changed selections/anchors/cursors, legacy source history and cleanup revocation.
- [x] **Twelve explicit native exports pass**: the new source handler fixture and eleven existing HTML/SVG/MathML fixtures. The new fixture carries the actual published original, canonical manifest and typed chunk responses.
- [x] **117 distinct client/phone/gateway cases pass**: 68 phone HTML/parser/CSS cases, 11 shared resource-verifier cases, 13 real gateway HTTP cases, nine relay cases and 16 phone portfolio/polling cases. There are no skipped source fixtures in the final applicable runs; repeat executions are counted once.
- [x] The 15-file phone HTML pass has **2,903 assertions**, zero failures/skips and a 6.92-second final serial run. The first concurrent pass hit two existing five-second timeouts while compilation/binding checks were active; the isolated rerun passed with those limits unchanged.
- [x] The relay test completes **1,600 original/source requests before expiry**, retains a bounded reply history and rejects changed nonce/query identity across restart. The same run rechecks maximum PDF transfer receipts and FULL/WAL durability. Source requests use full-size 16 KiB reply fragments in this load fixture; complete source digest verification is independently exercised by the native-to-client fixture.
- [x] The batch test proves at most **eight in-flight chunks**, complete byte preservation and no returned resource set on scope/digest failure. Codec-free checks pass with Buffer/TextEncoder/TextDecoder globals unavailable.
- [x] Complete shared-client/mobile/gateway TypeScript, strict new fixture types, generated-binding consistency, locked offline desktop/both-daemon checks and formatting/diff checks pass.

These are **270 distinct executed regression/export checks**. The final native transport build took 6m 22s, and desktop/both-daemon checking took 3m 12s. They are Windows host diagnostics, not phone performance measurements. The native fixture uses production handler output with synthetic owned data; real paired-device RPC and browser-origin/layout acceptance remain open. No visual/device test, app restart, paid call, cloud deployment or ion-prod merge is included.

## Re-run

Use the existing locked offline low-memory native test profile. The `html_resources` filter executes the four new native cases. Export `native_html_resources_phone_fixture` with `ION_HTML_RESOURCES_FIXTURE` set to the output JSON path, and explicitly export the eleven existing HTML/SVG/MathML fixtures with their chapter 73–100 environment variables.

Run `packages/waku-client/src/html-resources.test.ts`, `apps/mobile/src/lib/html-resource-readers.test.ts`, `apps/mobile/src/lib/gateway-portfolio.test.ts`, `packages/personal-gateway/src/portfolio-http.test.ts` and `packages/personal-gateway/src/portfolio-relay.test.ts`. Run the complete 15-file phone `html-*.test.ts` inventory with all twelve fixture/parser/selector paths. Do not count skipped exports or duplicate runs as new cases. Run the broad native publication/retention/HTML/owner matrix, TypeScript, `export_types --check` and formatting gates before release.

## Remaining implementation

- [ ] Create and connect the scoped native resource-document host for iOS/Android/web, preserving separate SVG/CSS document identities and existing script/file restrictions.
- [ ] Fence host requests and cached data on owner/route/grant changes, foreground loss, expiry and unmount.
- [ ] Verify actual native external SVG use/paint/path, dynamic names/cascade, fonts and source sizing when browser/device review is requested.
- [ ] Include retained originals in combined storage presentation.
- [ ] Complete release/platform/authenticated owner acceptance for existing harnesses. Unstarted harness adapters remain deferred.

## Owner review checklist

- [ ] Update daemon and phone client together. Publish a new HTML bundle with two SVG resources, a shared stylesheet and a font; keep the exact exported original.
- [ ] Read the original and source set through direct pairing and a gateway device granted Read workflow documents. A device without that grant must be refused.
- [ ] Remove workspace source files after publication, then repeat. Exact retained originals and source digests must remain stable.
- [ ] Revoke the gateway grant, expire pairing, change connection/route or background the app during a multi-chunk read. No partial resource set may be returned.
- [ ] Test changed parent selection, source index/hash, manifest body, chunk bytes and final cursor. Source verification must refuse substitution before resources reach a renderer.
- [ ] Check an older publication and exact completed invocation replay. The stored original remains available without newly invented source history.
