# Phone equation rendering

Updated 2026-10-17. Implementation branch: `codex/ion-agent-teams`.

## Available behavior

The shared HTML Rendered view now preserves presentation MathML in its native namespace. Fractions, radicals, operators, scripts and limits, matrices, tensor indices, spacing and semantic wrappers reach the phone WebView or web iframe alongside their authored HTML/CSS. Browser-native layout handles equations; no equation library, remote dependency or script execution is introduced. This extends embedded math in HTML, rather than adding a standalone MML artifact format.

Math attributes, CSS math properties, Unicode/entity text, author labels and hidden spacing remain authored. Interactive `maction` elements become static `mrow` elements containing their children. Content MathML, unknown elements and legacy external glyph resources are excluded. The existing Reading view still says that Math is not displayed; complete Source and verified original export remain available. It does not flatten a fraction into misleading plain text.

Text annotations accept `text/plain`, `application/x-tex` and `application/x-latex` without element children. XML annotations accept HTML/XHTML and presentation MathML encodings. Their descendants pass through the same namespace, active-content and attribute preparation as the surrounding document. Annotations with absent/unknown encodings or any href are removed. These media types are an allowlist, not a declaration that author content is trusted. TeX annotations remain inert text and are not evaluated.

## Isolation and preparation

Script, network, frame, object, form and file restrictions retain their existing settings. Legacy MathML resource attributes are removed. HTML5 table repair can move an HTML `mglyph` or `malignmark` into a MathML text integration point; a subsequent browser parse can then assign a different namespace. Both names are excluded across namespaces, including their descendant subtrees. Repeated preparation/reparse regressions cover this case and nested MathML/HTML/SVG integration.

The existing 16 MiB UTF-8 input, 32 MiB prepared output, node/depth/attribute bounds and cooperative generation checks still apply. Preparation remains shared by direct Artifacts and gateway workflow evidence. The document receives no app credential, daemon URL or message bridge. Native publication still stores the assembled HTML and its sealed source/resource receipt; it does not retain a separately addressable resource document tree.

The implementation follows the native presentation model in [MathML Core](https://www.w3.org/TR/mathml-core/) and uses the [Math Working Group's sanitizer guidance](https://w3c.github.io/mathml-docs/mathml-safe-list) for static actions and annotation handling. HTML5 namespace/integration behavior follows the [HTML parsing standard](https://html.spec.whatwg.org/multipage/parsing.html#html-integration-point).

## Verification

- [x] **76 native HTML/SVG bundle regressions pass**, including the existing parser, resources and private CSS identities.
- [x] **Four native publication/worker cases pass**: exact durable HTML replay, changed/unlisted/private resource refusal, native worker inspect/publish/summary/result flow and the new equation publication/replay case.
- [x] **Eleven native fixture exports execute and pass** on the final executable: ten existing SVG/CSS families and the actual stored equation publication. Ignored exports were explicitly run, not counted as passes from the default suite.
- [x] **64 phone cases pass, zero failures/skips, 2,892 assertions**, across fourteen HTML/CSS/parser/reading files using all eleven fresh native exports. The combined run took 6.28 seconds on this Windows/Bun host.
- [x] Complete mobile TypeScript and strict fixture TypeScript pass.
- [x] Rustfmt, Prettier and patch hygiene pass.

These are 155 distinct executed regression/export cases. They prove preparation, namespace/attribute controls, exact source retention and the actual publication-to-phone handoff. They do not prove browser math layout, glyph metrics, performance on a phone or assistive-technology behavior. The native test build took 4m 36s with the existing low-memory profile. Native production Rust code and protocol types are unchanged; the new native code is a publication fixture. No app restart, visual/device test, paid call or cloud deployment was performed.

## Owner review checklist

- [ ] Open a technical HTML report on iPhone 15 and Moto G32 through both direct Artifacts and gateway evidence. Check fractions, radicals, limits, matrices and tensor indices.
- [ ] Check available math fonts, glyph fallback, inline/block equations, narrow-screen overflow, zoom, rotation and large-text settings. Native engine/font behavior is not established by parser tests.
- [ ] Check accessible author labels and the exact Source/export alternative. Reading must identify unsupported math instead of presenting a flattened equation as equivalent.
- [ ] Confirm unknown/external annotations and glyph resources stay inactive; scripts and remote/file requests must remain blocked in actual native builds.
- [ ] Change pairing, foreground state and evidence grants while preparation runs. Old content must disappear as with other HTML.

## Remaining full-goal work

This closes the identified MathML-removal gap at the preparation/publication regression level. It does not prove phone layout, glyph selection or screen-reader behavior. General variable-supplied CSS names in flattened external SVG documents, complete cascade/timeline fidelity and actual browser/device acceptance remain open. Kimi signed release/platform/authenticated owner acceptance remains separate. Every unstarted harness adapter stays deferred by the user's latest instruction.

## Re-run

Build the locked native test executable with the existing low-memory profile, then run `html_bundle_` and the ignored `native_mathml_phone_fixture` with `ION_MATHML_NATIVE_FIXTURE` set to an output JSON path. That fixture publishes through the actual coordinator, verifies the stored evidence and summary, preserves original files, then removes them and reads the identical durable replay. It exports the actual publication plus SHA-256 for phone preparation.

Run `bun test apps/mobile/src/lib/html-mathml.test.ts` with that JSON path in `ION_MATHML_NATIVE_FIXTURE`, alongside the existing HTML/native CSS suite and its ten fixture paths. The fixture case must execute; a skip is not acceptance. Full mobile TypeScript and formatting/diff checks are also required.
