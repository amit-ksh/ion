# Portable CSS layers and named containers

Updated 2026-10-15. Implementation branch: `codex/ion-agent-teams`.

## Implemented behavior

Imported SVG reference documents now receive private CSS layer paths and query-container names. Two inspected drawings can reuse the same layer and container names while retaining distinct definitions after inlining. Source HTML keeps its original names. This extends the existing ID/class/font/keyframe/counter-style and scope transformations; it does not flatten CSS into inline styles.

Layer order statements, block rules, nested dotted paths and `@import ... layer(name)` modifiers use the same per-file identity. Each path segment retains its nesting relationship. Anonymous layers and import layer keywords stay anonymous; media/support conditions remain native. Invalid rule names and malformed paths retain their invalidity. Escaped identifiers, case and comments keep their roles.

Named `@container` conditions, comma-separated conditions, `container-name` and the name portion of `container` declarations receive the same namespace. Type keywords after the shorthand slash are retained. Nested size/style conditions are preserved. Style queries testing `container` or `container-name` follow the renamed value; arbitrary custom-property values, numbers and general-enclosed functions stay opaque.

All transformation runs during background publication. The existing hash/resource/provenance checks, 16 MiB output bound, CSS nesting bounds and no-network resolver apply. Name validation uses constant auxiliary memory. Phone preparation keeps its current byte limit, no-script CSP, owner/grant checks and cancellation. No app dependency or paid resource is added.

## Verification

- [x] 31 native portable HTML/SVG regressions pass after the final style-query update, including colliding definitions, layer/comment/keyword grammar, nested style queries, malformed paths and nesting limits.
- [x] One actual production-native fixture export and two phone acceptance cases pass: names/imports remain distinct and CSS Tree 3.1.0 independently validates layer/import/container grammar and values. Malformed layer-path and container-name controls are rejected.
- [x] Three native publication/replay/resource-refusal and trusted-worker inspect/publish/summary/result checks pass. All 31 existing phone HTML reading/preparation cases also pass, including source/Unicode/cancellation/resource controls.
- [x] Full mobile TypeScript after adding the new acceptance tests.
- [x] Locked offline desktop and both daemon binary checks (including the dev-binary feature), Rust/TypeScript/Markdown formatting and diff checks. Existing compiler warnings remain.

**68 distinct checks pass** (34 native regression/publication/worker cases, one explicit native export, 33 phone cases). The independent parser exceeded the default five-second budget once during the concurrent native build; its bounded rerun passed in 391 ms. That test now has an explicit 20-second budget for cold parser initialization. Structural/grammar checks do not prove browser cascade/layout or SVG use-shadow behavior. No visual or physical-device test is requested or performed.

## Owner review

- [ ] Publish a document with two pinned SVGs sharing nested layer and container names, including a shared imported stylesheet; confirm each drawing retains its own style.
- [ ] Review explicit layer ordering, nested dotted paths, anonymous layers and imports with media/support conditions.
- [ ] Check named size queries, `container` shorthand, multiple container names and nested style queries on iPhone 15/Moto G32.
- [ ] Compare the accepted artifact, offline export and phone reading through direct Artifacts and gateway evidence access.
- [ ] Change pairing/background or revoke a reading grant during preparation; stale content must disappear.

## Remaining goal work

Subsequent increment: [custom-property identities and registrations](80-portable-css-properties.md) are now implemented and checked through actual native-to-phone output. The following paragraph records the scope still open at this earlier checkpoint.

Names supplied through dynamic CSS variables or `ident()` remain untransformed and need further implementation. CSS variables for animation/counter/font names, `@property` registration, broader font features, other document formats/larger reading paths, native browser/use-shadow acceptance and the remaining harness contracts are still part of the active full goal. This document records an implementation increment, not full completion.

## Re-run native-to-phone checks

Use an existing disposable fixture directory and the isolated CSS parser cache (PowerShell, implementation worktree):

```powershell
$env:ION_CONTEXT_CSS_FIXTURE = "D:/personal/apps/waku/.waku-cache/ion-document-fixtures/native-context-css.html"
cargo test --locked --offline -p waku-core --lib native_context_css_phone_fixture -- --ignored
$env:ION_CSS_PARSER = "D:/personal/apps/waku/.waku-cache/ion-css-parser-3.1.0/node_modules/css-tree/cjs/index.cjs"
bun test apps/mobile/src/lib/html-context-css-native.test.ts
```

The explicit export and parser settings must be supplied: skipped cases are not acceptance. CSS Tree is a test-only dependency and is not loaded by the mobile app.

## Primary references

- [CSS Cascading and Inheritance Level 5: layer names, hierarchy, ordering and imports](https://www.w3.org/TR/css-cascade-5/#layer-names).
- [CSS Conditional Rules Level 5: named containers, shorthand and query grammar](https://www.w3.org/TR/css-conditional-5/#container-rule).

The private per-file transformation is Ion's implementation choice inferred from those semantics. The browser remains responsible for actual cascade and query evaluation.
