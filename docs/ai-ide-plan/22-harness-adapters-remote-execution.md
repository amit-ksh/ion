# Teams harness adapters and Nebius execution

Updated 2026-10-06. Implementation: `codex/ion-agent-teams` in the managed
worktree. No integration into `ion-prod`, cloud deployment or live provider
acceptance is implied by this checkpoint.

## Current execution coverage

| Selected harness | Local Teams completion | Registered sandbox worker | Authentication |
| --- | --- | --- | --- |
| Nebius | Native metered tool protocol | Yes | Local Settings key; remote host-owned key |
| Claude Code | Restricted CLI with one turn | Yes | That host's native Claude login |
| Codex | Isolated CLI plus one-use Responses transport | Yes | That host's native Codex login |
| OpenCode | Pure CLI plus one-use transport | Yes | Selected direct OpenAI, Anthropic or Nebius API login |
| Pi | Tool-free CLI plus one-use transport | Yes | Selected direct OpenAI, Anthropic or Nebius API login |
| Oh My Pi | Tool-free CLI plus one-use transport | Yes | Selected direct native SQLite API login; bundled exact model catalog |
| DeepSeek Harness | Private native headless composition plus one-use Chat transport | Yes | Native host DEEPSEEK_API_KEY; verified 0.1.5-rc.1 runtime |
| Kimi Code | Private tool-free native CLI plus bounded one-use Chat transport | Yes | Selected native direct coding API login; verified 2.1.1 runtime |
| Antigravity, Amp, Cursor, Fx, OpenCode 2, Grok | Teams adapter still required | Not enabled | Selection remains pinned |

The other six harnesses remain in the normal app catalog. Their existing solo
runtimes do not establish the permission, single-request and accounting contract
needed by this Teams executor. They are not silently replaced with another CLI
or a Nebius model. OpenCode/Pi/Oh My Pi/Kimi OAuth, plugins and custom model endpoints also need
separate verified transports before they can be enabled here.

### Contracts still needed for the other catalog harnesses

The current source transports establish the following gaps. A normal solo run
is not evidence that a Teams adapter can enforce these constraints.

| Harness group | Existing transport | Contract required before enabling Teams |
| --- | --- | --- |
| Antigravity | Native NDJSON session | Verified tool/resource disable controls, one-request boundary and exact model usage/caps |
| Amp | Native streaming JSON and CLI-managed modes | Restricted permission rules and a verified exact model/request/accounting boundary; the current solo launch requires Full access |
| Cursor, Fx, Grok | Long-lived ACP sessions | Disable native effects and auxiliary calls; pin a model, enforce request limits and receive complete usage |
| OpenCode 2 | Shared resident HTTP/event service | Isolated bounded completion sessions with enforceable effects and request/cap accounting |

Those adapters remain development work. Enabling a provider without these checks
would expand an agent's saved access or spend beyond its reserved calls. Unsupported
profiles block readiness before an attempt or billable request is created.

## What executes where

Ion's owner daemon keeps leases, budgets, approvals, app connections, hierarchy,
private checkpoints and isolated coding files. Each external harness prepares
one completion and returns Ion's structured action contract. Only Ion executes
those actions through the existing grant checks and durable receipts.

A sandbox-selected agent runs its selected harness completion in an immutable
ConTree image. The owner transfers bounded instructions, granted message/input
content and the available Ion tool descriptions. It receives bounded structured
output and provider usage. Files are read/edited in the owner's isolated Teams
workspace; app actions run on the owner after their exact approval.

**This increment does not implement a complete remote project filesystem,
arbitrary sandbox commands, remote test execution, GPU provisioning or ML
training.** Those require a remote tool/command host with resource and egress
limits, input/output transfers, command receipts and spending controls. ConTree's
verified instance request has a layer-size limit but no verified GPU/CPU resource
fields. Do not describe this checkpoint as an ML execution service.

## Local setup

1. Install and sign in to the exact selected CLI on the machine running the daemon.
   Claude and Codex retain native authentication. OpenCode and Pi currently use a
   selected direct API login stored by their own CLI. Oh My Pi reads its own
   SQLite `agent.db` login without modifying it.
2. Pin an exact model, including the provider namespace for OpenCode/Pi/Oh My Pi. Aliases,
   unknown endpoints, absent token limits and unavailable models block readiness.
3. Codex needs a fresh native `models_cache.json` (under `CODEX_HOME`, or
   `~/.codex`), less than 24 hours old. Refresh it by using Codex normally.
