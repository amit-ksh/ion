# Native owner fixture reconciliation

22 October 2026, Asia/Kolkata. This report preserves the failed native result and
records the narrower cause and verified recovery. It does not authorize a new
package launch or complete real-device acceptance.

## Original result

The original job 32910 completed after 112 minutes of release compilation. Its
13 owner cases produced **9 passed, 4 failed**, exit 101. Two cases were rejected
instead of applied, a portfolio case had the same rejection, and receipt recovery
could not find a run that never started. This is preserved in
`.waku-cache/ion-post-relay-native-owner-tests.log`.

The guarded follow-up 92797 stopped as designed, with no native step launched,
because the original result was not a pass. Its failed receipt remains at
`ion-current-relay-native-gates.json`. Do not overwrite it with a successful claim.

## Feedback loop and cause

The failing `durable_start_receipt_remains_readable_after_project_removal_without_new_effect`
case was rerun alone using the exact freshly compiled test executable. It failed
again, ruling out a purely parallel-fixture collision. Evidence:
`ion-native-owner-single-red.log`.

The fixture uses disposable control/task databases and a fake authenticated relay,
but its real `start_project_automation` path obtains an account scope through
`waku_credentials::cloud::status()`. That function defaults to the configured live
Ion cloud origin and attempts to open the real local account coordination store.
The restricted process cannot open that protected store. Account-read errors
correctly prevent an effect; they must not be converted to a successful start.

The credential module intentionally supports an **unconfigured** cloud mode:
an empty `ION_CLOUD_PUBLIC_ORIGIN` returns a default account status without opening
the live account store. A Python subprocess supplied this value only in the test
child environment. The parent's environment, application configuration, vault,
service controls and permissions were unchanged. No real account was used by the
owner fixture run and no network/provider workload was invoked.

The diagnostic serial run passed **13/13**. A second run restored the **original
default parallel scheduling**, changing only the child cloud-origin value. It
also passed **13/13** with the same executable and filter. This establishes the
ambient account configuration as the causal difference for these cases; changing
production authorization or adding a fake success fallback is unnecessary.

## Native authenticated presence

The same compiled binary then ran exactly
`server::tests::mobile_presence_requires_authentication_and_tracks_live_clients`:
**1 passed, 0 failed**. It exercises real local authenticated connections in a
fixture; it is not a Moto G32 connection or a visual device-display acceptance.

All evidence is local to `.waku-cache/`:

- `ion-native-owner-offline-fixture.{json,log}` — diagnostic serial run.
- `ion-native-offline-owner-parallel.log` — original scheduling, 13 passed.
- `ion-native-offline-mobile-presence.log` — authenticated presence, one passed.
- `ion-native-offline-owner-presence.json` — exact binary SHA-256, original source
  revision and child-environment witness.

The binary was built from 51c17b8. The tested core/protocol files are unchanged
through fd9a36a; the follow-up explicitly verifies those paths. This cannot prove
the new credentials fingerprint helper or desktop queue compiles. The current
source's isolated queue tests and backend tests are separate evidence.

## Remaining native and release gates

A separately pinned follow-up, job **24253**, now uses the successful owner and
presence evidence and preserves the original failures. It runs current-source
credential scope/transport tests, combined desktop/core/client type checking and
generated protocol bindings sequentially, with no duplicate native build. Its
receipt is `ion-reconciled-relay-native-gates.json`.

That receipt must actually pass before preparing a superseding release gate.
The previous release input manifest still points to the failed original gate;
do not modify it or weaken its checker to hide that failure. Preserve its source
and old-package hashes in the next candidate witness.

The package continuation launch remains unexecuted because automatic approval
review reported an exhausted usage limit, with retry stated for 02:18 AM on
22 October. No alternate launch path was used. Commit of this report likewise
remains pending. Approval access, actual native checks, new package verification,
upgrade/device checks and the original sandbox/pricing/QA gates remain required.
