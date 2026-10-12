# Priority integrations: billing, remote controls, previews and speech

Updated 2026-10-12. Work is on `codex/ion-agent-teams`; not merged into `ion-prod`.
The user has deferred further AI provider connections until these areas are done.
Unfinished native Pi/OMP/OpenCode2 receipt work is saved separately in Git stash;
new and previously deferred harness adapters remain outside the active scope.

## Delivered in this checkpoint

### Verified document reading

Desktop Artifacts, Inbox document notifications and team assignment documents
open a native Preview with Save and Refresh. Reads use the existing owner RPC,
verify the displayed artifact's immutable evidence against the complete payload,
and parse Markdown on a worker. Per-row Markdown trees and flattened text are
cached; the list is virtualized, with bounded source rows and a bounded fallback
for a giant Markdown block. No document I/O or full parse occurs in render.
Arrows, Home/End, Page Up/Down and Escape operate the viewer; controls support Tab,
Enter/Space and visible focus. Source remains selectable and can be copied.

Markdown supports a reading/source toggle; JSON supports formatted/source views.
Invalid JSON keeps its original source. Document links become readable text and
image references become labels; reading causes no external image load, navigation
or app command. CSV and plain text show inert source. The subsequent
[HTML/PDF checkpoint](42-native-document-rendering.md) adds static HTML layout
and native PDF page rendering while retaining verified export. Legacy artifacts
without integrity evidence cannot enter the new verified viewer.

Gateway workflow-document reading on the native phone adds the same Markdown/
JSON reading/source choice after its existing complete hash verification and
current pairing/run/foreground fences. Parsing is cached per document/mode and
rows remain virtualized. Links/images stay inert. Existing export remains.
This does not yet add a direct-phone cross-project artifact library or PDF pages.

### Shared speech readiness

Desktop Test and phone Test and save verify bounded HTTP model inventory plus the
authenticated transcription WebSocket session and PCM16 format acknowledgment.
They close without microphone permission, audio or a commit. Ordinary recording
retains its actual sample-rate negotiation and avoids an extra probe session.
Phone checks stop on screen exit/backgrounding; late responses cannot save. A
saved secret is reused only for an explicit Test of its exact saved origin.
Deployment/native TLS and recognition acceptance remain unverified.

## Remaining development, in priority scope

- [x] **Billing report/history:** owner-selected account/contract/currency scopes,
      bounded native GET-only FOCUS sync, exact signed charges, corrected monthly
      snapshots, historical provenance and desktop/direct-phone/gateway reads.
      See [native reports](38-nebius-billing-reports.md) and
      [gateway history](39-gateway-billing-history.md).
- [x] **Billing export discovery:** bounded explicit native GET of existing ready
      single-month FOCUS exports for the saved contract. Select an export, then use
      the separate Sync action. [Discovery setup/review](46-billing-export-discovery.md).
- [ ] **Billing reconciliation:** owner credential vault/refresh, original
      job/account/tariff/finality reconciliation and retained storage/transfer policy.
      Do not infer a final invoice or release a hold from a job terminal state or a
      completed export. Direct-phone discovery and scheduled sync are separate work.
- [x] **Remote inventory:** bounded cross-project owner/direct-phone collection,
      original-run navigation and stop/cleanup distinctions. See [inventory](40-remote-execution-controls.md)
      and [merged gateway phone parity](43-gateway-remote-controls.md).
- [x] **Command sandbox aggregate caps:** owner-pinned cgroup2 CPU/memory/no-swap/task
      verification and real local kernel enforcement tests. See [resource groups](45-sandbox-resource-groups.md).
- [ ] **Broader remote controls:** aggregate host/project CPU/RAM/process/storage
      and egress enforcement and wider lifecycle recovery controls;
      finish GPU device/FUSE/CUDA and image/runtime gates for remaining training work.
- [x] **Desktop document rendering:** verified static HTML layout/source and native
      PDF pages with background page/zoom rendering, bounded pixels, DPI handling and
      keyboard controls. [17 native tests pass](42-native-document-rendering.md).
- [x] **Direct-phone artifact library:** proven AI publication metadata, bounded
      project/task/format filters, verified Markdown/JSON/source reading and original
      chunked export. See [phone artifacts](44-phone-artifact-library.md).
- [x] **Phone document reading:** structured offline native HTML Reading in direct
      Artifacts and gateway workflow evidence; verified direct-owner native PDF pages,
      page/zoom controls and exact original export. See [HTML Reading](47-phone-html-reading.md)
      and [PDF pages](48-phone-pdf-pages.md).
- [ ] **Document acceptance/parity:** macOS compile/render, native browser/device/UI
      acceptance, additional publication formats/limits, gateway PDF page transport
      and full browser/CSS fidelity on phone. Structured Reading has an explicit
      supported subset; it does not reproduce authored page styling.
