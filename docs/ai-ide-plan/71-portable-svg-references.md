# Pinned SVG reference documents

Updated 2026-10-15. Implementation branch: `codex/ion-agent-teams`.

## Publication behavior

`save_html_bundle` resolves inspected local SVG symbol/use, text-path, gradient/pattern and CSS paint/filter/mask/clip references. Each referenced file is parsed as strict bounded XML and inserted once as definitions. The referring element keeps its geometry/style attributes and receives a local fragment. Local same-file IDs, Unicode/encoded target names and root-document references are preserved. A repeated target reuses its original definitions; other files with the same ID remain distinct.

Referenced documents keep private per-file IDs, classes and font-family aliases. SVG presentation/style URL references, accepted ID lists and stylesheet selectors are rewritten together. Exact ID/class attribute matching reads private original-value snapshots, while zero-specificity private attributes scope selectors and keep source-document rules out of imported nodes. CSS imports retain their resource base, media/group rules and native data image/font resolution. Standard namespace URI declarations remain identifiers, not network fetches. Modern and legacy pseudo-element placement, functional selectors and nth-child selector lists have regression coverage.

Definitions used by ordinary HTML CSS live in a zero-size, accessibility-hidden SVG wrapper; inline SVG definitions live within SVG. Pinned SVG images can also contain these references before data-image embedding. Original workspace files, the explicit ordered path/hash manifest and existing generated snapshot/receipt/replay behavior remain unchanged.

## Bounds and refusal

Existing 32-resource/8 MiB original, 2 MiB per resource and 16 MiB expanded/final limits remain. Reference nesting is bounded; invalid/duplicate target IDs, unlisted or wrong-type files, remote/data reference documents, queries, cycles, changed hashes, active/foreign XML and processing instructions refuse publication. No network/process is used for bundling, and all file reads remain behind the existing approved workspace/private-path/hash gates.

Source SVG XML retains the 64-attribute limit. Phone data-image XML now allows up to 71 attributes to accommodate generated namespace/isolation/original-value fields; its byte, image, node, depth, cancellation and owner/grant generation gates remain. Phone HTML still caps rendered input at 1 MiB and offers verified Source/export for larger snapshots.

## Verification

**42 distinct checks pass**: 19 native portable HTML/SVG publication regressions, one explicit production native fixture export, 21 phone HTML/SVG regressions and one actual native-output-to-phone preparation case. Native coverage includes duplicate/colliding IDs, repeated/nested references, CSS paint/path targets, font-family/shorthand aliases, original attribute selectors, namespace declarations, pseudo-elements and exact publication/replay after source removal. The cross-platform fixture verifies complete local targets, two symbols, scoped styles and Unicode through phone HTML5 preparation.

Locked offline desktop/daemon binary checks, full mobile TypeScript, Rust/Prettier formatting and diff checks pass. Existing Rust warnings remain. No visual or physical-device acceptance is claimed.

## Owner review

- [ ] Publish an HTML report using two inspected sprite files with the same symbol ID, local CSS/font, gradient, mask and a text path. Confirm both icons and the document keep their intended appearance.
- [ ] Include ordinary HTML CSS using an SVG filter/clip reference and inline SVG images using href/xlink:href. Check offline export, sizing and inheritance.
- [ ] On iPhone 15/Moto G32, read the accepted document through direct Artifacts and gateway evidence; inspect font fallback, scoped colors, responsive media rules and same-document references.
- [ ] Test failed/missing IDs, altered hashes and cyclic references. Exact completed invocation replay after source removal must retain its original artifact.
- [ ] Switch owner/pairing, revoke reading access or background the phone during preparation; old content must disappear.

## Remaining document and harness work

Actual use-shadow styling and native iOS/Android rendering still require owner acceptance. [Named document CSS](73-portable-css-names.md) now isolates direct/literal keyframe and custom counter-style names, with a production-native-to-phone grammar fixture. CSS variable-based names, property registrations, layer/scope/container names and font variables/features still need further isolation/fidelity work. XML stylesheet processing instructions and interactive SVG applications remain unsupported by the static preview. Remaining harness bridges/contracts and authenticated provider acceptance stay open in [current coverage](64-remaining-harness-integrations.md).

## Primary references

- [SVG 2 use shadow-tree style rules](https://www.w3.org/TR/SVG/struct.html)
- [Mozilla SVG linking and external reference restrictions](https://developer.mozilla.org/en-US/docs/Web/SVG/Guides/Linking)
- [Mozilla use element behavior](https://developer.mozilla.org/en-US/docs/Web/SVG/Reference/Element/use)

The chosen namespace/selector transformation is an implementation inferred from these rules. Structural fixtures do not certify every browser's SVG shadow styling behavior.
