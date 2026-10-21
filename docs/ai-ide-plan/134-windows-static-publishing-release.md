# Windows static-publishing release package

21 October 2026. Package build and artifact/runtime acceptance are complete.
Installation, native GUI interaction, physical microphone and paired-phone
acceptance remain open; these results do not stand in for those gates.

## Build and source

- Native source: `7ffbbdd08fb56740e6fd02d40a1bc2a9e009a3e6` on `ion-prod`.
- Preserved 111 pre-existing dirty QA files, patch SHA-256
  `eccec9c02d57993784a2b0ad683c69b896655cfb8a408fe93c60556a9d799ee4`.
  Untracked build-source hashes are recorded in the private build provenance.
- Build completed successfully in **43 minutes 38 seconds**, with existing
  production release optimization unchanged. Inno Setup completed successfully.
- Later committed source changes were docs and backend-only erased type/test
  changes. Exact changed paths were reconciled; native/mobile source and QA
  patch stayed unchanged. The emitted Worker bundle was byte-for-byte identical.
- The independent redesign worktree is not included in this package.

## Artifacts

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `ion-0.1.24-x86_64-Setup.exe` | 33715456 | `c2ca09edeb27e77fd0e1b0cfb9917ad5aac508e43aa56eb721f6c78cc24af16a` |
| `ion-0.1.24-x86_64-pc-windows-msvc.zip` | 48677952 | `35d95ba9f6399363d3227163b68edf5bb0b73a1e42772c27d6ce391977b2ef49` |

These earlier artifacts are now preserved under
`.waku-cache/ion-pre-account-binding-windows-20261021`; current files under
`target/release` are the newer candidate in [137](137-account-bound-windows-release-acceptance.md).
No Authenticode certificate is configured;
this is an **unsigned test release**, so Windows trust warnings may appear.
Neither certificate installation nor application installation was performed.

## Verified package behavior

- Bounded ZIP member/path checks and required Ion/daemon/helper/runtime files.
- No GGUF/Nemo model weights or credential/key files bundled in the package.
- Exact reviewed speech manifest and attribution notice are included. The notice
  uses the full Creative Commons Attribution 4.0 International name and official
  URL, with NVIDIA/Suno attribution and conversion disclosure.
- Both `ion.exe` and `waku-daemon.exe` embed the R2 model download URL.
- Extracted NeMo-Speech.cpp CPU runtime reports version 0.1.0.
- The exact R2-downloaded model matches 146029600 bytes and the pinned SHA-256.
- Fixed speech transcribes to the expected sentence; silence produces no text.

The first verifier rejected post-snapshot backend type changes by design. The
second verifier expected a license abbreviation absent from the full-name notice.
Both failed reports are preserved. Final verification reconciled exact sources
and checked the actual bundled notice against its reviewed source plus required
license/attribution text; it passed without changing or rebuilding the notice.

## Evidence and remaining work

- `.waku-cache/ion-static-windows-build.log` and exit marker: successful build.
- `ion-static-release-inputs.json`: immutable build snapshot and QA input hashes.
- `ion-static-native-source-reconciliation.json`: audited post-snapshot changes.
- `ion-static-windows-package-final-acceptance.json`: passed artifact/runtime check.
- Final runtime SHA-256: `1e2110e2a39c789f0c3bd49eea7898dfe62591d4dbebb73f4cb549a5b516a8e5`.

Remaining: installer clean install/upgrade, actual app launch/account restore and
switching, desktop and Moto microphone/dictation, native chat/team/workflow flows,
Android package refresh, sandbox permissions/images/lifecycle and invoices.

Automatic approval review subsequently rejected the host-process inspection
because its usage quota was exhausted. No rejected host action was executed.
Access returned at the scheduled 7:05 AM IST continuation; the existing debug
watcher/app were observed running and preserved. This package's source record
remains unchanged while the account-bound release refresh is prepared.


## Subsequent source hardening

A later source audit found a queued native publishing request could select the
account signed in after its review opened. An account-bound transport and view
invalidation fix is being verified in plan 135. The EXE/ZIP above predates that
fix: its verified packaging/runtime results remain valid for its recorded source.
The refreshed package and actual upgrade acceptance are now recorded in plan 137.
Actual GUI account switching remains open; do not claim the old artifact contains
new source.
