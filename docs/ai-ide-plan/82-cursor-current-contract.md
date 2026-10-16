# Cursor: current native Teams contract

Checked: 2026-10-16. This follows [the earlier contract](RESEARCH-cursor-teams-contract.md). Research and an executable native protocol fixture are implemented here; no production Cursor Teams adapter is enabled.

Verification: **5 native cases passed, 0 failed, 47 assertions**. Strict fixture TypeScript checking and formatting also passed.

## Result

There are now two concrete seams, with different boundaries:

- The native CLI's ACP transport executes a session-scoped MCP tool, exposes its actual call ID in both the main stream and permission request, accepts denial/cancellation, and returns a successful native tool result. This is exercised with the released executable.
- The TypeScript SDK supplies a native custom-tool callback context containing the native tool call and local session identities. Replacing ordinary ACP with this SDK would change its account and permission contract; it is not a transparent upgrade.

The unresolved source-task requirement is joining a particular native CLI invocation to the external MCP callback **before** an Ion team starts. The MCP JSON-RPC request ID, model-authored fields, and a random hook invocation ID do not establish that join. Bounded Cursor Teams completion also remains unfinished.

## Exact artifacts checked

| Artifact                                | Version / identity                                                               |
| --------------------------------------- | -------------------------------------------------------------------------------- |
| Official cached Windows CLI             | `2026.10.01-e373342`                                                             |
| CLI archive SHA256                      | `31896ce9a43f63c8ae7ff3c4bee868213d6d02d60fb49908bef6dc91d582f6ee`               |
| Executed `dist-package/index.js` SHA256 | `0e7e28c34e050c6b83fe574e1fefa8d95d7c80bea6c4a4d3ae2726e5191243ef`               |
| Latest npm `@cursor/sdk` checked        | `1.0.37`, published 2026-10-07T20:13:46.368Z                                     |
| SDK archive SHA256                      | `992e4889e298f1ac5dfeeda6cf4db5eb1008aa3b1ea03883a17c2ea052f1efd2`               |
| Bridge contract                         | `sdk.v1`, SDK `1.0.37`, source commit `659a285c40b503cd3cadf49bdeeb4e18d261d180` |

