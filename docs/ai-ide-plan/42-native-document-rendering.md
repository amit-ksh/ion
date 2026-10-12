# Native HTML and PDF document previews

## Scope

Desktop Artifacts, Inbox document notifications and team assignment documents
reuse the verified document viewer. HTML now renders its authored static layout
in a separate document browser surface. PDF now renders one selected page on a
worker using Windows.Data.Pdf on Windows or CoreGraphics on macOS. Published
artifact ownership, evidence and Save behavior use the existing owner RPC.

This feature stays on `codex/ion-agent-teams`; it is not merged into `ion-prod`.
Additional AI provider adapters remain deferred.

## Data and performance contract

- Verify the complete response against the selected artifact's immutable evidence
  before preparing any preview. Missing, changed or oversized content is refused.
- Keep the existing 64 KiB preview input limit; this change does not raise the
  publisher's separate binary publication limits. Support at most 32 PDF pages.
- Read bytes, prepare HTML/profile directories, parse documents and render PDF
  pixels on workers. Render methods only read cached entities, text and pixels.
- Fence asynchronous results by preview generation. Closing, changing selection
  or refreshing cannot allow an older worker result to replace the current view.
- Keep one PDF page raster at a time, at most 3.2 million pixels, with bounded
  dimensions. Windows render/stream operations have a shared 10-second deadline
  and bounded PNG decoding. Windows DIP-to-pixel conversion accounts for system
  DPI on the worker, restores the previous thread awareness context and normalizes
  rounding within the requested raster bounds. Native PDF parsing itself is not a
  process sandbox.
- Release the prior GPUI image on page replacement or close. PDF UI scroll state
  stays on the foreground entity, separate from the worker result.
- Source and Markdown/JSON reading keep their existing cached virtualized rows.

## HTML surface

The exact verified HTML is still available in Source and through Save. Reading
uses a wrapper that places a restrictive policy before authored markup. Inline
CSS and data images can render; authored JavaScript, external resources, frames,
objects, forms and base URLs are denied by the policy. Only `about:blank` and its
fragment navigation are accepted. App navigation and tools are not exposed.
Popups, downloads and developer tools are disabled in document mode. A reduced
motion style honors the browser's system preference.

Windows requires an InPrivate composition controller, disables web messages,
host objects, script dialogs and permission grants, and returns an empty 403 for
resource requests. Its document data directory is separate from the normal app
browser directory. InPrivate is not a promise that the directory is deleted.
macOS uses a nonpersistent WebKit data store, disabled JavaScript, the same
navigation policy and denied download/popup handlers. macOS uses the content
policy for subresource isolation; this implementation does not add a WebKit
network interceptor. An unavailable browser shows its host error; Source and
Save remain usable once document preparation succeeds.

The normal task browser keeps its existing browsing behavior and profile.
Document surfaces are shown only while the current preview is visible and in
Reading mode, and are hidden on Source, settings, close and covered overlays.

## PDF surface and keyboard controls

The PDF toolbar shows page number/count, previous/next page and three raster
zoom choices. Page and zoom changes render in the background. No PDF links,
JavaScript, attachments or document actions are executed by the raster viewer.
Save still exports the original verified bytes. Rendering failure gives an
explicit error with Save and Refresh available.

- Tab reaches viewer controls and the page viewport; focus is visible.
- Left/Right or Page Up/Page Down select a page; Home/End select first/last.
- Up/Down scroll within the selected page; mouse/trackpad scrolling is retained.
- Escape or Back closes the preview. Changing page resets the page scroll offset.
- The source viewer retains selection/copy and keyboard list navigation.

## Implementation references

The Windows composition controller uses the native
[InPrivate controller options](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2controlleroptions?view=webview2-1.0.4022.49)
and [environment creation API](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2environment10?view=webview2-1.0.4022.49).
HTML uses the supported
[local-content navigation](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/working-with-local-content)
API rather than a temporary file URL. PDF output uses the documented
[DIP dimensions](https://learn.microsoft.com/en-us/uwp/api/windows.data.pdf.pdfpagerenderoptions.destinationwidth)
with a scoped worker [DPI awareness query](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setthreaddpiawarenesscontext).
Scroll/native surface integration follows
the GPUI revision pinned in this repository, `f9bad894`.

## Automated verification

17 native tests passed: 11 document boundary/render tests and 6 existing browser
regressions. The real Windows PDF API test renders all three supported zooms,
verifies page metadata/raster dimensions and rejects invalid page/document input.
It exposed and verified the correction for this machine's 125% display scaling.
The test library and desktop/daemon/worker binaries pass locked compilation.
Rust formatting, whitespace, UTF-8 and frontend source checks also pass. macOS is not compiled
or executed on this Windows-only Rust target.

A native page metadata/pixel-buffer API test does not open a window or establish
visual app acceptance. No watcher,
second app instance, screenshot test, microphone, provider credential or paid
cloud deployment is used.

## Later owner review

- [ ] Open AI-published HTML from Artifacts, Inbox and a team result. Confirm layout,
      readable text and Source/Save fidelity for Unicode and inline CSS/data images.
- [ ] Confirm authored scripts, remote CSS/images/fonts, frames, form submissions,
      popups and downloads cause no requests or app actions on Windows and macOS.
- [ ] Open a multipage PDF; inspect orientation, color, page/zoom/scroll controls,
      keyboard focus, oversized/invalid input and rendering errors on both platforms.
- [ ] Close, refresh, switch Source/Reading, switch workspace pages and open settings
      while preparation is pending. Ensure an old document never reappears or overlays
      another pane. Check covered menus and window occlusion.
- [ ] Check light/dark themes, system reduced motion, long names and display scales
      in the freshly rebuilt app managed by the existing dev watcher.

## Remaining development

Direct-phone artifact discovery and rendered PDF/HTML parity, other binary
formats, larger publication limits and macOS/live browser acceptance remain open.
Billing settlement, broader remote/GPU enforcement and speech deployment/device
acceptance are tracked separately; this checkpoint does not complete the goal.
