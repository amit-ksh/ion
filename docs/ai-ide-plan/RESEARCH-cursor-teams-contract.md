# Cursor Teams adapter contract

Checked: 2026-10-07. Scope: selected native Cursor account and model, isolated local or registered Nebius worker execution, bounded metered completion, denied native effects, cancellation and replay.

## Conclusion

Current Cursor **SDK 1.0.36** provides a better implementation seam than the earlier CLI/ACP investigation: a supported empty built-in toolset, explicit API key, fixed model selection, ambient settings control, runtime usage, run handles and cancellation. This changes the next engineering action to a pinned native SDK transport investigation. It does **not** yet prove an enabled Teams adapter: the inspected public requests lack a completion token ceiling, the model catalog lacks verifiable context/output limits, and native callback enforcement still needs executable adversarial fixtures.

Do not replace the selected Cursor account with an Anthropic/OpenAI/Nebius account to get a convenient completion API. Do not convert those development gaps into a “configure a key” message.

## Evidence and boundaries

Only public primary documentation, public release schemas and the existing isolated native CLI cache were read. No installed user credential store, browser login, live model inference, account identity call, billing query or global installation was used. No provider behavior is claimed from a fabricated localhost response. The package was downloaded **in memory** for schema inspection, not installed or executed.

### Releases inspected

