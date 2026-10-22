# Stopped package run: preserve results before guarded recovery

22 October 2026, Asia/Kolkata. This corrects the stale `building-windows`
receipt from the previously live retry2 run. It is not a package acceptance.

## Actual terminal evidence

- At the 10:19–10:20 IST audit, exec handle 54764 returned `Unknown process id`.
- A Windows Toolhelp census found no Cargo, rustc, Java, Bun or rustup process;
  the previously owned wrapper/compiler PIDs were also absent.
- The Windows native exit marker contains **1073807364 (0x40010004)** and was
  written at 05:27:40 IST. The last compilation log entry was credentials
  compilation; there is no compiler diagnostic establishing a source failure.
- No fresh Windows package acceptance report exists. Android never started.
  Existing EXE/ZIP/APK files retain their earlier 21 October timestamps.

Microsoft's [NTSTATUS reference](https://winprotocoldoc.z19.web.core.windows.net/MS-ERREF/%5BMS-ERREF%5D.pdf)
defines 0x40010004 as `DBG_TERMINATE_PROCESS`. This status does not identify
which actor caused this particular termination. Do not call it an out-of-memory
failure or a compiler defect without evidence. The original receipts and logs,
including their incomplete parent state, remain unchanged.

Evidence: `.waku-cache/ion-reconciled-retry2-terminal-observation-20261022.json`.
The queue now explicitly records that no build is active; a stale JSON phase
must never be treated as a verified running process.

## Recovery prerequisites and scope

The reviewed fd9a36a code, 50 Android native inputs, external speech files,
historical artifacts and native/worker gates pass the exact guards again.
The worktree/index are clean. Latest head 90e8c3e adds documentation only.
At first observation, physical memory was about 2.44 GiB, below the existing
2.5 GiB release guard; no process was stopped to free memory.

A separate retry3 helper/wrapper/verifier/continuation set preserves every prior
run. The existing native stderr capture remains in use. The helper additionally
records exact native child PID, start state and terminal exit status to a
separate per-step process receipt. That receipt is observational; liveness still
requires checking the actual tool handle/process. No source, authentication,
resource threshold, package assertion or offline build setting is weakened.

The capture function passes success-with-stderr and real exit-7 fixtures, and an
actual short Bun stderr/sleep/completion case. All three record the native PID
and actual terminal status. Evidence:
`.waku-cache/ion-retry3-native-capture-acceptance.json`.
The PowerShell wrapper parses successfully.

Commit this record before launching the new guarded run through approval. Keep
tracked files unchanged while it runs. Launch only after actual older children
are absent and memory/disk/process gates pass; do not clean Cargo artifacts or
restart another session. Windows packages must actually verify before the
sequential Android ARM64 step. Install/device/microphone/UI and remaining real
sandbox image/pricing/lifecycle/billing checks remain unproven.
