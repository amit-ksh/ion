# Antigravity source-task Teams native contract

Updated 2026-10-15. Implementation branch: `codex/ion-agent-teams`.

## Current checkpoint

The checksum-pinned released Windows **Antigravity CLI 1.3.1** provides a verified native conversation/step correlation path for ordinary tasks to invoke saved Ion teams. This checkpoint adds reproducible native contract fixtures. It does **not** register production source-task Teams tools or close the full document/harness goal.

The fixture uses the normal native agent with its ordinary built-in tools, an isolated disposable native Gemini profile, fake credentials and local model/MCP endpoints. It does not replace the native agent with a JSON-only completion agent. Both identical calls, the native tool results, a following provider request and a separately resumed native process are exercised.

Native Windows executable SHA-512:

```text
f346d2cd9d68e7e5395cb94ff95e7e8ab2e0c65cf2ad835d0eb671fb41f12f5fc713d7a26ceb4510678eb9baa62c6cd9ab8205211e3661c6cd1e23ca7bc69781
```

The cached release manifest identifies `1.3.1-4582356770750464/windows-x64/cli_windows_x64.exe`. The fixture checks the version and, on Windows, the full binary digest. No production dependency or separate hook runtime is added.

## Verified native sequence

1. Native startup emits `init.conversation_id`. A tool's `ACTIVE` event includes the same conversation ID, `step_index`, `tool_name: "call_mcp_tool"` and `tool_info.parameters`. The parameters expose the original `ServerName`, `ToolName` and full nested `Arguments`.
2. The native MCP client supplies `params._meta["antigravity.google/conversation_id"]` and `params._meta.progressToken`. The progress value has a native UUID and a colon followed by the same step index. The HTTP JSON-RPC ID is a separate transport counter.
3. The fixture waits for the matching real stdout event before accepting a callback. Conversation, step, exact server/tool and complete original arguments must agree. Two identical calls in one model response have distinct step identities.
4. The returned fixture catalog reaches the next provider request under each actual native function-call ID. The original model-authored arguments remain intact.
5. A new CLI process started with `--conversation` retains the conversation and old history. Its new tool steps have greater indices; both newly returned results appear in the resumed provider request.

The proposed durable source identity is therefore the native conversation plus trajectory step, intersected with Ion's source-task identity. The opaque progress UUID is not substituted for the native conversation or source task. The fixture does not establish that an arbitrary provider function-call UUID is available to Ion's stream: the engine's step index is the observed identity.

Model-authored `Arguments._meta` remains ordinary argument data. A test deliberately supplies forged conversation/progress fields there; they do not replace the native envelope metadata. Production must read the envelope and never trust an argument field as native context.

## Workspace discovery and native permissions

A valid `.agents/mcp_config.json` containing `mcpServers.<name>.serverUrl` and an Authorization header is discovered by the native normal-task session. The fixture verifies workspace-only discovery and byte-for-byte preservation of an unrelated disabled app entry and unknown owner fields. Native global discovery is separately exercised under the disposable home.

An earlier exploratory probe added an unsupported eager-tool configuration field and failed discovery. Removing that probe field produced successful workspace-only discovery; the failure is not evidence that native workspace configuration is unsupported.

The released client can begin with `server/discover`, fall back to `initialize`, list tools, send notifications and use the lazy `call_mcp_tool` wrapper. A production bridge must handle the observed initialization/fallback and bounded notifications without registering unrelated tools.

The native strict headless policy refuses ungranted MCP work. The refusal fixture receives no MCP tool call and records the native denied action. Successful contract cases explicitly use Full Access **inside the disposable fixture only**. Production must retain the selected mode and native rules, and must not silently turn on Full Access. No owner profile, account, native app connection or permission is changed by these tests.

A separate exploratory pre-tool hook can rewrite nested arguments, but the native envelope/stream path above does not require that hook. The committed fixture uses no command hook and has no Python/JavaScript hook-runtime requirement.

## Verification

**Seven real-runtime cases pass, zero skipped, 169 assertions**, in 65.74 seconds:

