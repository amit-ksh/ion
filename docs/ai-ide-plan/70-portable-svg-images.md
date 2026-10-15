# Portable SVG image dependencies

Updated 2026-10-15. Implementation: `codex/ion-agent-teams`.

## Implemented path

`save_html_bundle` now accepts explicitly inspected SVG resources alongside CSS, raster images and fonts. SVG assets used by HTML images, responsive sources, inline SVG image/filter-image elements or CSS image URLs are embedded as data images. Their own local image/CSS/font dependencies resolve relative to the SVG path through the same pinned resource map. Nested SVG images are supported. No network fetch or workspace mutation occurs.

Strict XML parsing preserves SVG element/attribute case, namespaces, viewBox, same-document gradient/filter/use references, Unicode and named-view fragments. CSS imports, style elements/attributes and presentation URL attributes are rewritten before embedding. Byte-identical encoded assets are cached; distinct named views retain their own fragment. Existing exact artifact hashes, original inspected resource hashes, atomic publication/argument seals, replay after source removal and keep-until-cleaned retention remain in use.

The generated snapshot is an HTML artifact. Exact `save_file_document` SVG originals remain exact originals; that operation does not automatically resolve sibling files. A standalone drawing can be included as a pinned SVG resource in an HTML report. [Pinned SVG reference documents](71-portable-svg-references.md) now resolve external symbol/use and paint/path references.

## Isolation and bounds

Native SVG input is UTF-8 and at most 2 MiB per explicitly pinned resource, within the existing 32-resource/8 MiB original and 16 MiB expanded/output ceilings. XML is limited to 50,000 nodes, 128 nesting levels and 64 attributes per element. Resource cycles/deep CSS/SVG imports, unlisted/changed/remote files, XML entities, processing instructions, event handlers and active/foreign SVG controls refuse bundling. Nonrendering editor metadata is omitted from the generated drawing; originals are preserved.

Phone HTML preparation validates SVG data images before retaining image src/href or responsive candidates. It uses canonical base64, strict UTF-8 and namespace-aware incremental XML, yielding during decoding/parsing and checking the existing access/foreground generation. Unique decoded drawings total at most 1 MiB, 32 images and 50,000 elements; nesting is at most 128 XML levels and eight nested image edges. Cached named views share validated bytes, while cached image depth remains part of the nesting check. Unsupported content is removed, with the existing preview limitation indication and verified Source/export available.

SVG data URLs are permitted only in image contexts. Link, object, iframe and use data URLs remain inactive. Native JavaScript/file access/navigation controls and the existing document CSP remain enabled. Browser SVG image mode independently disables scripts and external resource loading; this image behavior does not apply to directly embedded SVG documents. [Mozilla's SVG image restrictions](https://developer.mozilla.org/en-US/docs/Web/SVG/Guides/SVG_as_an_image).

CSS data-image URLs retain the existing isolated image behavior. The new XML validator covers src/href and responsive image attributes; it does not claim to parse every authored CSS data image into an XML validation cache. Actual iOS/Android image/font rendering remains an acceptance check.

## Verification

**34 distinct tests pass**: 14 native portable HTML/SVG parser/publication/replay/trusted-worker cases and 20 phone HTML/SVG cases. Native tests verify nested pinned resources, XML case/namespaces, exact cache fragments, unsupported/changed/unlisted/remote/cyclic input refusal and durable replay without source files. Phone tests cover responsive nested drawings, active/foreign/remote content refusal, UTF-8 and canonical bytes, cancellation, cached depth and Unicode/named views.

Full mobile TypeScript, locked offline desktop/daemon binary checks, Rust/Prettier formatting and diff checks pass. Existing Rust warnings remain. No physical-device or visual test was performed.

## Owner review

- [ ] Generate a team HTML report with a pinned SVG drawing, local raster, nested SVG and stylesheet/font. Verify all hashes and the returned artifact identity.
- [ ] Open direct Artifacts and gateway evidence on iPhone 15/Moto G32; check image sizing, gradients, filters, nested images, responsive candidates, Unicode and fonts.
- [ ] Include a CSS SVG background and a named SVG view; check offline rendering in the phone WebView and exported HTML.
- [ ] Alter an inspected dependency before publishing; the new call must refuse. Replay an already completed exact call after source removal; it must return the original artifact.
- [ ] Background the phone, switch pairing/owner or revoke evidence access during SVG preparation. No old drawing may remain displayed.
- [ ] Confirm scripts, external images, link/use data URLs and foreign/active SVG content stay inactive. Complex/malformed drawings must retain verified Source/export.

## Remaining goal work

[Pinned SVG references](71-portable-svg-references.md) now inline bounded symbol/use/paint/path dependencies with private IDs/styles/fonts; that guide records remaining CSS-name/browser fidelity and acceptance work. XML stylesheet processing instructions and active SVG applications are not supported by this static document preview. Native harness bridges/contracts and live provider/device acceptance remain open in [current harness coverage](64-remaining-harness-integrations.md). This increment does not complete the full goal.
