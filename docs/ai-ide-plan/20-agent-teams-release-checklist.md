# Agent Teams release checklist

Updated 2026-10-06. Implementation branch: `codex/ion-agent-teams` in the managed
worktree. These changes have not been integrated into `ion-prod` by this task.
Use [implementation progress](19-agent-teams-progress.md) for checkpoint evidence
and [the confirmed specification](18-agent-teams-implementation.md) for scope.

## Implemented and covered by local regression tests

- [x] Reusable teams, direct reporting hierarchy and per-agent harness/model,
  instructions, expertise, expected output, permissions and concurrency.
- [x] Explicit Local / Nebius sandbox selection; no Auto mode. Confirmed unavailable
  sandbox can fall back locally with the same selected harness/model/grants.
- [x] Preserve normal New task; invoke a team with `@teamname`, `@{Team Name}`
  or the qualified picker address. Ambiguous names require explicit selection.
- [x] Normal native Nebius agents can invoke saved teams with source permissions,
  wait for their actual outcome and read accepted generated documents.
- [x] Durable task creation, pinned definitions, nested delegation, bounded private
  handoffs, transactional capacity and lease fencing across restart.
- [x] Actual local execution for native Nebius and the bounded Claude bridge.
  Other selected harnesses display their missing adapter rather than being replaced.
- [x] Parent acceptance/rework, linked correction tasks, output contracts, generated
  documents and child/dependency scope checks.
- [x] Important decisions and exact Composio account/action/version/argument approval,
  single execution with durable acknowledgement and AI-generated result documents.
- [x] Isolated coding workspace, serialized writers, bounded exact file writes,
  directory creation, file deletion and actual before/after change documents.
- [x] Source-task Stop propagation; closing an attachment does not stop owner work.
- [x] Consumed/unknown token accounting and aggregate model-call/token limits.
- [x] Explicit continuation from a safe checkpoint after a known Local worker exit,
  retaining task, attempt, host, grants, model and budget history.
- [x] Native cached run/hierarchy/task/document/status projections and background RPCs.
- [x] Human-reviewed combined source changes from native edit receipts, exact source
  conflict checks, explicit approval, durable application receipts and interrupted
  application reconciliation. Created/deleted files and directories are included.

These checked items record implementation and automated verification. They do not
assert live account acceptance, visual validation or production release readiness.

## Development still required for the full planned release

- [ ] Codex, OpenCode and other selected Teams harness adapters with proven permission,
  single-request budget, lifecycle, cancellation and receipt handling.
- [ ] Normal-task team invocation bridges for external solo harnesses.
- [ ] Registered Nebius execution host: readiness, bounded input transfer, actual
  chosen-harness launch, remote state/cancel/checkpoint retrieval and safe fallback.
- [ ] Executed test/check receipts and enforceable command isolation. Declared check
  labels and model-written test claims are not executed validation.
- [ ] Pinned price/currency and monetary/cloud spending reservations and usage.
- [ ] Confirmation of hosts left behind by owner restart and outcome reconciliation
  for pending file/app/structural effects. Safe checkpoint continuation does not
  authorize replay of an unknown effect.
- [ ] Additional resource/egress grants and host enforcement for general commands.
- [ ] Team attachment handoffs and team follow-up prompts.

Routines and dynamically generated workflow scheduling over this coordinator
remain subsequent workspace-control sprints; their presence in navigation is
not proof of a complete Teams execution backend.

## Your live review when resources are available

- [ ] Save the Nebius Token Factory key privately in Settings and Test it.
- [ ] Install/authenticate the exact selected Claude models/harness on Local.
- [ ] Configure Composio privately, connect an active account and grant exact actions
  to the lead and specialist so delegation can intersect the grants.
- [ ] Start a three-level team task in a clean committed Git project; follow the
  actual child task links, selected models and host labels through completion.
- [ ] Review one generated document and one correction task; verify unaccepted
  child output cannot finish the parent.
- [ ] Approve and decline exact app actions and inspect real provider outcomes.
- [ ] Create a source directory/file and remove a small obsolete file in the isolated
  workspace; inspect the change document and confirm the source checkout is intact.
- [ ] Open a completed coding run's Review changes, inspect the combined diff and
  Apply reviewed changes. Verify only those changes appear in the original project
  and remain uncommitted. Repeat Apply and confirm newer edits are preserved.
- [ ] Change the original source while reviewing. Verify conflicts prevent Apply;
  restore a compatible clean baseline and Refresh review before approval.
- [ ] In a disposable project, interrupt source application. Inspect the receipts
  and private backups, repair the source, and explicitly confirm its outcome.
- [ ] Restore provider access after a temporary request failure and use Continue
  from checkpoint; verify task identity and budget history remain unchanged.
- [ ] Stop a source/team task and verify real children stop; close an attachment
  separately and confirm the owner run continues.
- [ ] Check keyboard operation, light/dark legibility and long run/transcript lists.
- [ ] Supply a Nebius target and bounded cloud budget for remote acceptance after
  the registered host adapter is implemented. A key alone is not a remote host.

## Verification baseline

Current protocol is **23**. Rebuild desktop, daemon and clients together. The
progress document records the exact automated checks for each committed increment.
No live provider billing, app account mutation, cloud deployment, visual test or
second development watcher was performed during these checkpoints.