- Identical native calls, independent real-stream witnessing and provider result return.
- Native process restart/resume, retained conversation/history and advancing step identities.
- Forged model-authored argument metadata retaining its role as data.
- Missing owner-stream evidence refusing the callback.
- Changed owner-observed arguments refusing the callback.
- Native strict headless policy retaining its denial.
- Workspace-only MCP discovery preserving unrelated configuration bytes.

The successful case also sends consumed replay, foreign-conversation and wrong-credential controls. They create no additional accepted fixture operation. Callback reads, model/stream bytes, native process time and disposable cleanup are bounded.

Strict standalone TypeScript (`strict` and `noUncheckedIndexedAccess`), Prettier and diff checks pass. Production Rust code is unchanged by this checkpoint; it does not claim a new Rust build, actual durable Ion invocation or user-account/device acceptance. No paid inference, deployment, visual test or watcher restart is performed.

## Remaining production implementation

- [ ] Add a scoped project MCP entry for only the existing five Teams actions. Preserve existing app connections, unknown keys and native JSONC behavior; support simultaneous source tasks, driver handoff and owner edits, and clean only an entry Ion still owns.
- [ ] Keep the selected native account, model, agent, ordinary tools, workspace and permission mode. Gate the native metadata dialect to a verified release.
- [ ] Bind callbacks to the real main-conversation stream, exact wrapper target/arguments and the live source-task generation. Refuse other conversations/subagents, stale, changed, unwitnessed and consumed callbacks.
- [ ] Use the existing bounded source worker for owner approval, access/budget intersection, durable invocation replay and accepted document reads. These contract callbacks return a fixture catalog and do not test the real worker.
- [ ] Fence Stop before/during startup, pending approvals, terminal/error events, process exit, queued prompts, mode changes and driver replacement.
- [ ] Verify actual-driver saved-team invocation/result return, replay/resume, changed definitions, Stop during owner approval and configuration cleanup/preservation.
- [ ] Complete authenticated owner-project and platform acceptance. Signed-in Google routes and real owner app/tool interactions are not covered by the fake native Gemini account.

Ten production source-task bridges remain implemented. Antigravity, Cursor, Fx and Kimi source-task bridges and the native Amp/Cursor bounded completion contracts remain open. The broader document rendering/format/device work is still active.

## Re-run the native contract

PowerShell, implementation worktree:

```powershell
$env:ION_TEST_ANTIGRAVITY_BINARY = "D:/personal/apps/waku/.waku-cache/ion-team-harness-contracts/agy-1.3.1.exe"
$env:ION_NATIVE_FIXTURE_ROOT = "D:/personal/apps/waku/.waku-cache/ion-team-harness-contracts"
bun test scripts/fixtures/antigravity-source-teams-contract.test.ts
```

Without an explicit native executable the seven cases skip; a skip is not acceptance. Other versions or binary bytes fail rather than being counted as verified.

Standalone TypeScript:

```powershell
bun ./node_modules/typescript/bin/tsc --ignoreConfig --noEmit --target ESNext --module Preserve --moduleResolution Bundler --strict --noUncheckedIndexedAccess --skipLibCheck --types bun scripts/fixtures/antigravity-source-teams-contract.test.ts
```

Use the installed repository compiler; this checkpoint did not download a new compiler or dependency.

## Primary references

- [Official headless event stream and resume documentation](https://antigravity.google/docs/cli/headless/).
- [Official MCP configuration and workspace discovery documentation](https://antigravity.google/docs/mcp/).
- [Official hook contract](https://antigravity.google/docs/hooks/), inspected as a candidate; the selected native metadata path does not need a hook.
- [Native Windows release](https://storage.googleapis.com/antigravity-public/antigravity-cli/1.3.1-4582356770750464/windows-x64/cli_windows_x64.exe).

The metadata correlation and lifecycle design are Ion's inference, verified for the pinned release by the actual native fixture. Documentation alone is not used as proof of runtime correlation or full Ion Teams completion.
