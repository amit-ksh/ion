# Remaining harness integrations

Updated 2026-10-16. The active user goal reopens the remaining harness integrations previously deferred on 2026-10-08.

## Current implementation coverage

This table is source-level transport coverage. Native versions, selected account/model routes, configured grants/limits and owner acceptance still apply. A completion transport is not the same feature as an ordinary source conversation invoking a saved team. Direct `@team` execution retains its existing flow.

| Harness     | Bounded Teams completion transport                 | Source-task Teams tools                                         |
| ----------- | -------------------------------------------------- | --------------------------------------------------------------- |
| Nebius      | Implemented                                        | Implemented                                                     |
| Codex       | Implemented                                        | Implemented                                                     |
| Claude      | Implemented                                        | Implemented                                                     |
| OpenCode    | Implemented                                        | Implemented                                                     |
| OpenCode 2  | Implemented                                        | Implemented; verified native 2.0.29, owner acceptance remaining |
| Pi          | Implemented                                        | Implemented                                                     |
| Oh My Pi    | Implemented                                        | Implemented                                                     |
| Amp         | Native allowance/account/metering contract missing | Implemented; authenticated CLI acceptance remaining             |
| Cursor      | Native completion ceiling/account contract missing | Remaining                                                       |
| Antigravity | Implemented                                        | Implemented; verified native 1.3.1, owner acceptance remaining  |
| DeepSeek    | Implemented; pinned native fixture                 | Implemented; full web/owner acceptance remaining                |
| Fx          | Implemented                                        | Remaining                                                       |
| Grok        | Implemented                                        | Implemented; verified native 1.0.46, owner acceptance remaining |
| Kimi        | Implemented                                        | Remaining                                                       |

The 12 completion dispatch arms are explicit in `driver/team_worker.rs`; Amp/Cursor remain refused. The eleven source-task connections are explicit in `driver/mod.rs` through `start_with_workspace`; the other three retain their ordinary transport without source-team tool registration. No unsupported connection is counted as implemented.

Coverage is **23 of 28 distinct integration paths** (12/14 bounded completions and 11/14 ordinary-task bridges), about 82% by path count. Five paths remain development work; this is not an effort estimate for the whole application or a replacement for live acceptance.

[Windows ACP startup](68-windows-acp-startup.md) now uses the selected native executable instead of a Unix launcher. Its synthetic protocol fixture verifies framing/workspace propagation; real provider configuration/account and Stop acceptance remain separate.

## Pi and Oh My Pi normal-task Teams bridge

The native session now loads a scoped extension exposing the five existing Ion Teams actions: list, invoke, inspect runs/run, and read accepted team documents. Native streamed tool-call/session IDs witness each request, and changed arguments, failed tool callbacks, terminal events, Stop and bridge closure revoke pending authorization. Exact invocation replay uses the same durable source-task/tool-call identity across native restart.

Native account/model/settings stay selected. Source tool invocation uses the existing owner approval and scope intersection, with bounded bridge workers and no bypass of pinned run limits. Production availability follows the existing Full access mode.

Twelve shared source-bridge regressions pass, including changed/replayed arguments, Stop, failed native callbacks and delayed headers on explicitly nonblocking sockets. Actual installed **Pi 0.84.1** and **Oh My Pi 18.6.1** pass the combined native RPC fixture: the native extension calls `list_teams`, returns the saved Engineering team and sends that result back in a second provider request. Exactly the five Ion tools are advertised. This uses isolated fake accounts and localhost Responses, with no paid provider call.

The fixture now distinguishes Pi's `--no-tools` (which also disables extensions) from `--no-builtin-tools`, and handles OMP's `GET /models` catalog preflight separately. The real Windows bridge explicitly restores blocking mode on accepted sockets before bounded background reads. An intermittent Pi startup timeout was observed during investigation; a final run with the installed npm launcher passed in 25.47 seconds for both dialects. Broad startup reliability and owner-account acceptance remain review items.

## Amp

