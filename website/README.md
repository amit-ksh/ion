# Ion website

Existing TanStack Start / React / Tailwind site, served by Vite and optionally a Cloudflare Worker. No analytics script is installed. The native application and its development watcher are independent of this directory.

## Run locally

From `website/`, with Bun and Node on PATH:

```sh
bun install
bun run dev
```

Open `http://localhost:3000`. Dependencies already installed in this checkout can be reused.

```sh
bun run typecheck
bun test src/lib/release.test.ts
bun run build
bun run preview
```

`preview` serves the built Worker locally through the Cloudflare Vite plugin. These commands do not start, stop or rebuild Ion Desktop. Browser/device acceptance is separate from type/build validation.

## Configure releases

Edit `src/lib/releases.json`. Each platform has its own version, format, architecture, exact byte size, SHA-256, notes and **explicit** `url`. A `null`, malformed, insecure, localhost or upstream Waku URL keeps its button unavailable. No remote appcast is fetched, no URL is guessed and there is no fallback to an upstream installer. These records are bundled at build time; rebuild the website after changes.

The checked-in records select each platform's published download. They are not a promise that every feature has received device acceptance:

- Windows: `../target/release/ion-0.1.25-x86_64-Setup.exe`, 36,728,897 bytes. SHA-256 `9d9cca41aba76540dac0c7d6f843c02172f212fe2fb8f0c86d346f7edd1dd1e4` matches the public GitHub download. The data-preserving installed update and owner-confirmed mobile QR pairing passed. The installer remains unsigned. See `../docs/windows-release-0.1.25.md`.
- Android: `../dist/mobile/ion-android-1.0.2-arm64.apk`, 61,448,831 bytes. Version 1.0.2 / version code 3, ARM64, Android API 24+ (Android 7.0+). SHA-256 `0896e9c668f42d0de2a8d88798d4a6365b536615037ced787981ca79341318e8` matches the public GitHub download. The existing test certificate is retained; a data-preserving update and nonvisual startup passed on Moto G32 / Android 13. See `../docs/android-release-1.0.2.md`.

To enable a download:

1. Select and validate the actual build, including its signing and installation behavior. Do not label an older binary as containing newer source changes.
2. Publish that exact file to an authorized release host. No artifacts are uploaded by this site.
3. Verify the final HTTPS URL downloads the correct binary without owner credentials, then compare its SHA-256 and byte size against the selected local file. In PowerShell: `Get-FileHash -Algorithm SHA256 -LiteralPath <file>`.
4. Set that platform's `url` to the verified URL. Update all metadata and notes together, including remaining test/signing limitations. Do not copy a local disk path or invent a hosting URL.
5. Update availability language in `src/routes/index.tsx` only if needed; the page derives public availability from configured URLs. Build and verify the release link.

For a published Windows EXE, download it and run the installer. For a published APK, transfer/download it to a compatible Android device and open it in Files; permit installation from that source if prompted. Test-certificate builds are for testing. The companion needs your reachable desktop daemon or configured personal gateway and authentication; use an authenticated secure connection outside a trusted private network. Speech and push capabilities require their own configuration.

The old `/install.sh` endpoint now exits with an explanation and makes no changes. `/linux` leads to the download section. Linux/macOS/iOS releases are not advertised by this page.

## Site metadata and hosting

Optionally set `VITE_ION_SITE_URL` in `.env.local` to the **verified HTTPS origin** chosen for Ion. It is public build-time metadata, never a secret. Until configured, canonical and absolute social URLs are omitted. Ion logos are in `public/ion.svg` and `public/ion-favicon.svg`; PNG icons come from the mobile app's Ion assets.

The Worker name in `wrangler.jsonc` is `ion-website`; it is not a claim of a live deployment. Deployment needs a real Cloudflare account, reviewed domain/target and authorization. `bun run deploy` builds and deploys externally; do not run it as part of routine local validation. This task does not deploy or publish releases.
