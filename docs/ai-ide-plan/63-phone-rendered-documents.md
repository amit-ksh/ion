# Phone rendered documents and additional text formats

Updated 2026-10-14. Implementation worktree: `codex/ion-agent-teams`.

## Requested scope

The current goal adds additional formats, larger publications, fuller phone HTML/CSS rendering and the previously deferred harness integrations. This increment adds rendered HTML/SVG, four text formats and hash-pinned file publications up to 16 MiB. Native publication/recovery regressions and the desktop/daemon checks pass. It does not mark the complete goal achieved.

## Available behavior

- Direct phone Artifacts and gateway workflow evidence have **Rendered**, **Reading view** and **Source** choices for HTML/SVG.
- Native iOS/Android WebView renders inline CSS, responsive media queries, flex/grid, tables, gradients, embedded raster data images, SVG geometry and embedded fonts. Authored markup is parsed as HTML5 and serialized before loading.
- The web build uses an isolated sandboxed iframe with the same prepared document.
- Published PNG originals have static phone previews in direct Artifacts and gateway evidence. Preparation validates the complete chunk checksums/order and rejects animation, unknown critical chunks and images beyond 8192 pixels per edge/8 megapixels. Scope changes discard prepared image data. Native decode failures retain export; this does not claim that structural validation fully decodes compressed pixels.
- Full original bytes and selected immutable metadata are verified before preparation. Owner/connection/foreground/gateway scope changes fence preparation and discard the native view.
- HTML/SVG renders up to **1 MiB of UTF-8**, compared with the former 64 KiB Reading limit. Complete Source remains a virtualized list. The accessible semantic Reading view remains available within its existing structure/byte limits, with complete Source fallback beyond them.
- Preparation yields during byte counting, HTML parsing, tree filtering, serialization and source batching. DOM complexity is capped at 50,000 nodes/128 levels. Render reads cached prepared state.
- SVG, XML, YAML and YML are accepted by the trusted generated-document tools and ordinary AI artifact collection, library filters, retention protection, output contracts, workflow requirements and phone export MIME selection. SVG has a rendered phone preview; XML/YAML remain inspectable text.

## Document isolation

The initial view receives only reconstructed document markup, with no daemon/gateway URL, key, token, cookies or message bridge. Native JavaScript, DOM storage, file access, universal file access, popups, link previews and media playback are disabled. CSP precedes authored content and denies scripts, network, remote stylesheet imports, frames, objects and form submissions; data CSS imports support portable bundles. Remote links/resources and active event attributes are removed; inline styles and permitted embedded images/fonts remain. The new `save_html_bundle` publication path embeds pinned local stylesheets, CSS URL/import dependencies, raster images and fonts into a generated portable snapshot. [Bundle scope and remaining resource references](65-portable-html-bundles.md) are recorded separately. File-original export continues to preserve the exact published original.

Controls are rendered natively outside the document. Scope changes unmount the view. A failed native content process offers Reading/Source/export, and malformed/deep documents keep complete Source. A restrictive document preview is not a claim that authored interactive applications execute here.

## Verification

**54 distinct phone regression cases pass**, and the final full mobile TypeScript check passes. Phone regression tests cover responsive CSS and SVG retention, CSP placement, external/active content refusal, malformed foreign markup and mutation-XSS cases, invalid attribute names, 120 KiB complete styled content, UTF-8 byte ceilings, cancellation, navigation refusal and supplementary-Unicode row boundaries. Existing immutable artifact, gateway read/export and semantic Reading tests run alongside them. Full mobile TypeScript checking and the mechanical UI detector pass. The native core production/test code check passes. The final executable passes eleven binary-document regressions, including four large-file publication/recovery cases, and the workflow format validation regression. Locked desktop/daemon checks pass.

Six PNG byte-preservation/bounds/malformed/cancellation regressions and the updated full mobile TypeScript check also pass.

No visual test or physical-device acceptance was run. The user’s AGENTS guidance prohibits visual testing unless requested. Authored layout assertions and type checks do not replace native platform acceptance.

## Owner review checklist

- [ ] Rebuild the native mobile client with `react-native-webview` 13.16.1, which matches the installed Expo 57 compatibility table; update desktop/daemon/client together for new format choices.
- [ ] On iPhone 15 and Moto G32, open an accepted generated HTML/SVG in both direct Artifacts and gateway evidence; compare responsive styling, scroll, pinch zoom and landscape behavior.
- [ ] Switch among Rendered, Reading view and Source. Confirm complete source/export for a document larger than 64 KiB.
- [ ] Check large text settings and screen-reader reading, including the semantic Reading alternative, and return navigation from the document.
- [ ] Background the app, switch owner/pairing or remove its read grant during preparation and while the rendered view is open. Confirm old content is discarded.
- [ ] Verify scripts, redirects, remote links/images/fonts and form submissions stay inactive; embedded images and inline styles render.
- [ ] Generate SVG/XML/YAML/YML documents from a team and solo task, filter the library by format, export, and compare exact hashes and names.
- [ ] Open a published PNG through direct Artifacts and gateway evidence. Check portrait/landscape, readable image labels, export and scope/grant changes; unsupported or damaged images must offer export.

## Remaining development in the full goal

- Additional Office content interpretation; current DOCX/XLSX publication preserves opaque exact originals for export with an explicit agent summary. PNG now has a bounded static preview.
- Finish additional resource references beyond the new pinned CSS/import/URL/img/srcset/image-set bundle path; see [the bundle increment](65-portable-html-bundles.md).
- Complete remaining native harness contracts and provider acceptance. See the current harness guide; Amp/Cursor contract gaps remain explicit.

## Primary implementation references

[Expo WebView](https://docs.expo.dev/versions/latest/sdk/webview/), [native WebView properties](https://github.com/react-native-webview/react-native-webview/blob/master/docs/Reference.md), and [parse5 HTML5 parser](https://github.com/inikulin/parse5/tree/v8.0.0/packages/parse5). Dependency versions are pinned in the mobile manifest and Bun lockfile.

## Large file publication increment

`inspect_file_document` returns SHA-256/size for a nonprivate regular workspace file. `save_file_document` publishes the exact inspected original up to 16 MiB with the pinned lease/agent/project/task identity. Parent components and the file are opened without following links, and Windows parent/file handles deny write/delete sharing during reads. Changes since inspection refuse publication.

Payload, artifact evidence and a sealed path/summary receipt commit together. An identical replay returns the existing artifact even when its source has been removed; changed arguments or a removed/changed summary receipt refuse. Model reads and workflow handoffs carry the agent summary/hash, with an explicit `content_interpreted: false`, so binary/large text never expands model context. PDF/DOCX/XLSX/PNG envelope checks do not claim full content validity; native PDF rendering and external Office interpretation remain separate. Keep-until-cleaned retention and source-file preservation apply.

PDF preview input has also increased to 16 MiB in direct, gateway and desktop native paths; page count/pixel/PNG transfer limits remain bounded. Fourteen existing PDF transport/image cases pass, including the real gateway two-page 12 MiB HTTP transfer fixture. The larger-file core production/test code check and final publication/recovery regressions pass.