The official plugin AI helper can select an exact native model and set `maxTokens`, but `generate` returns only string/object without a metered usage receipt. Current declarative custom-agent configuration can pin a model; its completion token ceiling and metered usage still need proof. The tool execute context lacks a call ID. The current official plugin reference separately exposes `tool.call` with `toolUseID` and a synthesized-result return, which is a candidate for normal-task invocation. Its installed native runtime dispatches these as request methods; `plugins exec` sends the generic event method and therefore does not prove those request hooks. A private fake-profile probe against the unmodified installed runtime now passes the actual `tool.call` request: exact call/thread/arguments reach the callback and its synthesized result returns over RPC. The normal-task bridge is now wired through the owner stream and native request hook. Final approval/replay/Stop, cleanup/preservation, environment-credential and actual-runtime generated-hook fixtures pass. Authenticated full CLI discovery/resume acceptance remains open. [Implementation and remaining acceptance](66-amp-source-team-bridge.md) distinguish this from the still-unfinished Amp Teams completion adapter. No substitute provider/account is used. Teams completion remains an implementation gap until that native allowance/account/usage contract is verified.

## Cursor

**Current follow-up:** [the staged Cursor binding and hook transport](86-cursor-binding-transport.md) verifies working project-cache hook rewriting, native permission/content correlation, authenticated bounded loopback and adverse HTTP behavior. Its binding state and command template are implemented; native ACP lifecycle/shared-worker/owned-hook integration is still required before source tools can be enabled. The normal ACP default and existing account route remain intact.

**2026-10-16 native evidence:** [the current contract and runnable fixture](82-cursor-current-contract.md) prove selected fake-model native ACP tool discovery, distinct main-stream/permission call IDs, successful MCP results, denial, pending Stop and fail-closed missing-hook behavior. Five actual-runtime cases pass. MCP does not forward the native identity, and plugin hook discovery is not an execution boundary in the pinned release. Production correlation and the bounded completion ceiling remain unfinished; counts do not change.

Current official SDK 1.0.37 metadata and complete declarations were inspected. Its 150 declaration files (6,656,963 bytes) expose no `maxOutputTokens`, `maxTokens`, `maxSteps`, `maxTurns` or `maxCost` completion control. Archive SHA-256: `992e4889e298f1ac5dfeeda6cf4db5eb1008aa3b1ea03883a17c2ea052f1efd2`; npm SHA512 integrity verified. Disabling tools/retries and eventual usage accounting cannot prove a preauthorized spending ceiling. Native Cursor account/model stays selected; full Teams completion remains open.

