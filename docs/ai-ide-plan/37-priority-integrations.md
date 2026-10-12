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
or app command. HTML, CSV and plain text currently show inert source. PDF retains
verified export and an explicit unavailable page-preview message. Legacy artifacts
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

- [ ] **Billing:** bind the owner-selected Nebius billing account/contract, retrieve
  bounded native reports, ingest exact FOCUS charges with period/revision/currency
  identity and expose reported costs separately from held allowances. Handle
  overlapping snapshots/corrections, retained storage/transfer/tax and original
  Compute resource attribution. Do not infer a final invoice or release a hold
  from a job terminal state or a completed export.
- [ ] **Broader remote controls:** complete aggregate host/project CPU/RAM/process/
  storage and egress enforcement, original-operation inventory and lifecycle
  control across owner/direct phone/gateway grants; finish GPU device/FUSE/CUDA
  and image/runtime gates before enabling the remaining training submissions.
- [ ] **Document previews:** rendered HTML layout and PDF pages, additional binary
  formats/publication limits, consistent direct-phone artifact discovery and
  preview. Current source/reading views are not full HTML/PDF rendering.
- [ ] **Speech:** deploy the prepared shared service when a target is supplied;
  accept native Android/iOS TLS/redirect behavior, microphone capture, concurrent
  desktop/phone partial/final recognition and resource/latency limits.

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
original resource attribution are implemented. Provider/live acceptance, gateway
redaction, credential refresh/discovery and job/tariff/finality reconciliation
remain open. This does not complete broader remote controls, HTML/PDF rendering,
shared speech acceptance or the whole implementation goal.
