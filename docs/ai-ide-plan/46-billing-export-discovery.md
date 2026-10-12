# Existing Nebius billing report discovery

Updated 2026-10-12. Implemented independently in `codex/ion-billing-discovery`;
merge and verification status is recorded below. No Nebius account was contacted.

## Available behavior

Desktop **Usage → Cloud billing** now offers **Discover existing exports** for a
saved, selected billing connection. The owner explicitly requests one provider
page. The native list shows ready report month, export identity, precise source
creation time and expiration. **Next export page** reads the next provider page;
**Discover existing exports** starts again. Empty pages explain that no eligible
reports were found and preserve a continuation when further pages exist.

Choose **Select report** (or focus the list and press Enter/Space). It fills the
existing export ID field and explains the next action. **Sync report** remains a
separate explicit command. Manual export IDs continue to work. Selecting another
connection clears the selected export input. Discovery does not create exports,
download archives, import charges, release GPU holds or establish invoice finality.
The existing sync still performs a fresh Get, validates the approved archive
origin and verifies complete CSV account/currency/month evidence before saving.

Discovery checks the exact saved scope revision before credential access and
checks the full scope and revision again after the provider request. Stale scope
edits reject the whole result. Report discovery is contract evidence; only the
later archive CSV verification can establish the saved billing account identity.
Only successful, unexpired, single-month FOCUS 1.2 CSV exports are offered.
Scheduled/running/failed/archived, expired, unsupported-format or multi-month
exports are counted as examined and omitted from eligible rows. Foreign contract
identities, duplicate IDs, malformed supported reports, unknown states and page
size overruns reject the page. Provider download URLs and error details are ignored
and never included in replies, UI errors or stored report discovery metadata.

The GET uses the owner's existing private IAM configuration. It has no redirect
following, retries, archive authorization forwarding or credential refresh. All
network, JSON processing and SQLite work stays on daemon/background workers.
Native render uses cached virtualized rows and existing keyboard/focus controls;
closing the page fences late responses. Returned scope/revision/count/continuation
and report metadata are checked before the UI accepts a page.

## Pagination and supported bounds

- 1–32 existing provider exports per explicit request; a page may contain fewer
  eligible reports. Provider list metadata is bounded to 512 KiB per response.
- Continuations are random owner-local UUIDs. The provider's opaque page token is
  retained only in the owner's private SQLite cache, never sent to a phone or
  shown in a UI/log. The complete saved scope digest, revision, requested page
  size, original discovery timestamp, seen export IDs and token hashes are pinned.
- Continuations expire 15 minutes after the first page. Clock rollback, scope,
  revision or page-size changes, non-advancing/repeated tokens and repeated export
  IDs across pages fail. Returning a scoped page is fenced again after I/O.
- At most 64 provider pages in one discovery chain; continuing beyond the bound
  requires a new discovery. Token strings are at most 4 KiB, without controls.
- The cache holds at most 512 continuations. Expired rows are pruned when issuing
  another continuation. A full cache fails explicitly and can be retried after
  expiration. Restart preserves an unexpired cursor; it does not start a request.
- Report readiness is rechecked against receipt time. A report that expires while
  the request is in flight does not appear as ready.

The provider does not document a frozen list snapshot or stable sort for this
endpoint. This implementation preserves its page order and refuses pagination
cycles/identity repeats; it does not claim snapshot stability across concurrent
provider export creation. The user can start a fresh discovery on such a failure.

## Verified API references

The official [List REST reference](https://docs.nebius.com/rest-api/billing/v1alpha1/one-time-exports/list)
defines GET `/billing/v1alpha1/one-time-exports`, required `parentId`, optional
`pageSize` and `pageToken`, and response `items`/`nextPageToken`. The native
[List proto](https://github.com/nebius/api/blob/main/nebius/billing/v1alpha1/one_time_export_service.proto)
scopes exports to a parent contract. The
[export proto](https://github.com/nebius/api/blob/main/nebius/billing/v1alpha1/one_time_export.proto)
defines states, inclusive billing months, format, creation/expiration and the
sensitive download URL. A fresh download URL belongs to Get; list discovery does
not require or reuse it. Schemas were read on 2026-10-12; no fields were inferred.

## Owner setup and review

- [ ] Upgrade desktop and owner daemon together to the merged protocol version
      recorded in the release checklist. Old clients cannot send this new command.
- [ ] Configure `ION_NEBIUS_BILLING_IAM_TOKEN` privately in the owner process/service
      configuration (existing GPU IAM fallback applies). A Token Factory model key
      is not billing IAM. Confirm tenant billing read access with the chosen principal.
- [ ] Save the exact account, contract, currency and approved Nebius storage origin
      as described in [billing setup](38-nebius-billing-reports.md).
- [ ] Create or obtain an existing successful single-month FOCUS 1.2 CSV export
      through your Nebius account. Ion discovery does not create one.
- [ ] Discover, inspect month/source creation/expiration, select, then explicitly
      Sync. Check imported snapshot hashes/totals against the source account report.
- [ ] Exercise empty and mixed-state pages, multiple pages, expired reports,
      permission errors, page expiration/restart and a scope edit during a request.
- [ ] Confirm no IAM, signed URL or provider error body appears in UI, daemon logs,
      direct-client replies, gateway or mobile projections.
- [ ] Verify keyboard list navigation/selection, readable long export identities,
      focus, empty/error/loading states and page-close behavior in the watched app.

Direct-phone discovery, IAM vault entry/automatic token refresh, export creation,
automatic sync, invoice/job finality and larger billing archive support remain
separate work. Existing saved history reads and gateway billing projections are
unchanged. Additional AI provider adapters remain outside the current scope.

## Verification record

Branch gates passed on 2026-10-12:

- Locked desktop and owner daemon/worker (`--bins`) build check passed from this
  checkout after a narrow shared-target protocol cache clean.
- **30 tests passed, zero failures/ignored:** 27 complete core billing tests
  (including 10 new discovery/transport boundaries), one new strict protocol
  discovery query test and two new native reply acceptance tests. This adds 13
  tests and also reruns the existing 17 billing import/history regressions.
- Coverage includes actual loopback bounded HTTP reads, request method/encoding
  and sensitive authorization headers; scope/revision/limit/expiration fencing,
  scope edits during I/O, restart/cycles/repeated IDs, cache saturation/pruning,
  receipt-time report expiration, redacted wire metadata and unchanged report
  imports/GPU holds.
- Full Rust formatting check, changed-file source/UTF-8/whitespace checks and
  native UI source detector passed. Generated combined protocol 45 bindings and
  merged branch gates are owned by the main integration checkpoint.

The HTTP/body checks use local fixtures and synthetic report metadata; no
authenticated Nebius list/export was fetched. No visual app, cloud account,
physical-phone, watcher restart, paid provisioning or live credential test was run.
