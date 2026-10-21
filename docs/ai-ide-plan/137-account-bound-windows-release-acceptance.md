# Account-bound Windows release and upgrade acceptance

21 October 2026. This supersedes the candidate hashes in plan 134. Native
publishing/account-boundary implementation is in plan 135; the separate visual
redesign is not included.

## Source and build

- Native snapshot `d7e69ad686213017ca19b17f6cc287d4f3e06774` on `ion-prod`, plus
  the preserved 111-file QA patch, SHA-256
  `eccec9c02d57993784a2b0ad683c69b896655cfb8a408fe93c60556a9d799ee4`.
- The resumed build ran with the mobile fix in `65a36d6` also committed. Exact
  post-snapshot differences were reconciled as mobile-only source and docs;
  Windows inputs and the six recorded untracked build-source hashes were unchanged.
  This is not a release from a pristine checkout.
- Release compilation passed in **65 minutes 59 seconds**; Inno Setup passed in
  **74.875 seconds**. Production optimization settings were unchanged.
- The first attempt failed with linker LNK1108 when its C: temporary file could
  not be written. Evidence was preserved; subsequent builds used a checked
  workspace temporary directory on D:. A later missing job handle was reconciled
  against absent compiler/packager processes before resuming. No live build or
  other session's compiler was interrupted.

## Current artifacts

| Artifact under `target/release` | Bytes | SHA-256 |
| --- | ---: | --- |
| `ion-0.1.24-x86_64-Setup.exe` | 33703204 | `6a3a424aaf0f54d893bb3bf567899141c6cae2278f3627f4b06aaa1c4eca15ef` |
| `ion-0.1.24-x86_64-pc-windows-msvc.zip` | 48679911 | `9215e074c5355caf361bf150939c183eda2eac1472bf02f555c4a79e41aff357` |

These remain **unsigned Windows test packages**. The earlier accepted candidate
is preserved under `.waku-cache/ion-pre-account-binding-windows-20261021`.

## Verified package

- Exact reviewed ZIP member inventory, bounded safe paths and required files.
- No model weights or unexpected credential/configuration files bundled.
- Exact speech manifest and license/attribution notice, R2 model origin embedded
  in both native binaries, and the account-bound publishing guard in Ion.
- Exact pinned model size/hash and bundled NeMo-Speech.cpp CPU version 0.1.0.
- Fixed speech produced the expected sentence; silence produced no transcript.

Evidence: `.waku-cache/ion-account-bound-release-inputs.json`,
`ion-account-bound-windows-build-resumed.log` and exit marker,
`ion-account-bound-windows-package-acceptance.json`.

## Actual per-user upgrade

- No Ion/debug app was running at the fresh preflight. The original installation
  path `C:/Users/ksami/AppData/Local/Programs/Waku` was retained for compatibility.
- Backed up the installed program, a consistent SQLite database and existing
  daemon/app/navigation settings before running the installer. The backup's
  database integrity check passed. No credential-vault clearing was requested.
- The installer completed successfully, reported no Windows restart required
  and launched the installed `ion.exe`. All **75 packaged files** matched the
  installed bytes; the old `waku.exe` was removed. Registry display name is Ion
  (`ion`), version 0.1.24.
- Database integrity passed after launch. All original IDs remained: **14 chats,
  four projects and 67 messages**. Contents were not printed.
- A subsequent native admitted-account service read passed against the existing
  Windows credential vault. No model generation, provider effect or paid compute
  launch was requested. Actual account restoration in the GUI remains unverified.

The original PowerShell wrapper used `Start-Process -Wait`, which also waited for
the launched app. Its exit code was not observed while Ion remained open.
Installation was reconciled without rerunning it, using the completed installer
log, actual installed hashes/registry state and retained data. The evidence does
not invent an installer exit code. The helper now waits on the installer process
itself for future tests. The launched app was left running for the user.

Evidence: `ion-account-bound-installed-backup.log`,
`ion-account-bound-windows-upgrade.log`,
`ion-account-bound-windows-upgrade-reconciled.json`,
`ion-account-bound-upgrade-data-retention.json` and
`ion-account-bound-post-upgrade-account.log`, all under `.waku-cache`.

## Remaining gates

Fresh-user/clean-install acceptance, actual GUI account restoration/switching,
keyboard review, chat/Teams/workflow interactions and physical desktop/paired-phone
dictation remain open. No screenshots, native visual testing or unattended
microphone capture were performed. Token Factory sandbox access/images/pricing
and provider invoice reconciliation remain external gates. No Nebius resource was
created or started for these package/upgrade checks.
