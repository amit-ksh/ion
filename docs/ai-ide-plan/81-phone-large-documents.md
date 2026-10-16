# Larger phone document reading and rendering

Updated 2026-10-16. Implementation branch: `codex/ion-agent-teams`.

## Available behavior

Direct Artifacts and gateway workflow evidence now prepare supported UTF-8 text originals up to the existing **16 MiB publication limit**. HTML/SVG rendered input increases from 1 MiB to 16 MiB; other text source increases from 64 KiB to 16 MiB. The full original is verified before preparation and remains unchanged/exportable.

Supported text formats are Markdown, TXT, CSV, JSON, HTML/HTM, SVG, XML and YAML/YML. Existing PDF, PNG, DOCX and XLSX readers/export remain available. The previously added formats and original publication paths are recorded in [the document guide](63-phone-rendered-documents.md) and [Office reading guide](67-phone-office-reading.md).

Strict UTF-8 decoding and complete Source preparation run in bounded batches without requiring native TextDecoder/Buffer polyfills. Source rows retain Unicode scalar boundaries and are cached for virtualized rendering. Semantic Markdown/JSON/HTML reading retains its existing small-input bounds; a larger text document uses complete Source with an explicit note. HTML/SVG can use Rendered for authored styling. This does not claim full Office layout reproduction or semantic Markdown rendering beyond the original reading bound.

Preparation checks the owner/foreground/route or gateway grant/expiry generation at each checkpoint, after each native timer and before returning results. A shared pacer yields after a target four milliseconds or 65,536 counted work units, whichever comes first. HTML input pieces contain at most 8,192 UTF-16 code units; UTF-8 decoding checkpoints complete a scalar at an approximately 8 KiB byte boundary. Large serialization values are split into bounded pieces too. This is a cooperative scheduling policy, not a guaranteed frame duration on devices. Cancellation stops preparation without caching a stale result.

## Rendered HTML bounds and memory fix

HTML5 tree construction still uses pinned **parse5 8.0.0**. A typed subclass bounds its character-token accumulation, and the tree adapter stores adjacent text in bounded pieces. Long attributes/comments accumulate piece arrays and join only when their token is complete. Tags/attribute names, per-element attributes, node count and depth retain explicit complexity limits. The parser uses protected extension points from the pinned version; any dependency upgrade must pass the differential regressions.

The full-size test originally failed with Out of memory. A minimal 4 MiB paragraph used about **1.53 GB RSS** with the initial implementation. Parser-only probes separated character accumulation from subsequent rope flattening. The final same-sized complete rendered document used **257.7 MB RSS**, returned exact text, and allowed 438 event-loop heartbeat ticks during preparation. That final Windows/Bun sample took 6.91 seconds with a 30.88 ms maximum timer gap. These are diagnostic host measurements, not Android/iOS memory or frame-time acceptance; the final sample's elapsed time was higher than the initial 5.76-second sample while substantially reducing memory.

A second focused regression caught text loss when input compaction invalidated an unfinished entity decoder's offset. Compaction now occurs only after completed non-entity tokenizer steps, preserving pending lookahead. Exact entity text is checked across the buffer waterline in addition to stock-parser comparisons across one-byte and larger input boundaries. The HTML5 parser remains responsible for malformed markup, foreign content and table repair.

Serialization yields within large text/attribute values and caps the **prepared output at 32 MiB**. Input that exceeds complexity/expansion bounds keeps complete Source and original export. The seven-MiB ampersand fixture now reaches the actual expansion refusal without losing authored text. Native scripts, file/network access, message bridges, CSP, navigation restrictions and reading grants retain their previous behavior. Source/Reading/Rendered remain the existing product controls.

## Verification

