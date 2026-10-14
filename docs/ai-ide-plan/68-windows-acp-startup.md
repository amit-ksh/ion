# Native Windows ACP startup

Updated 2026-10-14. Implementation: `codex/ion-agent-teams`.

## Change

The shared ACP launcher used `/usr/bin/env -C <cwd> <binary> ...` on every platform. Windows now launches the selected native executable directly through the existing ACP SDK. macOS/Linux retain the original argument-vector launch. No shell command string is assembled by Ion.

This shared route serves Cursor, Fx, Grok and Kimi sessions, plus ACP catalog/history helpers. Native provider arguments, environment, selected account, model/mode negotiation and SDK framing are retained. The fix does not enable an unsupported Teams completion adapter.

The SDK's launch configuration has argv/environment but no process-working-directory field. Windows workspace identity is therefore supplied through ACP `session/new`, `session/load` and `session/resume`, as required by the existing protocol contract. Providers must honor that requested workspace. A real native provider's startup configuration discovery and child-process behavior still require owner acceptance; this increment does not claim process cwd equals the selected project on Windows.

## Verification

**21 ACP regression cases pass; 3 authenticated-provider cases are skipped.** Locked desktop/daemon binary checks, Rust formatting and source diff checks also pass.

Two new Windows cases cover:

- Literal selected executable/argv/environment; no Unix launcher or shell command string; all three ACP session requests retain the exact workspace path.
- A native Windows `.cmd`/PowerShell fixture driven through the production ACP driver and official Rust SDK: initialize, session creation, Unicode and shell-special characters in the workspace, prompt, streamed assistant text, completed turn and SDK shutdown.

The fixture is an isolated protocol peer without a provider account or model request. It uses `-NoProfile`, private test files and a synthetic prompt; it does not stand in for authenticated Cursor/Grok/Kimi/Fx inference. It verifies startup/framing and the owner-supplied protocol workspace, not provider billing or descendant-process cancellation finality.

Existing tests cover parameterized Cursor model/effort/fast selection, native access modes, questions, correlated tools, prompt settlement and provider-failure handling. Windows execution of the fixture does not verify macOS/Linux platform acceptance. No visual test or watcher restart occurred.

## Owner review

- [ ] Install/sign in to each selected native CLI on Windows, select its harness/model in Ion and create a normal task.
- [ ] Verify the correct project is used for reads, edits, configuration and app/tool connections, especially when Ion was opened from another directory.
- [ ] Check saved session discovery, resume, model/effort controls, supervised permissions and user questions.
- [ ] Check Stop and closing a task with native tools/children active; no claim of process-tree cancellation is made from the protocol fixture alone.
- [ ] Review the corresponding native account on an enabled Nebius worker separately; local startup proof does not certify remote credentials or platform support.

## Remaining harness work

[Current harness coverage](64-remaining-harness-integrations.md) distinguishes implemented completion transports from ordinary source-task Teams tools. Amp/Cursor native completion allowance/account/metering contracts, the remaining source-task connections and real account/platform acceptance remain open.

## Primary references

- [Cursor native Windows installation](https://cursor.com/docs/cli/installation).
- [Cursor ACP](https://cursor.com/docs/cli/acp) and [CLI parameters](https://cursor.com/docs/cli/reference/parameters).
- The pinned `agent-client-protocol` 2.0.0 source in Cargo.lock: `AcpAgentConfig` exposes command, argv and environment; `AcpAgent::spawn_process` provides native Windows process creation and stdio.
