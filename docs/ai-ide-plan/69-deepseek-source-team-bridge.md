# DeepSeek normal-task team invocation

Updated 2026-10-15. Implementation branch: `codex/ion-agent-teams`.

## Implemented flow

A normal DeepSeek source conversation can list saved teams, invoke a team, inspect its runs, await its result and read accepted team documents. The resident host loads a private temporary Cordis overlay. It registers only the existing five Ion tools and preserves the native web profile, account, selected model, personal settings and ordinary tools.

The loopback endpoint and random credential are passed through the native process environment; neither is written into the generated plugin. The owner's live session downlink must independently witness the exact main-session ID, call ID, tool name and complete arguments. History reads are never fresh authorization. Nested/PTC callbacks, other sessions and changed arguments refuse dispatch.

The shared source-team worker provides owner approval, original project/task access, definition revision checks, scope intersection, configured limits and durable invocation identity. A restarted native session replay returns the original invocation; it cannot enqueue a second run. Failed native tool results, turn end, Stop, host/stream failure, driver closure and pooled host shutdown revoke pending calls. Stop is fenced immediately before the native cancel RPC is queued. Tool results preserve current native top-level call IDs and error flags as well as older payload representations.

The native CLI launch uses `--profile web` with the additive `--patch`; the `web` convenience command forbids parent overlay flags. Existing capability-gated `--no-open`, host lifecycle and native session protocol remain in use.

## Verified checkpoint

- **38 distinct tests passed**: 20 shared bridge/lifetime regressions, 20 DeepSeek transport/composition regressions (three overlap), and one actual native generated-plugin fixture.
- The native fixture runs the pinned Windows DeepSeek SDK runtime **0.1.5rc1** against fake localhost Chat responses and a temporary fake profile. Exactly five Ion tools are advertised. Their native calls return the saved Engineering team to the next native model request, queue exactly one owner-approved team invocation, and reject a stale definition revision. Native failed-result identity is checked.
- Final locked offline desktop/daemon binary checks, Rust formatting and whitespace checks pass. Existing Rust warnings remain.
- Changed/foreign/nested calls, approval/replay/Stop and additive credential-free plugin files have explicit regression coverage. No real account, paid inference, cloud resource, visual test or watcher restart is used.

### Separate full web-host acceptance gap

The SDK runtime's shipped default web profile fails the resident metadata/session round-trip before serving requests: `@deepseek-ai/dsh-session-title-first-prompt-llm` imports missing `@deepseek-ai/dsh-session-title-llm`. This is evidence of incomplete packaging in that downloaded SDK executable. Its successful private headless fixture does not certify the default web profile. Ion retains the native profile rather than silently deleting the missing plugin or replacing native configuration.

The committed ignored `deepseek_session::tests::installed_harness_session_contracts_round_trip` can use `ION_TEST_DEEPSEEK_BINARY` to verify a complete supported native installation. Authenticated full web/session execution is unverified.

## Owner review

- [ ] Install a complete supported DeepSeek Harness and verify its native web profile starts without a missing package error.
- [ ] Run the resident session-contract fixture with `ION_TEST_DEEPSEEK_BINARY` pointing to that executable; verify metadata, session/history, model catalogue and permission/plan controls.
- [ ] Start a normal DeepSeek task using your selected account/model. Ask it to list saved Ion teams, then invoke one; review access/limits and confirm the run belongs to the same project/task.
- [ ] Stop during owner approval and while waiting for a team result; no late callback may start a new run. Check independent sessions remain scoped when sharing the native host.
- [ ] Resume the same saved native session and confirm a completed invocation is not duplicated.
- [ ] Verify native personal settings, ordinary tools and selected account/model still work. Confirm the temporary overlay is removed with its owned host and no live credential is left in a project file.

## Remaining goal requirements

Source-task Teams tools for Cursor, Antigravity, OpenCode 2, Fx, Grok and Kimi remain open. Amp/Cursor bounded completion contracts and native account/platform acceptance remain open. Document resource support and phone review continue separately; this checkpoint does not complete the full goal.

## Primary references

- [DeepSeek Harness](https://www.deepseek.com/harness/en/)
- [Pinned native source](https://github.com/deepseek-ai/deepseek-harness/tree/5badb15009ae1756c3afe0ae0cef1faafc290ccc)

The installed runtime fixture is authoritative for its exact released event/tool interface; newer source-only headless JSON options were not assumed to exist in that executable.
