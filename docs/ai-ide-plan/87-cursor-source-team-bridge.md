# Cursor source-task Teams integration

Updated 2026-10-16. Branch: `codex/ion-agent-teams`.

## Current status

The Windows production ACP driver now contains the Cursor bridge. All current-source Windows bridge gates below pass. The coverage ledger is now 24/28 source-level integration paths, with Cursor’s non-Windows and authenticated owner acceptance still open. Cursor bounded Teams completion remains separate work.

## Native session and ownership

- Verify native `2026.10.01-e373342` using the final configured executable, arguments and environment. Unsupported versions/platforms leave the ordinary session available and report unavailable team tools.
- Preserve the selected native account, model, native tools and session workspace. Add uniquely named session MCP tools with a private loopback credential.
- Compute the exact pinned native workspace-cache path after the caller's configuration has been applied. Fixture paths cannot accidentally resolve through the owner's inherited home.
- Exclusively create an unused native project hook file: Claude local settings first, Cursor hooks second, Claude project settings third. Existing owner files are never merged or replaced.
- Concurrent drivers in one Ion process share only the file that this lease registry created. Native file identity, original bytes, parent directory pins and link checks protect updates and cleanup. Changed/replaced files are preserved and disable further use.
- Cleanup runs off the UI thread. If all slots are occupied, refuse team setup. Crash recovery does not claim ownership from a filename or file contents: an untrusted leftover is preserved. A full set of stale/owner slots requires explicit cleanup by the owner.

## Call authorization

Native ACP observation supplies the real tool-call ID. The authenticated command hook supplies a one-use input binding. The native permission request independently witnesses the rewritten arguments under that ID. All three must agree before MCP dispatch can reach the shared Teams worker.

Ion tools offer per-call native approval. Remembered approval is removed from these tools only; ordinary native permission behavior is preserved. An unadvertised permission selection or an unwitnessed/preauthorized MCP route cannot create a team invocation. Worker-level owner grants, pinned definitions, source/project scope, budgets, limits and durable identities remain enforced by the existing worker.

Malformed or changed permission/callback input also revokes the shared worker’s pending approval, rather than only refusing the newer request. Its dedicated regression passes against the real shared worker and confirms that neither changed callback input nor a malformed repeated permission can enqueue work after revocation.

The bridge shares ACP turn queuing and cancellation with Grok while keeping each provider's wire dialect separate. Stop, completion, replacement and callback failures revoke active bindings. Binding fields, nonces, transport credentials and private tool prefixes are scrubbed before product events are emitted.

## Evidence

- [x] Five native-file lease tests: concurrent drivers, existing owner files, owner edits/replacements, owner-added hard links and incompatible JSONC commands.
- [x] **25 actual native protocol cases pass, 249 assertions, zero failures/skips in the final complete run.** Both Claude-compatible project slots coexist with an existing owner Cursor hook file. The native fixture now shares one controller with the production-driver tests.
- [x] Fixture TypeScript and formatting checks pass.
- [x] Final current-source Rust filter: **21 pass, zero failures**. Four native cases are ignored by that default filter and were executed explicitly in the next gate.
- [x] Actual production-driver invocation and durable result return.
- [x] Stop at shared-worker approval; unadvertised remembered approval refusal.
- [x] Final rebuilt actual-driver filter: **four pass, zero failures/ignored cases**. It covers replacement/resume, exact replay, changed-definition refusal, overlapping native drivers, Stop then a new turn, preauthorized-route refusal and final cleanup.
- [x] Locked offline desktop and both daemon binary checks pass, including the dev daemon. Existing dead-code, ts-rs attribute and future-compatibility warnings remain.
- [ ] Supported worker-platform contracts and authenticated owner-account/device acceptance.

**50 Cursor cases pass at this checkpoint:** 25 native protocol cases, 21 Rust cases and four explicitly executed native driver cases. Existing ACP (19 pass, three owner-account cases ignored) and Grok (seven pass, three separately configured native cases ignored) regressions also pass. A preceding protocol run had one slow hook-launch failure while Rust compilation was active; the final full run after compilation passes. Earlier runs are not added to these counts.

Protocol fixtures use a private fake account, a guarded loopback HTTP/2 service and disposable paths. They do not establish paid-model or owner-account acceptance. No watcher restart or visual/device test is included.

## Native fixture corrections

The raw contract fixture is not sufficient to prove the production bridge. Its strict hook endpoint now rejects a charset-bearing content type, matching the production parser. The PowerShell template explicitly clears that extra charset while still encoding the body as UTF-8. The native HTTP MCP transport and the Claude-compatible authenticated hook are covered directly.

The initial production fixture nested temporary roots until the native hook cache’s working directory reached 267 UTF-16 code units. Windows returned `ENOENT` while launching an existing PowerShell executable. Shortening only the disposable fixture prefix made the native invocation/Stop checks pass. Production rejects a cache working directory at or above 260 code units before installing a hook, with a shorter workspace/configured-data-directory remedy. A regression checks that bound and the native cache-name sanitizer.

The real native session requires a persisted conversation checkpoint before replacement can load its database. The fake service now emits the native checkpoint; it does not fake an ACP load success or force a session ID. The resumed actual runtime retains the same session identity and returns the same durable team run for the same native call.

`--approve-mcps` approves servers, not arbitrary per-call ACP tool execution in this release. The preauthorized-call fixture therefore uses the native configuration’s `Mcp(*:*)` allowlist in its private profile. It proves that the callback is refused and no team is created when native permission is skipped. Production never modifies the owner’s allowlist.

The native Claude compatibility conversion currently ignores the inner `failClosed` setting. Ion’s own independent permission/MCP binding is the enforcement boundary: a skipped or failed hook cannot authorize work. The owner’s existing hook behavior is not rewritten.

## Owner review checklist

- [ ] Use the supported native Cursor version with the intended signed-in account/model in an owner project. Confirm the ordinary native tool catalog and permissions still behave as configured.
- [ ] Invoke a saved team from a normal Cursor task; inspect its queued assignment, actual result, accepted document and source transcript. A queued run is not completed work.
- [ ] Test both an ordinary task and direct `@team` invocation. The new-task flow stays solo by default.
- [ ] Stop while native approval and while Ion team approval are pending. Start a fresh turn, then restart/resume the same source task and check the durable result without a duplicate run.
- [ ] Edit the saved team and confirm a stale definition is refused until the source refreshes it.
- [ ] Confirm existing native hook files are preserved, concurrent tasks release only their own entries, and owner edits disable mutation/cleanup of the changed file. Keep a short enough workspace/data-cache path for native Windows hook launch.
- [ ] Review the all-slots-owned and crash-leftover setup messages. Cleanup is explicit; Ion does not claim an existing file based on its contents.
- [ ] Verify the chosen worker platform before enabling source-task Teams there. This increment verifies Windows; non-Windows Cursor hooks remain unavailable.

## Remaining development

Cursor/Amp native completion allowance/account/usage ceilings, Fx/Kimi source-task Teams bridges, Cursor's remaining production/platform gates, and the outstanding portable CSS/font edge cases remain in the active goal. Nebius speech deployment requires an owner-provided target; phone acceptance requires the owner's iPhone 15 and Moto G32.
