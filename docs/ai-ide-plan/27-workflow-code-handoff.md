# Code handoff between workflow stages

Updated 2026-10-11. Implementation is on `codex/ion-agent-teams` in the managed
worktree. Protocol **37** requires matching desktop, daemon and phone clients.

## Behavior

Later stages have an explicit **Inherit code: off / on** control in the desktop
workflow editor. Old workflows keep this off. Enable it for a stage that must
review, test or extend the previous stage's code. The first stage cannot inherit.
Prompt generation can propose the setting; it remains editable before saving.
Desktop and phone details display the pinned setting. A started stage also records
its actual source team run, including when a workflow branch skips other stages.

The previous stage must have completed in the same project with an isolated
coding workspace. The owner verifies its Git identity, original base, settled
file/app/command/GPU effects, exact file receipts and final bytes. It rejects
unreceipted modifications. Receipted UTF-8 replacements, file creations,
deletions and directory creations are supported within the existing limits:
256 cumulative paths, files of at most 16 KiB and a 16 MiB manifest bound.

A durable reservation is created with the next team in the dispatch transaction.
The expensive Git/file checks run in the background workspace preparation path,
outside that transaction, reusing the workspace read connection. The owner pins
an immutable manifest and SHA-256,
creates a separate worktree at the original base, and installs its exact changes.
The worktree becomes dispatchable only after successful verification. The user's
source checkout and preceding stage worktree are preserved.

Human stage gates, the selected harness/environment, fallback policy, existing
read/write grants and workflow-wide calls/tokens/assignment/deadline limits still
apply. A read-only test/review team may inherit code without receiving write
permission. Local and existing Nebius snapshot execution use the same installed
workspace. Live Nebius acceptance remains separate.

When a later stage edits an inherited file, its receipt must start at the prior
stage's exact final content. Changes accumulate against the original project base;
a create followed by delete disappears from the final file manifest. Original
edit IDs remain attached. Generated change documents and the final team's source
integration review include inherited changes plus that team's own edits. Applying
the final changes uses the existing explicit human source-review action. Nothing
commits, publishes, deploys or merges automatically.

An opted-out stage starts with the existing independent-workspace behavior. To
carry code through a multi-stage chain, enable inheritance on each receiving
stage, including intermediate read-only review/test stages.

## Interruption and conflicts

An interrupted installation is marked uncertain on owner restart. No agent lease
can use an incomplete workspace. A retry adopts the existing copy only if all
pinned final files/directories are exact and no private installation staging files
remain. It never repeats a partial or unknown copy. Its setup message identifies
the recorded workspace and previous stage to inspect. Review the previous stage's
change documents/source preview and preserve any staging evidence before manual
repair. After restoring the exact pinned result and resolving staging files,
retry readiness to verify and adopt it. Otherwise stop that run and start a
corrected workflow; partial work is retained for inspection.

The original base remains pinned even if the user commits new source changes
between stages. The final source review reports the changed base/conflicts and
rejects application until they are resolved. A missing prior coding workspace,
changed receipt, extra unreceipted file, moved project or changed repository blocks
readiness rather than falling back to unrelated source files.

## Review checklist

- [ ] Create implementation → read-only review/test → final review, using saved
  teams and **Inherit code: on** on both receiving stages.
- [ ] Verify old workflows and new stages default off; the first stage rejects on.
- [ ] Confirm each receiving task sees the previous stage's code in a separate
  worktree while the original source files remain unchanged.
- [ ] Make replacements, new files/directories and a deletion. Extend one file
  in the second stage; inspect original before/final after and all edit IDs in
  the final source review. Apply only after reviewing that exact final diff.
- [ ] Verify the read-only team can read inherited files and cannot edit them.
- [ ] Branch directly to a later gated stage. Verify skipped tasks do not exist,
  the selected gate must be approved, and code comes from the actual predecessor.
- [ ] Change a saved workflow/profile during a run. Its original setting/grants
  must stay pinned, while the next run receives the edit.
- [ ] Set small shared budgets, stop or reach the deadline. Confirm inheritance
  cannot bypass a gate, add grants or dispatch a new agent after exhaustion.
- [ ] In a disposable fixture, add an unreceipted file or alter an accepted
  receipt. Readiness must fail without modifying the original project.
- [ ] Interrupt installation, restart and inspect the recorded workspace. Partial
  files must not be overwritten; only an exact complete result can be adopted.
- [ ] Commit new source work between stages; confirm it remains intact and final
  source application reports the changed-base conflict.
- [ ] Review native keyboard controls, themes and matching phone details later.

Automated verification: 24 workspace/source tests, 29 workflow coordinator tests
and the native dispatch fixture passed. The five new handoff groups were rerun
against the final implementation. Desktop/daemon/worker compilation, shared and
full phone TypeScript, generated protocol 37 equality, formatting and whitespace
checks passed. No visual test, live provider call or billable provisioning is
included. See the progress record for the native fixture timing correction.
