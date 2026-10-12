# Nebius billing reports

Updated 2026-10-12. Implemented in the Teams worktree, `codex/ion-agent-teams`.
Not merged into `ion-prod`; no live account/export has been read.

## Available implementation

Desktop Usage → Cloud billing manages named billing connections with an exact
account ID, contract ID, billing currency and approved HTTPS storage origin.
Save is local; it does not contact Nebius. Scopes are distinct by account,
contract and currency. Replaying an identical save preserves its revision;
conflicting edits fail. After importing a report, identity/currency/origin are
immutable, while the name can change. Create a new scope for another contract or
currency. The pre-release single-account schema migrates definitions/revisions
atomically without resetting reports or spending holds.

Sync report accepts an **existing** successful, single-month FOCUS 1.2 export ID.
The owner reads its native metadata, checks the original identity, parent contract,
format, month, creation time, expiration and approved archive origin, then downloads
the archive. Both calls are GETs. IAM authorizes the fixed native metadata endpoint;
the separate archive request carries no IAM, cookies or forwarded authorization.
Redirects are refused. Error messages omit response bodies, tokens and signed
URLs. Signed URLs remain transient and are never stored or forwarded to phones.

Bounded gzip/tar and CSV parsing runs on a worker with no filesystem extraction
or subprocess. Every CSV row must match the selected account, billing currency
and complete billing month. Signed costs support exact decimal/exponent input to
18 fractional places, including credits and corrections; unsupported precision
fails without rounding. Billed, effective and contracted totals remain distinct.
All categories and account resources are included, including work outside Ion.
Original container/resource/type, charge class/category and charge periods are
retained; absent resource attribution is counted. Prior-month charge periods are
allowed for corrections within the exported billing period.

Report provenance retains export ID/creation, archive and interpreted-CSV hashes,
fetch time and scoped rows. An exact repeated export returns its original receipt;
changed content fails. The newest export creation time selects one snapshot per
scope/month. Older exports remain historical; ambiguous identical creation times
with different data fail. Snapshots are not added together. A failing, partial or
stale sync leaves previous reports intact. Importing charges never releases,
changes or resets GPU monetary holds, even if the reported total is lower.

Desktop shows the latest 12 monthly snapshots and paginated **Imported exports**
with selected/historical labels, source creation and export identity. Charge details
are paginated and show original attribution. History reads are local and work
without a current Nebius token. Reads/parsing are on workers; cached native rows
are virtualized and support keyboard navigation/focus. Leaving the page fences
late results. The directly connected phone's Usage → Cloud billing reads the same
saved accounts, export history and scoped charge pages; it cannot save a connection
or call a provider from this screen. Owner/client, foreground and response identity
fences protect navigation. Amounts remain strings and do not lose precision.

## Owner setup, when resources are available

- [ ] Upgrade desktop, owner daemon/worker and phone together to protocol **41**.
- [ ] Configure the owner's `ION_NEBIUS_BILLING_IAM_TOKEN` privately in its process/
  service configuration. The existing `ION_NEBIUS_GPU_IAM_TOKEN` is a fallback
  when a separate billing token is absent. A Token Factory model key is not this
  IAM credential. Do not put credentials in chat, documents or a command argument.
  GUI vault entry/automatic IAM refresh remains separate integration work.
- [ ] Confirm the principal has the required tenant billing read access. Nebius's
  export guide calls for at least the tenant `viewer` role.
- [ ] Obtain an existing FOCUS 1.2 export for exactly one month from your Nebius
  console. Get the account ID and `BillingCurrency` from known export/account
  data, and the contract ID from Billing → Payments → Billing details.
- [ ] Copy only the HTTPS **origin** of the export's storage URL into the connection,
  such as `https://storage.eu-north1.nebius.cloud`. Choose the actual export origin;
  do not paste its path/query/signature. Accepted origins use the Nebius regional
  storage hostname. No arbitrary host, plaintext URL, user info or custom port.
- [ ] Save the connection; enter the existing export ID; Sync report. Saving or
  viewing history does not provision a VM or create a billing export.

