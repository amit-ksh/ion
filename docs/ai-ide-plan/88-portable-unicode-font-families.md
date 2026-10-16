# Portable Unicode font-family matching

Updated 2026-10-16. Implementation branch: `codex/ion-agent-teams`.

## Change

Imported SVG font families now use Unicode caseless matching instead of ASCII-only lowercasing when creating their private document identities. Both declaration discovery and family references share the same parser and folded key. ASCII keys remain stable.

The implementation uses a generated Unicode 18.0.0 C/F table from the [versioned official data](https://www.unicode.org/Public/18.0.0/ucd/CaseFolding.txt), pinned by SHA-256 `a004797658a457bec4dc11683e39f69249ea3b595b752dbea6721c4c9f587b0d`, with the source data and Unicode License V3 preserved beside it. The table contains 1580 non-ASCII mappings and retains the existing ASCII fast path. A scoped Git attribute preserves the source bytes even under Windows automatic line-ending conversion; its checkout-filter SHA-256 is verified. No dependency is added. It performs no locale-specific tailoring or Unicode normalization. CSS requires this comparison behavior; visually similar composed/decomposed sequences remain distinct. [CSS Fonts 3, family-name case sensitivity](https://www.w3.org/TR/css-fonts-3/#font-family-casing).

Generic keywords and CSS-wide keywords keep their previous meaning. Quoted family names still denote literal families. User-visible text, unrelated string literals, font bytes and exact original document exports are not rewritten by this change.

The first regression run caught incorrect three-character expansion order in cached `unicase` 2.9.0 (`ﬃ` became `iff`). That approach was removed rather than weakening the regression or patching a shared dependency cache. The replacement conformance test checks every Unicode scalar against the pinned C/F data, plus idempotence.

## Verification scope

- [x] Full native HTML bundle filter: **46 pass, zero failures**, including Latin expansions, Greek final sigma, Cyrillic, Kelvin sign, ligatures, supplementary Deseret, dotted/dotless I and normalization controls.
- [x] Actual native export of all six existing publication fixtures, including an expanded font fixture with Unicode definitions and references in two imported documents.
- [x] Phone preparation keeps each document's folded font identities private, preserves text and differentiates composed/decomposed family names.
- [x] Independent CSS Tree grammar verifies the resulting font rules/descriptors/properties.
- [x] Full mobile TypeScript check passes.
- [x] Locked offline desktop/both-daemon compiler checks pass. Existing compiler warnings remain.

**97 distinct cases pass at this checkpoint:** 46 native bundle cases, six explicitly executed native fixture exports, two exact publication/replay/resource-refusal cases, and 43 phone cases (732 assertions, zero failures/skips). One native conformance case checks all 1,112,064 Unicode scalars and idempotence against the pinned data. The initial phone grammar expectation was updated from three to five palette rules because both imported documents now carry an additional Unicode palette; rule/descriptor/property validation and invalid controls remain enforced. Earlier runs are not added to these counts.

The font bytes in the synthetic fixture establish publication/identity transport, not glyph fidelity. No visual test, watcher restart, device acceptance, paid inference or cloud deployment is included.

## Owner review

- [ ] Open a real color/variable-font publication on iPhone 15 and Moto G32. Compare case variants, Greek sigma, accented families and font fallback with the original in the same engine.
- [ ] Verify two imported documents with the same family names keep separate fonts. Check real embedded font glyphs, palette/feature support and SVG use-shadow behavior.
- [ ] Export originals and compare their digests; unsupported browser font features must not disable Source/export access.

## Still open

Variable-supplied animation/counter/font/palette/feature/property names and dynamic `ident()` require further implementation. Custom-property token data cannot be globally renamed by spelling: the same value can be used in different CSS roles. Browser/glyph and physical-device acceptance remains separate.

Harness coverage is 24/28 source-level paths after [the Windows Cursor source bridge](87-cursor-source-team-bridge.md). Amp/Cursor completion contracts, Fx/Kimi source bridges, additional worker-platform contracts and authenticated owner acceptance remain open. This increment does not complete the active goal.
