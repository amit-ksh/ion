# Gateway billing history

## Implemented

The personal gateway exposes three bounded, read-only owner portfolio queries:

- `cloudBilling`: account labels, currency, revision and up to twelve selected monthly report snapshots. UUID cursor; up to 32 accounts per request.
- `cloudBillingReports`: all imported exports for one billing scope, including retained historical exports. Export ID cursor; up to 32 rows per request (phone requests 16).
- `cloudBillingCharges`: original charge categories, resource type, class, charge periods and exact billed/effective/contracted decimals for one scope/export. Ordinal cursor; up to 32 rows per request.

The persistent owner daemon and temporary desktop gateway bridge dispatch these reads to the existing billing store. Reading gateway billing makes no provider request and cannot save a scope, import an export, release a monetary hold, start a job, or change billing data.

## Access and transfer

All three queries require the requesting phone's existing `usageRead` owner grant. Query submission, owner queue delivery, completion and receipt reads retain the gateway's publisher/requester/nonce checks and revocation handling. Read receipts expire under the existing portfolio retention policy. A dropped submission acknowledgement recovers the same nonce instead of creating a second request.

The gateway projections exclude account IDs, contract IDs, archive origin, container IDs, resource IDs, credentials and download/transport links. Account labels and currency remain visible. Charge category/type/class labels are bounded to 80 characters. No full cloud resource identity is sent to the phone; inspect those details in the desktop billing view.

Reports retain the original export ID, scope UUID, month, selected/historical flag, exact decimal strings, row counts, export creation timestamp (including fractional precision), fetched timestamp and CSV/archive SHA-256 digests. Historical exports remain separate snapshots and are never added together. A digest identifies saved evidence; it is not proof of invoice finality or charge attribution to an Ion job.

Responses are at most 60,000 UTF-8 bytes. Oversized account pages drop whole account rows and provide a continuation cursor, preserving all monthly evidence within every returned account and the full account count. Report and charge transfer helpers also retain whole-row continuation behavior. The client rejects unknown fields, reordered/nonadvancing rows, mismatched account/export identity, invalid digests or dates, unsafe counts, nondecimal monetary values and invalid/truncated cursors. Sub-millisecond charge periods retain their ordering.

## Phone flow

Open **Personal gateway → Gateway history → Cloud billing** after pairing with **Read owner usage**. Choose the owner, open an account's **Imported exports**, then open **View charges**. The owner remains pinned on nested views. Refresh updates the saved owner data; report import is performed on the desktop.

The screen uses the existing owner read hook: foreground refresh, pairing/grant/owner/query generation fences, one bounded receipt polling sequence, offline/rejected/expired messages, and no background polling. Invalid route parameters disable reads before transport. Owner, pairing or route changes reset paging and suppress earlier pages. Amounts and provenance are selectable; lists are virtualized, controls have accessible labels, and body text follows the current theme and text scaling.

## Verification

New focused checks cover typed read-only queries and redaction, exact signed decimals, account/export boundaries, strict cursor bounds, invalid calendar dates and nanosecond periods, phone route/grant checks, lost acknowledgements, late results, HTTP grant/publisher boundaries and revocation at completion. Existing portfolio transport, relay and HTTP tests exercise the same paths. Rust projection tests cover native query bounds, whole-account transfer pagination and omission of private connection/resource identities.

No billable provider operation, real account credential, gateway deployment, physical-phone session or visual app validation was used for these checks.

## User review checklist

- [ ] Import a real report on the desktop using the setup in [38-nebius-billing-reports.md](38-nebius-billing-reports.md).
- [ ] Run the personal gateway and owner daemon, pair the phone with **Read owner usage**, and open **Cloud billing** from Gateway history.
- [ ] Compare the selected month and billed/effective/contracted amounts on desktop and gateway phone views, including any credits.
- [ ] Open historical exports and confirm each export remains a separate snapshot with its own ID and digests.
- [ ] Page charges and confirm ordinal navigation reaches later rows without gaps or duplicates. Use the desktop view when full resource IDs are needed.
- [ ] Change owner, background the phone during loading, reconnect or replace pairing, then refresh. Confirm prior account/export pages are cleared.
- [ ] Revoke the Usage read grant while a read is pending and verify no result becomes available to that phone.
- [ ] Check text scaling, light/dark legibility, screen-reader labels and keyboard navigation on supported devices.

## Remaining scope

Live provider import review needs a configured Nebius billing account and IAM access. Billing finality, matching cloud charges to Ion requests/resources and releasing GPU holds require a separate reconciliation contract. Gateway billing intentionally remains read-only. No new AI provider adapter is included.
