# Local app logos and current readiness

23 October 2026, `ion-prod`. New source remains uncompiled and absent from the
previously verified Windows/Android installers. Preserve unrelated QA work.

## UI-08 — Local identity marks

Desktop catalog/connected-account rows and mobile Apps now select bundled marks
for Gmail, Google Drive, Outlook, GitHub, Supabase and existing GitLab. Unknown
integrations keep a generic icon and visible name; no integration is added.

Google PNGs and Microsoft's SVG retain their colors. GitHub uses theme ink for
contrast; Supabase uses its supplied Simple Icons color. GitLab reuses the
existing repository asset. Source URLs, byte hashes and attribution are in
`assets/app-icons/manifest.json` and `SOURCES.md`. Five new assets total **8,143
bytes**. Mobile PNG copies are byte-identical; SVG XML is embedded locally.
No row loads a remote icon or needs credentials to paint. Names, descriptions
and connection actions remain. Desktop catalog grouping now changes from two
columns to one below the available-width threshold, participating in the row
cache key. A narrow single column uses the full row width.

`.waku-cache/ion-app-icons-local-validation.json` verifies six hashes, PNG
equivalence, native embedding, four mobile SVG roots, three TS/TSX syntax checks
and Rust syntax/format/whitespace. That initial receipt predates the corrected
connected row enum match and adaptive column grouping. The later
`ion-local-ui-source-final-observation.json` pins the updated source and passes
Rust syntax/format, three mobile syntax transformations, whitespace and all six
asset hash checks. Neither receipt is a full type/native acceptance pass. No
screenshot or device test ran. The validated asset/provenance files and two
mobile PNG copies are committed independently as **5e39303**; pending application
source is not included in that asset milestone.

Download receipts preserve initial Google SVG 404s and the Microsoft redirect.
Working official Google PNGs replaced those URLs; the specific Microsoft
`res-1.cdn.office.net` destination was checked before retrieval. No app
credentials or paid provider API were used.

## Approval access and completed milestone

Plan 176's cancellation report is committed independently as **eb7ee9b**,
`docs: record installed Codex active cancellation acceptance`. Its receipt still
hashes to `02edeeae1367f926eea8300c266739755f312acc4bd51a73b84e328caefb09a9`.
This successful Git write supersedes the prior general approval-access blocker.
Other source and QA edits were left outside the commit.

## Current native readiness blocker

The newly authorized read-only attempt to obtain the installed daemon's selected
Codex executable failed with `WakuConnectionError`. Receipt:
`.waku-cache/ion-installed-codex-selected-binary.json`. No binary was selected,
runtime started, model called or setting changed.

A subsequent process/listener census found **no Ion app/daemon process and no
listener on the prior acceptance port 49837**. Saved exposure is disabled and
specifies port 34123. Only nonsecret configuration fields were printed. Nothing
was started or enabled. This does not prove unsupported Codex Teams flags.
Earlier solo prompts/cancellation retain their original scope; exact installed
Teams readiness and live team/routine/workflow acceptance remain open.

## Compile/test pass did not launch

`.waku-cache/ion-current-ui-native-checks.json` records 24 source hashes and
intended sequential desktop/credentials and mobile checks. Actual state:
`resource_guard_deferred`, **zero checks started**, 1.44 GiB free physical memory,
3.79 GiB commit headroom, no competing Cargo/rustc. Thresholds remain 2.5/4 GiB.
No process was stopped, gate lowered, build duplicated or package refreshed.
No live check job needs polling.

Source review also cleared/remasked all unsaved setup credentials and the pairing
invite on a workspace change, alongside the project binding. Personal setup
disclosures close so temporarily revealed input cannot remain visible under the
next account. The latest source observation covers this edit's syntax only;
actual account-switch and native field behavior remain required checks. Source
advanced after the deferred compile receipt, whose original hashes are preserved.

Sandbox worker identity/kernel, confirmed price/retention ceilings, bounded
managed policy and actual lifecycle/cost acceptance remain unresolved under the
USD10 monthly target/USD15 cumulative ceiling. No sandbox, VM or cloud build
was started. Final QA handoff remains unsent.
