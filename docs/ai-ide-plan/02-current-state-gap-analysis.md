# Current Waku state and gap analysis

## Executive assessment

Waku already contains the difficult local execution foundation: a standalone
daemon, a versioned protocol, many coding-agent provider adapters, persisted
sessions, Git/worktree operations, diffs, files, terminals, desktop/web/mobile
clients, and a Cua Driver bridge. The product gap is primarily orchestration,
information architecture, cloud identity, integrations, scheduling, and
production hardening—not an IDE rewrite.

## Reuse versus build

| Area | Current Waku state | Decision |
|---|---|---|
| Agent providers | Structured adapters for multiple coding-agent CLIs | Reuse behind a common team-member interface |
| Daemon/client split | Desktop is an RPC client of `waku-daemon` | Keep as the trust and execution boundary |
| Protocol | Versioned Rust/TypeScript contract over authenticated WebSocket | Extend with capability negotiation and new resources |
| Persistence | Daemon-owned SQLite task/session data and blobs | Migrate schema; do not add parallel client truth |
| Projects | Durable records exist, but desktop project groups are session-derived | Decouple project rows from task/session rows |
| Tasks | Waku sessions are presented as tasks | Introduce a task aggregate that can own multiple sessions/runs |
| Git/worktrees | Branch inspection, worktree creation, checkpoints, diffs, commit/push | Reuse for issue-to-PR and parallel agents |
| Files | Remote tree/read/write operations exist | Reuse, then add richer previews incrementally |
| Terminal | Desktop/local and daemon terminal paths exist | Reuse with explicit remote/device policy |
| Browser | Embedded desktop browser exists by platform | Keep separate from general Cua computer control |
| Computer use | Cua Driver 0.28 is embedded but release-disabled | Productize after permissions, audit, and stop controls |
| Web client | Browser client speaks the same protocol | Reuse for hosted demo/control plane surfaces |
| Mobile | Expo iOS/Android client connects to remote daemons | Reuse as remote control plane; harden identity and transport |
| App connections | No user-facing connected-app platform | Add through a server-side Composio gateway |
| PR/issues | Git diffs exist, but no first-class remote issue/PR domain | Add normalized Git-forge objects and views |
| Automations | No durable scheduler/run model | Add cloud/local scheduling and event triggers |
| Agent teams | Provider-native background/subagent events exist, but no cross-provider team model | Add daemon-level orchestrator and typed delegation |
| Voice | No speech-to-text composer flow | Add local Nemotron 3.5 ASR through NeMo-Speech.cpp |
| BYOK | No native Nebius credential/runtime | Add typed OS-vault credential service and native Nemotron provider |

## Evidence in the repository

### Daemon and security baseline

The daemon is loopback-only by default, requires a bearer token, requires an
explicit non-loopback opt-in, validates browser origins, and does not terminate
TLS. The token is currently a **full-control capability**, not a scoped user or
device identity. See the [daemon README](../../crates/waku-daemon/README.md).

This is adequate for a trusted LAN/private tailnet prototype. It is not adequate
for a personal remote-access product exposed directly to the internet.

### Mobile baseline

The repository contains an Expo client for iOS and Android that can store daemon
profiles and place tokens in the platform keychain. Its stated model is to connect
to one or more remote Waku daemons; see the [mobile README](../../apps/mobile/README.md).

The mobile code already includes:

- daemon selection, connection, retry, and reconnect;
- task/session list and transcript;
- composer, attachments, provider/model choices, and remote project picking;
- remote workspace file tree and text reads;
- Git review diffs;
- a remote terminal surface;
- rename/archive and task menus.

It does not make an iPhone or Android device the coding host. Provider processes,
files, Git, and terminals remain on the selected daemon host.

### Project visibility problem

`Project` currently stores ID, name, path, and creation time in
[model.rs](../../crates/waku-protocol/src/model.rs). The desktop sidebar's project
groups are built from started sessions in
[sidebar.rs](../../src/app/sidebar.rs). Therefore a project with no task/session
has no natural row in that grouped history.

The target design renders project rows from `state.projects` first and nests task
rows beneath them. Pin and order metadata belongs to the project/sidebar model,
not to an incidental chat sort.

