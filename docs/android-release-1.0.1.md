# Ion Android 1.0.1

Verified on 27 October 2026.

- Download: [ion-android-1.0.1-arm64.apk](https://github.com/amit-ksh/ion/releases/download/v0.1.24/ion-android-1.0.1-arm64.apk)
- Size: 53,651,331 bytes.
- SHA-256: `15e9f79bdbd34f90a680b59445810604ed85fa1ddb26803603cd05599a0255fc`.
- Version: 1.0.1, version code 2.
- Launcher name: **Ion**, using the existing Ion icon assets.
- Android: API 24+, ARM64. Physical acceptance: Moto G32, Android 13 / API 33.
- Package ID: `sh.waku.mobile`, retained for upgrade compatibility.
- Existing test signing certificate SHA-256: `fac61745dc0903786fb9ede62a962b399f7348f0bb6f899b8332667591033b9c`.

## Changes and evidence

The current mobile source was rebuilt with the Ion display name, Expo version 1.0.1, and Android version code 2. The generated native name and version were updated. The build completed successfully, including release lint and signing validation.

Subsequent launcher-icon investigation found that this APK retained the older “io” artwork in its generated mipmaps, despite the newer source PNGs. The original checks below did not compare launcher pixels. Android 1.0.2 regenerates those resources and adds a check against all launcher, round, adaptive and themed icon sizes.

The actual APK passed signature verification, package/version/name inspection, ZIP integrity, ARM64 library inspection, standalone JavaScript-bundle presence, and 16KB native ZIP alignment checks. The public GitHub download was then downloaded independently and matched the built APK byte-for-byte.

The Moto G32 accepted the APK through a data-preserving `adb install -r` update. Activity startup returned `Status: ok`; the process remained running, with no fatal startup exception detected in the checked process log. No uninstall or app-data reset was performed. This receipt establishes installation and nonvisual startup, rather than a full interaction test of all features.

The original reported browser message was only “App not installed.” The exact prior 1.0.0 website APK also installed successfully over USB; its original browser-installer failure was not reproduced and no retained Ion installer error was available. The verified replacement is independently versioned so the website selects the fresh APK.

## Installation

Download the APK, open it from Files, and allow installation from that source if Android requests it. Install over the existing Ion app to keep its settings. The package continues to use the existing test certificate; it is a sideload build rather than a Play Store signing claim.

Android download metadata is in `website/src/lib/releases.json`. The new APK and its checksum were added to the existing `v0.1.24` release, with the Android version stated separately from the desktop version. Older release assets remain available.

The website production build, TypeScript check, and three release-metadata tests passed. Cloudflare deployment `53e856bc-bae4-4ca0-8436-44d54f72fadc` was deployed, and a live HTTPS check confirmed `https://build-with-ion.tech/` advertises the exact Android 1.0.1 URL above.