CLI bytes were compared with the cached official [Windows archive](https://downloads.cursor.com/lab/2026.10.01-e373342/windows/x64/agent-cli-package.zip). This is a pinned CLI investigation; it does not assert that this cached CLI is the newest release. SDK npm metadata and SHA512 integrity were rechecked against the [version metadata](https://registry.npmjs.org/@cursor%2fsdk/1.0.37) and [package archive](https://registry.npmjs.org/@cursor/sdk/-/sdk-1.0.37.tgz). The [official bridge manifest](https://github.com/cursor/sdk-bridge/blob/v1.0.37/proto/manifest.json) and release agree. Integrity comparison proves the inspected bytes match the registry, not a separate publisher signature.

## Executable native evidence

[scripts/fixtures/cursor-source-teams-contract.test.ts](../../scripts/fixtures/cursor-source-teams-contract.test.ts) runs the pinned CLI and its bundled Node. It supplies a disposable profile, fake Cursor API-key exchange and model catalog, a harmless stdio MCP server, and Cursor's actual Connect/protobuf agent service over HTTP/2. The normal native ACP path and built-in executors are retained. The hidden `--agent-endpoint` override points the native agent service at localhost; `--local-agent` and another provider's chat API are not used.

The test's model identity is checked in the actual outgoing `AgentRunRequest.requested_model`. The minimal fake catalog does not exercise the complete ACP model-picker configuration UI. No live key, owner credential store, hosted inference, deployment or UI/device acceptance is involved.

The saved cases verify:

1. Two calls with identical arguments keep distinct native main-stream and permission call IDs. Both return successful MCP results to the native agent transport.
2. Denying the native permission prevents `tools/call` from reaching the MCP server.
3. Stop during the native approval wait produces ACP `cancelled`, with no MCP invocation.
4. A missing private-profile hook with `failClosed: true` and an explicit one-second timeout blocks before the MCP server or approval callback.
5. For this pinned ACP release, passing a plugin containing a fail-closed hook through `--plugin-dir` does not make that hook an execution boundary: the MCP call still succeeds. This is a regression observation, not a blanket statement about all Cursor versions or IDE plugins.

The MCP wire contains only the ordinary tool name and arguments: no native call/session metadata is forwarded. A deliberately model-authored `ion_native_token` value reaches it unchanged. The test specifically distinguishes those bytes from the independently observed native IDs. [Cursor ACP](https://cursor.com/docs/cli/acp) documents the embedding surface; these concrete observations come from the executable fixture.

Verification: **five native cases pass, zero failures/skips, 47 assertions in 24.68 seconds**. Strict standalone fixture TypeScript, Prettier and diff checks pass. The final run uses only disposable fake profiles and localhost services; these results do not establish authenticated account acceptance.

### Reproduce

From the implementation worktree in PowerShell:

```powershell
$env:ION_TEST_CURSOR_RUNTIME = 'D:/personal/apps/waku/.waku-cache/ion-team-harness-contracts/cursor-native/dist-package'
$env:ION_NATIVE_FIXTURE_ROOT = 'D:/personal/apps/waku/.waku-cache/ion-team-harness-contracts'
bun test scripts/fixtures/cursor-source-teams-contract.test.ts
```

The fixture is Windows-specific and requires the exact CLI hash above. Without the runtime setting it skips; a skip is not native acceptance. Temporary profiles are removed only after verifying their resolved directory remains inside the explicit fixture parent. A process-level network guard rejects nonlocal socket destinations. Owner credentials are not inherited into the native profile.

## What the hook source actually says

In the pinned CLI archive:

- `190.index.js`, `HookedMcpExecutor`: `preToolUse` gets a freshly generated random hook UUID. It is not the native `McpArgs.toolCallId`. The wrapper merges `updated_input` into MCP arguments and supports successful `postToolUse.updated_mcp_tool_output` replacement.
- `2633.index.js`, ACP session resources: the base hook `conversation_id` and `generation_id` both come from the ACP session ID. The latter is therefore not a per-prompt fence in this implementation. Hook configuration is constructed using the Cursor project-cache directory, rather than the supplied workspace, explaining why the disposable workspace hook was not discovered.
- `index.js`, `McpSdkClient.callTool`: the native tool ID is retained inside the client, but the external `tools/call` request serializes only name/arguments.

These are findings about inspected source, with runtime support limited to the cases above. Exploratory successful-hook commands currently returned no output through the native Windows hook executor; input overwrite, successful post-hook replacement, crashing/malformed hooks, and a trustworthy hook/main-stream identity join are **not** claimed verified. Do not solve this by adding a model-supplied ID field or weakening approval checks.

The current [hook reference](https://cursor.com/docs/hooks) documents input replacement, MCP output replacement, and fail-closed scripts. [Plugin documentation](https://cursor.com/docs/reference/plugins) defines Cursor manifests at `.cursor-plugin/plugin.json`, inline/path hook declarations, and `hooks/hooks.json` discovery. Documentation does not override the pinned ACP fixture's behavior.

## SDK seam and compatibility

Use the exact `@cursor/sdk@1.0.37` package and `import { Agent } from "@cursor/sdk"`. `Agent.create({ local: { cwd, customTools } })` and per-send `local.customTools` expose callback functions; results can carry text, `isError`, and structured content. `SDKCustomToolContext` has optional `toolCallId` and `sessionId`; missing identities must fail closed. The native stream separately emits `tool_call` with `agent_id`, `run_id`, and `call_id`. [TypeScript SDK](https://cursor.com/docs/sdk/typescript).

The published archive's `dist/esm/464.js` passes the native `McpArgs.toolCallId` and local runtime session state into the callback, independently of model arguments. `dist/esm/index.js` maps the native interaction call ID into the SDK stream. This is the next precise SDK runtime fixture to build, including child-session rejection and cancelled callbacks.

**Compatibility decision:** keep ordinary Cursor ACP as the default. SDK local execution can preserve `cwd`, fixed model parameters and the default advertised built-in tools by omitting a restrictive toolset. However SDK authentication uses its explicit Cursor API-key route, while existing ACP uses the owner's selected native CLI login. SDK custom callbacks also skip native interactive approval. Ion's owner approvals would still be required. Neither account equivalence nor ordinary permission equivalence has been established. [SDK authentication, tools and approval behavior](https://cursor.com/docs/sdk/typescript).

## Completion allowance and replay

The pinned public `AgentOptions`, `LocalAgentOptions`, and `SendOptions` contain no supported output-token, turn, step or dollar ceiling. `SdkModel` has identity/parameter/variant data, without authoritative output/context maxima. The bridge still omits the TypeScript retry-disable option. It is invalid to invent wire fields. [Pinned shared messages](https://github.com/cursor/sdk-bridge/blob/v1.0.37/proto/sdk/v1/sdk_messages.proto).

The archive's internal network agent client explicitly rejects `maxSteps`; it requires an in-process agent loop. It is not a hosted Cursor completion ceiling. Observing usage and cancelling later cannot prove a pre-submission spending bound.

The bridge offers cancellation and durable run observation; create/send idempotency is explicitly described for cloud agents. A local retry after lost acknowledgement cannot be assumed safe. [Pinned agent service](https://github.com/cursor/sdk-bridge/blob/v1.0.37/proto/sdk/v1/sdk_agent_service.proto). Keep an uncertain reservation until its original attempt is reconciled; do not resend through another account or harness.

## Next implementation gates

- [x] Drive the actual native hosted agent protocol with a fake account/model service and inspect MCP/ACP correlation.
- [x] Retain native approval, denial, pending-Stop and successful tool-result fixtures.
- [ ] Obtain a supported additive hook/callback surface that binds the external MCP invocation to its exact native call before dispatch, without changing the owner's login, default tools or permission policy. The callback must be fenced to the active source prompt and exact target/input.
- [ ] Alternatively, implement and verify an explicitly selected Cursor SDK account route; do not silently migrate ordinary ACP sessions into it.
- [ ] Verify successful hook payload delivery, overwritten forged fields, malformed/crashing/missing helpers, simultaneous identical calls, consumed/stale authority, Stop after dispatch, replay and restart recovery against the actual chosen transport.
- [ ] Establish a supported hosted completion allowance and complete native usage/model/account settlement before enabling bounded Teams completion.
- [ ] Repeat the chosen native contract on the registered Linux worker, followed by authenticated account acceptance when owner resources are available.

The fixture improves the available implementation evidence. It does not change the current harness implementation counts or finish Cursor Teams integration.
