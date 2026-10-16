# Portable computed counter styles

Updated 2026-10-16. Implementation branch: `codex/ion-agent-teams`.

## Change

Imported SVG documents now retain proven computed counter-style references alongside their private definitions:

- Complete literal `ident()` results in `list-style-type`, `list-style` and the style argument of `counter()`/`counters()`.
- Literal `var()` fallback values in those same roles, without selecting or flattening the fallback.
- Fixed author-name prefixes that can reach a collected local definition and cannot produce a built-in style or reserved keyword for any suffix. Subsequent functions and variable references remain native expressions.

Counter names and counter separators keep their own grammar. Opaque custom-property data and strings remain authored data; an explicit `counter()` style argument inside a custom property still follows the counter-style grammar. Descriptors and at-rule names do not gain computed substitution support.

The bounded literal evaluator and function wrappers come from [chapter 92](92-portable-computed-css-names.md). Built-in styles canonicalize ASCII case only when the complete computed result is proven; author names keep their case. Conditional built-in overrides retain a private baseline extending the browser style. An unsafe dynamic prefix such as `lower-` stays native because a suffix might select a built-in style. Prefix lookup uses the ordered collection rather than scanning all definitions.

Processing stays in background document preparation, with existing resource, output and nesting limits. Counter definitions remain bounded to 4,096 names. No network, shell execution or custom-property cascade evaluation is added.

## Standards and limits

[CSS Counter Styles 3](https://www.w3.org/TR/css-counter-styles-3/#the-counter-style-rule) specifies the case rules, immutable built-ins, per-property keyword conflicts and decimal fallback for unknown styles. [CSS Values 5](https://drafts.csswg.org/css-values-5/#ident) defines the identifier expression. These establish the transformation contract; actual phone support and computed rendering require separate device acceptance.

General variable-supplied style names, builtin-selecting dynamic prefixes and unproven arbitrary expressions remain open. No arbitrary cascade or font/glyph fidelity is claimed. The current harness deferral still applies: Amp/Cursor bounded Teams execution and Fx ordinary-task Teams invocation require a later user request.

## Verification

- [x] Complete native HTML/SVG bundler suite: **61 pass, zero failures**. Its eight default-ignored fixture exports were each executed explicitly and passed with the final binary.
- [x] Exact-original publication/replay and changed/unlisted/private/duplicate/unused-resource refusal: **two pass**.
- [x] Complete native-to-phone reading/rendering/CSS suite: **47 pass, zero failures/skips, 1,098 assertions**, using all eight fresh native exports.
- [x] Full mobile and standalone fixture TypeScript, Rust/Prettier formatting and Git diff checks pass.
- [x] Locked offline desktop/both-daemon binary checks pass. Existing dead-code, ts-rs serde-attribute and dependency future-compatibility warnings remain.

**118 distinct cases pass:** 61 native bundle cases, eight actual native fixture exports, two durable publication cases and 47 phone cases. Earlier failed or investigation runs are not added.

The initial native build exhausted memory. The successful final build disables debug symbols for waku-core; it completed in 3m 42s. One earlier retry lost its process handle without a new binary and is not treated as evidence. The final run retained a log and produced the binary used for all exports/publication checks.

Initial phone checks exposed test assumptions about the existing owner-selector isolation suffix, an ambiguous CSS Tree shorthand control and an unavailable forked-parser clone helper. The final tests assert the actual owner exclusion, use unambiguous malformed counter/dimension controls and copy test values with the parser's supported generate/parse APIs. Production transformations did not need changing. A sandbox-refused run executed no cases; it is not counted.

The native fixture imports two SVG documents sharing pinned counter CSS. It includes private literal/computed definitions, integer names, safe and unsafe prefixes, conditional predefined overrides, fallbacks, separate counter-name arguments and opaque data. Phone tests consume actual production native output through production HTML preparation, then an independent CSS Tree 3.1.0 AST.

Finite test-only evaluation checks identifier results and counter/property grammar across six fixed suffixes. It preserves the original AST for wrapper checks, skips unresolved grammar values and does not emulate a browser cascade, inherited custom properties or rendering. Malformed grammar controls must fail. Test-only parser behavior does not ship in the app.

## Re-run

Run the native `teams::documents::html_bundle` suite and `teams::tests::binary_documents::html_bundle_`. Explicitly execute all eight default-ignored native exports, including `native_counter_css_phone_fixture` with `ION_COUNTER_CSS_FIXTURE` set to a disposable output.

```powershell
$env:ION_COUNTER_CSS_FIXTURE='D:/personal/apps/waku/.waku-cache/ion-document-fixtures/native-counter-css.html'
$env:ION_CSS_PARSER='D:/personal/apps/waku/.waku-cache/ion-css-parser-3.1.0/node_modules/css-tree/cjs/index.cjs'
bun test apps/mobile/src/lib/html-counter-css-native.test.ts
```

For the complete phone regression run, use the nine files and seven fixture paths from chapter 92 plus this new file and export. Unconfigured fixture/parser paths cause skipped cases, which are not passing evidence.

## Owner review later

- [ ] Open the same accepted document on iPhone 15 and Moto G32.
- [ ] Compare list markers and generated counter text, including conditional lower-Roman styles, with the original document.
- [ ] Check unsupported-function behavior, fallback ordering and use-shadow inheritance.
- [ ] Verify original publication/export content remains available.

No visual test, app/watcher restart, cloud resource or paid provider request is included.