### Grouping and sorting problem

The desktop currently supports Project/Updated grouping and Newest/Oldest
ordering, including option menus and persisted preferences. This flexibility
works against the requested fixed product hierarchy. The plan removes those
controls and migrates existing users to:

- pinned items in manual order;
- projects in manual order;
- tasks within projects by meaningful activity.

### Computer use baseline

[Computer Use documentation](../computer-use.md) records a native Cua Driver SDK
integration for macOS, Windows, and Linux, but explicitly clamps it off in release
builds. It already includes native capture/action plumbing and preview handling.
The missing work is a production authorization product, not a second automation
engine.

### Licensing baseline

The repository is [GPL-3.0-only](../../LICENSE). Public rebranding must retain
license and attribution obligations. ion remains free/BYOK; user-borne Nebius
charges belong to the user's own account.

## Gaps by user request

### Project appears immediately in the sidebar

Gap: project presentation is coupled to started sessions.

Plan:

- make project rows first-class and always visible;
- create explicit project pin/order/archive fields;
- allow a project-level context menu: new task, pin/unpin, rename, settings,
  reveal/open, archive/remove;
- persist sidebar metadata in daemon state so desktop/web/mobile agree;
- create an empty-project state with **New task**, **Connect GitHub**, and
  **Create automation** actions.

### Pinned projects and grouped sections

Gap: no durable pinned-project model and the current group control changes the
entire task list.

Plan: fixed **Pinned** and **Projects** sections. Both projects and individual
tasks can be pinned. Groups are structural, not user-selectable presentation
modes.

### Remove sort/grouping options

Gap: menu and saved preferences currently exist.

Plan: remove the UI and migrate old values without deleting them immediately.
Ignore legacy values for one compatibility release, then remove the fields in a
later schema cleanup.

### Pull requests and issues

Gap: Waku understands local Git state but not remote forge records.

Plan: introduce forge-neutral issue/PR models and a GitHub adapter backed by
Composio for the MVP. Keep local Git worktrees and diffs authoritative for code;
use the integration only for remote metadata and actions.

### Task management

Gap: a session is currently the unit of UI and execution.

Plan: create a durable task aggregate over sessions, runs, work items, artifacts,
approvals, and external references. Migrate current sessions one-to-one.

### Automations

Gap: no scheduler, triggers, leases, or run history.

Plan: add an automation control plane with time, event, and heartbeat triggers;
durable run records; idempotency; explicit action policy; and local/cloud
execution targets.

### Apps

Gap: provider tools and skills exist, but there is no user account connection
catalog or cross-provider app-tool gateway.

Plan: use Composio through a backend adapter, expose connected accounts and
scopes as product objects, and translate allowed actions into provider-neutral
tools.

### Agent teams

Gap: no durable cross-provider supervisor/delegation graph.

Plan: add teams and role definitions to the daemon, with a Nemotron-backed
planner delegating typed native Nemotron tasks. Agent-to-agent commands are
validated through delegation policy and persisted as work items.

### Voice input

Gap: no audio capture or STT service.

Plan: add push-to-talk and streaming composer input, local transcription,
editable preview, language selection/auto-detection, and explicit send. Run
NVIDIA Nemotron 3.5 ASR Streaming 0.6B through NeMo-Speech.cpp on the daemon host
so microphone audio remains local and Hindi (`hi-IN`) is supported. Model
loading and inference stay off the UI thread and expose explicit
download/progress state.

### Models and usage

Gap: Waku is accountless and local-only by design.

Plan: add free BYOK Nebius setup, key/catalog verification, and informational
usage. The user's own Nebius account bears provider charges. No ion account,
subscription, entitlement, or paid feature gate is required.

## Constraints that should remain true

- Heavy work and network I/O never enter GPUI render paths.
- Long collections remain virtualized.
- Provider-native event ordering is preserved.
- Existing projects, sessions, checkpoints, and provider continuity survive the
  task-model migration.
- The app remains usable offline for local tasks.
- Remote clients never interpret daemon-host filesystem paths locally.
- A disconnected mobile client can safely resynchronize without overwriting
  newer daemon state.
