# Sandbox beta acceptance queue

21 October 2026. The owner reports receiving Nebius sandbox beta access and
requests that sandbox testing be added to the task queue. This supersedes the
assumption that beta activation has not yet been requested; it does not by
itself establish the effective permissions of the saved key and project.

Scope: existing supported Windows/Android/native execution paths, managed Token
Factory sandbox service and the existing combined budget. Static app publishing
remains on Cloudflare. No new harness adapter, iOS or app-host VM is included.

## Fresh access observation

At **13:20 IST**, the documented read-only `/sandboxes/v1/whoami` check using
the existing local operator key returned:

- `project-e04qppz9bc0026zt9y9ky7`: HTTP 200, with `spawn`,
  `spawn_disposable`, `list`, `cancel`, `import` and `set_image_tag` false.
- `project-e04f8x7qbc00cshy8v3t19`: HTTP 500. This is an inconclusive provider
  error, not evidence that access was granted or denied for that project.

The first response reports limits of 50 concurrent operations and 3,600 seconds
maximum timeout. Those provider limits neither authorize a launch nor set Ion's
pilot spending policy. No image import, upload or sandbox operation was issued.
Sanitized evidence: `.waku-cache/ion-sandbox-beta-access-20261021.json`.

## Ordered tasks

| ID | Task | Status | Dependency and required evidence |
| --- | --- | --- | --- |
| SBX-01 | Verify beta access for the intended key/project | In progress; access unresolved | Confirm actual `spawn_disposable`, `list` and `cancel`; inspect the transient 500 separately. Preserve existing credentials. |
| SBX-02 | Discover and inspect compatible worker images | Queued | SBX-01; immutable image UUID, exact manifest/worker entrypoint, supported harness and reviewed network/resource policy. |
| SBX-03 | Verify price, retention and combined-budget bounds | Queued | Provider-confirmed operation/upload/retention units and ceilings; current durable usage and unknown holds; stay below the existing $10 target/$15 cumulative ceiling. |
| SBX-04 | Enable only the reviewed managed sandbox registry | Queued | SBX-02–03; backend-only credentials, bounded production policy, correct account-bound native config. Save initial service controls for restoration. |
| SBX-05 | Run one short exact-approved command | Queued | SBX-04; real dispatch receipt, output and process exit. Verify refusal/changed arguments cause no dispatch, foreign account cannot access results, and nonzero exit remains a failure. |
| SBX-06 | Verify bounded file transfer and artifacts | Queued | Approved disposable fixture only; upload/download size and checksum, owned artifact receipt, no unrelated project files or credentials transferred. |
| SBX-07 | Verify cancel, timeout and recovery | Queued | Cancel a bounded operation; observe actual terminal state. Reconnect/restart observation without repeating an uncertain spawn or command. No local fallback after uncertain dispatch. |
| SBX-08 | Verify owner stop and restore controls | Queued | Block new work, confirm cancellation/terminal state for owned live operations, then restore the initial service switches. Never cancel another user's work. |
| SBX-09 | Reconcile resources/costs, commit and update acceptance | Queued | Enumerate every resource created by these tests, confirm no running compute remains and account for retained billable objects/unknown holds. Record actual invoices separately from raw diagnostic `cost`. |

Tasks are queued for execution when their prerequisites pass. None of SBX-02–09
is marked passed by local fixtures. SBX-05 onward must use the actual managed
gateway/native boundary; direct provider smoke tests alone do not prove Ion's
end-to-end behavior. Actual native UI interaction remains subject to this
session's existing no-visual-testing restriction and the device checklist.

No paid sandbox starts merely because beta access or the service switch is ON.
Keep the registry unavailable until image and price bounds are verified. The
existing local regressions in plan 125 remain useful evidence but do not replace
this live queue.

References: [effective limits and access query](https://docs.tokenfactory.nebius.com/sandboxes/operate/limits-retention-and-usage),
[managed sandbox implementation](125-managed-token-factory-sandbox.md),
[combined budget](121-combined-spending-envelope.md),
[physical/native acceptance](132-physical-device-acceptance.md).
