# Portable CSS custom properties

Updated 2026-10-16. Implementation branch: `codex/ion-agent-teams`.

Subsequent increment: [larger phone documents](81-phone-large-documents.md) now supports 16 MiB text/HTML input with bounded preparation. Limits/results below describe this earlier checkpoint.

## Implemented behavior

Each imported SVG reference document now has private, case-sensitive custom-property identities. Declarations, `var()` references, `@property` registrations, supported style/support queries, import supports conditions, literal transition/will-change property references and SVG presentation values use the same per-file identity. Two inspected SVGs sharing a stylesheet can reuse names without binding to each other's property registrations. Source HTML retains its authored names.

Values remain native CSS: fallbacks (including empty fallbacks), cycles, inheritance flags, priorities, typed syntax descriptors, case/Unicode distinctions, registration order and invalid grammar are preserved. Arbitrary tokens and quoted text remain data. SVG geometry/transform values that do not need rewriting retain their original bytes; labels and data attributes are not interpreted as CSS. The browser performs substitution, registration validation, computed typing, cascade and animation.

Publication transforms each declaration once. The native-to-phone fixture exposed duplicate renaming through nested selector bodies; the stylesheet traversal now serializes each nested body once and keeps custom-property curly blocks as value data. A fixture with active SMIL was refused by the existing static drawing policy and was corrected without changing that policy.

All work runs during existing background publication. The 16 MiB publication bound, 2 MiB SVG source bound, resource/hash/provenance checks, no-network resolver, CSS depth limits and resource/node limits remain in force. Phone preparation retains its current 1 MiB bound, no-script CSP, cancellation and owner/grant checks.

## Verification

**78 distinct checks pass:**

- [x] 38 native portable HTML/SVG cases, including seven new property identity/publication cases and all existing scope/layer/container/name/resource regressions.
- [x] Three additional durable publication/replay/resource-refusal and trusted-worker inspect/publish/summary/result checks.
- [x] Two explicit exports from the current production-native implementation (properties and layer/container contexts).
- [x] 35 phone reading/preparation/native-fixture checks, with 473 assertions and no skips in that phone run.
- [x] Independent CSS Tree 3.1.0 parsing and registration/descriptor/property-reference grammar, including malformed registration controls. Its value matcher explicitly does not support computed `var()` values; those presentation values are checked through its syntax tree instead. No computed-value or browser-layout claim follows from that check.
- [x] Full mobile TypeScript, locked offline desktop and both daemon binary checks, Rust/TypeScript/Markdown formatting and diff checks. Existing compiler warnings remain.

Five fixture exporters are ignored in the ordinary native suite; the two relevant exporters were explicitly run and are counted above. Skips are not acceptance. No visual, physical-device, authenticated provider or deployment test was performed. The dev watcher was not restarted.

## Owner review

- [ ] Publish two pinned SVGs with colliding custom-property names and a shared stylesheet. Compare the accepted artifact, offline export and phone document through Artifacts and gateway evidence access.
- [ ] Check typed registrations with `inherits` true/false, variable fallbacks/cycles and transitions on iPhone 15/Moto G32; include SVG use-shadow behavior.
- [ ] Check style/support conditions and original page CSS together; source page custom properties must retain their original meaning.
- [ ] Revoke the reading grant, change pairing or background the app during preparation; stale content must disappear.

## Remaining goal work

Dynamic variable-supplied animation, counter-style, font, layer/container and property-reference names, plus `ident()`, need further implementation. Wider font features, additional document formats, larger reading paths, actual browser/use-shadow/device acceptance and remaining native harness integrations/contracts remain part of the active full goal. This checkpoint does not close them.

## Re-run native-to-phone checks

Use an existing disposable fixture directory and the test-only CSS parser cache (PowerShell, implementation worktree):

```powershell
$env:ION_PROPERTY_CSS_FIXTURE = "D:/personal/apps/waku/.waku-cache/ion-document-fixtures/native-property-css.html"
cargo test --locked --offline -p waku-core --lib native_property_css_phone_fixture -- --ignored
$env:ION_CONTEXT_CSS_FIXTURE = "D:/personal/apps/waku/.waku-cache/ion-document-fixtures/native-context-css.html"
cargo test --locked --offline -p waku-core --lib native_context_css_phone_fixture -- --ignored
$env:ION_CSS_PARSER = "D:/personal/apps/waku/.waku-cache/ion-css-parser-3.1.0/node_modules/css-tree/cjs/index.cjs"
bun test apps/mobile/src/lib/html-property-css-native.test.ts apps/mobile/src/lib/html-context-css-native.test.ts apps/mobile/src/lib/html-reading.test.ts apps/mobile/src/lib/html-rendering.test.ts
```

The export paths and parser must be explicitly configured. CSS Tree is a test-only dependency and is not loaded by the app.

## Primary references

- [CSS Custom Properties Level 1](https://www.w3.org/TR/css-variables-1/): case-sensitive identities, native `var()` substitution/fallbacks and dependency cycles.
- [CSS Properties and Values API Level 1](https://www.w3.org/TR/css-properties-values-api-1/): native registrations and descriptors.
- [SVG 2 presentation attributes](https://www.w3.org/TR/SVG2/styling.html#PresentationAttributes): CSS values in supported attributes.
- [CSS Conditional Rules Level 5](https://www.w3.org/TR/css-conditional-5/#style-container): style-query grammar.

The per-file transformation is Ion's implementation choice inferred from those semantics. These references do not establish browser/device acceptance.
