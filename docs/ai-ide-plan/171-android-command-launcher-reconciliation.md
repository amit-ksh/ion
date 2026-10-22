# Android launcher: native Windows executable path

22 October 2026, Asia/Kolkata. Windows package acceptance in plan 170 remains
passed, with both artifact hashes unchanged. No Windows rebuild is required.

## Original Android follow-up result

The Android-only follow-up is terminal exit 1. Its native process receipt and
build log show `The syntax of the command is incorrect.` before Gradle starts.
No APK package verification, phone installation or microphone action occurred.
Memory/disk/source guards passed; available physical memory was about 7.21 GiB.
The failed job, wrapper log, command log and exit marker remain preserved.

## Minimal reproduction and cause

Using the same Python subprocess launcher:

| Probe | Actual result |
| --- | --- |
| `C:/Windows/System32/cmd.exe /d /c echo ion-probe` | Exit 1, command-syntax error |
| Native Windows path `C:\Windows\System32\cmd.exe` with identical echo args | Exit 0, expected output |
| Forward-slash cmd path with `gradlew.bat --version` | Exit 1, same syntax error |
| Native cmd path with otherwise identical Gradle version command | Exit 0, Gradle 9.3.1 |
| Existing JDK directly invoking the same wrapper JAR for version | Exit 0, Gradle 9.3.1 |

The first two probes isolate the executable-path separators from Java, Gradle,
Android configuration and JVM argument quoting. The Gradle probes confirm the
installed JDK and existing cached distribution work. They execute no assemble
task. No JVM from the probes remains active.
Evidence: `.waku-cache/ion-android-launcher-separator-diagnosis.json`.

## Bounded recovery

Prepared a separate retry4 Android helper, verifier and follow-up. The launcher
uses the native Windows cmd executable path; build flags, offline mode, single
worker, ARM64 architecture, source/runtime hashes, resource limits, stderr capture
and PID/exit receipts remain unchanged. The follow-up consumes the same verified
Windows final report and never rebuilds or replaces those Windows artifacts.

All previous retry3 Android receipts/logs remain untouched. The new attempt uses
separate retry4 build and job names. Both prepared upgrade scripts now require
the new Android package gate, the existing final Windows package gate and a
successful follow-up before installing anything. No source implementation,
signing identity, app data, provider permission or spending control changed.

Commit this diagnosis before launching retry4 through approval. Actual APK
metadata/signature/bytecode/launcher verification and authorized nonvisual
upgrade/data-retention remain required, along with the original physical/UI,
sandbox image/pricing/lifecycle and invoice/traffic-attribution gates. No final
QA handoff has been sent.
