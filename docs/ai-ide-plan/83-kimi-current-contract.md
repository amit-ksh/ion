# Kimi current native Teams contract

Checked 2026-10-16. Research only; no production adapter, native runtime acceptance, account access, paid inference, installation, configuration edits, visual testing or watcher changes.

Current follow-up: [released lifecycle analysis](89-kimi-source-host-contract.md) and [native source-host execution](90-kimi-native-source-host-fixture.md) verify the candidate below in a private fixture. Production worker wiring and a selected, distributable source host remain open; this historical research note is not installed-CLI acceptance.

## Result

The external-hook/MCP gap described in [remaining harness integrations](64-remaining-harness-integrations.md#kimi-source-task-contract-investigation) remains in the current released source. A more promising **in-process source-host contract** exists: the native registry can add an executable tool whose callback receives the engine's turn and tool-call identity and returns a successful native tool result. Native default tools and permission processing can stay assembled. This is an implementation candidate requiring a real source-host fixture and an explicit packaging/version policy; it is not a verified extension of an installed `kimi.exe`.

Do not count Kimi source-task Teams as implemented yet. Its existing bounded completion adapter and ordinary ACP transport remain unchanged.

## Versions and evidence

| Item                              | Authoritative identity                                                                                                                                                  |
| --------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Existing cache                    | Official source `f67e639`, previously inspected for CLI 2.1.1, under `D:/personal/apps/waku/.waku-cache/ion-team-harness-contracts/kimi-2.1.1/source/kimi-code-f67e639` |
| Latest released CLI at this check | `@moonshot-ai/kimi-code@2.2.0`, published 2026-10-14 15:31:16 UTC                                                                                                       |
| Annotated release tag             | `f9c6b3cfa0c232e651136b801c616df7e710a66b` resolves to commit `472192c34c1efad89a369870eff0da0b708ce9bb`                                                                |
| Current main                      | `ef727e517923cb312cb48d42825c49dfacd528e7`, 52 commits beyond cached `f67e639`                                                                                          |
| Current source SDK/core           | `@moonshot-ai/kimi-code-sdk` 0.21.0 and `@moonshot-ai/agent-core-v2` 0.5.0; both source manifests set `private: true`                                                   |
| Public npm package lookup         | Both SDK/core `/latest` endpoints returned HTTP 404 during this check; this does not exclude future publication or other distribution channels                          |

Release metadata was read through `gh-axi api`, including the annotated tag resolution. Relevant released and current file-tree identities were compared; unchanged cached files were verified by computing their Git blob SHA-1 from exact bytes, rather than assuming cached source represented current main. The SDK implementation changed after the release, so the current getter/default callback were also read directly at the current commit. No released 2.2.0 binary was downloaded or executed in this investigation.

Sources: [official 2.2.0 release](https://github.com/MoonshotAI/kimi-code/releases/tag/%40moonshot-ai%2Fkimi-code%402.2.0), [release commit](https://github.com/MoonshotAI/kimi-code/commit/472192c34c1efad89a369870eff0da0b708ce9bb), [current SDK manifest](https://github.com/MoonshotAI/kimi-code/blob/ef727e517923cb312cb48d42825c49dfacd528e7/packages/node-sdk/package.json), [current core manifest](https://github.com/MoonshotAI/kimi-code/blob/ef727e517923cb312cb48d42825c49dfacd528e7/packages/agent-core-v2/package.json), [SDK public metadata endpoint](https://registry.npmjs.org/%40moonshot-ai%2Fkimi-code-sdk/latest), [core public metadata endpoint](https://registry.npmjs.org/%40moonshot-ai%2Fagent-core-v2/latest).

## Paths that still do not provide the required bridge

### External hooks

`PreToolUse` has useful native witness data, but `runHook` interprets successful JSON as message/permission output. It has no successful replacement-tool-result or input-rewrite channel. Launch errors, timeouts and cancellation settle as allow outcomes. Running a team and then denying the original tool would create work while reporting the native call as failed, and cannot be the success path. The current and released `runHook.ts` Git blob is `59a023aba55fd5deb5dc95929a31236948402341`, identical to the cached implementation. [Exact native hook handling](https://github.com/MoonshotAI/kimi-code/blob/ef727e517923cb312cb48d42825c49dfacd528e7/packages/agent-core-v2/src/features/externalHooks/internal/runHook.ts).

### MCP

The current stdio call accepts only name, arguments and abort signal, then sends `client.callTool({ name, arguments: args }, ..., requestOptions)`. The native engine call/session/turn identity is not forwarded into the tool envelope. Released and current stdio-client blob: `203e5bd1b489bca6ed92d1ad658b80e70f4a7714`. A JSON-RPC request ID identifies that MCP transport request; it must not be relabeled as an independently witnessed engine tool-call ID. Pairing a separate hook with a callback solely by arguments remains ambiguous for identical calls. [Current native stdio client](https://github.com/MoonshotAI/kimi-code/blob/ef727e517923cb312cb48d42825c49dfacd528e7/packages/agent-core-v2/src/mcpCore/client-stdio.ts).

### Installed plugin manifests

The manifest loader explicitly records `tools`, `apps`, `inject`, `configFile`, `config_file` and `bootstrap` as unsupported runtime fields. Supported MCP/hooks/instructions do not load an arbitrary in-process JavaScript tool executor. `sessionStart.skill` injects text. Consequently, naming a directory a plugin or adding an unsupported `tools` field does not create the required native callback. [Official plugin reference](https://moonshotai.github.io/kimi-code/en/customization/plugins.html#plugin-manifest), [exact loader](https://github.com/MoonshotAI/kimi-code/blob/ef727e517923cb312cb48d42825c49dfacd528e7/packages/agent-core-v2/src/app/plugin/manifest.ts).

### ACP and remote klient

The ACP interaction bridge dispatches approval/question interactions and skips `user_tool`. ACP's registered request methods have no arbitrary registry-tool registration/result-return extension. Unknown methods are rejected. The klient remote agent contracts and memory service registry do not expose `IAgentUserToolService` or `IAgentToolRegistryService`, although session interactions can list/respond to existing requests. Their presence does not provide registration. These facts preclude treating stock ACP or IPC as an existing custom-tool SDK. [ACP interaction bridge](https://github.com/MoonshotAI/kimi-code/blob/ef727e517923cb312cb48d42825c49dfacd528e7/packages/acp-server/src/interaction-bridge.ts), [ACP methods](https://github.com/MoonshotAI/kimi-code/blob/ef727e517923cb312cb48d42825c49dfacd528e7/packages/acp-server/src/server.ts), [remote agent contracts](https://github.com/MoonshotAI/kimi-code/blob/ef727e517923cb312cb48d42825c49dfacd528e7/packages/klient/src/contract/agent/services.ts), [service registry](https://github.com/MoonshotAI/kimi-code/blob/ef727e517923cb312cb48d42825c49dfacd528e7/packages/klient/src/transports/memory/serviceRegistry.ts).

## In-process native contracts worth implementing next

### 1. Source-host composition and scope access

`SDKRpcClientV2.engineAccessor` is a declared getter, not a private-field bypass. Its own comment limits it to in-process app services and describes a temporary migration escape hatch. Agent/session services must be obtained through their lifecycle scopes. The source SDK remains private and its public-package lookups failed, so importing this from an arbitrary installation is unproven.

A second composition seam is `runAcpServerWithStream`: its result explicitly exposes the bootstrapped `core`, `klient` and connection. This would let a pinned source host retain the official ACP server while subscribing to session/agent creation and registering Ion tools in each main agent. The `extraSeeds` option is test-oriented and should not be used to replace production model, authentication or permission services. [SDK getter and warning](https://github.com/MoonshotAI/kimi-code/blob/ef727e517923cb312cb48d42825c49dfacd528e7/packages/node-sdk/src/sdk-rpc-client-v2.ts#L593), [official ACP composition](https://github.com/MoonshotAI/kimi-code/blob/ef727e517923cb312cb48d42825c49dfacd528e7/packages/acp-server/src/start.ts).

**Packaging gate:** a helper built against pinned official source is a separate native-host distribution. It is not a drop-in plugin for the owner's selected executable. Before production, prove version compatibility, reproducible dependency/build inputs, native account/model/workspace preservation and ordinary permissions. Never silently substitute a fresh provider or a generic inference loop.

### 2. Executable registry tools

`IAgentToolRegistryService.register` adds a tool and returns an identity-aware disposable. It replaces an existing same-name entry, so Ion must refuse a name collision before registration. Keep all built-in entries, register exactly the five existing Ion actions, and activate only those names through the profile service. The executable's `resolveExecution` receives the original arguments; its `execute` callback receives native `turnId`, `toolCallId` and cancellation signal and returns `ExecutableToolResult` with success/error semantics. Capture session/agent identity from trusted lifecycle scope, not model arguments. [Registry implementation](https://github.com/MoonshotAI/kimi-code/blob/ef727e517923cb312cb48d42825c49dfacd528e7/packages/agent-core-v2/src/agent/toolRegistry/toolRegistryService.ts), [tool execution contract](https://github.com/MoonshotAI/kimi-code/blob/ef727e517923cb312cb48d42825c49dfacd528e7/packages/agent-core-v2/src/tool/toolContract.ts), [profile interface](https://github.com/MoonshotAI/kimi-code/blob/ef727e517923cb312cb48d42825c49dfacd528e7/packages/agent-core-v2/src/agent/profile/profile.ts).

The native executor resolves the tool, runs before-execution decisions, waits for will-execute work, emits `ToolCallStarted`, then calls the actual executable with native identity. It finalizes the returned result through native hooks and truncation. This ordering can support an independently observed native event plus callback witness, while retaining normal native approval; it must be demonstrated with the real engine, not inferred from unit stubs. [Native execution and event ordering](https://github.com/MoonshotAI/kimi-code/blob/ef727e517923cb312cb48d42825c49dfacd528e7/packages/agent-core-v2/src/agent/toolExecutor/toolExecutorService.ts).

### 3. User-tool interaction service: useful evidence, awkward SDK default

`AgentUserToolService` already registers additive `source: user` tools and creates `user_tool` requests containing the native call ID, turn, name and args, tagged with native agent/session/call/turn identity. It waits for an `ExecutableToolResult`; cancellation settles an error. Its current/released blob `3abaf3fae2593169d29ba978501760c447fb6a7a` matches the cache. [Exact user-tool implementation](https://github.com/MoonshotAI/kimi-code/blob/ef727e517923cb312cb48d42825c49dfacd528e7/packages/agent-core-v2/src/agent/userTool/userToolService.ts).

The default source SDK wiring immediately routes user-tool interactions into `toolCall`; its base implementation returns an unsupported error. The callback request also omits session/agent/name, despite their presence in the interaction. A source-host experiment must deliberately replace that callback/consumer with a scoped implementation or choose direct executable registry callbacks, avoiding racing the stock unsupported response. Do not treat a method override alone as registration. [Session wiring](https://github.com/MoonshotAI/kimi-code/blob/ef727e517923cb312cb48d42825c49dfacd528e7/packages/node-sdk/src/v2/session-wiring.ts), [current default callback](https://github.com/MoonshotAI/kimi-code/blob/ef727e517923cb312cb48d42825c49dfacd528e7/packages/node-sdk/src/rpc.ts#L543), [callback request shape](https://github.com/MoonshotAI/kimi-code/blob/ef727e517923cb312cb48d42825c49dfacd528e7/packages/node-sdk/src/interaction.ts).

## Next executable contract fixture

Build a disposable **pinned official-source host**, preferably around the official ACP composition with direct registry callbacks. Do not patch the native engine or claim this is the released CLI. The fixture must:

1. Resolve/build official source and dependencies in an isolated cache, record source/dependency hashes and runtime version, and make clean startup/shutdown deterministic. Use a disposable Kimi home, temporary workspace and fake native provider credential; restrict provider/MCP traffic to localhost. No owner credentials or paid services.
2. Start the ordinary default profile and snapshot native built-ins, account route, model, project instructions and permission mode. Add exactly the five Ion tools without replacing defaults or changing modes. Confirm collision refusal and subsequent tool activation.
3. Have a local provider emit a real tool call. Independently record native `ToolCallStarted` and the executable callback's native session/agent/turn/call identity and full args. The callback must pass the shared Ion worker's owner approval, grants and run limits before work starts.
4. Return the accepted team's result successfully through the native executor. Verify it is attached to the same native tool call in the next provider request; ordinary native tools must still work in that session.
5. Emit identical arguments under different native call IDs, including one batch and a later turn. Demonstrate distinct identities, durable exact replay behavior and refusal of changed/foreign/model-authored bindings.
6. Exercise native permission denial, hook failure, malformed input, missing witness, changed saved-team definition, Stop during native and Ion approvals, Stop during work, EOF, process replacement and stale callbacks. No team work may start after revocation.
7. Resume the same native session without duplicating registrations or leaking callback authority. Verify disposal preserves any owner-created tool/configuration and reattaches only currently authorized Ion callbacks.
8. Keep fake-provider source-host acceptance separate from real selected-account acceptance and acceptance of the owner-selected installed binary. If only an unmodified installed binary is permitted, obtain an upstream supported tool-registration/native-identity extension instead of calling this fixture an installed-CLI integration.

## Current acceptance checklist

- [x] Revalidate latest release and current official source.
- [x] Check external hook, MCP, plugin, ACP, remote klient and in-process source contracts.
- [x] Identify a native success-return contract with engine-owned call/turn identity.
- [ ] Prove a reproducible source-host artifact and its account/configuration compatibility.
- [ ] Execute the native default-profile fixture above.
- [ ] Implement and verify the production source-task bridge, packaging/version gate, replay/Stop and owner-account acceptance.

This narrows the next engineering step; it does not remove Kimi from the active implementation goal.
