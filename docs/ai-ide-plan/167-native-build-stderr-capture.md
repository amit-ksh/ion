# Native package logging: preserve stderr and real exit status

22 October 2026. Release continuation retry 66504 is terminal **exit 1**.
Original logs and receipts remain preserved. No installer/APK pass is claimed.

## Reproduced cause

Windows PowerShell 5 with `ErrorActionPreference=Stop` and `*>` converts normal
native stderr to `NativeCommandError`. The wrapper stopped on Cargo's ordinary
`Compiling waku-protocol` progress line, not a reported compiler diagnostic.

The minimal native fixture writes `Compiling harmless fixture` to stderr,
prints completion to stdout and returns 0. The original PowerShell invocation
returns 1 with that stderr as `NativeCommandError`; this reproduced locally.
The exact diagnostic is preserved in `.waku-cache/ion-native-stderr-diagnosis.json`.
A Toolhelp process census found no Cargo/rustc/Java/Bun/rustup build child after
the terminal wrapper, so no running compiler was restarted or killed.

## Fix and regression

A separate retry2 helper uses Python subprocess stdout/stderr capture directly
to an exclusively created log file, then records the native exit code. The
PowerShell wrapper receives only a JSON status on stdout and checks the actual
exit code. A real nonzero build still stops the wrapper. Windows and Android
commands, source/security checks, CPU/memory/disk bounds, offline settings and
sequential build order are unchanged.

The actual capture function was tested through PowerShell Stop mode: the
stderr-writing success exits **0**, and a failing fixture's exit **7** is
preserved. The driver itself returns 0 only after asserting both outcomes.
Evidence: `.waku-cache/ion-native-stderr-fixed-acceptance.json`.
The retry2 PowerShell wrapper also parses successfully.

Prepared separate files preserve every previous attempt:

- `run-ion-reconciled-retry2-build.py` — raw native log capture and exit markers.
- `build-ion-reconciled-retry2-packages.ps1` — identical guards and build commands.
- `verify-ion-reconciled-retry2-{release,android-package}.py` — unchanged package
  assertions with new build-exit, extraction and report paths.
- `continue-ion-reconciled-retry2-packaging.py` — pins the new helper, wrapper,
  verifiers and original source snapshot before its single guarded launch.

This repairs packaging infrastructure, not application authorization or compiler
semantics. Commit this diagnosis before retry launch and keep tracked source
unchanged while the strict package source guard runs. Actual package, install,
physical microphone, native visual and external sandbox/billing gates remain open.