- [x] **97 phone regressions pass, 0 failed/skipped, 1,851 assertions** across 13 files.
- [x] Exact 16 MiB direct immutable read, full digest, batched decoding, complete Source recovery and event-loop yielding.
- [x] Larger gateway chunk/digest read and the same preparation path; invalid/changed evidence and revoked scope still refuse.
- [x] Full publication-sized 16 MiB styled HTML, 32 MiB prepared-output refusal, large attributes, malformed UTF-8, supplementary Unicode and cancellation.
- [x] Four parser cases cover stock HTML5 differential behavior across 95 input/batch combinations, bounded four-MiB text nodes, complexity refusal and exact entities across compaction. These are included in the 97 cases, not additional counts.
- [x] All five actual production-native HTML/CSS fixture families survive phone preparation. Independent CSS Tree/selector checks and all 15 Office extraction cases pass with the shared decoder.
- [x] Three explicit current-native fixture exports were rerun for scope/named/sprite families; property/context exports from the preceding verified checkpoint are also used. Skipped fixture exports are not counted as acceptance.
- [x] Full mobile TypeScript, browser-target document bundle (215 modules, 619.53 KB), formatting and diff checks pass.

The final combined run took 116.21 seconds. Its full-size HTML case took 40.33 seconds and direct 16 MiB reading took 18.14 seconds under that run's host conditions. They establish complete preparation within explicit test budgets; they do not establish device speed. No visual/physical-device check, paid inference, deployment, watcher restart or ion-prod merge occurred. Native publication limits and Rust code are unchanged by this increment; their preceding verification is in chapter 80.

## Owner review checklist

- [ ] Update native phone builds and daemon/client together, then open larger HTML/SVG, TXT/Markdown and JSON/CSV/XML/YAML through direct Artifacts and gateway evidence on iPhone 15/Moto G32.
- [ ] Check first-open time, memory pressure, scroll, rotation, app switching, accessibility text and return navigation. Include embedded styles, fonts and images; unsupported complexity must retain complete Source/export.
- [ ] Compare exact original SHA-256 after export, including a document at 16 MiB and supplementary Unicode at source-row boundaries.
- [ ] Change pairing/route, expire or revoke the evidence grant, and background the app during decode, parsing and serialization. Old content must disappear.
- [ ] Confirm scripts, remote/file requests and external navigation remain inactive in the actual native WebView.

## Remaining implementation and acceptance

The larger phone reading gap is closed at the implementation/regression level. The active full goal still includes dynamic CSS names/`ident()` and wider font fidelity, plus these five native harness integration paths:

- [ ] Cursor ordinary-task Teams tools.
- [ ] Fx ordinary-task Teams tools.
- [ ] Kimi ordinary-task Teams tools.
- [ ] Amp bounded Teams completion allowance/account/metering contract.
- [ ] Cursor bounded Teams completion ceiling/account contract.

[Current harness coverage](64-remaining-harness-integrations.md) is **23 implemented paths out of 28** (12/14 bounded completions and 11/14 source-task bridges), about 82% by path count. This is not an effort-weighted percentage for the entire app. Real authenticated provider/platform and phone browser/use-shadow/device acceptance remain open. No goal-complete claim follows from this checkpoint.

## Re-run

From the implementation worktree, with the existing native fixture/parser environment configured as in chapters 73-80:

```powershell
bun test apps/mobile/src/lib/large-documents.test.ts apps/mobile/src/lib/html-parser.test.ts apps/mobile/src/lib/html-rendering.test.ts apps/mobile/src/lib/html-reading.test.ts apps/mobile/src/lib/artifact-library.test.ts apps/mobile/src/lib/gateway-workflow-document.test.ts apps/mobile/src/lib/document-preview.test.ts apps/mobile/src/lib/office-reading.test.ts apps/mobile/src/lib/html-property-css-native.test.ts apps/mobile/src/lib/html-context-css-native.test.ts apps/mobile/src/lib/html-scope-css-native.test.ts apps/mobile/src/lib/html-named-css-native.test.ts apps/mobile/src/lib/html-sprite-native.test.ts
```

The native fixture environment must be supplied to avoid skips. The parser/selector engines are test-only dependencies and do not ship with the phone app.
