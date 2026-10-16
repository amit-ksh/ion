# Retained HTML resource originals

Updated 2026-10-17. Implementation branch: `codex/ion-agent-teams`.

## Purpose and current scope

A resource-preserving renderer needs the exact source documents, stylesheet bases and per-file identities after workspace files have changed or disappeared. The existing portable HTML is an assembled snapshot with imported SVG definitions rewritten into one document. Its private CSS transformations cannot implement the full native variable/cascade semantics of independent SVG documents.

New `save_html_bundle` publications now retain the inspected HTML source and every explicit pinned resource in the publication transaction. This increment implements that storage/read/cleanup prerequisite. The phone continues using its current prepared portable snapshot. Source transport and a native resource-document loader still need implementation before general variable-driven SVG styling can be claimed.

## Stored evidence and read interface

- The existing portable HTML, original export and public artifact hash remain the assembled publication.
- A versioned receipt seal binds new publications to retained-source history. Legacy receipt seals remain supported and explicitly have no source archive. Existing artifacts are not rebuilt or assigned fabricated originals.
- The original HTML and resources retain exact bytes, paths, MIME types, SHA-256 values and publication argument order. The HTML entry is first. Shared CSS data, registrations and primary variable token streams remain authored in these originals.
- `TeamCoordinator::read_html_bundle_sources(expected)` is a worker-only interface. Its trusted caller must authorize the expected artifact through the existing owner/project/task or workflow-evidence grant. No new model tool or unauthenticated network endpoint is introduced.
- One read transaction checks the exact current artifact, original publication evidence, source receipt/version, membership, media types, content lengths, individual hashes and complete byte total. It returns the artifact ID/hash, entry path, aggregate source bytes and ordered source data.
- Missing/corrupted sources for a new receipt cause an explicit source-read error. The verified assembled original remains exportable. Request replay preserves the original publication and cannot repair its archive from workspace files.
- Source bodies do not enter model summaries, checkpoints or handoff context. Existing model reads retain the authored summary and original artifact evidence.

## Bounds, storage and retention

The original HTML is at most 1 MiB. There are at most 32 explicit resources, each at most 2 MiB and at most 8 MiB combined. Additional retained originals therefore occupy at most 9 MiB of logical source bytes per new publication. Length checks precede body allocation on source reads. Existing final portable HTML limits remain unchanged.

Keep-until-explicitly-cleaned remains the default. Active task/team/review/workflow protection covers the parent artifact and its source copies. Owner cleanup clears the stored original and every retained source body for that artifact in one transaction. Publication, path/hash and cleanup receipts remain for identity/replay. Explicit removal of a bundle receipt cascades its archive/source rows, preventing orphaned copies. A source cleanup failure rolls back the original removal and cleanup receipt. Other artifacts' copies are unaffected.

Artifact evidence and the existing cleanup receipt's byte count continue to describe the published portable document. `bytes_total` on the archive interface separately describes retained-original bytes. These logical byte counts do not claim physical SQLite file shrinkage or secure erasure. Source transport/renderer storage presentation must use both values when reporting combined retained content.

## Verification

- [x] **125 native regression cases pass**: 21 binary/document publications, 27 retention cases, 76 portable HTML/SVG cases and the production native worker HTML-bundle flow.
- [x] The **nine new source/cleanup cases** are included in that native total. They prove exact ordered originals after workspace removal/restart, expected-artifact binding, summary-only model reads, corrupted/missing/oversized sources, legacy receipt behavior, publication rollback, actual Stop/report/cleanup/source-read lifecycle, receipt-deletion cascades, active protection and cleanup failure rollback.
- [x] **Eleven explicit native exports pass**. SHA-256 comparisons before and after regeneration prove every existing SVG/CSS/MathML preview fixture is byte-identical. Ignored exports were executed separately; the unrelated ignored PPTX export is not counted.
- [x] **64 phone HTML/CSS/parser/reading cases pass**, zero failures/skips and 2,892 assertions across fourteen files using all eleven fresh exports. The final phone run took 4.30 seconds on this Windows/Bun host.
- [x] Locked offline desktop and both daemon binary checks pass; final native test build took 3m 44s and binary checking took 1m 01s. Existing warnings remain.
- [x] Rustfmt, Prettier and patch hygiene pass.

These are **200 distinct executed regression/export cases**, with repeat executions counted once. No visual/device test, app restart, cloud resource, paid provider call or ion-prod merge is included. The current phone rendering path still uses the byte-identical assembled snapshot.

## Re-run

Use the locked offline low-memory native test profile from chapter 99. Run the `html_source` filter for the nine new cases; run `teams::tests::binary_documents`, `control::artifact_retention`, `teams::documents::html_bundle` and `driver::nebius::team::tests::native_team_html_bundle` for the complete native matrix. Explicitly execute the eleven HTML/SVG/MathML fixture exports using the environment paths in chapters 73–99.

Run all fourteen `apps/mobile/src/lib/html-*.test.ts` files with their fixture/parser/selector environment configured. A skipped fixture case is not passing evidence. The resource originals are exercised through the coordinator's production publication/read/Stop/report/cleanup interfaces; corruption and transaction-failure cases use isolated fixture databases.

## Remaining implementation

- [ ] Add bounded authenticated source transport for direct phone Artifacts and gateway workflow evidence, pinned to the same publication and grant/generation rules.
- [ ] Load separate resource documents through a scoped native resource host, retaining original CSS name/variable/cascade contexts. Refuse unlisted resources, path substitution and external navigation while retaining script/file restrictions.
- [ ] Prove native external SVG use/paint/path and style behavior in supported browser engines, then review iPhone 15/Moto G32 behavior when requested.
- [ ] Include retained originals in the resource renderer's combined storage presentation.
- [ ] Complete the existing release/platform/authenticated owner gates for implemented harnesses. Every unstarted adapter remains deferred.

## Owner review checklist

- [ ] Publish two SVG resources with colliding animation/property/font names and variable-supplied references. Keep the original source files and export the portable document for comparison.
- [ ] Remove or change the original workspace files after publication; the retained-source read must return the exact inspected originals and original argument order.
- [ ] Keep an active review/workflow reference and confirm cleanup is refused. After all holds end, explicitly clean the artifact and confirm both original/source reads are unavailable.
- [ ] Confirm older publications still export and report absent source history without silently rereading workspace files.

This storage increment does not establish phone rendering, browser-origin isolation or actual native device acceptance. [SVG resource document rules](https://www.w3.org/TR/SVG/latest/linking.html), [Android local-content loading](https://developer.android.com/develop/ui/views/layout/webapps/load-local-content) and [Apple custom scheme handling](https://developer.apple.com/documentation/webkit/wkurlschemehandler) inform the next loading seam; their existence does not prove the planned cross-platform loader works.
