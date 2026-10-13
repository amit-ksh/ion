# Priority integrations: billing, remote controls, previews and speech

Updated 2026-10-13. Work is on `codex/ion-agent-teams`; not merged into `ion-prod`.
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
Subsequent [direct-phone artifact](44-phone-artifact-library.md), [HTML Reading](47-phone-html-reading.md), [direct PDF](48-phone-pdf-pages.md) and [gateway PDF](49-gateway-pdf-pages.md) checkpoints complete the documented document modes.

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
- [x] **Command owner/project admission:** atomic cross-run/target CPU/RAM/process/storage reservations, strictest outstanding approvals, unknown-history refusal and proof-gated compute release. [Admission](51-command-resource-admission.md) and [native Usage diagnostics](53-command-resource-usage.md).
- [x] **Strict GPU worker boundary:** exact owner profile approval, root/child snapshot equality, IPC/mount/device restrictions and installed Landlock file/device enforcement. [Implementation and review](57-strict-gpu-worker-integration.md). Actual CUDA/provider acceptance remains open.
- [x] **GPU sandbox aggregate caps:** original shape-bound CPU/charged-memory/task approval, actual private cgroup verification in supervisor/child and Linux enforcement tests. [Setup/review](58-gpu-sandbox-resource-limits.md).
- [x] **GPU private workspace capacity:** exact allocated-byte/inode ceilings, dedicated fixed filesystem and supervisor/child witness equality. [Configuration and review](59-gpu-workspace-storage-limits.md); local tmpfs enforcement is proven, actual disk/provider acceptance remains open.
- [ ] **Broader remote controls:** physical host/storage/egress and wider lifecycle/verified storage cleanup; remaining GPU device/FUSE/CUDA and image/runtime gates. Logical command admission does not attest host capacity or release retained provider storage.
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
- [x] **Gateway PDF pages:** verified accepted workflow pages/zoom, bounded immutable chunks, expiring cache and durable relay identity. [Transport/review](49-gateway-pdf-pages.md).
- [ ] **Document acceptance/parity:** macOS compile/render, native browser/device/UI
      acceptance, additional publication formats/limits
      and full browser/CSS fidelity on phone. Structured Reading has an explicit
      supported subset; it does not reproduce authored page styling.
- [x] **Speech acceptance tooling:** executable bounded readiness and explicit
      recognition/concurrency checks; private launch preflight verifies pinned model
      bytes and stable client-compatible API name. [40 merged tests pass](41-speech-service-acceptance.md).
- [x] **Speech deployment preparation:** actual installed runtime/index/model inspection, strict source-only bundle/verification and private pending 46-case observations. [Preparation](50-speech-deployment-device-review.md) and [phone build preflight](52-physical-device-review.md).
- [x] **Actual local speech recognition:** installed CPU model, fixed synthesized English audio, single/two-concurrent streams and WER0. [Evidence](54-native-speech-recognition.md).
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
- [ ] Direct phone: current protocol46 owner, Artifacts filters, HTML Reading/Source,
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

## Latest combined checkpoint: protocol46

All three independent branches are merged into `codex/ion-agent-teams`: speech preparation `5bfa432`, command admission `ebdfeb4` and gateway PDF `1596955`. Protocol **46** bindings were generated from the merged root and passed freshness. Locked desktop/daemon/harness worker compilation, full client/gateway/mobile TypeScript, Rust formatting and source checks passed.

**398 distinct merged tests pass:** 112 protocol, 5 shared renderer, 56 command admission/driver, 6 gateway native/cache, 10 direct artifact/PDF, 65 phone document/history, 73 complete gateway and 71 speech/preflight. Two existing command tests requiring exported native Linux fixtures are ignored. The unchanged maximum real-HTTP phone fixture was deliberately excluded from the concurrent merged rerun; it passed on the feature branch in 70.844 seconds, transferring two 12 MiB pages in 768 chunks with both digests verified and at most 8 pending. This adds one distinct branch test, rather than another claimed merged benchmark. Source-only speech package 26 files/189,961bytes separately verified from its copied CLI.

The implementation goal remains active. Owner IAM credential vault/refresh, original job/account/tariff/finality billing reconciliation, verified retained storage cleanup and broader physical host/egress/GPU controls remain development/acceptance items. Live Linux speech deployment needs an actual Nebius/SSH target and HTTPS hostname. iPhone 15/Moto G32 native builds, 46 real speech observations, macOS and actual-runtime/device acceptance remain open. No visual test, watcher restart, cloud deployment, microphone capture or paid resource was used. No `ion-prod` merge is included; additional harness adapters remain deferred.

## Latest strict GPU integration checkpoint

The independent runtime/file-device branches are merged, and the complete installed worker boundary is wired and verified: **162 distinct increment tests pass**; locked desktop/daemon/core worker binary checks pass. [Evidence and exact remaining review](57-strict-gpu-worker-integration.md). Real CUDA/provider acceptance and wider physical host/VRAM/storage/egress controls remain open. Provider API research does not supply deletion proof, so retained uploads remain reserved. Actual CPU speech recognition is proven on fixed synthesized audio; cloud TLS and iPhone 15/Moto G32 human microphone acceptance still need infrastructure/native builds. The full objective remains active.

## Latest GPU sandbox resource checkpoint

Shape-bound aggregate GPU CPU/charged-memory/task approvals and actual cgroup checks are implemented. **109 distinct current-source tests pass**, including actual throttling/fork/memory enforcement under strict file/device confinement; locked native binaries and formatting pass. [Configuration and remaining review](58-gpu-sandbox-resource-limits.md). Runtime configuration is required before startup and no host controls are changed by the verifier. Actual CUDA/provider acceptance, wider driver/VRAM/HugeTLB/physical capacity/storage/egress and billing/credential work remain open; speech deployment and phone acceptance still need infrastructure/native builds. Historical strict-worker total is 162 after removing two duplicated runtime-test passes.

## Latest dedicated workspace storage checkpoint

Original shape-bound GPU workspace capacity and actual dedicated-filesystem verification are implemented, with block/inode limits covering all private work/home/temp allocations. **98 distinct cases pass**, including real tmpfs exhaustion, witness tamper refusal and four actual unsuitable mount configurations; final native binaries/formatting pass. [Setup and review](59-gpu-workspace-storage-limits.md). Dedicated ext4/CUDA/provider acceptance, XFS quota support and wider driver/VRAM/HugeTLB/host/collector/retained-upload controls remain open. Remote storage/financial holds are not released by workspace exhaustion or cleanup. Shared speech still needs a real target/HTTPS endpoint and native phone review.
