# Complete animation fallbacks

Updated 2026-10-17. Implementation branch: `codex/ion-agent-teams`.

## Change

Imported SVG animation shorthand now qualifies keyframe names inside a variable fallback when that variable fills a complete animation or complete comma-separated member. A fallback may itself contain several animations or nested complete variable fallbacks. Literal/computed names use the existing shorthand role parser from [chapter 97](97-portable-animation-shorthand.md).

The serializer leaves the variable wrapper in place. It borrows the exact fallback range and keeps the first argument, outer function, surrounding comments and trailing syntax at their authored positions. The existing custom-property namespace pass still applies afterward. It does not look up variable values, choose the fallback, flatten wrappers or clone producer declarations.

Authored top-level commas separate independent members. cssparser 0.37.0's documented next-token behavior skips an unconsumed nested block, so commas in functions, strings and nested fallbacks do not become member boundaries. The new scans borrow byte ranges instead of cloning function bodies. Recursion stays bounded to 32 levels, with existing output/resource limits and background preparation.

A variable used with surrounding shorthand components retains its partial fallback. Its role can depend on those components or on an earlier substituted value, so it does not receive the complete-value transformation. Opaque custom-property data and content strings also retain their existing roles.

## Standards and limits

[CSS Variables 1](https://www.w3.org/TR/css-variables-1/#using-variables) leaves fallback selection and shorthand parsing to computed-value time. Fallbacks can include commas, and a valid empty value differs from a missing/invalid variable. [CSS Variables 2](https://drafts.csswg.org/css-variables-2/#using-variables) adds computed first arguments; this pass preserves that argument and its existing evaluation path while rewriting only the complete fallback.

This is a transformation of the fallback's known context. Arbitrary primary variable values, partial shorthand fragments, general producer/cascade interpretation, computed-name dependencies beyond the existing namespace pass, modern timeline roles and actual phone browser behavior remain open. The code does not claim that all variable-driven animations are now isolated.

## Verification

- [x] Complete native HTML/SVG bundler suite: **76 pass, zero failures**. Its ten default-ignored CSS/SVG exports were each executed explicitly with the final native binary.
- [x] Exact-original publication/replay and changed/unlisted/private/duplicate/unused-resource refusal: **two pass**.
- [x] Complete HTML reading/rendering/native CSS phone suite: **51 pass, zero failures/skips, 1,386 assertions**, across twelve files using all ten fresh native exports.
- [x] Full mobile and strict standalone fixture TypeScript, Rust/Prettier formatting and Git diff checks pass.
- [x] Locked offline desktop/both-daemon binary checks pass. Existing dead-code, ts-rs serde-attribute and dependency future-compatibility warnings remain.

**139 distinct cases pass:** 76 native bundle cases, ten actual native exports, two durable publication cases and 51 phone cases. Earlier and repeated executions are not added. The native build took 4m 23s, the phone suite took 9.72 seconds and the desktop/daemon check took 56.23 seconds on this host; these are not phone performance acceptance.

Native cases cover complete/nested/list fallbacks, partial and opaque refusal, raw first-argument/wrapper preservation, empty/default/quoted-comma values, implicit closing, priority and the 32-level bound. The production publication fixture imports two documents with identical keyframe names and a shared pinned stylesheet, plus a separate owner animation.

Phone checks consume actual native output through production HTML preparation. An independent CSS Tree 3.1.0 AST retains wrapper/argument assertions. A finite test-only model uses missing, present and valid-empty inputs, then inspects animation keyframe roles with the independent grammar. It does not implement inheritance, registrations, cycles, the complete cascade or actual browser rendering. Partial-fragment cases remain explicit structural controls.

## Re-run

Run the complete native `teams::documents::html_bundle` suite and explicitly execute all ten native CSS/SVG exports, including `native_animation_fallback_phone_fixture` with `ION_ANIMATION_FALLBACK_FIXTURE` set to its disposable output. Run the two durable `teams::tests::binary_documents::html_bundle_` cases.

Run `apps/mobile/src/lib/html-animation-fallback-native.test.ts` with the output path and `ION_CSS_PARSER` configured. Include the eleven existing HTML reading/rendering/native CSS files, all ten native fixture paths and the independent selector engine for the complete phone gate. Skipped cases are not passing evidence.

## Owner review later

- [ ] On iPhone 15 and Moto G32, compare missing-variable fallback animations with the original accepted HTML publication.
- [ ] Check nested fallbacks, multiple animations, quoted names and keyword names after timing/fill values.
- [ ] Check present values, valid empty values and cycles; the fallback must remain browser-selected.
- [ ] Verify partial fragments, inherited variables and unsupported computed first arguments with representative documents.
- [ ] Check accessible Reading/Source alternatives and exact original export.

No visual/device test, watcher restart, paid provider call or cloud resource is included. Unstarted harness adapters remain out of scope, and the full document/harness goal remains active.
