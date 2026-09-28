# ion product plan

This README is the canonical decision record for the ion hackathon build. The
chapters below expand it; if older research notes conflict, this page wins.
Milestone 1 established the distribution, credential, branding, and planning
boundaries. Milestone 2 implements the native coding runtime; its live provider
acceptance check remains pending until a Nebius key is available.
The user explicitly authorized Milestone 3 implementation before that live
check; Milestone 2 acceptance remains open for later review.
The user subsequently authorized Milestone 4 implementation while deferring
Milestone 2 and 3 review until a Nebius key is available. All three checklists
remain open until their evidence is reviewed.
The user also authorized Milestone 5 development with those live reviews
deferred. Its mobile and submission review remains open; the roadmap currently
defines five milestones.

## Locked decisions

- Extend Waku directly. Customer-facing identity is **ion** (lowercase). Keep
  internal `waku-*` crates, storage paths, environment variables, database
  schema, and wire identifiers stable through the hackathon.
- Ship a Nebius-first **NebiusHackathon** distribution profile. New tasks and
  model/settings surfaces offer only Nebius. Historical tasks stay readable,
  while starting or resuming a legacy provider is rejected. A development-only
  `ION_DEV_FULL_PROVIDERS=1` override enables the **Full** profile in debug
  builds; there is no user-facing bypass. Do not rewrite `disabled_providers`.
- The native coding runtime calls NVIDIA Nemotron through the user's own
  Nebius Token Factory API key. The daemon still owns tasks, local workspaces,
  Git, permissions, and execution. The runtime proposal, tool actions, and
  model content cross typed boundaries; raw secrets never enter them.
- ion is free and BYOK. There are no payments, subscriptions, entitlements,
  hosted inference credits, or paid feature gates.
- Local speech uses only **NVIDIA Nemotron 3.5 ASR Streaming 0.6B** through
  **NeMo-Speech.cpp**. Microphone audio stays local unless the user explicitly
  chooses a future remote workflow. Speech needs no cloud credential.
- Cua uses local permissions and approvals, not a separate user credential.
  Eve is excluded. NemoClaw is only a later optional, feature-flagged adapter.
- A guided personal Nebius gateway is a later capability. The user owns the
  Nebius account and infrastructure. Composio uses a scoped project key.
- Windows Credential Manager, macOS Keychain, and Linux Secret Service hold
  typed desktop secrets. If the vault is unavailable, storage is memory-only.
  Never persist a secret to settings.json, SQLite, logs, transcripts, or model
  context.

## Product and runtime

ion is a native control plane for AI work: projects, durable tasks, coding
runs, agent teams, connected apps, automations, artifacts, and approvals.
The local daemon remains the authority for project access and state. Nebius
provides model inference and, when explicitly selected, personal cloud
execution. A model proposes work; ion validates and executes authorized
actions. Mobile is a remote control surface, not an on-device coding runtime.

For this hackathon, the first usable path is one native Nemotron coding task
with streaming transcript, tool approvals, local workspace changes, and
resume. Later milestones add teams, connected apps, local speech, personal
gateway, and mobile steering in that order. Legacy provider code and history
remain intact for the Full profile after the event.

## Milestone approval workflow

1. The agent implements **only the current milestone** and preserves unrelated
   user changes. It uses the running development watcher, runs proportionate
   static/build checks, and does not add or run tests or perform visual testing
   for Milestone 1.
2. The agent reports exactly what changed, known limitations, credential and
   platform behavior, and a concise manual test checklist.
3. The agent stops. The user tests the freshly rebuilt app and explicitly
   approves or requests fixes.
4. Only after explicit approval may the agent start the next milestone.

## Chapters

1. [Product, brand, and user experience](01-product-brand-ux.md)
2. [Current state and gap analysis](02-current-state-gap-analysis.md)
3. [Target architecture](03-target-architecture.md)
4. [Feature plans](04-feature-plans.md)
5. [Agent teams and automations](05-agent-teams-automations.md)
6. [Mobile remote control](06-mobile-remote-control.md)
7. [Hackathon roadmap and milestone gates](07-hackathon-roadmap.md)
8. [BYOK, Nebius, and development resources](08-byok-nebius-development-resources.md)
9. [Native Nebius runtime and manual check](09-native-nebius-runtime.md)
10. [Workflows and GitHub review check](10-workflows-github.md)
11. [Local speech and personal gateway review check](11-speech-gateway.md)
12. [Mobile control and polish](12-mobile-polish.md)

[Research inventory](RESEARCH.md) and [NVIDIA/Nebius research](RESEARCH-NVIDIA.md)
are background source notes. They are not product decisions.