- Official cached CLI: `2026.10.01-e373342`, from [Cursor's Windows x64 archive](https://downloads.cursor.com/lab/2026.10.01-e373342/windows/x64/agent-cli-package.zip). Archive SHA256: `31896ce9a43f63c8ae7ff3c4bee868213d6d02d60fb49908bef6dc91d582f6ee`.
- Official current SDK: `@cursor/sdk` **1.0.36**, published 2026-10-05. The [official bridge release](https://github.com/cursor/sdk-bridge/releases/tag/v1.0.36) matches that version.
- [npm release metadata](https://registry.npmjs.org/@cursor%2fsdk/1.0.36) and its [package archive](https://registry.npmjs.org/@cursor/sdk/-/sdk-1.0.36.tgz) were read. SHA256: `8425ae7bf96eb94c4d20cc6556605e60f4d64d7663ebbb04dfc09e60c644b102`. Computed SHA512 matched the metadata's exact `dist.integrity`; this checks the fetched bytes against npm metadata, not an independent publisher signature.
- [Bridge manifest at v1.0.36](https://github.com/cursor/sdk-bridge/blob/v1.0.36/proto/manifest.json): protocol `sdk.v1`, SDK `1.0.36`, source commit `85960629ea7bf6a0b0d64eecfbceb8ded1038303`.

Local cache paths are `.waku-cache/harness-contracts/cursor-native/dist-package`, `cursor-package.zip`, `cursor-help.txt`, and the existing `cursor-contract.py`, `cursor-hidden-contract.py`, `inspect-cursor.py` probes. None of those files was modified.

## Supported native seam

The TypeScript SDK's Local runtime runs the agent loop on the selected execution host while inference remains Cursor-hosted. `tools: []` offers no built-in tools; `disallowedTools: ["mcp", "task"]` additionally excludes custom MCP tools and delegation. Restrictions must be supplied again on resume. `local.settingSources` controls ambient configuration; `local.enableAgentRetries: false` disables transport/stall retries. Fixed model IDs should be used rather than Auto/Router; `run.model` and result model record that run's selection. These are documented capabilities, not fixture evidence for Ion. [Official TypeScript SDK](https://cursor.com/docs/sdk/typescript).

For Rust, the official bridge embeds the same SDK and provides loopback Connect HTTP/1.1/protobuf or JSON RPCs. Its process bearer token is separate from the Cursor API key. Pin the bridge/version and validate its ready handshake before sending requests. [Official bridge protocol](https://github.com/cursor/sdk-bridge/blob/v1.0.36/docs/protocol.md).

### Important wire distinctions

The pinned `AgentOptions` includes `model`, `api_key`, local/cloud runtime, MCP/agent maps, and a presence-bearing `ToolList tools`. **Present but empty** `tools.names` means no built-in tools; omitting `tools` means the default toolset. `LocalAgentOptions` exposes setting sources, sandbox settings, store, auto-review, custom tools and directories. It has **no retry-disable field** in this bridge release, although the TypeScript API does. `SendOptions` offers model/mode overrides and stream callback flags, not output/token/step ceilings. `SdkModel` contains identity, parameters and variants, without context or maximum-output token fields. [Pinned shared messages](https://github.com/cursor/sdk-bridge/blob/v1.0.36/proto/sdk/v1/sdk_messages.proto).

Therefore a direct TypeScript wrapper can preserve the SDK retry switch; adding `enable_agent_retries` or `max_output_tokens` to bridge JSON would invent unsupported fields. Do not rely on such fields silently being ignored.

## What remains unproven

### 1. A hard pre-submission completion allowance

Inspection of the package's public `.d.ts` option shapes found no `maxTokens`, `maxOutputTokens`, `maxTurns` or `maxSteps` completion control. Those similarly named strings elsewhere belong to MCP sampling, transcript reading or background context features. The native network client in `dist/esm/867.js` explicitly rejects its internal `maxSteps` option before request execution: it requires an in-process agent loop. This is source evidence for this release, not a guarantee about a future Cursor API. [Pinned native SDK archive](https://registry.npmjs.org/@cursor/sdk/-/sdk-1.0.36.tgz).

**Ion inference:** cancelling after N text bytes or after a token observation cannot establish a pre-authorized provider spending ceiling. Post-completion usage can validate accounting, but cannot repair an already exceeded reservation. Reserving an invented model maximum is also insufficient when the native catalog supplies no such limit. A supported hard ceiling or authoritative full-request maximum must be found and verified before Teams readiness.

### 2. Denying every native effect, including reads

The SDK's empty toolset closes the documented advertisement gap left by CLI `ask`/`plan`. It does not itself prove that a native backend cannot dispatch an unsolicited tool/query. Package inspection shows the allowed/disallowed tool list being transported in headers to the hosted agent backend; default local approval is permissive. An executable fixture must verify the native client rejects unexpected reads, directory listings, searches, shell, edits, MCP, delegated work and images **before** any native effect occurs.

Cursor's `preToolUse` hook covers all tools, including Read and Task; returning `permission: "deny"` can block them. Command-hook exit 2 blocks. Other execution failures are normally fail-open; invalid successful permission JSON blocks. Hooks are file-based, not a programmatic SDK callback. [Official hook controls](https://cursor.com/docs/hooks).

**Ion inference:** a private deny hook could provide additional evidence, but a hook that sometimes fails open is not a complete dispatch guard. Test a missing hook, timeout, crashed hook, malformed result and unexpected direct filesystem query. Public sandboxing is an additional filesystem/command boundary, not evidence of a credential-free completion contract by itself.

### 3. Usage and account identity

`TokenUsage` separates input, output, cache reads/writes and optional reasoning. Reasoning is already inside output. SDK run usage may be absent. Billed `getUsage()` is eventually consistent; local access is enabled per account and may return `403 feature_unavailable`. A final charge can include Cursor's token fee. [Official SDK usage](https://cursor.com/docs/sdk/typescript).

The bridge `Me` and `ListModels` accept an explicit API key and return that key's account identity/model catalog. Leaving the key empty may fall back to process environment. No bridge model-catalog field proves maximum token limits. [Pinned account/catalog service](https://github.com/cursor/sdk-bridge/blob/v1.0.36/proto/sdk/v1/sdk_cursor_service.proto).

**Ion inference:** use the exact native `CURSOR_API_KEY` route selected on the execution host; reject conflicting keys or an unsupported saved OAuth route. Do not copy user tokens into prompts, use another model provider, or infer service-account team identity solely from a missing user ID. Require complete usage before settlement and retain the original allowance after unknown submission or missing usage.

## Cancellation and replay

The pinned agent service supports `CancelRun`, durable `GetRun`/`ObserveRun`, and close. Its create/send idempotency comments specifically promise safe retries for **cloud** agents. Local resend safety must not be inferred from the mere existence of an idempotency field. [Pinned agent service](https://github.com/cursor/sdk-bridge/blob/v1.0.36/proto/sdk/v1/sdk_agent_service.proto).

**Ion design:** retain Ion's durable one-attempt reservation and pinned identity; persist native agent/run/request IDs before accepting output. Never resend a Local completion after lost acknowledgement. Query the original run/store only, then record completed, cancelled, failed or unknown. Cancellation acknowledgement does not prove zero billed tokens; keep the original allowance until authoritative final usage is known. Kill/dispose the process tree after bounded shutdown, and never fall back to Local after confirmed or uncertain remote submission.

## ACP/CLI are separate contracts

ACP supports stdio JSON-RPC, session model/mode selection, permission requests and cancellation. Permission callbacks apply when a tool requires approval; `ask`/`plan` remain read-only behavior. They do not document a universal native-effects denial or completion ceiling. [Official ACP](https://cursor.com/docs/cli/acp).

CLI `--print` has native shell/write tools, whereas `--mode ask`/`plan` changes mode. Its public parameter list does not expose a completion-token limit. [Official CLI parameters](https://cursor.com/docs/cli/reference/parameters). The inspected hidden `--single-turn` means the initial turn **and delegated subagents**, so it does not prove a single metered model call. Hidden `--local-agent` selects a separate provider runtime and account contract; it is not evidence for bounded Cursor-hosted usage.

## Concrete implementation and acceptance plan

The following is proposed work, not completed implementation:

1. Pin SDK 1.0.36 and explicit native account/model parameters. Prefer a direct SDK wrapper until a bridge version exposes the retry switch; never invent fields.
2. Build the wrapper against a private empty workspace, private home/store/cache and explicit setting sources. Omit cloud configuration, ambient MCP, plugins, agents, browser and computer resources.
3. Pass an empty toolset, explicit MCP/task denial and retries false. Verify the native request carries each restriction; keep input hashes and selected identity in the receipt.
4. Add a local transport fixture for the actual hosted agent protocol, not an OpenAI-compatible substitute. Prove missing and changed models fail before completion and no account fallback occurs.
5. Establish a supported native token ceiling or authoritative exact model/request maximum. If none exists, keep completion readiness disabled with that precise reason while implementing independent transport work.
6. Exercise unsolicited read/write/shell/search/MCP/task/image callbacks, direct resource queries, retry and auxiliary requests. Native effects must stay at zero; granting Ion tools never grants native tools.
7. Validate complete usage, exact model and attempt correlation. Reject missing, negative, fractional, overflowing, inconsistent or over-reservation counts; settle once.
8. Exercise a lost response, restart, cancellation, process death and duplicate observation. Original attempt replay must not cause a new native send or a different harness/account.
9. Repeat the actual native fixture on Windows Local and registered Linux worker versions. The same account contract and pre-submission fallback rules must apply.
10. Only then enable readiness. Live account acceptance remains a later review; it cannot replace missing implementation guarantees.

The useful next development seam is now precise: **native SDK tool-free completion with explicit Cursor account and retries disabled**, followed by native request-cap and unexpected-dispatch verification. The full Cursor Teams contract remains unfinished.
