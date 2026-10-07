# Amp Teams completion contract

Checked 2026-10-07 against current official documentation and native
`0.0.1791302439-g888480`. This investigates the unfinished Amp adapter;
it does not enable Teams, authenticate, change an account, or perform inference.

## Conclusion

Amp exposes exact custom-agent model selection and a capped plugin AI helper.
Its current native completion runs through an Amp thread actor. The supported
public interfaces do not yet establish the complete combination Ion requires:
a bounded single call, verified complete usage, the same selected native account,
no auxiliary inference, and no native effects. There is a concrete actor route to
investigate further; replacing it with a direct OpenAI/Nebius account would change
the selected harness/account and is not an implementation of this contract.

## Current primary contracts

- Custom agents can specify `provider/model`, instructions, explicit tool lists,
  reasoning effort and features. Explicit lists replace default/delegation tools,
  but `tool_search`/`code_exec` have special MCP behavior, so exclusion must be
  verified. `CreateAgentConfig` has no completion-token or maximum-turn field.
  `amp.ai.generate` accepts `model`, `system`, `maxTokens` and a thread ID but
  returns text or a schema-validated object. Its return type contains no usage
  receipt. `Agent.run` returns thread ID/text; its timeout rejects while the child
  can continue. Cancellation therefore requires the actual thread handle and
  acknowledged terminal state. [Plugin API](https://ampcode.com/docs/plugin-api).
- Streaming JSON documents optional assistant and final usage, including input,
  output and cache counts. This is stronger than the earlier claim that only
  thread-level usage exists. Optional fields and counters alone do not prove
  a completion cap or complete aggregate billing.
  [Streaming JSON](https://ampcode.com/docs/cli/streaming-json).
- Account routing tries active personal connections, workspace connections, then
  Amp. Exact model IDs can map to different provider IDs. API keys, subscriptions,
  gateways and cloud platforms have distinct routes. A model name cannot prove
  which connection billed the request. `check-access` performs actual inference
  and is unsuitable for a free readiness check.
  [Model routing](https://ampcode.com/docs/customize/model-routing).
- External API `GET /threads/{threadID}/usage` returns model request counts,
  tokens, costs and included subthreads. It requires workspace-scoped external
  application authorization. It is not a verified substitute for a native user
  account receipt. BYOK costs are estimates, exclude router/subscription costs,
  and do not include negotiated prices. This endpoint cannot enforce a cap before
  execution. [External API](https://ampcode.com/api/external),
  [OpenAPI](https://ampcode.com/api/v2/openapi.json).

## Verified native evidence

Inspected the already cached official Windows executable, without credential
stores. Its SHA-256 is
`95329292530e295786504d1d1b31926c4cbb8f7648de690b30398a9cd2885651`,
which matches the freshly fetched
[official checksum](https://static.ampcode.com/cli/0.0.1791302439-g888480/windows-x64-amp.sha256).
The [installer](https://ampcode.com/install.sh) specifies the versioned download
layout; the corresponding
[binary](https://static.ampcode.com/cli/0.0.1791302439-g888480/amp-windows-x64.exe)
was not installed globally or upgraded. Evidence below is release-specific
bundled JavaScript, not a claim that private server APIs are stable/public.

1. Native execute creates/resumes via `POST /api/thread-actors`, then exchanges
   actor messages. The local CLI is the executor; model orchestration is remote.
   Intercepting `AMP_URL` alone does not turn inference into a direct provider
   completion with verified request limits.
2. Plugin `ai.inference` resolves that thread actor and POSTs `/inference` using
   native Amp bearer authorization. The request is `{prompt, options, schema?}`;
   options forward the helper's exact model/system/reasoning/maxTokens. The native
   wrapper returns the actor's `result`; plugin code then returns only text or
   validated content. Complete usage is not exposed by this helper.
3. Native assistant usage permits `estimated: true`. Stream conversion emits
   counters but drops that flag and model identity; absent input/output becomes
   zero. Therefore Ion must inspect the original authenticated usage witness,
   reject estimates/missing values, and cover every request, rather than trust
   the stream mapping or treat missing usage as zero.
4. Catalog RPC `getAgentModelCatalog` includes exact ID/provider/name,
   `contextWindow`, `maxOutputTokens` and capabilities. These support bounded
   metadata readiness; catalog ceilings do not constrain actor auxiliary calls.
5. `plugins exec` is an offline hook test runner. Its request handler set has no
   `ai.inference` handler. It cannot be repurposed as a promptless capped bridge.
6. Native `check-access` returns usage/routing, but custom prompt and provider-key
   overrides are explicitly staff-only. Its request schema has no output cap.
7. Bundled schemas name `prepareActorInference` and `recordActorInference`.
   Preparation can resolve native provider/router credentials and limits;
   recording binds request ID and usage with deduplication. This binary contains
   schemas, not callable actor-side implementations. Ordinary native-user
   authorization and public availability are unverified. Do not enable an
   adapter from the names alone or try privileged requests against an account.

## Local fixture results

Cached native binary was run with a fake key, localhost service, clean environment,
private HOME/config/cache/data/temp directories, empty cwd, no MCP, disabled
plugins/updates and no user profile access.

- `plugins show-agent-options --json`: exit 0, exactly one
  `POST /api/internal?getAgentModelCatalog`, body
  `{method: "getAgentModelCatalog", params: {}}`, fake bearer accepted by fixture.
  CLI preserved fixture ID, input/output ceilings and capabilities. This proves
  metadata transport, not real account catalog availability.
- `plugins exec` with the capped helper during plugin initialization: native
  plugin loaded, then reported `No handler for plugin request method: ai.inference`.
  Earlier event-hook-only probes were inconclusive because the hook did not run;
  they are not counted as contract verification.

Ignored artifacts: `.waku-cache/harness-contracts/amp-contract-catalog-mpoyf9zb/`
contains `result.json`, `plugin-exec-result.json` and the fixture plugin. No model,
account or cloud calls, native credentials, global installs or UI tests occurred.

## Implementable route and remaining proof

A future adapter should keep the selected native Amp identity and exact CLI
release, fetch the exact catalog, create a private thread with explicitly excluded
native effects/features, and use the actor's bounded helper path. It must receive
authenticated original per-call usage plus a complete thread/request receipt,
verify the exact connection and provider model, deny auxiliary dispatch, preserve
an uncertain submitted request through restart, and confirm cancellation before
cleanup/replay. Those are concrete requirements, not conditions satisfied by the
metadata fixture.

The next decisive evidence is a supported actor/helper response contract with
complete metering and account identity, or a supported route to the same selected
native account that permits Ion's existing one-use transport gate. Server-side
caps, retries, auxiliary inference, estimated usage and cancellation finality
remain unproven. Preserve the current specific readiness failure and pinned Amp
selection until that evidence exists. Ordinary solo Amp remains a separate,
already implemented flow.