4. Pi requires its current offline/resource-disable flags. OpenCode requires
   `run --pure`. Oh My Pi requires its own no-tools, no-session, no-LSP,
   no-PTY, no-extensions, no-skills, no-rules, no-title and no-prewalk flags.
   Readiness checks these controls before an attempt starts.
5. If a package-manager shim fails, set the installed executable in ion's harness
   settings. The local fixture check uses the actual installed binary; no harness
   is substituted. Windows Volta wrappers can fail when their install directory
   cannot be resolved in a contained test process.
6. Budget reservations use actual native context/output limits, including cache
   and reasoning usage. A small token cap can block a call before billing. Codex
   reserves a full context for both input and output because its subscription
   transport does not expose a verified smaller completion cap.

CLIs run in private temporary directories with tools, hooks, extensions or plugins
removed where their contract supports that control. The one-use gate additionally
removes native tool definitions and rejects native effects before returning any
provider events to the CLI. It permits one authenticated request to a fixed
provider origin, checks the pinned model and complete usage, and forbids redirects
and retries. Temporary selected-account files and private scratch data are cleaned
up after the contained process tree exits. OpenCode and Pi remove inherited direct
provider key overrides so the copied selected account supplies authentication.
Oh My Pi clears inherited environment variables except the OS/runtime allowlist.
It uses private SQLite/YAML configuration, pins every model role and disables
fallbacks, retries, updates, setup and auxiliary work. A native startup discovery
request cannot consume the inference capability: only the exact model route is
changed to that gate. Readiness rejects native brokers, profile/overlay selection,
custom model files, ambiguous accounts and OAuth; those routes need their own
verified transport. `PI_CONFIG_DIR` must be relative to the native home.
DeepSeek uses the verified published runtime **0.1.5-rc.1** and exact
`deepseek-official/deepseek-v4-flash` or `deepseek-official/deepseek-v4-pro`.
Configure `DEEPSEEK_API_KEY` privately on the actual execution host. Web-managed
authentication and custom endpoints remain unsupported. Its native release uses
Chat Completions; newer source contracts are not assumed compatible. Ion creates
a private profile with an empty bundle list and a verified 15-plugin composition,
without native effect producers, workspace resources, credentials services,
compaction or retries. It verifies `--version` and the full `--dump-config` before
launch, and reserves the explicitly pinned native defaults of 1,000,000 input and
256,000 output tokens. Other releases/model IDs need a contract check first.
Kimi Code uses the verified native **2.1.1** runtime and exact
`kimi-code/k3`, `kimi-code/kimi-for-coding` or
`kimi-code/kimi-for-coding-highspeed`. Its selected native `config.toml` under
`KIMI_CODE_HOME` (default `~/.kimi-code`) must contain a direct `kimi` coding
account, the official coding endpoint and published context limits. Literal
`api_key` or a selected `api_key_env` is supported; OAuth, custom headers/routes,
renamed aliases and model overrides still need verified transports. Ion does not
change this file. Private configuration contains only the selected model and a
key environment name; only the contained child receives the actual key. The
native agent has no tools or subagents. Skills, hooks, MCP/plugins, updates,
telemetry, cron, title generation, retries, watchers and background continuation
are excluded. JSON input remains literal through native prompt templating and
both raw and escaped input are bounded. Full native contexts are reserved:
1,048,576 for K3 or 262,144 for the other two models, plus 32,768 output tokens.
The gate rejects missing, enlarged or ambiguous output caps before forwarding.
Raw credentials/events are not stored in team transcripts or process failure messages.

### Kimi acceptance later

- [ ] Install the selected Kimi Code 2.1.1 runtime or set its executable in
  harness settings. The archived Python kimi-cli runtime is not this contract.
- [ ] Configure that host's native direct coding account privately and select one
  exact supported model. Ordinary OAuth login still requires development work.
- [ ] Run one Local team task and inspect provider identity, usage and generated
  documents. Verify a model/context/endpoint change blocks readiness and that
  source native configuration, app connections and credentials stay unchanged.
- [ ] For sandbox execution, register the same supported model and full limits in
  the immutable Linux host manifest, then verify native Linux execution later.

## Prepare a registered sandbox host later

Protocol is **28**; rebuild desktop, daemon and client bundles together.
No deployment target has been supplied. Build/provisioning and live acceptance
remain your setup steps; this implementation sends no real sandbox requests.

- [ ] Build the worker **on Linux for the execution image**, from this same source:

  ```sh
  cargo build -p waku-core --bin ion-harness-worker --release --locked
  ```

- [ ] Install it at `/usr/local/bin/ion-harness-worker` in a private ConTree image.
  Install the exact selected CLI and its runtime too. The image needs HTTPS access
  to that model provider and the native catalog needed for token verification.
