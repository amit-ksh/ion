# Ion Android 1.0.2

Verified on 27 October 2026.

- Download: [ion-android-1.0.2-arm64.apk](https://github.com/amit-ksh/ion/releases/download/v0.1.24/ion-android-1.0.2-arm64.apk).
- Size: 61,448,831 bytes.
- SHA-256: `0896e9c668f42d0de2a8d88798d4a6365b536615037ced787981ca79341318e8`.
- Version 1.0.2, version code 3; package `sh.waku.mobile`; launcher name **Ion**.
- Android API 24+, ARM64. Update and nonvisual startup checked on Moto G32 / Android 13.
- Existing test signing certificate SHA-256: `fac61745dc0903786fb9ede62a962b399f7348f0bb6f899b8332667591033b9c`.

## Launcher icon

The 1.0.1 APK retained the older “io” artwork in generated Android mipmaps even though the canonical source PNGs had already changed. Regenerating the native Android project from `app.json` updates the launcher, round, adaptive foreground and monochrome resources.

`python scripts/check-android-icons.py path/to/app.apk` resolves the APK's real resource table (including optimized file names) and compares all 20 images against current source artwork. The old 1.0.1 APK fails this check; the new APK and the copy pulled back from the installed phone pass. The owner also confirmed that the phone's home-screen/app-drawer icon now shows the new Ion mark. No screenshot or automated launcher UI test was captured.

Before future local Gradle builds, run `bun run --cwd apps/mobile android:prepare` to synchronize native configuration and artwork.

## Scan to pair

Both the initial pairing sheet and the standalone Pair desktop form offer **Scan to pair**. The camera opens only after permission is granted. A valid QR fills the name, address and token; the user reviews the fields and taps **Add** before any connection is saved. Cancel, denied permission, invalid-code retry and manual entry are supported. Backgrounding the app unmounts the camera preview; reduced motion is honored.

Generate a desktop QR with `bun scripts/mobile-pairing.ts` and open its printed localhost URL. Paste the address and token from Desktop → Settings → Daemon. Encoding happens in browser memory, with no external QR service, credential request or persisted token. Clear the code when done. The QR contains the same full-control token as manual direct pairing. This does not create a restricted personal-gateway invite.

## Verification

- Mobile TypeScript check passed.
- 20 pairing/profile/connection tests passed; branding and image-resolution checks passed.
- A real fixture PNG encoded with the QR helper's library was decoded offline using ZXing and parsed into the expected pairing fields.
- APK ZIP integrity, embedded JavaScript, ARM64-only libraries, native camera module presence, API/version/name, retained signing certificate and 16KB native ZIP alignment passed.
- `adb install -r --no-streaming` updated the phone. Its first-install timestamp was retained. Pulling back the installed APK produced the same SHA-256 as the verified artifact.
- MainActivity started, its process remained alive, and the checked process log contained no fatal startup exception. No uninstall or app-data clearing was performed.
- The public GitHub download was downloaded again and matched the installed APK byte-for-byte.

Live camera scanning and denied-permission/retry interactions have not been manually exercised on the phone. Installation and nonvisual startup are verified; this receipt does not claim full feature acceptance or an iOS build.

The website's Android metadata selects this independently versioned asset under desktop release `v0.1.24`; existing desktop assets and older Android APKs remain available. This APK retains the existing test certificate and is intended for sideload testing.

Website TypeScript, release-metadata tests and production build passed. Cloudflare version `bd593b19-f21c-40d6-a8d1-2d99f3db5095` was deployed; a live HTTPS check confirmed that [build-with-ion.tech](https://build-with-ion.tech/) advertises the exact Android 1.0.2 URL above.
