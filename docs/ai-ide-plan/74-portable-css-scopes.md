# Portable SVG CSS scopes

Updated 2026-10-15. Implementation branch: `codex/ion-agent-teams`.

## Implemented increment

Imported SVG styles now rewrite both `@scope` root selectors and lower-limit selectors into the reference document's private namespace. IDs, classes, original-value ID/class attribute selectors, `:root`, nested selector functions and zero-specificity document markers use the same transformation as ordinary style rules. The `to` delimiter remains a delimiter; `:scope`, relative selectors and `&` keep their native meaning.

Nested scopes, scopes inside conditional rules and scopes in pinned stylesheet imports retain their browser syntax. Scope roots may be omitted, including `@scope to (...)` and `@scope { ... }`. Style elements stay under their original SVG parent, so implicit roots continue to use that parent. The publication path does not flatten scopes into descendant selectors or simulate proximity in JavaScript. Unknown/invalid prelude tokens are preserved instead of being interpreted as new selectors.

The work runs during background publication. The existing CSS nesting bound (32), resource/hash/provenance checks, expansion limit, exact publication replay and owner approval remain in force. Phone preparation retains its current byte bound, no-script CSP and reading-grant/cancellation checks. No application dependency, setting or paid resource is added.

## Verification

**56 distinct checks pass**: 27 native portable document/publication/worker cases, three explicit production-native fixture exports and 26 phone document cases. Native coverage includes malformed/unpinned/changed resources, exact publication replay after source removal and the trusted-worker inspect/publish/read-summary/report flow. The fixture includes two SVGs with colliding IDs/classes, root/limit selectors, nested/limit-only/implicit scopes and a shared CSS import. It is the actual production-native bundle, then processed by phone preparation. CSS Tree 3.1.0 independently checks scope grammar, including a malformed-selector negative control. CSS Select 5.2.2 independently resolves emitted roots/limits against the resulting DOM, including original-value class selectors and relative `:scope` queries. This validates selector identity and stylesheet ownership; it does not prove browser cascade, layout or SVG use-shadow behavior.

Locked offline desktop/daemon binary checks, full mobile TypeScript, Rust formatting and diff checks pass. Existing compiler warnings remain. No visual or physical-device test was performed.

## Owner review

- [ ] Publish a report containing two pinned SVGs that use the same scope IDs/classes. Confirm each drawing keeps its own fills and nested styling.
- [ ] Review a scope's lower boundary, a nested scope and a conditional scope; confirm excluded descendants retain their expected styling.
- [ ] Review a style element inside a nested SVG group using `@scope { ... }`, plus a limit-only scope and a shared imported stylesheet.
- [ ] Compare accepted artifact, offline export and phone reading on iPhone 15/Moto G32. Check both direct Artifacts and gateway evidence access.
- [ ] Change pairing/background or revoke the reading grant during preparation; stale content must disappear.

## Remaining work

Browser cascade/proximity and native use-shadow acceptance remain open. This is not an `@scope` polyfill for browsers that lack the feature. CSS variable-based animation/counter/font names, `@property` registrations and wider font features remain development work in [CSS name coverage](73-portable-css-names.md). [Literal layer/container namespaces and related style queries](77-portable-css-contexts.md) are now implemented; their dynamic variable/ident forms remain open. Remaining harness contracts are tracked in [the current matrix](64-remaining-harness-integrations.md). The full document/harness goal remains active.

## Re-run native-to-phone checks

Use a disposable existing export directory (PowerShell). Parser paths are test-only packages, not mobile runtime dependencies:

```powershell
$env:ION_SCOPE_CSS_FIXTURE = "D:/personal/apps/waku/.waku-cache/ion-document-fixtures/native-scope-css.html"
cargo test --locked -p waku-core --lib native_scope_css_phone_fixture -- --ignored
$env:ION_CSS_PARSER = "D:/personal/apps/waku/.waku-cache/ion-css-parser-3.1.0/node_modules/css-tree/cjs/index.cjs"
$env:ION_SELECTOR_ENGINE = "D:/personal/apps/waku/node_modules/css-select/lib/index.js"
bun test apps/mobile/src/lib/html-scope-css-native.test.ts
```

## Primary reference

[CSS Cascading and Inheritance Level 6: scoping styles](https://www.w3.org/TR/css-cascade-6/#scoping) defines scope roots/limits, omitted roots, nested scopes and native proximity. The private namespace transformation is Ion's implementation choice inferred from those semantics.
