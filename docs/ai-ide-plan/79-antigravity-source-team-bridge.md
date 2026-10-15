# Antigravity source-task Teams bridge

Updated 2026-10-15. Implementation branch: `codex/ion-agent-teams`.

## Implemented behavior

Ordinary Antigravity source tasks can now discover and invoke the existing five Ion Teams actions through the native CLI's MCP client. Native authentication, model, ordinary tools, workspace, global/project customizations and permission mode remain selected. Direct `@team` execution retains its existing flow.

Registration is gated to native **Antigravity CLI 1.3.1**. A bounded background version probe runs before setup. If the installed dialect is unverified or additive plugin setup fails, the task reports that Teams tools are unavailable and continues its ordinary native conversation. Ion does not silently choose a different model/account, enable Full Access or grant project permissions. Successful native fixtures explicitly select Full Access in a disposable profile; the shared worker is separately exercised in supervised mode so a real Ion owner approval is required.

The [preceding native contract](78-antigravity-native-source-contract.md) records the checksum-pinned runtime. The expanded contract now verifies additive plugin discovery as well as the earlier envelope/stream binding.

## Additive native plugin

Ion creates one fresh `.agents/plugins/ion-teams-<random-id>/` directory per driver. It contains only a native manifest and an MCP definition for this task's authenticated loopback server. The marker is published last. Existing plugin names are never adopted or overwritten. Existing `mcp_config.json`, comments, trailing commas, unknown fields, app connections and native permission files are not edited.

Released 1.3.1 namespaces plugin MCP servers by the **directory name**. The fixture now deliberately uses different manifest/directory names and verifies the real hyphenated directory namespace. This distinction was found by the actual-driver fixture and corrected in production.

Directory handles pin the workspace, customization parents and private bundle. Unix creation opens children relative to the pinned parent with no-follow flags. Windows pins reject reparse points and prevent parent replacement. Cleanup runs on a background thread, compares both generated files, preserves changed/adopted bytes, removes only exact unchanged files and uses nonrecursive directory removal. Extra owner/native documents survive. Overlapping drivers have distinct directories: closing an earlier driver does not remove its replacement's plugin.

The transport credential is scoped to the private loopback endpoint. It is not authorization by itself and is never included in model tool arguments or UI events. A retained owner-edited bundle refers to a closed endpoint after shutdown.

## Native identity and bounded worker

The driver reads Antigravity's main-conversation stdout. Native `init.conversation_id` establishes the source session. An actual active tool step must expose the exact plugin server, `call_mcp_tool` wrapper, Teams action and full original arguments.

An MCP callback must independently provide the native envelope conversation and trajectory-step metadata observed in that stream. The opaque native process/progress UUID is pinned for the active turn. Model-authored argument metadata remains ordinary data. Foreign conversations, unrelated wrappers, changed inputs or targets, failed/completed steps, missing witnesses, consumed callbacks and stopped generations are refused.

The bridge retains at most **2,048 native step tombstones**, **40 KiB per argument object** and **1 MiB of active arguments per turn**. Callback/pipe delivery lag has a bounded four-second witness wait. The shared strict HTTP parser keeps its authentication, Host, browser-origin, framing, header/body and timeout checks. HTTP workers and their queue are bounded; startup handles the observed discovery fallback without adding unrelated tools.

Accepted callbacks use the existing source worker for project/task scope, owner approval, access intersection, pinned limits, durable invocation identity and accepted document reads. The durable key includes the original Ion source task, native conversation and trajectory step. An exact native replay after driver replacement returns the existing invocation; a new native step is a new request and keeps the source task's four-invocation cap. A queued run is reported as queued work.

## Stop, resume and driver lifecycle

Prompts enter a bounded queue with individual cancellation flags. Stop fences the active prompt and every already queued prompt, revokes pending Teams approval and terminates/reaps the owned native process tree. Cancellation uses no subprocess from the UI thread. A prompt explicitly submitted after Stop receives a new flag.

The native conversation ID received before Stop is retained. An explicit subsequent prompt resumes that conversation. Turn end/error, reader failure, process exit, mode-driven replacement and driver shutdown cancel the old bridge authority. Tombstones prevent an old step from becoming fresh authorization. Event lines have a one-MiB byte bound, and the reader forwards only public response text.

