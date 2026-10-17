# Scoped HTML resource hosts and phone preview integration

Updated 2026-10-17. Branch: `codex/ion-agent-teams`.

## Implemented source path

The direct artifact screen and gateway workflow evidence screen now read the publication-bound sources from [chapter 101](101-html-resource-transport.md), verify the original/manifest/source digests, prepare a disposable resource document, and use the resource host in Rendered view. Reading, Source and exact-original export retain their existing path. A source verification/preparation failure leaves those alternatives available and refuses the resource renderer.

Older publications without a source commitment keep the existing snapshot preview. New publications with a commitment use their original entry, stylesheets, SVGs, raster images and fonts. The resource path does not flatten SVG definitions into the entry or rewrite their native CSS names, variables, registrations or cascade. This supplies the document identities needed for the remaining browser fidelity checks.

## Preparation

The entry passes through the existing yielded HTML5 sanitizer with a pinned-resource policy. Only a stylesheet link to a retained CSS file, image references to retained images, SVG references to retained SVG documents, and document fragments gain URL authority. Responsive images keep pinned candidates. Resolution honors the referring document's folder, Unicode/space filenames, exact media types and the publication root. Active HTML, refresh/base elements, remote links, forms and event handlers keep their restrictions.

Separate SVG files receive strict yielded XML validation before hosting: no external entities/doctype, processing instruction, script, foreign namespace/content, XML base, event handler or SMIL action. Their original styles, IDs and document references remain unchanged. Native CSS is served unchanged; the isolated origin, exact file lookup and response CSP restrict its requests.

Only the sanitized entry and supplied resource bytes enter the host. No gateway pairing, provider key, owner filesystem path, app message bridge or author script is exposed. Preparation yields and checks owner/route/foreground scope between batches. The host input is disposable memory; nothing is written to a phone document cache.

## iOS and Android

A local Expo module supplies an HTTP origin bound specifically to `127.0.0.1`, a random ephemeral port and a per-opening UUID capability path. It is a standard HTTP origin so relative stylesheets/fonts and same-origin external SVG references can retain separate browser documents. It neither opens filesystem paths nor forwards network requests.

The Android origin uses Java sockets on background executors; the iOS origin uses Network.framework on a private queue. Both enforce:

- GET/HEAD only, exact Host/Origin, bounded ASCII headers and no request body/transfer encoding.
- Exact capability/file lookup, known MIME types, no query, traversal or encoded separator routes.
- At most 33 files; entry at most 32 MiB prepared, resources at most 2 MiB each/8 MiB total; no second HTML entry.
- CSP refusing scripts, frames, media, forms, workers, external origins and base changes; `no-store`, `nosniff`, and no referrer/cookie/CORS authorization.
- Bounded connections, cancellation of open sockets, and an expiring 30-second lease renewed every five seconds while the current owner/route/grant/foreground scope is valid.
- Native background/module teardown closure; delayed native opening is rechecked before display and closed if superseded.

The existing native WebView remains script-disabled, incognito, without file access, storage or an application message handler. Navigation accepts only the selected entry and its fragments. A missing native module or host failure shows the existing Reading/Source/export alternative.

Expo Autolinking resolves the Android class and Apple pod/module in this worktree. This is new native code: an existing phone binary or Expo Go does not acquire it through a JavaScript refresh. A fresh development/release build is required.

## Web

The static app includes a dedicated `/ion-document/` service worker. It stores supplied files only in worker memory, binds leases to the opening client, and serves no network fallback. Its response CSP restricts style/image/font requests to that opening's capability path, rather than granting access to the rest of the app's origin. The iframe allows same-origin resource identity and grants no script, form, popup, download or top-navigation execution.

The worker checks close/renew ownership, expiry, file/MIME/size bounds, method and route. Worker restart discards documents; lost leases fail explicitly and can be reopened. Registration requires a secure browser context. Browser origin/CSP execution remains an acceptance gate; executing its actual fetch/message code in a VM is not that proof.

## Verification

- [x] **94 passing TypeScript/Bun cases, 3,074 assertions**, zero skips/failures across the complete 18-file phone HTML matrix plus shared source verification. The final isolated run took 5.43 seconds; an existing CSS grammar test timed out during concurrent verification and passed alone with its timeout unchanged. This includes the actual durable native source fixture through its production reader/verifier into prepared host files, separate CSS/SVG/font identity, invalid XML/HTML authority, portable encoding, root escapes, delayed open/revocation, worker ownership/expiry and existing HTML/CSS/MathML regressions.
- [x] **46 actual Java HTTP/lifecycle assertions** after compiling the production origin with JDK 17 targeting Java 8. Requests cover Unicode/percent/plus routes, linked CSS/SVG/font bodies, MIME/CSP/cache headers, wrong capability/Host/Origin, traversal/queries, request methods/body, pending socket closure, lease expiry and no revival.
- [x] Complete mobile TypeScript and actual Expo Autolinking resolution for Android and Apple.
- [ ] Android Kotlin/Expo application build and device WebView acceptance. This host has no Android SDK installed; the JVM contract does not prove the Android wrapper/build.
- [ ] Apple Swift/Expo compilation and WKWebView acceptance on macOS/iPhone. Windows cannot establish this gate.
- [ ] Real web service-worker registration, CSP enforcement, reload/restart and browser resource rendering acceptance.
- [ ] Phone performance/accessibility/large-text and source collection/storage presentation.

No visual/device test, app restart, infrastructure allocation, paid provider call or `ion-prod` merge was performed. Unstarted harness adapters remain deferred. The full document objective remains active.

## Owner review checklist

- [ ] Rebuild the phone app with this native module. Android: from `apps/mobile`, `bun run android`. Apple: on the configured Mac/Xcode host, `bun run ios`. Use the existing pairing/setup instructions.
- [ ] Publish a new HTML bundle with nested folders, linked/imported CSS, two SVG definition files, a font, Unicode/space paths and variable-driven names/registrations. Open Rendered through direct pairing and gateway Read workflow documents.
- [ ] Check external SVG use/paint/path, font selection, CSS variable inheritance/registration/cascade and layout against the original browser document on iPhone 15 and Moto G32.
- [ ] Use Reading, Source and exact-original export. Remove workspace originals, repeat, and confirm retained files still load.
- [ ] Revoke access, change owner/run/route, background/lock the phone and leave the screen during opening/resource loads. The host and pending requests must close; no stale result can appear after returning.
- [ ] Attempt remote/file/app links, wrong capability paths and unsigned content substitutions. No external action or document resource should load.
- [ ] On web, use HTTPS or localhost. Verify service-worker scope, sandbox/CSP, no app-account fetches, worker restart and expiry, and unmount/visibility revocation.

## Primary interface references

The implementation uses the installed Expo module lifecycle/async interfaces and [Expo's local-module guidance](https://docs.expo.dev/more/create-expo-module/). [Apple's local-networking ATS rules](https://developer.apple.com/documentation/bundleresources/information-property-list/nsapptransportsecurity/nsallowslocalnetworking) explain the existing `NSAllowsLocalNetworking` app configuration. [WebKit's external SVG use loader](https://github.com/WebKit/WebKit/blob/main/Source/WebCore/svg/SVGUseElement.cpp) uses same-origin fetching and excludes data URLs; the standard local origin is the implementation choice made here. These references support the design; they do not establish physical-device acceptance.
