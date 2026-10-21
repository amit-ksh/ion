# Release approval restored; guarded packaging resumed

22 October 2026, Asia/Kolkata. This supersedes the current-state approval
blocker in plans 163–165, while preserving those original observations.
It does not complete the Ion goal or claim new packages exist.

## Verified progress

- Branch remains `ion-prod`; reviewed code is fd9a36a. The strict source/native/
  worker gates passed again, including all 50 generated Android inputs and
  external CPU speech runtime hashes. Historical failed receipts are preserved.
- Preflight found that the reconciled wrapper referenced a missing
  `measure-ion-reconciled-release-resources.py`. Restored it as a byte-identical
  copy of the existing read-only `measure-ion-current-release-resources.py`.
  SHA-256: `049cc24a4bd4baba23eff1abfedcf50d4bda2ab372f9eb8a2c83f90d361c96c7`.
  No threshold, security guard or production source changed. Evidence:
  `.waku-cache/ion-reconciled-resource-helper-repair.json`.
- Retried the prepared continuation through the same elevated automatic approval
  system after its stated 02:18 AM reset. Review approved the action. Actual exec
  session **58114** launched the single continuation; it was not bypassed.
- The continuation reverified source/native/worker gates and is waiting for
  resource headroom. At launch, available physical memory was about 1.77 GiB,
  below the unchanged 2.5 GiB threshold. Commit headroom and disk passed; no
  competing Cargo/rustc/Java process was found. No process was stopped.

The owned continuation has a 90-minute total bound and checks memory every
30 seconds. Before building it checks for competing jobs and verifies exact
inputs again. It builds Windows first, verifies its actual package outputs,
then checks resources before Android ARM64. It installs neither package and
starts no native visual or microphone test. Do not start another build while
this tool handle is live. Current state must come from the actual handle and
`.waku-cache/ion-reconciled-package-continuation.json`, not this snapshot.

## External gates unchanged

The current official [sandbox usage guidance](https://docs.tokenfactory.nebius.com/sandboxes/operate/limits-retention-and-usage)
was rechecked. It instructs users to confirm units before interpreting raw
operation resource/cost fields and to use billing records for invoice
reconciliation. It still does not provide a numeric operation/upload/retention
ceiling for this project. Beta access is verified; it is not a price guarantee.
No image import, paid operation, service switch or permission expansion occurred.

Compatible immutable provider image/kernel/harness, confirmed price bounds,
managed sandbox lifecycle, actual physical Android dictation/accessibility,
new-package upgrade/data retention and post-client-rollout Cloudflare usage
attribution remain unproven. No final QA handoff has been sent.

## Historical milestone reports

Plans 163 and 164 describe their then-current jobs, approval limit and incomplete
native checks. Plan 165 records the native gates' later successful completion.
Their obsolete job/status wording is historical, not the current running state.
This report records restored review access and the new actual continuation.

## First continuation stopped before compilation; separate retry prepared

The original session 58114 returned **exit 1**, rather than remaining live.
Memory had recovered to 2.86 GiB and the wrapper entered preflight, but its
second strict source check overlapped the completion-checklist documentation
edit. It correctly refused an uncommitted tracked worktree. This was caused by
this session's concurrent documentation work, not an application compilation
failure. Neither Windows nor Android build logs/exit markers exist, so the
continuation's `buildStarted=true` means wrapper entry, not a compiler launch.

The checklist is now committed as `7813306`; the native reports are committed
as `4bdd8ba`. Index/worktree are clean, exact candidate code/native/runtime
inputs and native/worker gates pass again. Original failed continuation, wrapper
state and log are preserved. No source/security check or resource threshold
was relaxed.

A separate `continue-ion-reconciled-retry1-packaging.py` and wrapper were
prepared. The wrapper changes only its owned job receipt filename, while
retaining the original guarded build sequence and verifiers. The continuation
uses new receipt/log names and pins the retry wrapper before launch. Preparation
evidence: `.waku-cache/ion-reconciled-package-retry1-preparation.json`.
No compiler, Gradle, installer, phone or paid workload was started by the failed
attempt. Launch the retry through approval only after this report is committed;
keep tracked files unchanged while its strict preflight/package checks run.
