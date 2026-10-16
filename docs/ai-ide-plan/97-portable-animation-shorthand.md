# Portable animation shorthand names

Updated 2026-10-17. Implementation branch: `codex/ion-agent-teams`.

Subsequent update: [complete animation fallbacks](98-complete-animation-fallbacks.md) adds known-context variable fallback handling while retaining native wrappers. The evidence below records the earlier shorthand increment.

## Change

Imported SVG documents now preserve proven computed keyframe names in animation and vendor animation shorthand. Complete literal identifier expressions follow the earlier keyword slots; fixed author-name prefixes are transformed only when they cannot produce a reserved keyword or dashed timeline name. Native identifier and variable wrappers remain in the output.

The serializer tracks timing, iteration, direction, fill and play slots as free, used or uncertain. An arbitrary substitution can insert commas and other components, so it makes earlier slot evidence uncertain. A valid identifier expression produces only one identifier; it can fill a free keyword slot but cannot undo an already used one. Known nonkeyword names and quoted names still receive their private identity. Unknown keyword roles remain native, and variable fallback token streams are not expanded or guessed.

This also fixes ordinary shorthand where none fills the fill-mode slot before a following keyword becomes an animation name. The existing implementation had treated none as globally reserved without marking that slot used. Each authored comma resets the slot evidence. Longhand handling and opaque custom-property/text data keep their existing roles.

Processing remains in background document preparation. Slot tracking uses constant memory per value and existing output/depth/resource bounds. It adds no render-path I/O, browser script, network or custom-property cascade evaluation.

## Standards and limits

[CSS Animations 1 shorthand](https://www.w3.org/TR/css-animations-1/#animation-shorthand) assigns keywords to unfilled non-name properties before treating them as keyframe names, including the documented none/backwards case. [CSS Values 5 identifiers](https://drafts.csswg.org/css-values-5/#ident) defines computed identifier expressions and their substitution behavior.

[CSS Animations 2](https://drafts.csswg.org/css-animations-2/#animation-shorthand) also introduces duration/timeline roles. Computed auto and dashed identities are preserved as unproven roles here. This increment does not establish complete animation-timeline support, arbitrary variable-supplied names, math-derived role inference or a general cascade. Browser support and actual computed animation behavior remain separate acceptance requirements.

## Verification

- [x] Complete native HTML/SVG bundler suite: **69 pass, zero failures**. All nine default-ignored native exports were each executed explicitly with the final binary.
- [x] Durable exact-original publication/replay and changed/unlisted/private/duplicate/unused-resource refusal: **two pass**.
- [x] Complete HTML reading/rendering/native CSS phone suite: **49 pass, zero failures/skips, 1,264 assertions**, across eleven files using all nine fresh native exports.
- [x] Full mobile and strict standalone fixture TypeScript, Rust/Prettier formatting and Git diff checks pass.
- [x] Locked offline desktop/both-daemon binary checks pass. Existing dead-code, ts-rs serde-attribute and dependency future-compatibility warnings remain.

**129 distinct cases pass:** 69 native bundle cases, nine actual native exports, two durable publication cases and 49 phone cases. Earlier executions are not added. Two direct-helper guards were strengthened to pass the identifier body rather than its outer function; the final native run includes those corrected checks. No production behavior needed changing after the first full run.

Native cases cover all five keyword slots, none fill behavior, unknown substitution/identifier ordering, safe and reserved prefixes, commas/vendor properties/priority, opaque data, invalid/modern roles and two imported documents. The final native build took 3m 40s; the phone suite took 7.24 seconds on this Windows host. These are observed verification times, not phone performance acceptance.

The phone consumes actual production native output through production HTML preparation. Independent CSS Tree 3.1.0 AST/grammar checks distinguish keyframe names from timing/fill keywords, rather than proving only that a value is syntactically valid. A finite test-only identifier evaluator checks fixed suffix cases; unresolved variable expressions remain separate wrapper assertions. The test grammar is not a phone browser engine or a cascade implementation.

## Re-run

Run the native `teams::documents::html_bundle` suite, explicitly execute all nine native CSS/SVG exports including `native_animation_css_phone_fixture`, then run the durable `teams::tests::binary_documents::html_bundle_` cases. Set `ION_ANIMATION_CSS_FIXTURE` to the disposable output used by the phone tests.

Run `apps/mobile/src/lib/html-animation-css-native.test.ts` with that path and `ION_CSS_PARSER` configured. Run the ten existing HTML reading/rendering/native CSS files alongside it, with all nine fixture paths and the independent selector engine configured. Skipped fixture cases are not passing evidence.

## Owner review later

- [ ] Open the same accepted HTML publication on iPhone 15 and Moto G32.
- [ ] Compare animation names, timing, fill mode, list ordering and unsupported-function behavior with the original document.
- [ ] Check native reduce-motion behavior and accessible Reading/Source alternatives with representative documents.
- [ ] Check inherited variables and use-shadow behavior in the real phone WebViews.
- [ ] Confirm the original publication/export content remains exact.

No visual/device test, app/watcher restart, paid provider request or cloud deployment is included. Unstarted harness adapters remain deferred. The full document/harness goal remains active.
