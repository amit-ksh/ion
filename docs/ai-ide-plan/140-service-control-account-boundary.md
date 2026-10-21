# Service controls retain the displayed account

21 October 2026. Windows desktop and Android; static publishing scope unchanged.

## Reproduced defect

A delayed Settings switch selected whichever account was in the credential
store when the request finally ran. The view did not pass the identity whose
controls it displayed. Desktop background usage/control reads could also mix
separate account snapshots. Mobile only checked whether the view was mounted
before applying a result.

The deterministic mobile regression first failed with `Cloud request failed`
instead of rejecting the unbound request before HTTP. It uses an isolated
gateway and synthetic identities, not real operator credentials. The failure
is preserved in `.waku-cache/ion-service-account-boundary-red.log`.

## Fix

- Native service reads and switch mutations require the captured workspace.
  The credential boundary validates that identity before refresh and again
  before dispatch. Only the exact allowed method/path pairs are accepted.
- Desktop captures the displayed identity before queuing a switch, binds usage
  and control reads to the verified snapshot and rejects a changed snapshot at
  the end of its background refresh. Failed switches invalidate cached usage
  and controls before refreshing.
- Mobile has a separate account-bound service transport. The generic transport
  cannot dispatch control/usage requests. Each switch callback carries the
  identity from its rendered row; logout/account replacement invalidates
  pending view generations, controls and usage. An old operation's completion
  cannot clear a newer operation's busy state.
- Existing native acceptance helpers now carry the same captured identity for
  their reads, temporary switches and restoration. The already completed Drive
  action was not repeated.

All native credential I/O remains on the existing background workers. No
render-time I/O, new model provider, backend deployment or visual redesign was
introduced.

## Verification

- Mobile controller and shared session suites: **18 passed, 165 assertions**.
  The original regression is now green. Replacement, logout, malformed routes,
  forbidden methods and ordinary signed-in reads/switches are covered.
- Mobile TypeScript: passed, exit 0.
- Native account suite: **22 passed, seven live tests ignored**. The separate
  real-vault test was excluded from this fixture run. New tests reject stale
  account switches before refreshing another identity and reject a foreign
  identity returned by refresh before any service dispatch.
- One opt-in live native account/catalog/control/combined-usage read passed
  through the fixed transport. It generated no inference, app action or compute
  and did not enable or change a service switch.
- Desktop/daemon `cargo check --offline -p waku -p waku-daemon`: passed, exit 0.
  Existing unused-code/ts-rs warnings remain; no native visual test was run.

Logs are under `.waku-cache/ion-service-account-boundary-*`. This is source
and transport acceptance, not actual native GUI account-switch acceptance.

## Packages and remaining gates

The packages verified in plans 137–138 predate this additional fix. They remain
valid evidence for their recorded source, but must not be described as
containing the service-control change. Refresh packages from the new committed
source and preserve exact source/QA/build provenance before replacement.

Physical microphone and paired-phone dictation, actual native account/team/
workflow interaction, sandbox execution access/images/pricing, provider invoice
reconciliation and the conditional final QA handoff remain open. See plans 120,
132 and 139. No additional provider tool effect or paid compute was started.
