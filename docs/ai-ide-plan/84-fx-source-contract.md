# Fx ordinary-task Teams contract

Checked 2026-10-16. Research only; the existing bounded completion adapter and ordinary ACP sessions retain their current behavior. The source-task bridge is still unimplemented.

## Current evidence

Official [ACP documentation](https://fx.sh/docs/using-fx/acp) describes the native account/settings route, one active session/prompt per connection, cancellation, and client-supplied MCP transports. Native MCP discovery does not by itself establish the call identity required by Ion's shared Teams worker.

The cached official CLI 0.0.13 source was inspected under `D:/personal/apps/waku/.waku-cache/ion-team-harness-contracts/fx-source/fx-0.0.13`:

- `src/core/tooling/tool_mcp_registry.zig` calls the MCP runtime with the tool name, arguments and call options.
- `src/core/tooling/tool_mcp_runtime.zig` `CallOptions` carries binding/runtime generation, cancellation, progress, continuation, input responder and access. It does not carry the engine's native tool-call/session ID.
- `src/core/mcp/tool_operations.zig` reserves a dispatcher/server request ID. Its wire progress token is that same transport ID when progress is enabled.
- `src/core/mcp/protocol_messages.zig` builds the tool request from name/arguments and optional transport metadata.
- `src/acp/mcp_carrier.zig` wraps it with `requestId: fx-mcp-<logical_id>`. That logical ID is not the native tool-call identity emitted in ACP tool activity.

Ion's inference: matching these transport IDs or matching arguments alone does not prove the independently witnessed native call. Identical calls must remain distinguishable across concurrent work, replay and resumed sessions. No production source tool is enabled from this source inspection.

## SDK candidate and its limits

The newer [libfx tool contract](https://fx.sh/docs/lib/tools) supplies session ID, a durable execution ID and cancellation signal to executable callbacks, and returns successful results. It also explicitly separates host-tool authorization from CLI permissions. The [SDK API](https://fx.sh/docs/lib/api) exposes host-defined tools; this is a promising separate host path, not evidence that the installed CLI's built-ins, permission prompts and selected account are preserved.

The inspected native CLI is for Linux/macOS. This Windows host currently has no usable Linux runtime for that fixture. No WSL disk reset, owner-profile change or alternative provider loop was performed. Source-only evidence is not native execution acceptance.

## Remaining implementation gates

- [ ] Run a pinned official CLI on a supported host with disposable account/configuration and localhost model/MCP services.
- [ ] Prove an additive native executor or native metadata path linking each successful result to the exact main-session call ID and original arguments.
- [ ] Preserve the owner's selected native account/model, default tools, instructions and permission mode; treat any SDK-host distribution as a separate explicit route.
- [ ] Wire shared owner approval, grant/limit intersection, durable replay, changed-definition refusal, Stop and stale-callback fencing.
- [ ] Exercise identical calls, resume, native failure, malformed/foreign callbacks and cleanup in the actual production driver.
- [ ] Complete authenticated owner-account/platform acceptance.

Current coverage stays at 11 of 14 ordinary-task bridges and 12 of 14 bounded completion paths. No paid inference, deployment or device acceptance is claimed.