All native process/file/network work runs on background workers. No rendering, pulse-clock or transcript-frame path is changed.

## Verification

- [x] Eight checksum-pinned real-runtime TypeScript contracts pass, zero skipped, **195 assertions**: native identical calls/results, process resume, argument metadata isolation, missing/changed witnesses, strict native policy, workspace configuration preservation and additive plugin discovery.
- [x] Two production-driver cases pass through the actual released executable: approved durable invocation/result return, resumed conversation/new step, changed-definition refusal, overlapping driver cleanup; and Stop during owner approval, canceled queued prompt, no persisted canceled invocation and explicit same-conversation recovery.
- [x] Twenty-one existing shared source bridge/worker/strict HTTP regressions pass. Explicit native fixtures for other providers are skipped and excluded from this count.
- [x] Twelve final Antigravity regressions pass: the public decoder, six native binding/worker cases, three additive-bundle preservation/cleanup cases and the existing account/catalog constraints.
- [x] Locked offline desktop and both daemon binary checks pass, including the dev-binary feature. Strict standalone fixture TypeScript, all eight changed Rust files against rustfmt, Prettier and diff checks pass. Existing compiler warnings remain.

**43 distinct checks pass**: 12 Antigravity regressions, two actual production-driver cases, 21 shared source bridge/worker/HTTP regressions and eight explicit native contract cases. The native contracts have 195 assertions. Skipped optional runtime fixtures are excluded. Actual-driver acceptance was repeated after the final EOF/Stop update.

The native fixtures use normal built-in tools, a private native Gemini profile and localhost provider/MCP servers. Native auxiliary title requests remain on the local fixture, including the observed Flash Lite title model. Initial auxiliary-model and plugin-namespace mismatches were investigated rather than counted as successful native acceptance. Owner app settings and workspace documents remain byte-for-byte intact.

No paid inference request, deployment, watcher restart or visual/device test is included. Authenticated owner-account/platform acceptance remains open.

## Owner review

- [ ] Start a normal Antigravity task on a real project with native CLI 1.3.1 and its configured model/account. Confirm ordinary tools and existing app connections still work.
- [ ] Ask the task to find and invoke a saved team. Review the shared approval details when required by Ion; verify the queued run, source task/project, limits and eventual accepted result.
- [ ] In the native review mode, confirm a denied headless action stays denied. Enable Full Access only as an explicit owner choice when appropriate.
- [ ] Stop during team approval, then start a new prompt. Confirm no canceled invocation appears and the new prompt keeps the same native conversation.
- [ ] Run two tasks or replace a driver in one project. Confirm each uses its own Teams authority and the closing driver cleans only its own unchanged bundle.
- [ ] Review authenticated Google/account routes and native macOS/Linux installations. The Windows fake Gemini route does not prove these routes.

## Re-run implementation checks

PowerShell, implementation worktree and existing native cache:

```powershell
cargo test --locked --offline -p waku-core --lib antigravity -- --test-threads=1
$env:ION_TEST_ANTIGRAVITY_BINARY = "D:/personal/apps/waku/.waku-cache/ion-team-harness-contracts/agy-1.3.1.exe"
cargo test --locked --offline -p waku-core --lib antigravity_source_team_native_driver_ -- --ignored --test-threads=1
$env:ION_NATIVE_FIXTURE_ROOT = "D:/personal/apps/waku/.waku-cache/ion-team-harness-contracts"
bun test scripts/fixtures/antigravity-source-teams-contract.test.ts
```

The native-driver cases require the explicit executable and ignore opt-in. Skipped cases are not acceptance. This checkpoint does not complete Cursor/Fx/Kimi source bridges, Amp/Cursor bounded completion contracts, the remaining document/CSS/font/format work or browser/device acceptance. The full goal stays active.

## Primary references

- [Official native plugin structure and discovery](https://antigravity.google/docs/plugins/).
- [Official native MCP configuration](https://antigravity.google/docs/mcp/).
- [Official headless stream and conversation resume](https://antigravity.google/docs/cli/headless/).

The scoped native binding, lifecycle and cleanup rules are Ion's implementation choice; actual released-runtime evidence supplements the documentation.
