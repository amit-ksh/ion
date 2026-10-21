# Mobile MCP warning integration

21 October 2026. Ported the integration-specific warning presentation from
redesign commit `81827ff` into `ion-prod` without merging that branch.

## Change

- The mobile composer names a failed optional MCP server, such as GitLab,
  instead of presenting its HTTP/HTML response as an agent startup failure.
- HTML agent connection failures get a short connection message. Useful plain
  agent errors, including a Codex thread-open timeout, remain unchanged.
- The original runtime error remains in runtime state; presentation changes
  only. No endpoint, OAuth, account, provider capability or global setting changes.
- A separate `runtime-error-summary.ts` module avoids editing the existing
  QA-modified disconnect helper. The composer change is applied independently
  to its committed and working versions; reversing it restores the original
  working bytes exactly. Existing QA changes are retained outside the commit.

## Evidence and limits

Six focused tests passed, with 16 assertions, covering the new presentation and
existing stale-disconnect handling. Mobile TypeScript passed. No APK was built
or installed, no microphone was used and no native visual acceptance is claimed.
The source correction still needs a newly built package and actual UI acceptance.

Both parallel review agents failed at the account usage limit before producing
findings. The broader redesign source review is incomplete; their failures are
not a passing review. Its inherited QA snapshot is `c9d94d4`, and the selected
review endpoint is `81827ff`. New UI tasks in plan 146 remain queued.

Release preflight observed 1.18 GiB physical and 2.50 GiB virtual available
memory, below the existing 2.5/4 GiB guards. No build started or other process was
stopped. Historical candidate input receipts are preserved; a new explicit
source snapshot is required before packaging this correction.
