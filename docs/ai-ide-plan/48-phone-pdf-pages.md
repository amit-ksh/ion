# Verified phone PDF pages

Status: implemented in the Teams branch; live paired-device and macOS acceptance remain open. Direct owner/phone wire protocol is **45**. Upgrade phone, desktop and daemon together. Additional provider adapters remain deferred.

## User flow

Open an AI-published PDF from the phone Artifacts library. After the complete original bytes and immutable selected metadata are verified, the phone requests one native-rendered page from that same owner. Previous/Next, Fit, 150% and 200% are available in the existing document screen. Zoomed pages scroll horizontally inside the normal vertical reading view. A rendering/connection error gives Retry preview and preserves original Export.

The owner uses the same `waku-document` renderer as the desktop. Windows Data.Pdf and macOS CoreGraphics render static pixels offline on a worker. A directly connected Android/iOS phone displays the verified PNG; it does not install a PDF engine, open the PDF in a browser or execute authored PDF actions. Unsupported owner platforms explicitly refuse page preview and retain verified original export. HTML Reading is separate structured native content, not the PDF renderer.

## Integrity and bounds

`renderArtifactPdfPage` takes the complete selected artifact item, zero-based page and one of 800/1200/1600 target pixel widths. It accepts only a proven published PDF up to 64 KiB and pages below 32. Existing larger original exports remain available up to the library's existing 16 MiB bound; no publisher limit is increased.

The owner rechecks provenance, exact item and original bytes/hash before rendering, then rechecks the private publication digest and complete original content before responding. Replacement/removal during the render refuses stale output. One process-wide nonblocking permit bounds simultaneous PDF render/encode work; another request receives an explicit retryable error. There is no rendering on the app UI thread, no cloud/provider request, subprocess, authored link or document network action.

Raster dimensions are bounded to 1600 by 2000 and 3.2 million pixels. Windows applies the existing DPI conversion, bounded decode and ten-second async operation deadline. The shared raster stays BGRA for desktop display; PNG encoding is explicit for the owner response, preserving the desktop's direct pixel path. The derived PNG is separately bounded to 12 MiB and carries its own SHA-256 plus exact original item, page count, selected page/zoom and dimensions.

The phone validates the complete narrow reply, exact original item/page/zoom, dimension/pixel ceilings, canonical full base64 and PNG digest before display. Decode yields and rechecks current scope in bounded chunks. Static RGBA8 PNG signature/header/chunk bounds are checked; APNG, profiles, unknown chunks, a second header, trailing/truncated bytes and more than 1,024 chunks are refused. Only an inert data image URI reaches the native Image. The app retains one current page and clamps even extreme-aspect display height to 4,000 points. Page replacement clears the old image before requesting another.

The existing parent route pins owner ID/address, client/connection/foreground and the verified original. The page component adds request generation, current-selection and mounted checks around read, decode, digest and image errors. Backgrounding, navigation, owner changes or Refresh unmount/discard the page. Late results cannot restore it. Controls reuse existing native 44-point targets and accessible labels, and status/error copy remains explicit in both themes. No decorative motion was added.

## Automated evidence

- Five shared renderer tests cover dimension/zoom/byte bounds, PNG color channels, DPI conversion and actual Windows native page rendering/PNG decode at all three zooms.
- The owner artifact suite covers selected source, hash and provenance, native PDF result identity/hash, incorrect page/zoom/format/size and changed source refusal.
- The protocol test covers the exact source contract, byte serialization and refusal of private fields.
- Seven phone PDF tests cover request/response scope, original item/page/zoom substitution, canonical encoding and SHA, static PNG boundaries, late read/hash and yielded large-decode cancellation. Existing library/export/reading regressions also run.

The PDF checkpoint passed **49 tests**: 5 shared renderer, 10 owner artifact, 1 protocol and 33 phone PDF/library/export/reading tests. Locked desktop/daemon/worker compilation, complete TypeScript, protocol45 generation, formatting and source checks passed. After the HTML merge, all **43 phone document tests** pass together. Final combined billing integration gates are recorded in the progress and release checklist. Tests use local published PDF/image fixtures and the real Windows renderer. Native phone Image/UI, macOS and real paired-owner behavior are not established by these automated results. No watcher, visual test, deployment, provider credential, microphone or paid resource was used.

## Owner review later

- [ ] Pair current protocol45 phone/owner; publish a supported PDF and compare all pages with the verified original and desktop.
- [ ] Previous/Next boundaries, Fit/zoom, horizontal/vertical scroll, narrow/landscape display, long names, light/dark, native keyboard and VoiceOver/TalkBack.
- [ ] Refresh/change/delete the source during rendering; reconnect, switch owner/address, navigate or background during page read/decode/digest. No old image returns.
- [ ] Two simultaneous requests produce a bounded retry path; renderer/image errors preserve original export and cannot show success.
- [ ] Larger/unsupported/encrypted or malformed PDFs refuse previews clearly while an independently verified original remains exportable.
- [ ] Compile and verify macOS orientation/color/rotation and PNG output on the exact target. Windows tests cannot establish CoreGraphics acceptance.
- [ ] Test physical Android/iOS native Image memory/zoom/scroll behavior with the largest supported page and realistic PDFs.

## Remaining document work

Gateway workflow phones now also have [verified PDF page previews](49-gateway-pdf-pages.md) through bounded derived-image chunks, sharing this native renderer and phone viewer. Larger publication/preview contracts, other binary formats and full browser CSS fidelity remain separate development. Actual device/macOS acceptance also remains open. These changes do not complete the full Ion implementation.
