# Grok source-task Teams native contract

Updated 2026-10-15. Implementation branch: `codex/ion-agent-teams`.

## Current checkpoint

The released Windows Grok **1.0.46 (2765805b9442)** has a verified candidate path for ordinary source tasks to call saved Ion teams. This checkpoint adds a bounded native hook helper to the daemon executable and reproducible real-runtime contract fixtures. It does **not** yet register production Grok Teams tools; the source-task bridge remains open in [current harness coverage](64-remaining-harness-integrations.md).

The contract fixture uses an isolated disposable profile/repository, a fake provider credential and loopback-only model/MCP servers. Normal native startup, its initial title request, the native deferred MCP `use_tool` path, file hooks, ACP reverse requests, tool results and the following provider request all use the released executable. It does not modify an owner account, workspace configuration or production Teams state, and makes no paid inference request.

Native executable SHA-256: `47ad7fbc51767b8b301714111a1e452acc2c89912c08be260e1779a383302a4c`. The cached native package is `@xai-official/grok-win32-x64@1.0.46`; the separately cached source investigation is revision `2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8`. Current source and the released binary are distinct evidence; release behavior is authoritative for the version gate.

## Verified native sequence

1. A settings-file `PreToolUse` hook receives the native session ID, native `toolUseId` and parsed arguments. For deferred MCP calls the arguments retain the `use_tool` wrapper.
2. The command hook can return `hookSpecificOutput.updatedInput` with a fresh, owner-created binding in the nested MCP arguments. The tool target stays unchanged.
3. A separately registered ACP `PreToolUse` client hook sees the resulting rewritten call before MCP dispatch. It can allow or deny, so it provides an independent owner-side identity witness.
4. MCP receives the binding. The fixture validates it against the matching independently witnessed session/call/arguments before returning a native tool result. Two calls in one model batch with identical arguments retain distinct native IDs and bindings. Exact transport replay after consumption is refused.
5. The following provider request retains the model's original authored arguments and receives each result under its actual native call ID.

Released 1.0.46 omits `promptId` from both hook payloads even when `session/prompt` supplies that metadata. Production correlation must therefore retain native session/call identity and Ion-owned turn/generation cancellation, rather than assume a prompt ID arrives on the wire. Current source documentation describes fields that the released fixture does not supply.

An HTTP hook rejects ordinary HTTP even on loopback; its native HTTPS validation is retained. The local command hook uses a bounded stdin/stdout helper. The fixture's JavaScript helper is only a contract control; the production helper is the shipped daemon executable.

## Native helper

`waku-daemon --source-team-hook` (also compiled into `waku-debug-daemon`) runs before daemon authentication, configuration, database access or normal server startup. It accepts one JSON object on stdin and emits native hook JSON. `--help` is the only accepted flag. Hook protocol JSON is required by Grok and is not a general user-facing CLI format.

The owner supplies `ION_SOURCE_TEAM_HOOK_ENDPOINT` and `ION_SOURCE_TEAM_HOOK_TOKEN` in the child environment. The endpoint must be the literal `http://127.0.0.1:<port>/hook`; aliases, userinfo, extra paths/query/fragment, HTTPS and non-loopback routes are refused. Credentials must be 32–256 URL-safe characters. No secret is printed or saved by the helper.

Input and response each cap at **44 KiB**. The HTTP exchange has a one-second connection timeout and five-second overall timeout, no proxy discovery and no redirect following. Non-success, malformed, excessive and non-object replies return a generic native deny response with exit code 2. Unknown flags also deny with exit code 2. The native hook runner supplies the outer process/stdin deadline. The helper does not authorize or create team work; the live owner remains the authority.

## Verification

**10 cases pass with the freshly built Ion daemon helper, zero skipped** (205 assertions): four released-Grok cases cover headless/ACP success and failed-hook/model-forgery refusal, plus consumed/missing/forged binding and changed-argument controls. Six helper cases cover early CLI handling, authenticated Unicode round-trip, hostile input, route/credential refusal, hostile owner replies and an owner that never responds. The stalled owner returns a generic deny response after about five seconds.

The locked offline production daemon build, standalone fixture TypeScript, Rust/Prettier formatting and diff checks pass. Locked offline desktop and debug-daemon checks also pass. Existing compiler warnings remain. Real owner accounts and device acceptance remain separate; no visual test or watcher restart occurred.

## Remaining adapter implementation

- [ ] Add an additive, uniquely owned project hook with safe file leases and exact cleanup; retain existing hooks, account/model settings and folder trust. Do not grant blanket project trust to enable it.
- [ ] Inject only the five existing Ion Teams MCP tools through native ACP new/load/resume setup, retaining other configured MCPs.
- [ ] Bind the command hook's per-call value to the main owner's independent ACP reverse hook; validate full arguments, source task/project, native session/call and current owner generation. Refuse subagent/foreign/truncated/changed/expired/unwitnessed calls.
- [ ] Reuse the bounded source worker for owner approval, access intersection, durable invocation replay and result return. A queued run remains a queued run.
- [ ] Fence Stop, turn completion/error, failed native callbacks, disconnect, mode change and driver replacement; preserve tombstones so old calls cannot become new authorization.
- [ ] Remove private binding fields from user-facing tool presentation and errors, while retaining native provider content ordering.
- [ ] Run actual driver fixtures for approved durable invocation, replay, changed definitions, Stop during approval, native resume, cleanup and owner plugin preservation.
- [ ] Complete authenticated owner-project and platform acceptance.

These are unfinished implementation requirements, not deferred successes. The full document/harness goal remains active.

## Re-run fixtures

PowerShell, from the implementation worktree. Use a disposable existing cache directory and a built Ion daemon; the explicit executable paths keep native/version selection visible:

```powershell
$env:ION_TEST_GROK_BINARY = "D:/personal/apps/waku/.waku-cache/ion-team-harness-contracts/grok-contract/runtime/grok.exe"
$env:ION_TEST_ION_DAEMON = "D:/personal/apps/waku/target/debug/waku-daemon.exe"
$env:ION_NATIVE_FIXTURE_ROOT = "D:/personal/apps/waku/.waku-cache/ion-team-harness-contracts/grok-contract"
bun test scripts/fixtures/native-team-hook.test.ts scripts/fixtures/grok-source-teams-contract.test.ts
```

Without `ION_TEST_ION_DAEMON`, the Grok fixture uses its disposable JavaScript helper as a control. It cannot prove the shipped helper. Missing executable environment variables skip the respective native cases, so skipped tests must not be reported as native acceptance.

## Primary sources

- [Official Grok hooks reference](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-pager/docs/user-guide/10-hooks.md)
- [ACP client hook response/registration](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-shell/src/extensions/hooks.rs)
- [Native pre-tool hook and rewrite ordering](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-shell/src/session/acp_session_impl/tool_calls.rs)
- [Native HTTP-hook URL validation](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-hooks/src/runner/http.rs)

The proposed identity binding and lifecycle rules are Ion's design, inferred from source and checked against the released native executable. The contract fixture alone does not establish end-to-end Ion Teams functionality.
