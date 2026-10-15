# Portable document CSS names

Updated 2026-10-15. Implementation branch: `codex/ion-agent-teams`.

## Implemented increment

Inlined SVG reference documents now give animation/keyframe names and explicitly declared custom counter-style names a private per-file identity. Two inspected SVGs may use the same names without one definition replacing the other. Inline styles, nested media/support rules and hash-pinned stylesheet imports use the same namespace. Source HTML keeps its original names.

The transformer parses CSS tokens and property roles. Animation names retain case, escaped and quoted names; shorthand timing/iteration/direction/fill/play keywords keep their meaning. Keyframe selectors (`from`, `to`, percentages) are not element selectors. `!important` stays outside name interpretation. The existing font-family/shorthand aliasing also runs within keyframe declarations.

Counter style references follow `list-style-type`, `list-style`, `counter()`/`counters()` style arguments, `system: extends`, `fallback` and `speak-as`. Literal symbol/separator/content strings are preserved. Reserved rule names stay invalid, standard built-in names retain their normalization, and declared built-in overrides receive a baseline alias before conditional rules. Running counter names themselves are retained.

All discovery and rewriting runs in the existing background publication path. Source/resource manifests, hash checks, artifact provenance, generated snapshot replay, no-network resolution and manual retention remain unchanged. The existing nesting/resource/byte/output bounds apply; a reference document additionally caps discovered custom counter-style names at 4096. Phone rendering retains its 1 MiB bound and existing CSP, cancellation and owner/grant checks.

## Verification

**50 distinct checks pass**: 24 native portable HTML/SVG publication/worker cases, two explicit production-native fixture exports and 24 phone preparation/grammar cases. Coverage includes colliding names across two sprite documents and shared imports, shorthand keywords/strings/case/escapes/priority, invalid/reserved names, conditional built-in defaults, original source styles and unknown local animation references. The native trusted-worker inspection/publication/summary/result flow and exact publication replay also pass.

Actual production-native HTML goes through phone preparation before independent CSS Tree **3.1.0** grammar validation. A malformed `counters(section)` control must fail; standard `counters(section,".",decimal)` and the scoped custom style must pass. The installed older 1.1.3 parser rejected the standard value too, so this verification uses an isolated integrity-checked parser cache. No application package dependency was added. SHA-256 archives: CSS Tree `9d87a606c7ce6cff51a8c546a0a601b83c0a7017988fceec9f21a4d382d59815`, MDN Data 2.12.2 `dd2106eddb30be3c55761e864010a2db3a17cea211e97b9884814806c4027490`, Source Map JS 1.2.1 `f126a6f9fca487a43219d8cb8c3a955279187a966119d548eb5cd47e999d4853`. npm SHA-512 integrity was verified before extraction.

Locked offline desktop/daemon binary checks, full mobile TypeScript, Rust formatting and diff checks pass. Markdown/test formatting is verified before commit. Existing Rust warnings remain. No visual or physical-device acceptance has run.

## Owner review

- [ ] Publish a report referencing two SVGs with identical keyframe and marker-style names, including a shared CSS import; verify each keeps its own style.
- [ ] Check literal animation shorthand/name values, escaped/quoted names, keyframe opacity/font changes and reduced-motion media rules on iPhone 15/Moto G32.
- [ ] Review list marker strings, counter formatting/fallback, case-sensitive custom names and a conditional override of a predefined counter style.
- [ ] Export and open offline; compare the accepted artifact and confirm original workspace files are unchanged.
- [ ] Repeat direct Artifacts and gateway evidence access, then revoke the reading grant or change pairing/background during preparation; stale content must disappear.

## Remaining full-goal work

This increment covers direct/literal named CSS references. Names supplied through CSS variables (including variable fallbacks and inherited dynamic values) need additional rewriting before imported SVGs can claim equivalent behavior for those forms. `@property` registrations, font variables/features and cascade layer/container names also remain open. [Scope-rule root/limit selectors](74-portable-css-scopes.md) are now implemented; browser scope/use-shadow acceptance remains open. Browser-specific use-shadow styling, predefined counter overrides interacting with owner-defined global styles, and physical-device acceptance remain unverified. Source HTML's own authored CSS continues to use the browser directly.

Remaining native harness bridges/contracts and owner-account acceptance are tracked in [current harness coverage](64-remaining-harness-integrations.md). This is an implementation increment, not completion of the active document/harness goal.

## Primary references

- [CSS Animations Level 1: keyframe names and shorthand interpretation](https://www.w3.org/TR/css-animations-1/)
- [CSS Counter Styles Level 3: names, predefined styles and descriptors](https://www.w3.org/TR/css-counter-styles-3/)
- [CSS Lists and Counters Level 3: list shorthand and counter functions](https://www.w3.org/TR/css-lists-3/)

The file namespace and token transformation are Ion implementation choices inferred from these rules. Structural and grammar tests do not prove native browser pixel/layout equivalence.

## Re-run the native-to-phone fixture

Use an absolute fixture path inside a disposable test directory and an installed CSS Tree 3.x module path (PowerShell):

```powershell
$env:ION_NAMED_CSS_FIXTURE = "D:/personal/apps/waku/.waku-cache/ion-document-fixtures/native-named-css.html"
cargo test --locked -p waku-core --lib native_named_css_phone_fixture -- --ignored
$env:ION_CSS_PARSER = "D:/personal/apps/waku/.waku-cache/ion-css-parser-3.1.0/node_modules/css-tree/cjs/index.cjs"
bun test apps/mobile/src/lib/html-named-css-native.test.ts
```

The export directory must already exist. The parser is a test-only dependency, supplied explicitly by the environment; it is not loaded by the mobile application.
