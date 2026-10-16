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
deferred. Its mobile and submission review remains open. Milestone 6 adds an
optional, approval-gated NemoClaw execution adapter; its live sandbox review
also remains open.
Milestone 7 adds phone dictation to a selected desktop daemon's local ASR and
headless personal cloud task execution. Its live review remains open.

## Locked decisions

- **2026-10-16 current scope correction:** defer the not-started Amp/Cursor bounded Teams execution paths and Fx ordinary-task Teams invocation. Keep their research for later. Kimi ordinary-task Teams invocation has started and remains active, alongside document improvements and existing integration/platform/account/device acceptance. The [current harness ledger](64-remaining-harness-integrations.md) records these as deferred, not implemented. This supersedes the 2026-10-14 reopening for those three paths.

- **2026-10-14 current goal:** the user reopened remaining harness integrations alongside additional document formats/larger publications and fuller phone HTML/CSS support. This supersedes the adapter deferral below; [the current harness checklist](64-remaining-harness-integrations.md) records implementation and remaining native contracts. Owner-account/device review remains separate from source-level coverage.

- **2026-10-08 adapter scope correction:** adapters not implemented yet are
  deferred at the user's request. Amp/Cursor Teams completion, remaining normal-task
  Teams bridges (including Pi/Oh My Pi), and additional unsupported account/provider
  routes are outside the current implementation scope. Retain the twelve existing
  Teams adapters and the existing Nebius/Codex/Claude/OpenCode normal-task Teams
  connections. Deferred adapters do not block current completion and require a new
  user request to resume. Remote filesystem/commands/GPU work remains in the plan;
  existing adapter setup and live review remain separate acceptance tasks.

- Extend Waku directly. Customer-facing identity is **ion** (lowercase). Keep
  internal `waku-*` crates, storage paths, environment variables, database
  schema, and wire identifiers stable through the hackathon.
- The standard ion app exposes the **Full** harness catalogue, following the
  user's 2026-10-04 correction. An explicitly packaged `ION_DISTRIBUTION=nebius-only`
  profile remains available. The former `ION_DEV_FULL_PROVIDERS` escape is no
  longer needed. Preserve `disabled_providers` as the user's preference.
- The native coding runtime calls NVIDIA Nemotron through the user's own
  Nebius Token Factory API key. The daemon still owns tasks, local workspaces,
  Git, permissions, and execution. The runtime proposal, tool actions, and
  model content cross typed boundaries; raw secrets never enter them.
- ion is free and BYOK. There are no payments, subscriptions, entitlements,
  hosted inference credits, or paid feature gates.
- Speech uses **NVIDIA Nemotron 3.5 ASR Streaming 0.6B**. Without a cloud
  configuration, NeMo-Speech.cpp runs locally. The user can explicitly save a
  personal HTTPS service URL/token for direct desktop and phone uploads.
  Deployment is prepared; no Nebius VM/cluster target has been supplied.
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
gateway, and mobile steering in that order. The native app now exposes the full provider catalogue; existing provider code
and history remain intact.

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
13. [Optional NemoClaw execution adapter](13-nemoclaw-adapter.md)
14. [Phone dictation and personal cloud execution](14-phone-asr-cloud-execution.md)

## Proposals under review

- [Paperclip-inspired task and team experience](15-paperclip-inspired-ion-experience.md)
- [Cross-project workspace control plane](16-ion-workspace-control-plane.md):
  Dashboard, Decisions, Inbox, hierarchical Teams, Routines, prompt-generated
  Workflows, document Artifacts, and Usage.
  This later user brief supersedes chapter 15's compressed navigation proposal;
  neither proposal is implemented or approved as a milestone.
