# BYOK, Nebius, and development resources

The [plan README](README.md) is the decision record. This chapter is the
implementation resource map. Check upstream documentation before coding against
an API; URLs and available models can change.

## Free BYOK boundary

ion is free software. Each user supplies a Nebius Token Factory API key for
their own inference account. There is no checkout, subscription, entitlement,
paid feature gate, managed inference balance, or resale of Nebius capacity.
The Token Factory base URL is fixed to
`https://api.tokenfactory.nebius.com/v1`; it is not user editable. A key check
requests `GET /models` over HTTPS and shows the returned model IDs in memory.
The native Nemotron runtime uses this same boundary. The desktop and local
daemon read the same user vault entry through `waku-credentials`; the key does
not cross the task protocol. Memory-only keys can verify a catalog in the
desktop process but cannot start a daemon task. The runtime reports that the
key must be saved to the OS vault.

Three typed desktop secrets exist: `NebiusTokenFactoryApiKey`,
`ComposioScopedProjectKey`, and `GatewayPairingToken`. The desktop credential
service offers set, test, delete, and status only. Windows Credential Manager,
macOS Keychain, and Linux Secret Service are the persistent adapters. If an OS
vault is unavailable, the value lives only in process memory and the UI says
so. Do not put any secret in `settings.json`, SQLite, logs, transcripts,
telemetry, model context, CLI arguments, or a child process environment.
The daemon and the model receive authorized results, not raw credentials.
Composio and pairing validation become network checks when their integrations
exist; until then their test operation reports local storage status only.

## Guided personal Nebius gateway

The guided setup asks the user to provision their own Nebius account and
gateway, then pairs the desktop with a short-lived token. The gateway is
personal infrastructure, not an ion-operated control plane. Its credential
vault and Composio project key stay outside model context. The wizard must
show which Nebius resource will run, expected user-borne cost, scope, and
deletion path before provisioning. Pairing stores only its token in the OS
vault. Mobile pairs to that personal gateway with scoped device grants.

## Primary development resources

| Concern | Official resource | Use |
| --- | --- | --- |
| Token Factory API | [Nebius Token Factory documentation](https://docs.tokenfactory.nebius.com/) | API keys, OpenAI-compatible calls, models, limits |
| Token Factory model catalog | [Model API](https://docs.tokenfactory.nebius.com/api-reference/models/list-models) | Verify current model IDs and metadata |
| Nemotron model | [Nemotron 3 Super cookbook](https://github.com/nebius/token-factory-cookbook/blob/main/models/nemotron/nemotron3-super-120B.md) | Prompt and tool-calling examples; verify live catalog ID |
| Nebius sandbox | [Token Factory Sandboxes](https://docs.tokenfactory.nebius.com/sandboxes) | Optional isolated execution after local runtime works |
| Nebius compute | [Serverless AI](https://docs.nebius.com/serverless) | Jobs for finite work, Endpoints for served workloads |
| Local ASR runtime | [NVIDIA NeMo-Speech.cpp](https://github.com/NVIDIA/NeMo-Speech.cpp) | Native speech runtime, SDK, and server |
| ASR model and CLI | [NeMo-Speech.cpp ASR models](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/main/docs/asr/models.md) and [CLI](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/main/docs/cli.md) | Pull `nemotron-3.5`, stream locally, choose language |
| Native ASR SDK | [NeMo-Speech.cpp SDK](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/main/docs/sdk.md) | Direct streaming integration without cloud audio |
| Composio | [Composio documentation](https://docs.composio.dev/) | Scoped project setup, connected accounts, sessions, tool execution |
| GPUI | [Pinned Zed GPUI source](https://github.com/egoist/zed/tree/waku-webview/crates/gpui) | Match repository-pinned native UI APIs |
| Waku internals | [Protocol](../../crates/waku-protocol/README.md), [core](../../crates/waku-core/README.md), [daemon](../../crates/waku-daemon/README.md) | Existing compatibility and runtime seams |

## Build and milestone discipline

Read the repository [AGENTS.md](../../AGENTS.md). The existing
`bun ./scripts/dev.ts` watcher owns the debug app. Let it rebuild after edits;
do not start another watcher or bundle manually. Keep I/O off GPUI render
paths. Run proportional static/build checks; do not add tests or run the test
suite for Milestone 1. Do not perform visual testing: the user validates the
rebuilt app. Finish one milestone, report changes, limits, credential behavior,
and a short manual checklist, then stop. Begin the next milestone only after
the user explicitly tests and approves the completed one.
