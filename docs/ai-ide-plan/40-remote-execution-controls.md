# Remote execution inventory and controls

## Implemented

Desktop **Usage → Remote executions** and direct phone **Usage → Remote executions** list saved remote model calls, command jobs and GPU jobs across projects. The default is **All projects / All types / Unresolved**. Project, type and resolved-state filters have independent pages; changing a filter resets pagination.

The collection is read-only: opening or refreshing it performs no provider request, submission, cancellation, resource creation or cost release. It joins durable operation identities and run/assignment metadata in one SQLite read transaction. Each type contributes at most the requested page size plus one candidate before the final merge. Pages are capped at 64 native / 32 phone entries and sorted by immutable `(kind, request UUID)`, not activity time. Cursor identity must match the project/type collection; a cursor that becomes terminal remains valid while paging. Refresh starts a new observation; state changes can remove entries from an unresolved page.

Rows show the saved receipt state, request, original operation/job identifiers, missing-handle status and GPU cleanup state. They exclude request bodies, commands, environment values, policies, input files, output contents and credentials. A missing operation handle remains unresolved until the original submission is reconciled; the list never resubmits or guesses that nothing exists.

Desktop **Open run controls** loads the current team run and its existing stop/recovery actions. Phone **Open team task** opens the owning root task, whose existing team cancellation path reloads the current run revision. Stopping the run affects the whole team; this inventory does not add a separate per-job cancellation authority or claim a successful stop from a click. Recovery still binds the original provider operation and checks its terminal state. GPU cancellation and resource cleanup remain distinct. Verified cleanup/terminal observations do not settle an invoice or release monetary holds.

## Native and mobile behavior

RPC and database work run off the UI thread. Native rows cache display strings and use a virtualized list. Arrow keys, Home/End, Page Up/Down, Enter/Space and Escape work from the list; filters and actions retain native keyboard/focus behavior. The inventory has no polling loop. Closing, changing pages or changing filters invalidates old results. A failed load clears old rows rather than enabling controls from a stale page.

The phone view reads only the currently connected owner daemon while visible and in the foreground. Reconnects and owner changes require fresh data, reset paging and prevent old rows from enabling navigation. Responses must match project/type/unresolved filters, ordered unique identities, page bounds and the final-row cursor. It uses a FlatList and explicit refresh.

Protocol version is **42**. Rebuild/reconnect the desktop daemon and phone client together; generated wire bindings are included.

## Later review checklist

- [ ] Open desktop Usage → Remote executions with no records; confirm the empty state and keyboard focus.
- [ ] With saved operations in two projects, verify All projects, project filters, all three types and Unresolved / All records.
- [ ] Page forward/back; mark a previous-page operation terminal, then continue paging and refresh.
- [ ] Open a run from a remote row; confirm the current team, assignment and original provider operation.
- [ ] Stop an active team; confirm the UI distinguishes Stop requested from a provider terminal observation.
- [ ] For GPU work, confirm cancellation alone remains unresolved until cleanup is recorded; billing holds remain unchanged.
- [ ] Restart the daemon with an unknown submission outcome. Confirm there is no repeated POST or automatic local fallback.
- [ ] On the phone, open the owning team task; verify cancellation uses its current revision. Switch daemon owners during refresh and verify old records disappear.
- [ ] Try keyboard-only navigation and both themes on the watcher-managed app. No visual test was run in this implementation turn.

## Remaining broader remote work

The current sandbox API only declares a writable-layer size limit. Worker per-process CPU/address-space/file/process limits are implemented separately; this view adds no aggregate CPU/RAM allocation guarantee. Resource usage observations do not establish an enforced allocation.

Production GPU enablement still needs verified aggregate budget/resource policy, network/egress and metadata/service-account boundaries, accepted images/tariffs and real lifecycle/cleanup evidence. The collection observes records already accepted by the existing guarded paths and does not weaken their launch gates. Gateway remote control/write authority and full phone recovery parity remain separate development items. No provider credentials, paid resource or live deployment were used.
