# Gateway remote execution inventory and scoped task controls

## Implemented

Personal gateway **Gateway history → Remote executions** shows saved model-call, command and GPU receipts across the selected owner’s projects. Defaults are **All projects / All types / Unresolved**. Project/type/resolution changes reset pages; changing owner or pairing resets all filters and paging.

The typed portfolio query uses the existing **Read owner usage** (`usageRead`) permission. It reads the existing native remote inventory; no provider query, job launch, cancellation, cleanup request or monetary reconciliation is performed while listing or refreshing it. The collection uses immutable `(kind, request UUID)` cursors with up to 32 entries per page and a 60,000-byte transfer ceiling. A truncated response retains whole rows and the last retained cursor. Native cursor membership verifies the persisted project/type collection, including an operation that has since become terminal.

Rows preserve the saved state, whether an original operation/job/cancellation handle exists, stop-requested status and cleanup state. They show local task/project IDs only for navigation and local request UUIDs only for paging. Provider project/operation/job IDs, run/assignment/attempt/agent source IDs, command bodies, environment, inputs, policies and credentials are excluded from gateway inventory responses. Team labels and state labels are capped at 80 characters; objective summaries at 500 characters.

## Original task navigation

**Open team task** requires the independent **Read tasks** (`taskRead`) permission. It pins the selected owner and the original root task/project. A new read-only lookup `GET /v1/device/tasks/{taskId}?publisherId={owner}` retrieves one matching published task without walking the recent-task directory. A task from another publisher returns unavailable, and the client rejects an owner/task/project mismatch or invalid runtime metadata.

The existing task screen remains the control surface. **Control tasks** (`taskControl`) is still separately required for its existing stop, steer, permission and answer actions. No new action, recovery authority, GPU launch permission or cleanup permission is introduced. Owner-scoped writes include an expected publisher check inside the existing relay transaction; both a new request and an idempotent replay must retain the original publisher and runtime. New requests with an old runtime are rejected. Existing receipts can still be recovered under the same nonce after the task’s runtime changes.

The task view fences reads, writes, delayed prompts and receipts by foreground visibility, route, pairing token/grants/expiry and generation. A return from the background pauses controls until a fresh matching task read arrives. Owner changes, replaced pairings and route changes clear prior state. For an unchanged owner/pairing/route, pending command nonces and receipt IDs remain available across background/focus changes; an uncertain submission retries the same identity instead of authorizing a second action. The server’s grant and runtime checks remain authoritative.

## Semantics preserved

- An unavailable original handle remains an unresolved outcome. The inventory never retries the submission or switches execution to another environment.
- **Stop requested** is distinct from a recorded provider terminal observation. A queued task command is not proof that a remote operation stopped.
- GPU cancellation and verified cleanup are distinct. A cancellation handle does not imply resource cleanup.
- A recorded terminal/cleanup observation does not establish invoice finality or release a GPU spending hold.
- Task stop applies to the original owning team task; this view adds no per-job stop or remote-recovery controls. Recovery and cleanup review remain in the desktop’s existing run controls.

## Compatibility and verification

The native portfolio query is added in protocol **43** by the accompanying owner/desktop bridge changes. Rebuild/reconnect desktop and phone together. Existing protocol 42 remote inventory RPCs and records are reused; provider adapters and GPU launch gates are unchanged.

Focused checks exercise Usage read enforcement, denied writes, redacted response validation, project/type/settled-state filters, immutable cursors, truncated-page continuation, owner-scoped task lookup, runtime changes, original-nonce replay, owner mismatch, grant revocation, pairing identity changes and late-result suppression. Full gateway regression tests and mobile/gateway/shared-client TypeScript checks are included in the merge gate.

No real provider operation, external deployment, paid resource, physical phone session, watcher or visual test was used for this slice.

## User review checklist

- [ ] Pair a phone with **Read owner usage**, open Gateway history → Remote executions, and verify default filters across two projects.
- [ ] Compare model/command/GPU receipt state with desktop Remote executions. Confirm actual provider IDs remain desktop-only.
- [ ] Page forward/back, change filters, switch owners, and replace the pairing. Confirm earlier rows cannot populate the new view.
- [ ] Give the phone only Usage read and verify it cannot open task feeds or send task commands.
- [ ] Add **Read tasks** and open the original team task. Confirm another owner’s task is unavailable and the original project is preserved.
- [ ] Add **Control tasks**, request stop, and confirm queued/uncertain/applied status is shown independently of provider termination and cleanup.
- [ ] Background or navigate away during loading or a delayed permission prompt. Confirm controls stay paused until fresh owner data returns.
- [ ] Resume with the same pairing after an uncertain command. Confirm the same nonce/receipt is recovered; change runtime or revoke Control tasks and confirm no new stale-runtime command is accepted.
- [ ] Verify missing-handle and GPU cleanup cases remain unresolved until the original native recovery/cleanup path records evidence.
- [ ] Check readable text scaling, light/dark themes and accessible labels on a supported physical device.

## Remaining broader controls

Desktop recovery binding, per-job cleanup review and guarded GPU provisioning remain separate existing flows. The gateway task mirror must be published for navigation/control to be available; inventory history can outlive that mirror. No new AI provider connection, automatic GPU launch or billing-settlement authority is part of this change.