- [Agent teams implementation proposal](18-agent-teams-implementation.md):
  daemon-owned hierarchical delegation, distinct child tasks, enforced grants,
  mixed harnesses, and staged local/cloud execution. The user confirmed
  automatic delegation and a user-selected Local or Nebius sandbox environment
  per agent, with local fallback for sandbox failures. No Auto environment mode.
  Each agent also selects its own harness/model; teams may mix harnesses.
  These later Teams decisions supersede chapter 16's execution preferences.
- [ion workspace HTML prototype](PROTOTYPE-ion-workspace.html): interactive
  visual proposal for the Paperclip-style sidebar, hierarchical multi-team
  delegation, document Artifacts, and prompt-generated Workflows in ion's app
  shell, with a light Nebius-green glass theme and the full supported harness
  catalog alongside native Nebius execution.

[Research inventory](RESEARCH.md) and [NVIDIA/Nebius research](RESEARCH-NVIDIA.md)
are background source notes. They are not product decisions.

The [submission draft](SUBMISSION-DRAFT.md) holds the reviewer setup, demo
outline, and evidence fields for Milestone 5.

# Native workspace review

The native workspace changes and their remaining implementation tasks are
tracked in [17-native-workspace-review.md](17-native-workspace-review.md).
The Paperclip proposal and HTML prototypes are separate from verified native
behavior. Personal cloud speech deployment is prepared but awaits a Nebius
target; see [the deployment checklist](../../services/speech/README.md).

- [Agent teams implementation progress and review checklist](19-agent-teams-progress.md):
  durable coordination, native dispatch, parent review/rework and token budgets;
  implemented adapters, remote execution work and live acceptance; the current
  reopened harness scope is recorded in chapter 64.

- [Agent Teams release checklist](20-agent-teams-release-checklist.md): implemented
  coverage, remaining development and your live setup/review tasks in one place.

- [Workflow/routine execution and review](21-workflows-routines.md): durable
  sequential stages, assigned teams, human gates and restart-safe interval schedules.

- [Teams harness adapters and registered remote execution](22-harness-adapters-remote-execution.md)
  records exact coverage, native Nebius/Codex/Claude solo-to-team tools, host setup,
  lifecycle behavior and pending live review.

- [Remote commands and GPU jobs](23-remote-command-gpu-jobs.md): bounded command/file
  execution, recovery and worker setup; separate GPU lifecycle transport and
  durable bound-job waits/results, and remaining GPU launch, resource, recovery
  and monetary integration.

- [Inbox state and artifact integrity review](24-inbox-artifact-review.md): durable
  notification controls, desktop policy, original agent document evidence and
  remaining historical/mobile Inbox, preview and usage work.

- [Nebius GPU recovery contracts](RESEARCH-nebius-gpu-recovery.md): official
  operation/key witnesses, immutable SECRET input verification and the limits of
  missing historical VM/disk inventory.

- [Nebius GPU native price receipts](RESEARCH-nebius-gpu-price-receipt.md): pinned
  calculator schema, exact estimate/reservation arithmetic and atomic witnesses;
  unverified native Job support, billing-account binding and remaining finality.

- [Amp native Teams contract](RESEARCH-amp-teams-contract.md): actual native
  catalog/plugin fixtures, checksum provenance and deferred actor usage contracts.
- [Cursor native Teams contract](RESEARCH-cursor-teams-contract.md): released Local
  SDK/bridge options, account identity and deferred output/effects/usage contracts.
- [Linux harness worker release recipe](../../packages/harness-worker/README.md):
  reviewed tracked source, pinned bases, nonroot identity and actual-binary checks.

- [Headless registered-owner gateway controls](31-headless-gateway-owner.md):
  daemon workflow publication, private Linux service pairing, durable claims,
  exact-start recovery and outstanding deployment/device review.

For registered-owner paged Inbox/usage reads, phone read-grant setup and review,
see [gateway history](32-gateway-inbox-usage-history.md).

33. [Gateway workflow history and accepted documents](33-gateway-workflow-history-documents.md)

34. [Gateway Inbox notification controls and protocol-38 upgrade](34-gateway-inbox-controls.md)

