# Next Ion release candidate: source and build preparation

21 October 2026. Current candidate source is
`91e275884af394c29844e76413c05705b0b4772a`, plus the preserved 111-file QA patch.
This is preparation and input verification, not a completed build or acceptance.

## Verified inputs

- Exact QA patch SHA-256 remains
  `eccec9c02d57993784a2b0ad683c69b896655cfb8a408fe93c60556a9d799ee4`.
- Six untracked build-source hashes, 40 generated/native Android input hashes
  and both previously verified Windows package backups match their snapshots.
- Changes after the prior `69c7ec7` snapshot are reviewed docs, the sandbox
  AI-project parser/test, and the mobile dictation hook/test/isolated fixture.
  Windows native source is unchanged from that snapshot; mobile requires a new
  bundle. The separate redesign is not merged or included.
- The source/input verifier passed for the new candidate. Historical snapshot,
  verifiers, packages and receipts are preserved with their original names.

## Prepared build and verification

The ignored cache contains a distinct candidate series:

- `ion-final-candidate-release-inputs.json`
- `verify-ion-final-candidate-inputs.py`
- `build-ion-final-candidate-packages.ps1`
- `verify-ion-final-candidate-release.py`
- `verify-ion-final-candidate-android-package.py`

The PowerShell build syntax check passed. It starts no wait helper, refuses an
existing candidate job report, checks source/QA freshness, requires no competing
Cargo/Rust/Java jobs, and requires at least 2.5 GiB physical / 4 GiB virtual
available memory. Disk guards are unchanged. Windows and ARM64 Android build
sequentially with the existing locked/offline settings and release profile.
No installation or device interaction is automatic.

Windows acceptance retains package inventory, account-bound publishing/service
guards, R2 model manifest/notices and bundled CPU speech/silence checks. Android
acceptance retains package/ABI/minimum SDK/signing identity and both account
guards, and additionally requires the microphone Stop recovery and active-stream
guard in the compiled bundle. A new uniquely named APK preserves the prior copy.
String presence is packaging evidence; lifecycle behavior is covered separately
by plan 143, and actual device acceptance remains in plan 132.

## Current result

At **17:50 IST**, preflight observed 1.81 GiB physical and 4.34 GiB virtual
available memory, with no Cargo/Rust/Java build. Physical memory was below the
guard, so **no candidate build started**. No other process was stopped. A future
start must recheck actual resources and source; this receipt is not a live job.

Existing EXE/APK files still predate the service-control and dictation changes.
RD-04's source-only warning copy also remains in the separate redesign and is
not included here. Deliberate authorized integration is needed before that
correction can be claimed in a later package; this preparation does not grant
another redesign APK build or merge.