- [ ] Configure the selected provider account on the execution host, using the
  host's credential/bootstrap mechanism. Nebius's worker reads
  `ION_TEAM_NEBIUS_KEY` **on that host**. The owner never exports local vaults,
  auth files or provider keys into the sandbox request.
- [ ] ConTree's `shell:false` contract does not inherit image environment variables.
  A host-owned wrapper may supply its native credential/bootstrap environment
  before executing the worker. Test that on the registered host. Keep credentials
  outside prompts and public images; no secret is included in the example below.
- [ ] Add `/ion/host.json`, version 1, listing exact installed harness/model limits.
  Example shape (replace the placeholder ID and limits with native catalog values):

  ```json
  {
    "version": 1,
    "capabilities": [
      {
        "provider": "codex",
        "model": "EXACT_NATIVE_MODEL_ID",
        "input_cap": 272000,
        "output_cap": 272000
      }
    ]
  }
  ```

  The worker verifies actual native limits before inference. A smaller registered
  allowance, unsupported harness or changed identity fails without completing.
- [ ] Register/pin the resulting immutable image UUID. The agent's sandbox target
  accepts `UUID` or a readable `name@UUID`; an unpinned image alias is rejected.
- [ ] Configure `ION_NEBIUS_SANDBOX_IAM_TOKEN` and
  `ION_NEBIUS_SANDBOX_PROJECT` privately on the **owner daemon's process**.
  Sandbox IAM credentials are separate from inference keys. Do not paste them
  into chat or save them in this repository.
- [ ] Enable **Token Factory sandbox** in Settings. Choose Nebius sandbox on the
  agent and save its immutable target. Keep Local fallback enabled if desired.

## Durable remote lifecycle

The owner uses the official asynchronous ConTree REST contract at
`https://api.tokenfactory.nebius.com/sandboxes/v1`:

- Check `/ion/host.json` through image inspection before claiming remote work.
- Reserve the model call and persist a unique request record and exact private
  input **before** POST. A second preparation/submission for that ID is rejected.
- Spawn one instance with bounded base64 stdin, 150-second process timeout,
  64 KiB output and 256 MiB layer limit. The request environment contains only PATH.
- Persist the acknowledged operation UUID. The run audit includes the request ID,
  hostname and operation ID for operator inspection, without the POST response's
  echoed environment or credentials.
- Poll that exact operation and image. Stop requests cancellation, then waits for
  SUCCESS, FAILED or CANCELLED. A 202 acknowledgement alone keeps capacity held.
- Validate process exit, output truncation, worker version, request, harness, model,
  tools and usage before allowing any owner action. Valid late usage may settle
  accounting after Stop; it does not authorize tools or another completion.
- Restart checks/cancels existing operation IDs. It never repeats the POST.
  Lost acknowledgement has no trustworthy operation identity: the owner retains
  the reservation/capacity and asks for operator inspection rather than resubmitting.
  Open **Review recovery** for the uncertain assignment. If its acknowledgement
  was lost, enter the operation UUID found in Nebius and use **Verify and bind
  operation** on the corresponding request. Ion fetches the operation in its
  original project and checks the image, hostname, executable, environment,
  limits and complete saved input (including harness/model/caps/messages/tools).
  A typed UUID or human stop assertion alone cannot clear it. Binding never POSTs.
- Use **Verify sandbox stopped**. Nonterminal operations receive cancellation;
  acknowledgement alone retains capacity and all unknown token allowances. Refresh
  and verify again after a terminal result. Valid matching late model usage can
  settle accounting without authorizing output actions.
- After terminal verification and any owner file/app reconciliation, use **Prepare
  continuation**, then **Continue from checkpoint**. The same attempt, actual
  host, harness/model, grants, deadline and all budget history are retained. Cached
  provider actions are not replayed. Further completions are new calls within the
  remaining budget; the old POST is never repeated or moved to Local.
- Requests saved before this checkpoint lack the full input witness and cannot
  be automatically bound after a lost acknowledgement. They require external
  operator inspection and retain their unresolved reservation/capacity. Recovery
  on a stopped/canceled run can record terminal outcomes but cannot resume it.

Local fallback is selected before execution when a registered host is unavailable
or incompatible and the same local harness/model is ready. An unavailable local
CLI remains a setup blocker. A failure after submission does not automatically
launch a second worker, even if its remote outcome is uncertain. Review that
operation before further work. Unknown usage stays reserved.

## Verification and live review

Automated fixture evidence is recorded in
[implementation progress](19-agent-teams-progress.md). These tests use localhost
and fixture credentials; they do not prove live account or cloud acceptance.