## Current resource, document and speech review guides

- [Gateway PDF pages](49-gateway-pdf-pages.md): accepted workflow evidence, bounded derived image transfers, page/zoom and integrity review.
- [Speech deployment/device review](50-speech-deployment-device-review.md): installed runtime/model inspection, source-only package and private 46-case owner observations.
- [Command resource admission](51-command-resource-admission.md): owner/project holds across targets/runs and conservative proof requirements.
- [Physical device setup/review](52-physical-device-review.md): Moto G32/iPhone 15 native builds, bounded prerequisite inspection and actual-device checklist.
- [Command resource Usage](53-command-resource-usage.md): cached native diagnostics, project scope and keyboard review.

Live Nebius speech deployment and native phone acceptance remain open. The 2026-10-14 goal reopens remaining harness integrations; use the latest harness checklist for current readiness.

- [Actual local speech recognition](54-native-speech-recognition.md): installed CPU model, single/concurrent synthetic audio, bounded memory and remaining phone/cloud gates.
- [GPU runtime boundary research](RESEARCH-gpu-runtime-boundary.md): current kernel/device/mount/IPC contracts and the distinction between startup snapshots and confinement.
- [Command storage cleanup contract](RESEARCH-command-storage-cleanup.md): documented native lifecycle/storage limitations; no guessed deletion/release proof.

- [GPU runtime snapshot](55-gpu-runtime-snapshot.md): bounded real mount/device/image checks and root/child witness equality.
- [GPU file/device confinement](56-gpu-file-device-confinement.md): mandatory kernel Landlock, explicit driver metadata and inherited-handle limits.
- [Installed strict GPU worker](57-strict-gpu-worker-integration.md): integrated original approval/guard/receipt checks, 162 passing increment tests and actual CUDA/provider review checklist.

- [GPU sandbox resource limits](58-gpu-sandbox-resource-limits.md): owner shape-bound CPU/charged-memory/task ceilings, actual kernel verification and CUDA/runtime review.

- [GPU workspace storage limits](59-gpu-workspace-storage-limits.md): dedicated filesystem block/inode capacity, installed exhaustion/refusal evidence and actual disk/CUDA review.

- [Manual artifact retention](60-artifact-retention.md): explicit owner review/cleanup, original evidence, protected active work and durable retries.
- [GPU collector retention](61-gpu-collector-retention.md): original encoded receipt admission, intersecting scope limits and owner storage diagnostics.

- [First-launch setup](62-first-launch-onboarding.md): skippable desktop connection screens, resumable progress and a persistent setup indicator.

- [Phone rendered documents](63-phone-rendered-documents.md): authored inline HTML/CSS/SVG rendering, larger previews, additional text formats and verified larger binary/file publications.

- [Remaining harness integrations](64-remaining-harness-integrations.md): Pi/Oh My Pi native Teams bridge and current Amp/Cursor contract gaps.

- [Portable HTML resource bundles](65-portable-html-bundles.md) — pinned local CSS, images and fonts, generated snapshot receipts and owner review.

- [Amp source-task team bridge](66-amp-source-team-bridge.md): native request hook, owner-stream identity, temporary plugin lifetime and acceptance gates.

- [Phone Office reading](67-phone-office-reading.md): bounded DOCX/XLSX extracted content, original export and device review.

- [Windows ACP startup](68-windows-acp-startup.md): native executable launch, exact protocol workspace and account/platform review.

- [DeepSeek source-task team bridge](69-deepseek-source-team-bridge.md): additive native tools, owner-call approval/replay and full web-host acceptance gap.

- [Portable SVG image dependencies](70-portable-svg-images.md): pinned vector drawings, nested image/CSS/font resources and bounded phone preparation.

- [Pinned SVG reference documents](71-portable-svg-references.md): symbol/paint/path inlining, scoped IDs/styles/fonts and native-to-phone evidence.

