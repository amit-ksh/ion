# Phone Office document reading

Updated 2026-10-14. Implementation branch: `codex/ion-agent-teams`.

Subsequent update: [PowerPoint reading](96-phone-powerpoint-reading.md) adds PPTX publication/original export and the same bounded direct/gateway extracted-text flow. The Word/Excel behavior and historical evidence below remain applicable.

## Available behavior

Direct Artifacts and gateway workflow evidence now read verified **DOCX** and **XLSX** originals on the phone. The existing original metadata/SHA-256 checks run before extraction. The original bytes remain unchanged and exportable.

- Word: body paragraphs, Heading 1–6 paragraph styles, literal text/Unicode, tabs/breaks, inserted text and table cell locations. Deleted text, moved-from text and field instructions are omitted. Images with a Word drawing description have a text label.
- Excel: sheet order/names, hidden-sheet labels, shared/rich/inline strings, sparse cell addresses, booleans, errors, stored numbers/dates and formula expressions. A formula cell shows its saved result, or an explicit unavailable/empty state. Shared/array formula cells without their own expression are identified. Formula execution and external workbook resolution are absent.
- Extracted text uses cached, virtualized native reading rows and existing typography. It does not become authored HTML or an executable link. The UI labels it **Extracted text** and states the fidelity limits. Office binaries are never presented as HTML Source.
- Direct preview failure retains original export. Gateway preview failure returns to the evidence list with the error and its existing original export action. Gateway Office reading also offers original export beside the fidelity note.

## Parsing and access bounds

The reader operates only on bytes already returned by the trusted direct/gateway read. It has no filesystem or network operation. It does not run Office macros, formulas, field instructions, embedded objects or remote resources.

The ZIP directory is indexed before selected XML parts are inflated. Central/local names, flags, sizes, descriptors and nonoverlapping payload ranges must agree. Selected parts must match their declared size and CRC. This is selected-content extraction, not complete Office package validation: unrelated images/media are not decompressed or interpreted.

| Resource | Preview ceiling |
| --- | --- |
| Original | 16 MiB |
| ZIP entries / total declared expanded bytes | 2,048 / 32 MiB |
| Selected XML part / total selected XML | 2 MiB / 8 MiB |
| XML nesting / events per part / attributes per tag | 128 / 100,000 / 64 |
| Workbook sheets / worksheet rows / cells | 32 / 10,000 / 20,000, across the workbook |
| Extracted text / native reading rows | 512 Ki UTF-16 code units / 12,000 |

Encrypted/unsupported ZIP entries, ZIP64 sentinels, unsafe/ambiguous names, links, overlaps, changed checksums, external required relationships, wrong format content types, DTDs/entities, malformed XML and excessive structures refuse preview. UTF-8 XML and standard/strict OOXML document namespaces are supported; UTF-16 XML, legacy DOC/XLS and macro-enabled document types are not supported.

Inflation uses 256-byte compressed pushes with bounded output accounting, followed by yielding. Byte decoding and XML parsing also yield between chunks. The existing owner, pairing, connection, foreground, route generation, evidence grant and credential-expiry checks fence every phase. Cancellation discards derived rows; rendering reads prepared state only.

## Fidelity limits

This is a content reader. It does not reproduce Word page layout, fonts, numbering, headers/footers, footnotes/endnotes, comments, complete revision history or embedded media. Excel styles, number/date formatting, charts, images, comments, layout, formula recalculation and dynamic/external data are not reproduced. Cell values use their stored representation. Export is the route to the complete original in an Office-compatible app.

Native agent `read_document` replies retain the existing explicit publication summary and `content_interpreted: false` for these file originals. Phone extraction does not silently expand the model context or claim that a model interpreted the binary.

## Verification

Fifteen new regression cases pass: real stored/deflated/streaming ZIP archives, Word text and table ordering, namespace variants, literal markup, large Unicode text, spreadsheet order/hidden/sparse/shared/formula data, cached empty results, malformed/unsafe packages, CRC/header changes, expansion limits and scope cancellation during indexing, inflation and XML parsing. The full mobile TypeScript check and a browser-target parser bundle also pass. The combined Office/immutable artifact/export/HTML/image/direct/gateway PDF run passes **92 distinct cases** (4,389 assertions), including the actual two-page 12 MiB HTTP transfer fixture.

No visual test or physical-device acceptance was run, following the repository's user instruction. Desktop Office layout rendering and the full remaining harness/resource scope are not marked complete by this increment.

## Owner review

- [ ] On iPhone 15 and Moto G32, read DOCX and XLSX publications in both direct Artifacts and gateway workflow evidence.
- [ ] Check larger accessibility text, screen-reader order, long cells, Unicode, table cell labels, hidden sheets and return navigation.
- [ ] Compare formula/cached-result labels with Excel; verify stale results are not presented as recalculated values.
- [ ] Export from the reader and compare the complete original hash; repeat after a preview limit/error.
- [ ] Change owner, pairing/read grant or foreground while preparing and reading; old content must disappear.
- [ ] Review fidelity notes with representative Word and Excel files. Export complex/unsupported documents in the external document app.

## Primary references

- [fflate streaming decompression](https://github.com/101arrowz/fflate), pinned at 0.8.3 through its browser entry.
- [saxes strict streaming XML](https://github.com/lddubeau/saxes), pinned at 6.0.0.
- [Microsoft SpreadsheetML sheets](https://learn.microsoft.com/en-us/office/open-xml/spreadsheet/working-with-sheets) and [stored cell values](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.spreadsheet.cellvalue). A formula's stored cell value is its last calculated result.
