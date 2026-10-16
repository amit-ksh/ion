# Portable SVG font palettes and named features

Updated 2026-10-16. Implementation branch: `codex/ion-agent-teams`.

## Implementation

Imported SVG documents now give literal font-palette rule names and named font-feature keys a private per-file identity. Their rule declarations and literal uses follow the same identity through local styles, pinned CSS imports, inline declarations, font presentation values and supported property queries. Registered font-family aliases remain aligned with feature and palette rule family descriptors; installed family names remain available.

Palette descriptors, feature integers, case-sensitive names, feature/variation tags, priorities and document order remain native data. Generic font fallbacks stay generic even when a quoted family has the same spelling. Invalid mixed quoted/unquoted families and malformed palette rule names are preserved without being repaired into a valid alias. Function/string roles are parsed rather than replacing arbitrary matching text. Color-space names in palette interpolation stay separate from palette references.

The browser still selects glyphs and interprets authored font rules. Existing resource inspection and byte/depth limits apply. Original source HTML names remain native. The publication fixture embeds a small synthetic font payload to verify exact resource plumbing; it does not validate real glyph rendering or a color font.

Standards reference: [CSS Fonts Level 4](https://www.w3.org/TR/css-fonts-4/). The implementation and its limits above describe Ion's serializer; browser acceptance remains separate.

## Verification

- [x] All six production-native CSS fixture exports pass, including the new two-document font fixture. The small font payload is synthetic; glyph fidelity is not established.
- [x] 42 phone HTML/CSS cases pass, zero failures/skips, 682 assertions. These include all six native fixture families, independent CSS Tree 3.1.0 rule/property grammar, invalid grammar controls, and existing selector identity checks.
- [x] Exact publication/replay, invalid-resource refusal and native worker document publication/summary/report cases pass.
- [x] Full mobile TypeScript, Rust/Prettier formatting and diff checks pass.
- [x] Complete native HTML bundle suite: 44 pass, zero failures. Its six ignored export cases were each executed explicitly and passed, as listed above.
- [x] Locked offline desktop/both-daemon binary compiler check passes, including the dev daemon. Existing dead-code, ts-rs attribute and future-compatibility warnings remain.

The first runs exposed two fixture expectations: source selectors already receive an owner/import exclusion, and existing CSS/XML serialization normalizes tag quotes. Assertions now check the actual scoped representation and decoded CSS strings. No visual/device test, watcher restart, authenticated account inference or deployment is included.

**95 distinct checks pass at this checkpoint:** 44 native bundle cases, three native publication/worker cases, six explicit native exports and 42 phone cases. Earlier runs and historical checkpoint counts are not added to this total.

## Owner review checklist

- [ ] Open a real portable publication on iPhone 15 and Moto G32 with two imported SVGs sharing family, palette and feature names but different definitions.
- [ ] Check installed and embedded families, ordinary serif/sans-serif fallback, variable fonts and a real supported color font. Confirm authored palette/feature behavior against the original in the same browser engine.
- [ ] Check import order, nested styles, SVG use-shadow behavior, offline direct/gateway access, and document switching/revocation during preparation.
- [ ] Export originals and compare their exact digest. Unsupported browser font features must not change original files or prevent Source/export access.

## Remaining work

Literal palettes and named features are this increment's scope. Variable-supplied animation/counter/font/palette/feature/property names and dynamic `ident()` remain development work. Custom-property token data cannot be globally renamed by spelling because one value can serve several CSS roles. [Unicode family-name matching is now implemented](88-portable-unicode-font-families.md). Physical browser/glyph fidelity still requires owner acceptance.

Harness work remains separate: Cursor/Fx/Kimi ordinary-task Teams bridges and Amp/Cursor bounded completion contracts. Authenticated provider/platform acceptance, Nebius deployment and phone review remain open. This checkpoint does not complete the active full goal.

## Re-run

Run the complete native `teams::documents::html_bundle` library filter and the three publication/worker filters from chapter 80. Explicitly execute `native_font_css_phone_fixture` with `ION_FONT_CSS_FIXTURE` set to a disposable HTML output, alongside the five existing fixture exports. An ignored export is not counted unless executed.

With all six native fixture paths, `ION_CSS_PARSER` and `ION_SELECTOR_ENGINE` configured as in chapters 73-81:

```powershell
bun test apps/mobile/src/lib/html-font-css-native.test.ts apps/mobile/src/lib/html-property-css-native.test.ts apps/mobile/src/lib/html-context-css-native.test.ts apps/mobile/src/lib/html-scope-css-native.test.ts apps/mobile/src/lib/html-named-css-native.test.ts apps/mobile/src/lib/html-sprite-native.test.ts apps/mobile/src/lib/html-rendering.test.ts apps/mobile/src/lib/html-reading.test.ts
```

The independent parser and selector packages are test-only tools; they do not ship with the phone app.
