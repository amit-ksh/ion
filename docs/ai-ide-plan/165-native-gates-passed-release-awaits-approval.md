# Native gates passed; release launch awaits approval access

22 October 2026, Asia/Kolkata. `ion-prod` HEAD is c20862e; the reviewed code
revision is fd9a36a. The later commit is documentation only. The overall Ion
implementation/acceptance goal remains active.

## Completed native gates

The original follow-up job **24253** completed with **exit 0**. Its terminal
receipt is `.waku-cache/ion-reconciled-relay-native-gates.json`.

| Gate | Actual result | Scope |
| --- | --- | --- |
| Owner recovery | 13 passed | Same native 51c17b8 binary, default parallel scheduling, isolated child-only unconfigured cloud fixture; original 9/4 failure preserved in plan 164 |
| Authenticated mobile presence | 1 passed | Exact native server test with real local authenticated fixture connections; not a physical phone |
| Current credential scope/transport | 6 passed | Includes every pairing component/token rotation and cached read versus fresh effect authorization |
| Combined desktop/core/client type check | Passed | Current queue, UI account-refresh integration, credentials, core and client compile together |
| Generated protocol bindings | Passed | Exporter executed with `--check`, printed `TypeScript protocol bindings are current`; no generated files were rewritten |

The credential test release build took 5m44s, combined type checking 2m39s and
the protocol exporter build/check 12m15s. Existing dead-code, future-compatibility
and ts-rs attribute warnings remain warnings; they are not reported as new errors
or silently fixed outside scope.

Logs are `.waku-cache/ion-reconciled-relay-{credential-scopes,combined-typecheck,protocol-bindings}.log`
and the owner/presence evidence in plan 164. The credential test executable was
rebuilt during this run. Historical live-read receipts retain the hash of the
binary actually used then; do not rerun an old hash guard against the new bytes
and relabel that as the original test. The new scope helper is now verified by
its current native test, not by the earlier historical live-read executable.

## Release gates prepared and verified

The completed native receipt and exact log contents were checked before creating
the new reconciled release gate set. The fd9a36a source snapshot, 50 native Android
inputs, 63 CPU speech-runtime files, previous packages and historical verifiers
were reverified. Original failed native receipts and both older release candidates
were retained and pinned by hash rather than overwritten or relaxed.

Prepared local files:

- `ion-reconciled-release-inputs.json`.
- `verify-ion-reconciled-release-inputs.py` and
  `verify-ion-reconciled-build-gates.py`.
- `verify-ion-reconciled-release.py` and
  `verify-ion-reconciled-android-package.py`.
- `build-ion-reconciled-packages.ps1` and
  `continue-ion-reconciled-packaging.py`.

The strict input/build gates passed, and the sequential build wrapper parsed
successfully. Resource preflight also passed: about **4.36 GiB physical memory,
13.28 GiB commit headroom, 12.02 GiB C: free and 6.75 GiB D: free**, with no
competing Cargo/rustc/Java processes. These are observations, not reserved memory;
the wrapper will measure them again immediately before a real build.

**No reconciled package build or continuation was launched.** Automatic approval
review previously refused the equivalent package-continuation action because
its usage limit was exhausted, with retry stated for **22 October, 02:18 AM**.
Do not bypass that review by launching the prepared equivalent script through
another tool or permission mode. Restore access or retry through the approval
system after the stated reset. The new continuation receipt and the original
blocked continuation receipt were both confirmed absent.

The live backend remains the successfully verified
`8c731253-d440-4129-bf00-0c6da5ef6c2c` version from plan 162. Its worker tests,
deployment and authenticated read acceptance are already recorded; the generic
native helper's final phase text does not reopen those completed gates.

## Current device dependency

A read-only `adb devices -l` query reused the existing exact SDK server, PID
52984. It reported only the existing **sdk_gphone64_x86_64 emulator**, with
**zero ready physical devices**. No install, app opening, shell command,
screenshot, microphone capture or emulator restart occurred.
Evidence: `.waku-cache/ion-android-device-inventory-20261022.json`.

This does not contradict historical Moto G32 upgrades or sign-in; it establishes
only current ADB availability. Physical speech, device UI and upgrade acceptance
cannot be claimed from this emulator inventory.

## Still open

1. Approval access and actual Windows EXE/ZIP plus Android ARM64 builds, package
   verification, and authorized upgrade/data-retention acceptance.
2. Physical Android recording to the paired desktop, native UI/keyboard/theme
   and accessibility/TalkBack acceptance, and outstanding actual-account flows.
3. Token Factory provider image/kernel/harness compatibility, documented price
   ceilings, backend policy and bounded live lifecycle/cost reconciliation.
4. Original Cloudflare meter identity/plan and live traffic attribution after a
   new native client rollout. Local counter results do not prove live reduction.
5. Commit the locally prepared milestone reports once approval review is
   available, then complete the full remaining matrix before sending the
   authorized **Test and fix Ion** handoff.

The USD10 target / USD15 ceiling, static-only publishing, private R2 speech,
Windows/Android scope, deferred harness exclusions and preserved redesign/QA work
remain unchanged. No paid workload, service switch, permission or live data
deletion occurred in this verification milestone.
