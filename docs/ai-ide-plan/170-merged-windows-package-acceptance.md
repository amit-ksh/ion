# Merged Windows release: actual package verification passed

22 October 2026, Asia/Kolkata. The Windows EXE/ZIP now contain the merged redesign,
service-account controls, recording-related shared code and relay changes from
reviewed source **fd9a36a**. Later commits through 6578d9e are documentation only.
This is package acceptance, not a clean install, GUI, physical microphone or
whole-goal pass.

## Actual build and artifacts

The existing retry3 queue resumed after memory recovered and completed Windows
compilation and Inno Setup with **exit 0**. No compiler was restarted while live.
The daemon, desktop, JavaScript REPL and computer-use executables were rebuilt.
Older verified EXE/ZIP backups and the previous Android APK remain unchanged.

| Artifact in `target/release/` | Bytes | SHA-256 |
| --- | ---: | --- |
| `ion-0.1.24-x86_64-Setup.exe` | 33,735,912 | `b11347317825f250507d4db7c2a157859d04d4b1da2687b78355c799e955267a` |
| `ion-0.1.24-x86_64-pc-windows-msvc.zip` | 48,739,671 | `3d4fd66b8be5066abeb843590a6c3046bd393d51f55354fa3f8e0971be5910a0` |

These are sideload/test artifacts. The build log reports no Windows signing
certificate configured; do not describe them as Authenticode-signed production
releases. No security setting was changed to build or verify them.

## Package-check failure preserved and reconciled

The first package checker correctly stopped its queue, but its failing assertion
required the **Settings service-control transport marker in both executables**.
That Settings transport is used by `src/app/cloud_account.rs`; it is not exposed
by the daemon/core call paths. Core speech uses local ASR. The daemon's managed
inference, app and sandbox paths use their distinct account-bound transports.

The actual desktop contains both Settings account guards. The actual daemon
contains inference, app and sandbox account-change guards and restricted-route
guards. It also contains the new owner batching route; the desktop contains the
new task batching route. Source/runtime inputs and freshly built file hashes
match the reviewed candidate. There was no application-code change to make the
assertion pass.

Two existing native Settings transport fixtures were run from the current-source
credential test executable: **2 passed, 0 failed**. They reject account replacement,
malformed method/routes and a foreign identity returned during refresh before
dispatch. A package-scope helper checks actual reachable capabilities per binary;
four negative cases remove one required guard family and all four are rejected.
Its hash is pinned by the separate final verifier.

The original failed report and extracted package remain untouched. The final
verifier requires the **same two artifact hashes as that failed check**, retains
every other package assertion, and writes a new report/extraction. It does not
turn a missing guard into success or relax authentication in application code.

## Passed checks

- Exact source, generated Android inputs and external speech-runtime hashes.
- Fresh installer/ZIP and native output timestamps; old package preservation.
- Identical reviewed archive inventory, bounded safe paths, no bundled model or
  credential/configuration files and complete required binaries/licenses.
- Extracted executable hashes match the freshly built desktop and daemon.
- R2 manifest and attribution match source; R2 model origin is embedded in both
  executables; exact model bytes/hash verified before CPU fixture use.
- Desktop account-bound publishing/Settings guards, daemon execution guards and
  both new batched relay routes are present in the appropriate binaries.
- Actual bundled runtime version and **speech and silence fixtures both pass**.

Evidence under `.waku-cache/`:

- `ion-reconciled-retry3-windows-build.{log,exit}` — actual build exit 0.
- `ion-reconciled-retry3-windows-package-acceptance.json` — preserved first failure.
- `ion-package-service-guard-fixtures-20261022.{json,log}` — two native fixtures.
- `ion-package-binary-scope-acceptance.json` — scoped binary and negative checks.
- **`ion-reconciled-retry3-windows-package-final-acceptance.json` — passed**.

The original full queue is terminal exit 1 at its inaccurate package assertion;
do not relabel that old queue as successful. Android was never started by it.
A separate Android-only follow-up consumes the final Windows report and the
unchanged source/resource gates. It must not rebuild Windows. Existing prepared
nonvisual upgrade checks now require that final report and the separate Android
job's successful package gate before executing either installer.

No installation, device change, screenshot, microphone capture, service-switch
change or paid provider operation occurred in this milestone. Actual upgrade/
data-retention, physical speech, account/UI accessibility, sandbox image/pricing/
lifecycle and billing/traffic-attribution gates remain open. Final QA handoff
has not been sent.
