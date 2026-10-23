# Installed active cancellation passed; further privileged acceptance blocked

23 October 2026. The completed test below ran on 22 October at 12:35 IST;
its preserved terminal receipt was inspected again on 23 October. This is not
a new device or runtime test on 23 October. Reviewed code remains `fd9a36a`;
the recording baseline `0c3bad8` added previously recorded milestone documentation.

## Actual cancellation during a streamed reply

The existing installed daemon ran one transient Codex runtime in Ask mode,
computer use disabled, in the isolated acceptance folder. It received one
bounded no-tools counting prompt. The client sent Cancel on the first nonempty
text event, before completion, and observed:

- An active reply before cancellation.
- Cancel acknowledgement.
- A terminal unsuccessful/interrupted turn **448 ms** after the cancel request.
- Close acknowledgement and no runtime remaining when reattached.
- No permission answers, settings changes or app task-state writes.

This directly extends plan 175, whose Cancel acknowledgements occurred after
successful completion and therefore did not establish active cancellation.
No physical microphone, screenshot, UI operation, Nebius workload or global MCP
change occurred. The single runtime's unsuccessful turn is the expected result
of cancellation, not a harness launch failure.

Evidence: `.waku-cache/ion-installed-active-cancel.json`, terminal `passed: true`.
The test helper is `.waku-cache/accept-ion-installed-active-cancel.ts`.
This proves the measured Codex/native protocol path; it does not pass active
Claude cancellation, Teams cancellation, outage recovery or mobile/UI behavior.

## Readiness inspection remains unresolved

A separate shell-selected Codex help probe returned exit 0 with 509 output
characters, and none of the four expected Teams-isolation flag substrings.
This is insufficient to decide whether it selected the same executable as the
installed daemon, whether the output was actual Codex help, or whether the
Teams adapter is compatible. No unsupported-CLI conclusion is claimed.

The next action was to inspect that public help text and the selected executable.
Automatic approval review rejected the action before execution because its usage
limit had been reached. The returned message gave **27 October 2026, 3:16 PM**
as the retry time, or restoration of review access. It explicitly identified a
review-availability failure rather than a determination that the action is unsafe.
No alternative execution path was used to bypass that decision.

No Teams/routine/workflow run was launched as part of this readiness check.
No new process or provider request is awaiting observation from the completed
cancellation test; its original tool session returned terminal exit 0.

## Remaining work and next actions

1. Restore automatic approval-review access, then inspect the exact executable
   selected by the installed daemon and its supported Teams isolation contract.
   Preserve the original ambiguous shell probe rather than calling it a pass.
2. Complete bounded native Teams delegation/review and routine/workflow execution,
   including approval refusal, active cancellation and recovery. Do not replace
   real-provider checks with the already-passed local fixtures.
3. Finish prolonged/matched idle and active relay observation, actual account
   switching and native interaction/device checks under their existing scopes.
4. Obtain provider-compatible immutable sandbox images and confirmed project
   operation/upload/retention price bounds before enabling or dispatching paid
   sandbox work. Existing USD10 monthly target / USD15 cumulative ceiling holds.
5. Reconcile actual provider billing and the original Cloudflare meter/plan.
   No data deletion, paid-plan upgrade or permission expansion is authorized.
6. Commit this completed cancellation milestone independently when review access
   permits the Git write. At creation this document was uncommitted and the last
   milestone was `0c3bad8`; its prepared report is now being submitted as a
   separate documentation milestone. New UI work remains uncompiled and outside
   that report's acceptance. Final QA handoff remains unsent.

Windows/Android package and upgrade passes from plans 172–173, native account/
publisher passes from 174 and supported solo-harness passes from 175 retain
their recorded scope. Full goal completion remains unproven.