[Official SDK metadata](https://registry.npmjs.org/@cursor%2fsdk/1.0.37), [official SDK documentation](https://cursor.com/docs/sdk/typescript), [Amp plugin reference](https://ampcode.com/manual#plugins), and the earlier [Amp](RESEARCH-amp-teams-contract.md)/[Cursor](RESEARCH-cursor-teams-contract.md) contract investigations record the native seams and missing proof. No live account or paid provider call was used.

## DeepSeek normal-task Teams bridge

The resident native host now receives an additive temporary Cordis patch registering the five existing Ion Teams tools. Native main-session tool-call IDs and arguments are witnessed by the owner before approval and dispatch. Exact replay, Stop, failed callbacks and changed definitions retain the shared worker's durable scope and lifecycle rules. Native account/model/profile settings remain selected.

The actual pinned Windows SDK runtime passes a private headless fixture with native tool registration, catalog return, owner-approved invocation and stale-definition refusal. Its bundled default web profile fails separately because a required session-title plugin is missing; this does not prove production web-host acceptance. A full supported native installation and authenticated owner review are still needed. [Implementation, native evidence and review](69-deepseek-source-team-bridge.md).

## OpenCode 2 normal-task Teams bridge

The checksum-verified released Windows 2.0.29 runtime now passes real driver fixtures for owner-approved team invocation/result return and Stop at owner approval. The additive plugin keeps credentials in memory through authenticated location RPC; the owner witnesses native session/message/call/name/arguments independently. It preserves existing plugins/account/model/service and shares generated-file leases across driver replacement. Release identity/inbox compatibility, stale binding revocation and final cleanup are verified. The bridge is gated to the verified native version. [Implementation, 102 distinct passing checks and owner review](72-opencode2-native-plugin-contract.md). Authenticated native resume/hot-upgrade/platform acceptance remains open.

## Grok source-task native contract

The released Windows 1.0.46 runtime now proves a file-hook input rewrite followed by an independent ACP client-hook witness, then MCP result return under the actual native call ID. Identical arguments in one batch keep distinct call identities. Failed hooks/model-forged bindings and consumed replay are refused in the isolated contract fixture. The release omits prompt IDs in both hooks; production must retain native session/call identity with Ion-owned turn/generation cancellation. The shipped daemon now provides a bounded stdin/stdout helper, and all ten helper/native contract cases pass with that executable, without a separate JavaScript runtime for hook execution. [Native evidence, helper checks and the remaining adapter implementation](75-grok-native-source-contract.md). The production source-task bridge now passes approval/durable invocation/result return, native resume/replay, changed-definition refusal, Stop at Ion approval, untrusted-project refusal and final hook cleanup through the actual released runtime. Seven Grok bridge regressions and the shared ACP/source suites pass (57 distinct cases total). Native account/model remain selected, project trust is retained and no paid request is made by the fixtures. [Implementation, evidence and owner review](76-grok-source-team-bridge.md). Authenticated owner-account and other-platform acceptance remain open.

## Antigravity source-task native contract

The checksum-pinned Windows 1.3.1 CLI now passes seven real-runtime contract cases (169 assertions) using the normal native agent, a disposable fake native account and localhost services. Native MCP envelope conversation/step metadata matches independently observed main-stream tool steps and full original wrapper arguments. Identical calls, native provider result return, process resume with advancing step identities, model-authored metadata isolation, missing/changed witness refusal, native strict-policy denial and workspace-only MCP discovery/configuration preservation are verified. Consumed replay, foreign-conversation and invalid-credential controls also refuse. A command hook is not required by this path.

The production bridge now uses a uniquely owned additive workspace plugin and the shared worker. Actual released-runtime driver cases prove owner-approved durable invocation/result return, native resume with a new step, changed-definition refusal, Stop at approval, queued-prompt cancellation, same-conversation recovery and overlap/final cleanup. Existing native app configuration is not rewritten. The release namespaces plugin servers by directory name; the expanded eight-case contract verifies this explicitly (195 assertions). [Native evidence](78-antigravity-native-source-contract.md), [production implementation and remaining owner review](79-antigravity-source-team-bridge.md). Eleven production source-task bridges are now implemented; authenticated owner-account/platform acceptance remains open.

## Kimi source-task contract investigation

**2026-10-16 update:** [current Kimi source research](83-kimi-current-contract.md) revalidates released 2.2.0 and identifies an in-process source-host candidate. Installed-plugin/ACP support and production/native runtime proof are still missing. The cached 2.1.1 analysis below records the earlier external-hook/MCP path. [Current Fx evidence](84-fx-source-contract.md) separately records its native transport identity gap. Counts remain unchanged.

The cached official Kimi Code 2.1.1 source (`f67e639`) exposes native session/tool-call IDs to `PreToolUse`, but that hook returns allow/block decisions and a reason, not a successful replacement tool result. Hook launch failures, timeout and cancellation return an allow decision. Its MCP client forwards the tool name, arguments and abort signal without the engine's native call ID. The MCP JSON-RPC request ID is not that native identity.

A hook that executes work then blocks the original call would report failure after creating work. Matching a separate MCP callback only by arguments would be ambiguous for identical concurrent calls, and hook failure can remove the expected ordering. Neither approach is accepted as an implementation. The remaining bridge needs a supported native executor carrying session/turn/call identity through successful result return, or an equivalently proven native contract. No Kimi source-task Teams tools are registered by this investigation; ordinary Kimi sessions and its existing bounded completion adapter retain their current behavior. No owner account or paid call was used.

Primary source: [native MCP request](https://github.com/MoonshotAI/kimi-code/blob/main/packages/agent-core-v2/src/mcpCore/client-stdio.ts), [external hook result handling](https://github.com/MoonshotAI/kimi-code/blob/main/packages/agent-core-v2/src/features/externalHooks/internal/runHook.ts), [official hook reference](https://github.com/MoonshotAI/kimi-code/blob/main/docs/en/customization/hooks.md). The conclusion about insufficient correlation is Ion's inference from those interfaces; it is not a claim that all future Kimi extension paths are unsupported.

## Review and remaining work

- [x] Fresh native Pi/Oh My Pi fixture acceptance after final compilation.
- [ ] Review Pi cold startup under memory pressure and slow native catalog/extension initialization.
- [ ] Verify native authenticated accounts, delegation/result review, Stop and restart on actual owner projects.
- [x] Bind Amp source callbacks to native owner-observed thread/tool-use identity; see the dedicated source-task bridge guide.
- [ ] Complete Amp native model/allowance/account/usage proof for bounded Teams completion.
- [ ] Complete Cursor native completion ceiling and account/catalog limits, with adversarial dispatch/cancel fixtures.
- [ ] Complete source-task Teams tools for Cursor, Fx and Kimi, with exact native call identity, approval/replay/Stop and native fixture acceptance.

These are current goal requirements, not deferred successes. The full goal remains active.