From this worktree:

```powershell
cargo test -p waku-core team_ --lib --locked -- --test-threads=4
cargo test -p waku-core teams:: --lib --locked -- --test-threads=4
cargo check -p waku -p waku-daemon --locked
cargo build -p waku-core --bin ion-harness-worker --locked
```

Installed CLI fixture checks (no live model request):

```powershell
cargo test -p waku-core driver::team_ --lib --locked -- --ignored --nocapture --test-threads=1
```

The optional `ION_TEST_PI_BINARY`, `ION_TEST_OPENCODE_BINARY` and
`ION_TEST_OMP_BINARY` and `ION_TEST_DEEPSEEK_BINARY` variables select
an installed executable for these fixture checks. They do not change app settings.

Your live review:

- [ ] Create a mixed Nebius/Claude/Codex/OpenCode/Pi/Oh My Pi/DeepSeek team with your exact available
  models and grants; complete a delegated document and an isolated code edit.
- [ ] Confirm task/run labels show each selected harness, model and actual host.
- [ ] Set a token cap below one required reservation; verify no provider call starts.
- [ ] Deny an app or filesystem action; verify it cannot execute via native CLI tools.
- [ ] Enable a registered image and complete a small sandbox-selected team request.
  Check its actual remote operation and the owner-generated document.
- [ ] Disable/unconfigure the sandbox before starting; verify authorized Local fallback
  keeps the same harness, model and grants, with a visible fallback reason.
- [ ] Stop during remote inference; verify the operation reaches a terminal state and
  no output actions execute after Stop. Missing usage must remain reserved.
- [ ] Interrupt the owner after spawn acknowledgement; restart and verify GET/DELETE
  reconciliation of the same operation without a second POST or local worker.
- [ ] Simulate a lost spawn acknowledgement in a disposable environment; inspect
  the recorded hostname/request in Nebius. It must stay unresolved automatically.
  Enter the operation ID in Review recovery, verify and bind it, then verify its
  terminal status. Try an unrelated operation and changed metadata: both must fail.
- [ ] Prepare and explicitly continue a verified stopped sandbox assignment; confirm
  the same task/attempt/host and a new request ID, with old unknown usage retained.
  Retry a lost binding/confirmation response and verify no duplicate POST or GET.
- [ ] Stop the run before recovery completes; it can reconcile terminal state but
  cannot prepare or start another completion.
- [ ] Review actual provider usage and host startup timing. Live subscription routes,
  CLI versions, Linux worker build, image bootstrap and cloud billing remain unverified
  until performed against your configured resources.

## Primary contract references

- [Codex configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference)
- [Codex app-server protocol](https://learn.chatgpt.com/docs/app-server)
- [Subscription API limitations](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations)
- [OpenCode agents and permissions](https://opencode.ai/docs/agents/),
  [CLI and environment](https://opencode.ai/docs/cli/),
  [configuration layers](https://opencode.ai/docs/config/)
- Pi installed package `@earendil-works/pi-coding-agent` 0.84.1:
  `docs/models.md`, CLI flags, native RPC, model registry and auth storage source.
- [Oh My Pi v18.6.1 source and release](https://github.com/can1357/oh-my-pi/releases/tag/v18.6.1):
  native CLI flags, RPC catalog, model registry, SQLite credential storage and
  native directory resolution. Its contract differs from Pi.
- [DeepSeek Harness 0.1.5-rc.1 source](https://github.com/deepseek-ai/DeepSeek-Harness/tree/dsh-v0.1.5-rc.1),
  [published native runtime](https://pypi.org/project/deepseek-harness-runtime-bin/):
  release adapter, headless runner, private profile composition and default model limits.
- [ConTree OpenAPI contract](https://eu-north.nebius.computer/static/api.yaml),
  [instance spawn](https://docs.tokenfactory.nebius.com/api-reference/sandboxes/instances/spawn-a-new-container-instance),
  [operation cancellation](https://docs.tokenfactory.nebius.com/api-reference/sandboxes/operations/cancel-an-operation).

Kimi contract references: [published 2.1.1 release](https://github.com/MoonshotAI/kimi-code/releases/tag/%40moonshot-ai%2Fkimi-code%402.1.1),
[native configuration](https://github.com/MoonshotAI/kimi-code/blob/f67e639/docs/en/configuration/config-files.md),
[agent tool controls](https://github.com/MoonshotAI/kimi-code/blob/f67e639/docs/en/customization/agents.md),
[request/runtime switches](https://github.com/MoonshotAI/kimi-code/blob/f67e639/docs/en/configuration/env-vars.md).
