# Portable HTML with pinned local resources

Updated 2026-10-15. Implementation: `codex/ion-agent-teams`.

## Publication flow

A team agent with workspace read access uses `inspect_file_document` for its HTML source and each CSS, image or font file. It then calls `save_html_bundle` with the source path/hash, output filename, summary and explicit resource path/hash list. The tool publishes a new HTML artifact with embedded resource bytes and returns its artifact ID for result review.

The owner reads files through the existing private-path, regular-file and no-link gates. Source files remain in the workspace. Each file must match its inspected SHA-256. The portable HTML artifact has its own exact content hash and assignment/project/task provenance. Its output bytes are a generated snapshot; `save_file_document` continues to publish an exact file original.

Stylesheet links become inline styles. CSS `@import` and `url()` resolve relative to their own stylesheet, including escaped CSS URL tokens. Imported stylesheets become data CSS; raster images and font files become data URLs. Inline style attributes and style elements use the HTML source's directory. Responsive `img`/picture `srcset` candidates, quoted CSS `image-set()` alternatives and inline SVG `image`/`feImage` href/xlink:href use the same pinned raster resource map. Same-document SVG fragments remain local. Media rules and stylesheet media attributes remain. All resolution uses the explicit resource list and performs no network fetch.

## Scope and bounds

- UTF-8 HTML source: at most 1 MiB.
- At most 32 explicitly hashed resources; each at most 2 MiB, total original resource bytes at most 8 MiB.
- Resources: CSS, static SVG, PNG/JPEG/GIF/WebP and WOFF/WOFF2/TTF/OTF.
- Final portable HTML and total expanded resource cache: each at most 16 MiB.
- HTML token count: at most 50,000. CSS block nesting: at most 32; CSS import depth: at most 16.
- Cycles, private/unlisted/duplicate/unused resources, changed hashes, malformed serialized names and HTML base elements refuse publication.

The phone rendered view still accepts up to 1 MiB and uses its bounded HTML5 preparation. Larger originals remain available for verified export. Script execution, app credentials, file access and network requests stay disabled in preview. CSP now permits data stylesheets for pinned CSS imports, alongside inline styles and existing data image/font sources.

This increment covers stylesheet links, CSS imports/URL functions, inline styles, `img src`, responsive `srcset`/picture sources, quoted `image-set` alternatives and raster references in inline SVG images/filter images. Phone preparation preserves only raster data URLs on SVG image elements, with remote/file/data-SVG/data-HTML references inactive. Resource MIME is rechecked on encoded-cache hits, so a stylesheet/font cached for CSS cannot satisfy an image reference. Pinned standalone SVG images now resolve their image/CSS/font dependencies through the same resource map; see [SVG image publication](70-portable-svg-images.md). [Pinned SVG reference documents](71-portable-svg-references.md) now inline bounded external symbol/use/paint/path targets. Device rendering and font behavior require the owner checklist below.

## Durable replay and recovery

Document payload, generated artifact and a sealed argument receipt commit in one transaction. The receipt binds the exact artifact metadata, source path/hash, output name, summary and ordered resource path/hash list. An identical request returns the original artifact after the source files have gone. Changed arguments, missing/altered receipts and removed original payloads refuse replay. Recovery matches the durable receipt without rebundling files or fetching resources.

Model reads and workflow handoffs receive the bounded agent-authored summary and artifact hash. Large assembled HTML and embedded binary resources stay out of model checkpoints. Existing keep-until-explicitly-cleaned retention and active review/workflow protection apply to the published artifact.

## Verification

- Six initial native parser/publication cases pass: media/import/image/font bundling, escaped CSS URLs, inline styles, remote/unlisted/cyclic/deep refusal, stylesheet closing-tag escaping, publication/source preservation, exact replay/recovery and changed/private/duplicate/unused resource refusal.
- Twelve phone HTML preparation cases pass, including data CSS import/font/image retention, embedded responsive image candidates and remote/invalid candidate refusal.
- The native trusted worker inspection/publication/summary-read/result flow passes. Two responsive-image/resource-refusal cases also pass; nine native bundle cases pass in total. The final executable also rechecks elapsed lease time inside the publication transaction; all nine native cases pass on that build.
- The SVG raster/cache-MIME increment passes eleven native bundle cases and fourteen phone HTML cases. Mobile TypeScript and Rust formatting checks pass.
- The latest SVG asset-dependency increment passes fourteen native bundle cases and twenty phone HTML/SVG cases; locked desktop/daemon checks, mobile TypeScript and Rust/Prettier/diff checks pass. See the dedicated SVG guide for its bounds and native-device acceptance.
- Core production/test code and locked desktop/daemon binary checks passed for the preceding bundle increment. Native device/visual acceptance has not run.

## Owner review

- [ ] Generate a portable report through a saved team with a local stylesheet, a nested CSS import, an image and a font; review the returned artifact in the result.
- [ ] Open it in direct Artifacts and gateway evidence on iPhone 15 and Moto G32; inspect media rules, text legibility, image sizing and font fallback.
- [ ] Include an inline SVG diagram with image/filter image references using both href and xlink:href; confirm the pinned raster appears and same-document fragments remain usable.
- [ ] Export the verified HTML and open it with network disabled; compare supported embedded resources.
- [ ] Switch owner/pairing, expire or remove evidence-read permission, or background the phone while preparation is pending; stale content must disappear.
- [ ] Remove or alter one source file after inspection and confirm a new publication refuses the changed hash; an exact completed-request replay must retain its original artifact.

## Remaining full-goal work

[Named document CSS](73-portable-css-names.md) now isolates direct/literal animation and counter-style definitions/references. Broader variable/scope/browser fidelity for inlined SVG references, native harness contract gaps and live provider/device acceptance remain open. [Phone Office content reading](67-phone-office-reading.md) now extracts bounded DOCX/XLSX content; native model reads retain the explicit publication summary. This document records the current increment.