Native references: [Get export](https://docs.nebius.com/rest-api/billing/v1alpha1/one-time-exports/get),
[export workflow, permissions and lifetime](https://docs.nebius.com/signup-billing/usage/export),
[FOCUS field definitions](https://docs.nebius.com/signup-billing/usage/focus-fields).
A successful export is report availability; it is not a final per-job invoice.
The native format/state schemas are [export](https://github.com/nebius/api/blob/main/nebius/billing/v1alpha1/one_time_export.proto)
and [format](https://github.com/nebius/api/blob/main/nebius/billing/v1alpha1/billing_report_exporter.proto).

## Supported bounds and remaining work

Maximums: 64 connections, 4,096 imported exports, one million stored charge rows,
8 MiB compressed archive, 32 MiB expansion, 128 entries/columns, 100,000 rows per
export and 16 KiB per CSV field. Archives currently contain directories and regular
CSV files only; links, duplicate/unsafe paths, non-CSV files, malformed/trailing
payloads and mixed account/currency/months reject the whole sync. An empty export
cannot establish account evidence and stays unavailable rather than reporting a
verified zero. These bounds and the actual archive shape need live acceptance.

- [ ] Accept a real export/roles/archive layout and source precision with the chosen
  account; compare the displayed totals with the native report.
- [x] Read-only gateway billing views with Usage read grants and redacted private
  billing references were added in [39-gateway-billing-history.md](39-gateway-billing-history.md).
- [ ] Add owner credential-vault/refresh and automatic discovery or scheduled report
  sync. Current imports use an explicit existing export ID and operator IAM setup.
- [ ] Finish original-job/account/tariff reconciliation and retained cost policy,
  verified finality and broader resource/egress controls before enabling the
  remaining gated GPU training paths. Resource IDs can be absent and reporting
  does not promise that all costs can be attributed to an Ion job.
- [ ] Complete rendered HTML/PDF and shared speech deployment/device acceptance.
  Additional AI provider connections remain deferred.

## Later review checklist

- [ ] Save/reopen/rename; stale edit, duplicate tuple and immutable imported identity.
- [ ] Check Tab, focus, arrows/Home/End/Page Up/Page Down, Enter/Space, both themes,
  readable exact decimals and long account/resource identifiers.
- [ ] Sync positive usage, submicro amounts, credits, taxes, prior-period corrections
  and nullable attribution; preserve separate currencies and cost columns.
- [ ] Repeat/restart, import a newer snapshot and then an older one. The selected
  monthly amount stays singular; older exports remain readable locally.
- [ ] Malformed, oversized, mismatched, expired, redirected and incomplete reports
  fail with no partial import or secret/signed URL in messages.
- [ ] Change the connection while a sync runs; verify its stale result cannot save.
- [ ] Compare GPU held/available allowances before/after import: no hold is released.
- [ ] Phone: account → exports → charges, paging/refresh, owner switch, background/
  reconnection, exact credits and wrong response identity. No provider call occurs
  from these reads.

## Automated evidence

Core checks cover native metadata/format/origin/expiry, exact arithmetic, archive
and CSV boundaries, scoped storage/restart, replay/snapshot order, stale rollback,
immutable identity, schema preservation and unchanged GPU holds. Request-builder
and loopback checks cover GET-only behavior, IAM isolation, redirect refusal and
sanitized errors. Phone helper checks cover read-only commands, exact values,
scoped original export identity and rejected routes/cursors. All provider data and
HTTP responses in tests are local fixtures; no real IAM token was accessed.

Nonvisual builds/type checks, generated binding freshness and formatting/source
checks accompany this checkpoint. No dev watcher, visual test, deployment, paid
resource or live billing call was started. The full implementation goal is active.

Checkpoint verification: **22 distinct tests passed** (17 core billing/store/
transport boundaries and 5 phone reader helpers). Complete mobile, gateway and
shared-client TypeScript, locked desktop/daemon/worker checks, protocol-41 binding
freshness, formatting, whitespace and frontend source checks passed. Tests use
fake/local source data; these results do not establish live report or UI/device
acceptance. Existing Rust warnings remain.
