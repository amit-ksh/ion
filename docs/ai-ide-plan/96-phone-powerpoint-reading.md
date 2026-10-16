# Phone PowerPoint document reading

Updated 2026-10-17. Implementation branch: `codex/ion-agent-teams`.

## Available behavior

PPTX now follows the existing AI document path: file publication and ordinary agent-edit collection, artifact library/filtering, team output contracts, workflow document requirements, retention inventory/protection, original export and direct/gateway phone reading. The phone format label is PowerPoint. Agent file-publication tools advertise the format.

The phone uses the existing **Extracted text** view and fidelity note. It retains presentation slide order, explicit title placeholders, text runs/Unicode, line breaks, saved field text, grouped-shape text, table cell locations and image descriptions. Hidden slides remain readable and carry a Hidden label. Empty presentations have an explicit empty state. Source bytes stay unchanged and exportable with the PPTX MIME type.

Selected slide relationships resolve inside the archive. The reader checks the PPTX main-part type, each selected slide type, slide identifiers and unique targets. It supports transitional/strict OOXML namespaces, relocated parts and content-type overrides/defaults. Missing, external, escaping, duplicate or wrong-kind required relationships refuse preview. The archive/XML implementation and cancellation checks are shared with [Word and Excel reading](67-phone-office-reading.md).

The reader has no filesystem/network operation, Office runtime or executable renderer. It does not run formulas, field instructions, macros or embedded media. Native agent document replies retain the sealed publication summary and `content_interpreted: false`; phone extraction does not expand model context.

## Bounds and fidelity

| Resource                                            | Ceiling                           |
| --------------------------------------------------- | --------------------------------- |
| Original                                            | 16 MiB                            |
| ZIP entries / total declared expanded bytes         | 2,048 / 32 MiB                    |
| Selected XML part / total selected XML              | 2 MiB / 8 MiB                     |
| XML nesting / events per part / attributes per tag  | 128 / 100,000 / 64                |
| Slides                                              | 256                               |
| Tables / table rows / cells across the presentation | 1,024 / 10,000 / 20,000           |
| Extracted text / native reading rows                | 512 Ki UTF-16 code units / 12,000 |

ZIP names/header ranges, supported compression, selected-part sizes/CRCs, XML validity and existing owner/foreground/gateway scope checks still apply. Unrelated media is not inflated or decoded. Required parts are validated for content extraction; this is not complete Office package/schema validation. Publication validates the existing ZIP envelope and provenance; the phone separately validates required PPTX parts.

The view reads slide XML content order. It does not reproduce slide layout, master/layout text, fonts, numbering, merged-cell geometry, charts, SmartArt, media, speaker notes, comments, transitions, animations or custom-show ordering. AlternateContent uses base/fallback content; unsupported Choice/extension data is omitted. Explicit title placeholders receive headings; inherited layout semantics are not resolved. Legacy PPT and macro-enabled/template/slideshow formats are not supported by this reader. Original export remains available when extraction refuses or a complete Office presentation is needed.

Preparation yields through the existing ZIP inflation, byte decoding and XML phases. Rendering consumes cached native rows. The bounds and host regressions do not establish iPhone/Android performance or layout acceptance.

## Verification

- [x] **117 phone cases pass, zero failures/skips, 2,423 assertions**, across 17 files. All thirteen PPTX cases, fifteen Word/Excel cases and the existing immutable read/export, larger-document, parser and HTML/CSS cases execute.
- [x] **51 native cases pass**: thirteen binary-publication cases, one explicitly executed PPTX publication/export fixture, ten artifact-library cases, twenty-five retention cases and two workflow/output-contract cases.
- [x] Full mobile TypeScript, strict standalone fixture TypeScript, Rust/Prettier formatting and Git diff checks pass.
- [x] The production Office reader builds for browsers: **27 modules, 107.33 KB**. No Node runtime dependency is added to the reader.
- [x] Locked offline desktop/both-daemon binary checks pass. Existing dead-code, ts-rs serde-attribute and dependency future-compatibility warnings remain.

**168 distinct native/phone cases pass.** Repeated investigations, filtered-out/skipped cases and earlier failed attempts are not added. The final phone run took 107.39 seconds on the Windows host, including full-size HTML/source preparation; that is not device performance acceptance.

A generated ZIP/XML fixture travels through actual native hash inspection, Teams publication, artifact discovery, protocol serialization, original export and restart replay. Its source file is removed after publication to prove durable replay; deleting the publication receipt then blocks further reads. The phone consumes that actual exported response through its production immutable metadata/digest verifier and Office reader. The transport fixture does not establish live pairing or physical-device acceptance.

Initial phone checks caught fixture issues: an extra origin field violated the strict gateway document shape, the expected cancellation wording differed from the existing access error, and the combined boundary test exceeded Bun's default five-second budget on Windows. The final boundary test has a twenty-second test budget; production parsing limits are unchanged. The first combined run also caught regeneration of the source ZIP with new timestamps after native publication. Test ZIP timestamps are now fixed, and source export has a separate, explicit environment variable so verification does not rewrite its immutable input. The final actual native round trip and complete phone gate pass. Failed, skipped and sandbox-refused attempts are not passing evidence. Automatic approval review temporarily blocked a test edit because its usage quota was exhausted; work resumed after the stated reset.

## Re-run

1. Set `ION_PPTX_EXPORT_FIXTURE` to a disposable source PPTX path, then run the presentation-order case in `apps/mobile/src/lib/office-presentation.test.ts` to produce the generated source fixture. Clear the export variable before verification, and set `ION_PPTX_SOURCE_FIXTURE` to that same input path.
2. Set `ION_PPTX_NATIVE_FIXTURE` to a disposable JSON output path, then explicitly execute `native_pptx_phone_fixture --ignored --nocapture` in the current native test binary.
3. Re-run the phone tests with both paths configured. The optional native case must execute, with zero skips.
4. Run the existing Office, artifact, gateway, larger-document and HTML/CSS regressions with their required fixture/parser paths. Run full mobile TypeScript, the browser-target production Office reader bundle and desktop/both-daemon checks.

Example fixture paths in the task cache are `phone-source.pptx` and `native-pptx.json` under `D:/personal/apps/waku/.waku-cache/ion-document-fixtures`.

## Owner review later

- [ ] Update daemon and phone builds together; generate a PPTX from an agent/team and locate it with the PowerPoint artifact filter.
- [ ] On iPhone 15 and Moto G32, open it through direct Artifacts and gateway workflow evidence.
- [ ] Compare slide order, hidden labels, titles, Unicode, saved field text and table cell labels against PowerPoint.
- [ ] Check large accessibility text, screen-reader order, scroll, rotation and return navigation.
- [ ] Export the complete original and compare its SHA-256; repeat after a refused/unsupported preview.
- [ ] Revoke the read grant, switch pairing/route or background the app during preparation and while reading. Prior content must disappear.

No visual/device test, watcher restart, paid provider call, cloud deployment, push or merge into ion-prod is included. Unstarted harness adapters remain out of scope.

## Primary references

- [Microsoft PresentationML structure](https://learn.microsoft.com/en-us/office/open-xml/presentation/structure-of-a-presentationml-document): separate presentation/slide parts and relationships.
- [Microsoft slide text extraction](https://learn.microsoft.com/en-us/office/open-xml/presentation/how-to-get-all-the-text-in-a-slide-in-a-presentation): presentation slide list and DrawingML text.
- [Microsoft Slide schema](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.presentation.slide?view=openxml-3.0.1): slide visibility defaults and slide content.
