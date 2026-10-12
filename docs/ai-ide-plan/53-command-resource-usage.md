# Command resource Usage view

Implemented on `codex/ion-agent-teams`; runtime and native UI acceptance remain open.

Open **Usage > Command resources**. The view reads command reservations across the owner database, or the selected Ion project. **Refresh** performs an explicit background request. Rows show concurrent commands, CPU millis, RAM bytes, process/thread count and storage bytes, alongside the effective approval limit and available reservation capacity.

These values describe logical allocations reserved by [command admission](51-command-resource-admission.md). They are not sampled host CPU/RAM consumption, Nebius billing, or proof of a physical host quota. Separate [kernel group controls](45-sandbox-resource-groups.md) constrain each actual managed command. GPU limits remain in the existing GPU budget/resource policy path.

## Interpretation

- **Held:** allocations for unresolved original executions and retained storage.
- **Limit:** the strictest outstanding owner/project approval. No active approval means no current aggregate cap can be inferred from this collection.
- **Available:** the approval limit minus held reservations. Unknown historical quantities show **Unknown**, preventing a misleading free-capacity claim.
- The note reports commands retaining compute, retained archive bytes and commands with unknown historical quantities. Storage reservations conservatively include layer allowance and retained/uploaded archive allowance; archive bytes alone do not equal the full storage hold.

Compute stays held after uncertain stops/failures until a verified original supervisor result is accepted. Storage stays reserved after execution; this screen does not free storage or authorize cleanup. Capacity is checked before command submission/project upload in the owner, rather than in this read-only client view.

## Implementation

`LoadCommandResourceUsage` returns only scoped totals/limits through the versioned daemon contract. The UI performs RPC and formatting on a background executor, stores five prepared rows and fences responses by generation/project/current page. Closing the view or navigating elsewhere invalidates late reads. Render accesses only memory; no polling, filesystem/database query, provider operation or subprocess occurs per frame.

The project picker and refresh/back controls reuse Ion's native menu/button primitives. Tab reaches controls and the scrolling body, focus has a visible accent, arrows/Page Up/Page Down/Home/End scroll, and Escape returns to Usage. Wide numeric columns remain horizontally scrollable in a narrow viewport. No decorative animation is added. Both themes use existing theme tokens.

## Owner review

- [ ] Open/refresh **Command resources** with no jobs and with actual approved commands.
- [ ] Confirm All projects sums jobs across targets/runs; project filtering changes only the scope displayed.
- [ ] Compare the saved private runtime registry's approved limits and outstanding original request allocations. CPU is shown in millis; RAM/storage in bytes.
- [ ] Try a request that exceeds any aggregate cap; confirm refusal occurs before native POST/upload. Omitting/changing target approval must not bypass an existing hold.
- [ ] Confirm Stop/FAILED/CANCELLED without a valid original receipt leaves compute held. A validated result releases compute and retains storage.
- [ ] Refresh with unknown legacy data; confirm **Unknown** and no invented available capacity.
- [ ] Navigate away or change project during the request; no stale scope/result reopens the view.
- [ ] Keyboard: Tab, picker arrows/Enter/Escape, refresh Enter/Space, body scrolling and Escape. Review narrow windows, long project names and both themes.
- [ ] Accept the exact Linux runtime/image/kernel limits separately using [resource group review](45-sandbox-resource-groups.md).

Native source compilation and automated admission/transport gates are recorded in the current release checklist after merged verification. No visual/native-device acceptance is claimed by this document.