- [OpenCode 2 native plugin contract](72-opencode2-native-plugin-contract.md): verified native 2.0.29 source-task Teams bridge, actual driver approval/Stop fixtures and owner acceptance checklist.

- [Portable document CSS names](73-portable-css-names.md): private literal animation/counter-style names, native-to-phone grammar checks and remaining variable/browser work.

- [Portable SVG CSS scopes](74-portable-css-scopes.md): private root/limit selectors, nested and implicit scopes, independent native-to-phone selector checks and remaining browser/device review.

- [Grok source-task native contract and hook helper](75-grok-native-source-contract.md): released hook/MCP identity evidence, bounded native helper and the remaining production bridge requirements.

- [Grok source-task Teams bridge](76-grok-source-team-bridge.md): released native approval/result/resume/stale-definition/Stop fixtures, bounded identity/argument controls and owner review.

- [Portable CSS layers and containers](77-portable-css-contexts.md): document-local layer/import/container names, style-query grammar, native-to-phone evidence and remaining dynamic/device review.

- [Antigravity source-task native contract](78-antigravity-native-source-contract.md): verified native metadata/stream identity, workspace-only discovery, resume/refusal fixtures and remaining production bridge work.

- [Antigravity source-task Teams bridge](79-antigravity-source-team-bridge.md): scoped additive plugin, native identity/shared worker, actual driver invocation/resume/Stop recovery and remaining owner review.

- [Portable CSS custom properties](80-portable-css-properties.md): private registrations/variable references, one-pass nested declaration traversal, actual native-to-phone evidence and remaining dynamic/browser/device work.

- [Larger phone documents](81-phone-large-documents.md): complete 16 MiB text/HTML preparation, bounded parser memory and serialization, native/phone evidence and current remaining work.

- [Current Cursor contract](82-cursor-current-contract.md): five real native ACP/MCP fixture cases, hook discovery limits and remaining production identity/allowance gates.

- [Current Kimi contract](83-kimi-current-contract.md): official source-host candidate, installed-plugin limits and remaining runtime/packaging proof.

- [Fx ordinary-task contract](84-fx-source-contract.md): native MCP identity gap, SDK-host distinction and remaining supported-host acceptance.

- [Portable font palettes and named features](85-portable-font-features.md): private imported document identities, native/phone grammar evidence and remaining browser/device review.

- [Cursor binding and native hook transport](86-cursor-binding-transport.md): exact-call state, the verified Windows hook template, native approval limits and remaining production wiring.

- [Cursor source-task Teams integration](87-cursor-source-team-bridge.md): verified Windows production wiring, owned native hook files, 50 passing cases, per-call approval and remaining platform/owner gates.

- [Portable Unicode font families](88-portable-unicode-font-families.md): pinned Unicode 18.0 C/F identities, all-scalar conformance, native-to-phone evidence and remaining dynamic/browser/device work.

- [Kimi source-host contract](89-kimi-source-host-contract.md): pinned released lifecycle, native tool activation, identity and packaging/login gates.

- [Native Kimi source-host fixture](90-kimi-native-source-host-fixture.md): real official engine/HTTP execution, permissions, built-in preservation, cancellation/resume and remaining production integration.

- [Dynamic CSS names contract](91-dynamic-css-names-contract.md): primary standards, variable inheritance, invalid values, computed identifiers and isolation limits.

- [Portable computed CSS names](92-portable-computed-css-names.md): bounded native identifier/fallback handling, 107 native-to-phone checks and remaining arbitrary substitution/browser/device work.

- [Kimi production source-task bridge](93-kimi-source-team-bridge.md): explicit host selection, additive native tools, shared worker authority, local source-host preparation and remaining release packaging/account acceptance.

- [Kimi compiled host](94-kimi-compiled-host.md): relocated self-contained Windows/x64 engine/runtime/assets/notices, full inventory checks, native Bash/Teams acceptance and remaining owner/release gates.
