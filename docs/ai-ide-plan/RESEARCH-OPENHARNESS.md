# OpenHarness research for ion

Checked 2026-09-28 against the projects' own repositories and documentation. This is product research, not a decision to adopt their code.

## Which OpenHarness?

The name is ambiguous. This note primarily covers [Autonomous's `openharness`](https://github.com/autonomous-ai/openharness), a desktop/mobile command center for agent sessions across machines, because that is the closest comparison to ion's client and daemon. [HKUDS's `OpenHarness`](https://github.com/HKUDS/OpenHarness) is a distinct Python agent framework with an `ohmo` personal agent. [Max Gfeller's `open-harness`](https://github.com/MaxGfeller/open-harness) is another distinct TypeScript SDK. Confirm the intended repository before implementing any OpenHarness-specific integration.

## Autonomous OpenHarness: verified capabilities

- A daemon on each machine owns agents and execution state. Desktop, mobile, web, CLI, and an optional USB device are clients over a shared command/event contract; closing a view does not stop the work. The architecture explicitly says remote access does **not** imply cross-machine task migration or portable sessions. [Architecture](https://github.com/autonomous-ai/openharness/blob/main/docs/architecture.md)
- It launches existing agent CLIs in tmux rather than wrapping their model loops. The daemon consumes vendor transcripts and hooks, normalizes turn/tool/question events, and reconciles pane state. This is a useful interoperability pattern for ion to study before adding another native model runtime. [README](https://github.com/autonomous-ai/openharness#built-the-way-developers-work), [architecture](https://github.com/autonomous-ai/openharness/blob/main/docs/architecture.md#daemon-sessions-and-transport)
- Remote machines dial outward; direct WebRTC, TURN, and an encrypted relay carry traffic. Pairing pins identities and the relay forwards ciphertext. Its own architecture describes the relay's metadata and the crypto primitives; these are design claims from the project, not an independent security audit. [README](https://github.com/autonomous-ai/openharness#how-it-works), [architecture](https://github.com/autonomous-ai/openharness/blob/main/docs/architecture.md#daemon-sessions-and-transport)
- Its domain package contract (`harness.json`) combines an engine, instructions/skills, project template, pinned toolchain, a `doctor` check, an optional live viewer, and a machine-readable verdict with findings and phases. Packages are data/files/processes loaded by the daemon rather than imported into the app; the catalog can refresh independently of client releases and retains an offline fallback. [Store guide](https://github.com/autonomous-ai/openharness/blob/main/store/README.md#what-a-package-is), [publishing and catalog](https://github.com/autonomous-ai/openharness/blob/main/store/README.md#live-catalog)
- Their device sends microphone audio to its host daemon for transcription, and the mobile client hosts no agents. Both reinforce an execution-host boundary that applies to ion's phone dictation and personal cloud work. [Architecture](https://github.com/autonomous-ai/openharness/blob/main/docs/architecture.md#ownership-boundaries), [device design](https://github.com/autonomous-ai/openharness/blob/main/docs/architecture.md#the-harness-device)
- The repository's app, CLI, daemon, relay and device are under [MIT](https://github.com/autonomous-ai/openharness/blob/main/LICENSE). The README says upstream tools and packages retain their own licences, so any copied package or bundled tool needs its own review. [README](https://github.com/autonomous-ai/openharness#contribute), [store licensing rules](https://github.com/autonomous-ai/openharness/blob/main/store/README.md#the-built-in-shelf)

## Ideas worth evaluating for ion

These are proposals, not claims about ion's current implementation.

1. **One action queue across sessions.** Surface pending agent questions/approvals with source session, machine, waiting time and a direct answer action. OpenHarness's shared command/event contract and question state show a practical basis for this. A phone view could focus on these decisions instead of mirroring the full desktop UI. [Architecture](https://github.com/autonomous-ai/openharness/blob/main/docs/architecture.md#system-overview)
2. **Execution-host clarity.** Show which local or personal cloud daemon owns each task, its connection state, and whether work continues when the client closes. Do not imply a disconnected host has migrated its running process. [Architecture](https://github.com/autonomous-ai/openharness/blob/main/docs/architecture.md#ownership-boundaries)
3. **Provider-neutral task events.** For supported external CLIs, normalize only observed lifecycle/tool/question events while retaining the raw source transcript for ordering and debugging. The adapter should expose uncertainty when a CLI format changes, rather than inventing state. [Architecture](https://github.com/autonomous-ai/openharness/blob/main/docs/architecture.md#daemon-sessions-and-transport)
4. **Small workflow packages.** Test a manifest for an ion workflow with setup checks, a pinned toolchain, optional artifact viewer, and a verdict file. Start with one existing coding workflow; validate manifest and executable paths before offering installation. OpenHarness's `doctor` and verdict schema are concrete precedents. [Store guide](https://github.com/autonomous-ai/openharness/blob/main/store/README.md#anatomy)
5. **Remote trust work before relay work.** The current owner-hosted cloud path can remain simple. If ion later offers machine discovery or a relay, model device identity, pairing, ciphertext forwarding, metadata retention, and failure paths explicitly; avoid treating TLS termination alone as an equivalent architecture. [Architecture](https://github.com/autonomous-ai/openharness/blob/main/docs/architecture.md#daemon-sessions-and-transport)

## Fit with ion's current work

ion already has a [daemon-owned typed protocol](../../README.md#architecture),
[normalized provider activity](../providers.md#the-driver-contract),
[Git worktrees and checkpoints](../../README.md#highlights),
[token/cost history](../../src/app/usage_page.rs), and
[phone-to-daemon ASR](14-phone-asr-cloud-execution.md). Rebuilding these as a
tmux-first harness would replace working seams without supplying the clearest
user benefit.

| Priority | Addition | First useful slice | Why it fits now |
| --- | --- | --- | --- |
| 1 | Task readiness preview | Before a Nebius or cloud task starts, report whether the selected daemon is connected, the key is available, the model can be reached, the workspace exists, and required tools are installed. Show the exact blocking step without starting an agent. | The [native runtime](09-native-nebius-runtime.md) and [cloud setup](14-phone-asr-cloud-execution.md) currently expose these as separate checks; the HKUDS dry-run is a product precedent. |
| 2 | Action queue | Start with one selected daemon: list pending approvals and questions across sessions, including task title, proposed action and waiting time, then jump to the existing review control. Expand to multiple daemons only after the per-daemon view is reliable. | The [typed task and approval model](05-agent-teams-automations.md) already exists; the missing layer is a focused cross-session view. |
| 3 | Execution-host labels | Put the owner daemon, local/cloud location, and connection/continuation state on task headers and mobile controls. Make a disconnected host visibly distinct from a completed task. | [Milestone 7](14-phone-asr-cloud-execution.md) added a cloud host while desktop and mobile can select daemons. This is a clarity change before adding new transport. |
| Later | Workflow package experiment | One local manifest for a coding workflow: instructions, allowed tools, setup checks, and an artifact viewer. Validate and preview it before allowing installation. | This extends ion's [skills and team contracts](05-agent-teams-automations.md) without committing to a marketplace or untrusted executable packages. |
| Later | Easier trusted remote linking | Research device identities and outbound encrypted access after the owner-managed cloud path is exercised live. | The current [cloud checklist](14-phone-asr-cloud-execution.md) still needs a real host and phone run. A relay is a larger infrastructure and security product. |

## Distinct HKUDS project

HKUDS OpenHarness supplies a Python agent loop, tool/skill/plugin system, permission rules and hooks, persistent memory, multi-agent coordination, and an `ohmo` gateway for Telegram, Slack, Discord and Feishu. Its README also documents an unreleased `--dry-run` that previews auth, skills, commands, tools and MCP readiness without invoking the model. The reusable idea for ion is a **preflight view** that tells a user exactly which provider credentials, local tools or MCP connections block a planned task, before execution. This is separate from Autonomous's multi-machine client and should not be conflated with it. [HKUDS README](https://github.com/HKUDS/OpenHarness#-openharnesss-key-harness-features), [dry-run description](https://github.com/HKUDS/OpenHarness#-whats-new), [architecture](https://github.com/HKUDS/OpenHarness#-harness-architecture). HKUDS's repository declares [MIT](https://github.com/HKUDS/OpenHarness/blob/main/LICENSE).