- [x] **Speech acceptance tooling:** executable bounded readiness and explicit
      recognition/concurrency checks; private launch preflight verifies pinned model
      bytes and stable client-compatible API name. [40 merged tests pass](41-speech-service-acceptance.md).
- [ ] **Speech deployment/devices:** deploy when a target is supplied; accept native
      Android/iOS TLS/redirect behavior, microphone capture, concurrent desktop/phone
      partial/final recognition and target resource/latency limits.

### Verified billing contract evidence

Nebius one-time exports are parented by a billing **contract**, not the inference
key or project. The successful enum is `ONE_TIME_EXPORT_STATE_SUCCESS`; fresh
presigned download URLs are sensitive. [Native export schema](https://github.com/nebius/api/blob/main/nebius/billing/v1alpha1/one_time_export.proto),
[REST list](https://docs.nebius.com/rest-api/billing/v1alpha1/one-time-exports/list).

The FOCUS data identifies billing account, currency, periods, resource and
hierarchy container (`x_ContainerId`) and includes billed/effective/contracted
costs, corrections, taxes and credits. Some attribution fields are nullable;
none proves final per-Ion-job settlement. Periodic exports incur separate Object
Storage costs. [Field reference](https://docs.nebius.com/signup-billing/usage/focus-fields),
[export guide](https://docs.nebius.com/signup-billing/usage/export).
Existing detailed research: [GPU billing and identity](RESEARCH-nebius-gpu-price-receipt.md),
[launch/reconciliation contract](RESEARCH-nebius-gpu-launch-policy.md#7-billing-reconciliation-uses-original-compute-resource-ids).

## Owner review checklist

- [ ] Desktop Artifacts/Inbox/team results: Preview, Save, Refresh and Back.
- [ ] Markdown headings, lists, code and tables; formatted JSON and exact Source;
      Unicode and a long single block; malformed JSON preserves original content.
- [ ] Tab, visible focus, arrows/Home/End/Page Up/Page Down, Escape and selection/
      copy; both themes and long document names.
- [ ] Changed/missing evidence refuses preview; navigating away during a read
      cannot reopen it. HTML scripts/image/link references cause no requests/actions.
- [ ] Direct phone: current protocol45 owner, Artifacts filters, HTML Reading/Source,
      PDF page/zoom/scroll, changed source and interrupted decode/digest/export.
- [ ] Gateway phone: current publisher/run, reading/source toggles, background/
      pairing change while reading, revocation and exported content integrity.
- [ ] Speech: Test without a microphone prompt; inventory-only service fails;
      rejected/missing format fails; backgrounding cancels; same-origin saved-token
      reuse works and changed-origin reuse fails.
- [ ] After VM/DNS/token and billing resources are provided, run the existing live
      GPU/remote and speech acceptance guides. These resources are not configured yet.

## Automated evidence

Four native document-boundary tests, four phone document tests, ten credential/
streaming tests and nineteen existing/new phone stream/readiness/WAV tests pass
(37 distinct tests). Locked desktop/daemon/worker compilation and full mobile
TypeScript pass. Formatting and whitespace/source checks are recorded alongside
this checkpoint. Transport tests use local/fake services and real installed
Markdown parsers. No watcher, visual test, deployment, paid provider call or real
microphone/phone acceptance was run. The full implementation goal remains active.

## Billing checkpoint (2026-10-12)

[Native billing report implementation and review](38-nebius-billing-reports.md)
adds desktop connections/sync and direct-phone account/export/charge history.
Reported exact charges, scoped immutable identity, snapshot replacement and
original resource attribution are implemented. [Gateway billing redaction and
history](39-gateway-billing-history.md) and [cross-project remote inventory](40-remote-execution-controls.md)
are also implemented. Existing export discovery and phone document reading/pages
are added in [discovery](46-billing-export-discovery.md), [HTML Reading](47-phone-html-reading.md)
and [PDF pages](48-phone-pdf-pages.md). Provider/device acceptance, owner credential
vault/refresh, original job/account/tariff/finality reconciliation, wider GPU/resource/
egress controls and actual shared speech deployment remain open. The full implementation
remains in progress.

## Latest combined checkpoint: protocol45

Billing discovery and phone HTML/PDF integrations are merged and verified: **197 distinct tests pass**, with freshly rebuilt merged core/protocol, locked desktop/daemon/worker, complete TypeScript and binding freshness gates. See the [current release checklist](20-agent-teams-release-checklist.md#2026-10-12--combined-protocol45-integration-checkpoint). Physical-device/macOS/live acceptance and the remaining development listed above stay open.
