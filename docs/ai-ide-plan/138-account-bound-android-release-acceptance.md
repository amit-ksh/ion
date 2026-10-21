# Account-bound Android release and Moto G32 upgrade

21 October 2026. Android counterpart to plan 137. The separate visual redesign,
iOS and deferred harness adapters are excluded.

## Source and build

- Source `65a36d6c1d782541860f37d2f2695a4343da20e5` on `ion-prod`, with the mobile
  publishing fix in plan 136 and the preserved 111-file QA patch, SHA-256
  `eccec9c02d57993784a2b0ad683c69b896655cfb8a408fe93c60556a9d799ee4`.
- Forty recorded generated-native/configuration inputs were verified unchanged.
  App configuration, plugins, native modules and lockfile had no committed changes
  from the previous successful native prebuild. No new prebuild was needed.
- One offline incremental ARM64 release build passed in **18 minutes 23 seconds**:
  728 tasks, 78 executed and 650 up to date. It followed Windows packaging and
  resource guards, with one worker and workspace-local temporary files.
- The first package checker expected the older `aapt` field `sdkVersion`, while
  the invoked `aapt2` reports `minSdkVersion`. Inspection confirmed the actual
  minimum remained **24**, target **36**. The failed report was preserved. A new
  report checks the actual tool field; no source, APK or requirement was changed.

## APK

`target/release/ion-android-1.0.0-20261021-account-bound-arm64-v8a.apk`

- Bytes: **53534952**.
- SHA-256: **`f036cb002fdbe7474dc6ac6f8a20171de79a0050b02663b4c804a7e6ed72de58`**.
- Package `sh.waku.mobile`, display name `ion`, version 1.0.0/code 1; ARM64 only.
  The package ID is retained for existing user-data/upgrade compatibility.
- Signature Scheme v2 verifies against the unchanged local Android debug identity,
  certificate SHA-256
  `fac61745dc0903786fb9ede62a962b399f7348f0bb6f899b8332667591033b9c`.
  This is a **sideload test APK**, not a store-signed release.
- Compiled assets contain the account-bound publishing guard and static-export
  publishing copy. ZIP paths/ABI, source/QA provenance and release-copy hash pass.
- The prior accepted APK and its original hash remain preserved.

## Actual device acceptance

The previously offline Moto G32 was reachable at 11:15 IST. The earlier scoped
reconnect failure remains historical evidence; it was not hidden by a repeated
install or shared-server restart.

- `adb install -r` succeeded against the existing installation, without uninstall,
  app-data clearing or permission grants by the test.
- The installed `base.apk` SHA-256 equals the verified release above.
- Nonvisual cold start returned `Status: ok`, `LaunchState: COLD`, total time
  **2424 ms**. Its process remained alive after startup.
- A bounded log read for that app PID detected zero fatal/native-JS startup-error
  signatures. Raw logs, account data and transcript contents were not printed.
- The primary Android user's existing microphone permission remained granted.
  No microphone access, screenshot or UI hierarchy capture was performed.

## Evidence and remaining gates

Evidence under `.waku-cache`:

- `ion-account-bound-android-inputs.json`, `ion-account-bound-android-build.log`
  and successful build exit marker.
- `ion-account-bound-android-package-acceptance.json`: preserved checker failure;
  `ion-account-bound-android-package-final-acceptance.json`: passed.
- `ion-account-bound-android-final-badging.log` and
  `ion-account-bound-android-final-signature.log`.
- `ion-account-bound-android-device-acceptance.json` and matching log/exit marker.

Actual account restoration/refresh/logout/switching in the refreshed UI, desktop
pairing, physical dictation/cancel/backgrounding, publishing confirmation dialogs
and native chat/Teams/workflow interactions remain open. Startup and retained
permission do not prove those interactions. No new model generation, provider
tool effect or Nebius compute launch was requested.
