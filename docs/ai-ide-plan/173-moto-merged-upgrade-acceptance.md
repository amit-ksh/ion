# Merged Moto G32 upgrade: nonvisual acceptance

22 October 2026, Asia/Kolkata. The user explicitly authorized the data-preserving
physical Moto upgrade and nonvisual startup/hash checks after the scope rejection
recorded in plan 172. This authorization did not include microphone use,
screenshots, permission changes or broader native interaction testing.

## Verified result

The guarded replacement install and cold launch passed at 12:04 IST. The exact
installed APK matches the verified merged release:

- Source: `fd9a36a0b9a6511230873e55fb11718d0ad83ef2`.
- APK: `target/release/ion-android-1.0.0-20261022-reconciled-relay-arm64-v8a.apk`.
- SHA-256: `d75a37f669f6b71a85559bc97b081a017c44ca536167483f2919f28ca6f3de97`.
- Replacement installation succeeded; installed APK hash matched.
- Android launch status `ok`, state `COLD`, reported total launch time 5,760 ms.
- App process was present; the bounded startup check found zero fatal or
  JavaScript errors. This is startup evidence, not a claim that every feature works.
- No uninstall, app-data clear, active-app stop, microphone access, screenshot
  or permission grant occurred. Existing microphone permission was retained.

Evidence: `.waku-cache/ion-reconciled-android-device-acceptance.json`.
The successful Windows upgrade and its database-retention checks remain in
[172](172-android-package-and-windows-upgrade.md).

## Initial Cloudflare observation after Windows upgrade

A read-only analytics query returned HTTP 200 without GraphQL errors for
12:00:00–12:03:07 IST. Available minute buckets contained six invocations,
148 rows read and zero rows written for Platform namespace
`f75aac9a812b483fad34d669cfa1ebd3`. No AccountRelay bucket appeared in this
short window. No SQL stored-byte sample was returned.

Evidence: `.waku-cache/ion-cloudflare-postupgrade-20261022.json`.
The sample is short and subject to analytics ingestion delay. It does not prove
paired idle behavior, active synchronization correctness, daily rates or the
original dashboard meter's denominator. Those live acceptance gates remain open.
No permissions, service switches, billing plans or Durable Object data changed.

## Remaining release gates

Actual mobile account restoration/switching, pairing, physical dictation,
TalkBack and remaining native interaction checks are not established by this
upgrade. Clean Windows install and GUI account restoration remain open. Sandbox
image/kernel compatibility and verified pricing still gate paid execution.
Matched live idle/active relay observation and provider billing reconciliation
also remain open. No final QA handoff or full goal completion is claimed.
