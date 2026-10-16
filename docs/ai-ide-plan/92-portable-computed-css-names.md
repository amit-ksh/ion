# Portable computed CSS names

Updated 2026-10-16. Implementation branch: `codex/ion-agent-teams`.

## Change

Imported SVG documents now keep proven computed-name references aligned with their private definitions. The serializer retains `ident()` and `var()` wrappers, so a browser that does not implement those functions still takes its native invalid/fallback path. It does not flatten new functions to old bare identifiers.

Implemented paths:

- Animation-name longhands, including literal fallback lists and safe fixed-prefix identifier expressions. Animation shorthand stays on the existing path because unknown variables can change keyword roles.
- Container-name values, with reserved keywords and invalid lists preserved. Container shorthand remains separate.
- Font-palette and named font-feature arguments, including fixed dashed prefixes for palettes and fixed nonreserved prefixes for feature names.
- Literal complete font families assembled from identifier expressions, including Unicode caseless matching against collected document fonts. Whole-family literal fallbacks follow the same private identity. Presentation font-family values use the same property grammar.
- Computed custom-property references in transition/will-change values and the first `var()` argument defined by the evolving Variables 2 contract. Unknown suffix functions and referenced variables remain native expressions.

A bounded literal evaluator accepts decoded strings, identifiers and exact plain integer tokens. It refuses unresolved math/functions, noninteger or out-of-range numeric interpretations, empty expressions, comma grammar and other invalid inputs. Fixed-prefix rewriting changes an existing decoded first segment only when it guarantees a nonempty result and rules out all reserved keywords in that role. Dashed roles require a fixed `--` and at least one additional character. The serializer leaves subsequent arbitrary substitutions in place rather than evaluating their values.

Custom-property token data, text strings, unknown installed font families, generic font keywords and the authored ordering of fallbacks remain native. Current CSS-wide `revert-rule` is preserved alongside the existing defaulting keywords, including in literal name parsing; a generated keyword is not turned into a valid private author name. Complete-family matching avoids changing `var(--x, Report) Display` into a different family. Descriptors and at-rule name preludes do not gain computed-name support. The independent native control cases include these distinctions. Existing explicit resource inspection, expansion/depth bounds and exact-original publication/export behavior remain in force. Work stays in background document preparation.

The primary standards findings and counterexamples are recorded in [chapter 91](91-dynamic-css-names-contract.md). The implementation follows the [CSS Values 5 ident algorithm](https://drafts.csswg.org/css-values-5/#ident); browser support and actual computed/glyph behavior remain separate acceptance requirements.

## Verification

- [x] Complete native HTML/SVG bundler suite: **53 pass, zero failures**. Its seven default-ignored export cases were each executed explicitly and passed using the final native binary.
- [x] Exact-original publication/replay and changed/unlisted/private/duplicate/unused-resource refusal: **two pass**.
- [x] Full native-to-phone reading/rendering/CSS suite: **45 pass, zero failures/skips, 871 assertions**, using all seven freshly exported fixtures.
- [x] Full mobile TypeScript, Rust/Prettier formatting and Git diff checks pass.
- [x] Locked offline desktop/both-daemon binary checks pass. Existing dead-code, ts-rs serde-attribute and dependency future-compatibility warnings remain.

**107 distinct cases pass:** 53 native bundle cases, seven actual native fixture exports, two durable publication cases and 45 phone cases. Earlier failed/investigation runs are not added. No synthetic font bytes or AST assertion establishes real glyph/browser fidelity.

The initial native run exposed partial rewriting of an invalid `none other` container list. A validation pass now retains the whole invalid value, and the final complete native run passes. CSS Tree 3.1.0 also exposed its older Variables 1 first-argument parser: the new test explicitly proves that legacy parser rejects `var(ident(...), ...)`, then uses its public `fork` API to remove only the static Value-scope `var` recognizer. The independent generic AST retains all function wrappers; explicit checks verify the computed custom-name shape. The cached package and production parser are not patched, and legacy grammar/selector checks remain active.

A first publication test filter matched zero cases because its module name was wrong. The actual current test names were listed, and both `teams::tests::binary_documents::html_bundle_` cases were then executed and passed. Zero-case output was not counted as verification.

The native fixture imports two SVG documents with the same authored names, separate namespaces and a shared pinned CSS/font resource. Phone tests run the actual native output through production HTML preparation and an independent CSS Tree 3.1.0 AST. Finite identifier evaluation checks fixed suffix cases and generated property grammar; it does not emulate a browser cascade, inherited custom properties or glyph selection.

No visual test, app/watcher restart, paid provider request or cloud deployment is performed.

## Re-run

Use the native `teams::documents::html_bundle` library filter, explicitly execute all seven native exports, then execute `teams::tests::binary_documents::html_bundle_`. The new export is `native_computed_css_phone_fixture` with `ION_COMPUTED_CSS_FIXTURE` set to the disposable output. Chapters 80–88 record the existing six export/parser paths.

```powershell
$env:ION_COMPUTED_CSS_FIXTURE='D:/personal/apps/waku/.waku-cache/ion-document-fixtures/native-computed-css.html'
$env:ION_CSS_PARSER='D:/personal/apps/waku/.waku-cache/ion-css-parser-3.1.0/node_modules/css-tree/cjs/index.cjs'
bun test apps/mobile/src/lib/html-computed-css-native.test.ts
```

Run the eight phone files from the verification command in chapter 85 plus `html-computed-css-native.test.ts`, with all seven paths and the independent selector engine configured. An unconfigured native fixture is skipped and is not a pass. Test-only CSS Tree does not ship in the app.

## Still required

- [ ] General arbitrary variable-supplied names, partial family/shorthand substitution, dynamic counter-style references and unknown installed-font selection.
- [ ] Real computed browser behavior and unsupported-function fallbacks on iPhone 15 and Moto G32, including use-shadow inheritance and real fonts.
- [ ] Complete the active Kimi production Teams integration and acceptance of existing harnesses. The user deferred unstarted Amp/Cursor bounded completion and Fx source-task invocation; those remain outside the current requested implementation.

This is progress on the active document goal, not a claim that every dynamic CSS name or the full application is complete.
