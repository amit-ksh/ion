# Ion Windows 0.1.25 — native mobile pairing QR

Built and installed on 27 October 2026 from source commit
`c0a0032f9161040c61a954c8c5fd1949a86fd955`.

The installed desktop app adds **Settings → Daemon → Connection details →
Show pairing QR**. Enable the managed listener and apply pending settings
first. Android Ion 1.0.2 supports **Pair desktop → Scan to pair**. See the
[pairing guide](desktop-mobile-pairing.md).

## Local build artifacts

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `target/release/ion-0.1.25-x86_64-Setup.exe` | 36,728,897 | `9d9cca41aba76540dac0c7d6f843c02172f212fe2fb8f0c86d346f7edd1dd1e4` |
| `target/release/ion-0.1.25-x86_64-pc-windows-msvc.zip` | 52,744,255 | `b64b1d4de655292c48f38a591b5a54a513a3bf7f6f0ccfc9667f979643d82403` |

The verified pilot artifacts are now published: [Windows installer](https://github.com/amit-ksh/ion/releases/download/v0.1.24/ion-0.1.25-x86_64-Setup.exe) and [portable ZIP](https://github.com/amit-ksh/ion/releases/download/v0.1.24/ion-0.1.25-x86_64-pc-windows-msvc.zip). Both public downloads were downloaded independently and match the packages above byte-for-byte. The versioned [Windows checksums](https://github.com/amit-ksh/ion/releases/download/v0.1.24/ion-0.1.25-SHA256SUMS.txt) are published alongside them. Older assets remain available.

The website metadata selects Windows 0.1.25 and Android 1.0.2.

Website TypeScript, the three release-metadata tests and the production build
passed. Cloudflare deployment `6a89a86d-6fba-44e3-adb2-5ecb22d545ed` is live;
an HTTPS check confirmed that both download buttons select these versions.

## Verification

- The normal development watcher built and launched the updated debug app.
- The optimized release build passed for the GUI, daemon and Windows helpers.
- Two native contract/address tests passed. An independent ZXing decoder
  read the native-generated QR images at original and 256-pixel display size;
  the mobile parser recovered the expected name, address and token. The check
  also covered the real Windows route-selection path and a full-length
  fixture token. Actual pairing secrets were not used in the fixture images.
- The bundled runtime resources came from the public 0.1.24 portable ZIP,
  verified against its published SHA-256
  `f964f4ab473554193c447dee073eb69afb5af701573f7241f1b3ac364cb7b834`.
  All four Windows executables came from the successful current release
  build. The retained Linux worker matches the current verified worker.
- Inno Setup produced the installer; the portable ZIP passed its integrity
  check. This remains an unsigned pilot installer.
- Installation updated the existing per-user Ion directory. All 76 installed
  package files matched the verified package, and the installed program
  started from that directory. Windows records version **0.1.25**.
- The existing SQLite database passed `PRAGMA quick_check` before and after.
  Session, project and message counts were retained exactly: 14, 3 and 67.
  No uninstall or app-data reset was performed.

The owner confirmed successful pairing from the phone using the native
desktop QR code. No automated visual test or screenshot was performed.
