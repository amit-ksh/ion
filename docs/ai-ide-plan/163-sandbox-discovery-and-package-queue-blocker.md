# Sandbox discovery and package-queue blocker

22 October 2026, Asia/Kolkata. The implementation goal remains active. This
report adds current evidence to plans 141, 147 and 162; it does not mark any
remaining provider, device or release acceptance complete.

## Bounded provider discovery

Using the existing saved key and owner-confirmed project
`aiproject-e00h49rtswkv9bz46s`, a metadata-only request sequence read eight
100-image pages. All eight returned HTTP 200. The scan verified **800 distinct
image UUIDs**, with no repeated IDs and no Ion-tagged candidate in those pages.
The final page was full, so discovery is **partial**, not proof that the whole
registry lacks an Ion worker. A tag would not establish file/kernel/harness
compatibility without inspection in any event.

No image was imported, tagged or uploaded, no operation was submitted, no
service was enabled, and no paid workload was started. The operator key was
loaded only locally and is absent from the sanitized report:
`.waku-cache/ion-sandbox-image-page-observation.json`.

The [official images manager reference](https://docs.tokenfactory.nebius.com/sandboxes/sdk/python_sdk/reference/images)
documents catalog inspection separately from import-capable helpers. Do not use
an automatic resolve/import helper merely to inspect a candidate: it can trigger
an import when the image is absent.

## Pricing gate remains open

The current [sandbox overview](https://docs.tokenfactory.nebius.com/sandboxes/overview)
describes fresh compute per operation and identifies the service as Beta. This
does not establish zero cost. The linked
[limits and usage guidance](https://docs.tokenfactory.nebius.com/sandboxes/operate/limits-retention-and-usage)
requires confirmed units before interpreting raw operation cost, and billing
records for invoice reconciliation. The current
[billing guide](https://docs.tokenfactory.nebius.com/other-capabilities/billing-new)
describes organization Usage filters and card settlement; it supplies no numeric
sandbox operation/import/upload/retention ceiling for this project.

Accordingly, do not convert effective API permissions, available credits or an
undocumented `cost` value into a spending guarantee. The existing USD10 monthly
target / USD15 cumulative ceiling still requires a supported price ceiling and
known combined usage/holds before provider import or execution. Production
`SANDBOX_POLICY_JSON` remains absent as recorded in the prior rollout.

## Package continuation did not launch

The sequential Windows EXE/ZIP and Android ARM64 wrapper, input verifiers and
native/resource gates were prepared in plan 162. A local continuation was then
prepared to wait for the pinned native gates and launch exactly one wrapper,
without installing apps, capturing microphones or modifying devices.

**Automatic approval review rejected the continuation launch because its usage
limit was exhausted.** The tool explicitly reported that the action was not
executed and that this was an approval-review failure, not a determination that
the action was unsafe. Its stated retry time was **22 October 2026, 02:18 AM**.
No alternate route was used to launch it. Restore approval access or retry the
same reviewed action after the stated reset; do not bypass that review.

Prepared but unexecuted:
`.waku-cache/continue-ion-current-packaging.py`.
The receipt `.waku-cache/ion-current-package-continuation.json` was confirmed
absent after the rejection. No new package build, installer execution or APK
installation is claimed.

## Existing jobs preserved

- Native owner recovery job **32910** remains live; actual Rust process 63040
  continues consuming CPU. Quiet output is not treated as failure or permission
  to restart it.
- Native follow-up **92797** remains live, pinned to fd9a36a. It waits for the
  existing job's successful result and adequate memory before running the
  required presence/scope/combined/protocol checks. It is independent of the
  rejected package continuation and was already authorized and running.
- At the latest observation, physical memory recovered above 2.5 GiB, but the
  original Cargo/rustc job still prevents a competing build. No other process,
  emulator, watcher, user app or redesign worktree was stopped or changed.

The delayed release-input check, session 39819, subsequently returned **exit 0**:
fd9a36a code, all 50 Android inputs, external speech files, historical package
backups and preceding verifiers still match. Its observation delay did not
justify restarting it. The later documentation-only commit is permitted by the
candidate's existing guard; this report is not new build source.

A read-only Windows Toolhelp census found zero processes with the exact known
Ion/Waku app/daemon executable basenames. It confirmed the original compiler and
its two Cargo parents were present. This is a narrow current Windows process
observation, not proof about WSL processes, internal publisher loops, previous
duplicate instances or the source of live Cloudflare traffic. No command lines,
credentials, daemon IPC or network traffic were inspected. Evidence:
`.waku-cache/ion-relay-process-census-20261022.json`.

Preserve the original check logs and both job receipts. If the native follow-up
times out, verify the original handle/process first, then prepare a separately
pinned continuation after observing a real state change. Never restart the
original job merely because a wait expires.

## Next authorized sequence

1. Capture the original owner result. Let the existing native follow-up execute
   the four pinned gates when its conditions pass; inspect any failure honestly.
2. With approval access restored and native/resource/source gates passed, launch
   the prepared package continuation once. Verify its actual output hashes,
   speech fixtures, account guards, signature and preserved previous packages.
3. Complete authorized native/device/upgrade acceptance when the user/device is
   available. No unattended microphone or unrequested native visual test.
4. Finish sandbox image/kernel/harness and price verification, then its bounded
   live lifecycle and invoice/resource reconciliation within the original limit.
5. Complete the remaining acceptance matrix and only then send the authorized
   **Test and fix Ion** handoff. No final QA handoff has been sent.

This report was written locally. Its milestone commit remains pending while
automatic approval review is unavailable; existing committed source and the
deployed worker are unchanged.
