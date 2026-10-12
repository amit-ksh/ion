# Phone structured HTML Reading

## Implemented behavior

Verified HTML/HTM documents now have **Reading view** and **Source** in the direct
phone Artifacts reader and the personal-gateway workflow evidence reader. The
original document is still authenticated and checked against its complete
SHA-256 before any content is read. **Source** preserves the complete original
UTF8 text; **Export document** retains the existing verified byte-for-byte export
and native folder-picker flow.

HTML Reading uses the existing native Markdown renderer and theme. It is a
structured reading layout, not a browser or a reproduction of authored page CSS.
The screen says this explicitly. Headings, paragraphs, strong/emphasis/deletion,
inline code, code blocks, line breaks, lists, blockquotes, separators and simple
tables use the app's native text/layout elements. Ordered lists retain their
bounded starting number. Long lists have one item per virtualized row. Table
sections retain their column labels and scroll horizontally through the existing
native table renderer; the first row supplies the column-label treatment.

Authored script and style bodies, head metadata, templates and noscript content
are omitted. Forms/controls, images, video/audio, frames, embedded objects,
vector images, canvases and MathML become inert text labels rather than active
content. Bounded image alt text is retained in its label. URLs, event handlers,
CSS, classes, IDs and all other attributes are discarded; only image alt and an
ordered list's starting number influence the derived layout. Anchor text remains
readable without navigation. No authored link, image request, app command,
JavaScript, arbitrary CSS, WebView or external viewer is created by Reading.

Numeric and a small set of common named character entities decode once into
native text. Decoded `<script>` or `<img>` text never becomes parser input.
Unknown named entities remain visible as authored entity text. Unsupported
presentation such as grid/flex layout, colors, column/row spans and collapsed
controls is not reproduced. The original source/export is the way to inspect
that information. Full browser CSS fidelity is still a separate parity task.

## Parser, cache and scope limits

The mobile package declares Markdown parsing but no HTML parsing dependency.
This increment adds a small strict semantic parser with no new package or install.
It accepts complete explicitly closed documents/fragments, simple `<!doctype html>`
and standard void elements. Ambiguous/malformed structures, duplicate attributes,
unsupported declarations, missing closing tags and unsafe complexity fail closed
into the complete original **Source**, with an explanatory note. It does not
attempt browser HTML error recovery or repair omitted closing tags.

The complete textual preview must be no more than **64 KiB UTF8**, before any
HTML tree is allocated. Original payload transfer/export limits remain unchanged.
The gateway refuses a larger Reading request before fetching its document; the
original can still be exported separately. Invalid UTF8 follows the existing
unavailable-text/export behavior. A partial document is never shown as complete.

The HTML input tree has at most **1024 nodes**, **32 levels**, **4096 tokens**,
**4096 code units per tag** and **32 attributes per tag**. Tables have at most
16 columns. The derived native tree is checked against 4096 nodes, 32 levels
and 64 KiB text, including repeated table labels and unsupported-media notices.
Reading falls back to complete Source when this expansion exceeds its budget.
One native row contains no more than 128 tree nodes or 4096 text code units;
giant/dense blocks become plain native paragraphs of at most 1000 Unicode code
points, retaining every visible text character. Table rows are grouped in
sections of at most eight data rows. Source is separately chunked without
truncation or HTML interpretation.

The phone prepares Source rows and Reading rows once after full document digest
verification, outside React rendering. HTML parsing yields after bounded batches
of characters/nodes and before conversion. The existing owner/client/credential,
route, focus, foreground and generation predicates are checked before every
batch and before commit. A superseded or backgrounded read cannot publish a
partial or late Reading result. Row builders read only cached native nodes/text.
Changing between Reading and Source performs no parse, read or network operation.
Existing Markdown/JSON preparation also moves out of render into this cache.

Direct artifact scope fences remain intact. The gateway stores its reading
preview with the exact current owner/run/credential scope and hides it as soon
as that scope or foreground/focus state changes, before cleanup effects run.
No document payload is added to a shared query cache. No publication, task,
workflow approval, cloud execution or provider access changes.

## Setup and automated verification

No additional mobile dependency, parser polyfill, provider credential or speech
setup is needed. Use the existing paired daemon/personal gateway and publish an
HTML/HTM document through an approved agent or workflow. The same daemon/gateway
read permissions and immutable document metadata are required as before.

The independent `codex/ion-phone-html` checkout uses the existing primary
checkout's installed JS dependencies through ignored TypeScript path mapping
and test resolver files. The Markdown test resolver forwards to the real
installed parsers; it does not substitute a semantic mock.

Verification completed in this branch:

- HTML parser and document preview suites: **16 passed, 117 assertions**.
- Full mobile TypeScript check passed.
- Prettier, whitespace/UTF8/LF checks and the frontend static detector passed.

The tests exercise native semantic headings/emphasis/lists/tables/code,
quoted attributes, comments/raw-text closing tags, dropped scripts/resources,
inert unsupported-media labels, single-pass numeric/named entities, Unicode
without browser TextEncoder, exact Source, malformed/duplicate/deep/wide/dense
content, output expansion limits, virtualized long text/list/table rows and
cancellation/owner-generation changes between asynchronous parser batches.

No dev watcher, package install, provider request, deployment, microphone or
visual/device test was started. The user has not requested live visual testing.
This automated evidence does not establish iOS/Android visual or device acceptance.

## Owner/device review checklist

- [ ] Publish an HTML report with headings, inline formatting, code, a long list
      and a table; open Reading and compare its content/order with Source.
- [ ] In both direct Artifacts and gateway workflow evidence, switch repeatedly
      between Reading and Source. Confirm complete Source remains unchanged and
      no extra payload reads occur merely because the mode changes.
- [ ] Add script/style, remote URLs, forms, images, frames and embedded media.
      Confirm they remain inert and unsupported content has explanatory labels.
- [ ] Try malformed/nested/dense HTML and a document beyond 64 KiB. Confirm the
      complete-source fallback or explicit export instruction; no truncated result.
- [ ] Export the original and compare its SHA-256/bytes against the owner document.
- [ ] Switch owner, credentials or run; background, reconnect or navigate away
      during preparation. No stale owner content or late Reading result appears.
- [ ] Review phone/tablet scrolling, long table columns, dark/light contrast,
      Dynamic Type, TalkBack/VoiceOver and keyboard access to Reading/Source.
- [ ] Full browser page/CSS fidelity and actual device acceptance remain open;
      structured HTML Reading does not claim those checks are complete.
