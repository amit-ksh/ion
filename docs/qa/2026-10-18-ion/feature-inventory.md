# Implemented feature inventory

This inventory supplements the scenario/result checklist in [README.md](README.md). A source path establishes implementation, never acceptance. Scenario IDs refer to that checklist; its native, component and service evidence must be read separately. The local branch is `ion-prod`; owner changes from the baseline remain included.

## Setup, preferences and shell

| Implemented features | Owner / native source | Acceptance scenarios |
|---|---|---|
| Welcome, optional provider/app/speech/gateway steps, Skip, Open Ion, resume missing setup, Finish setup count | `src/app/onboarding.rs`, `settings.rs`, `crates/waku-core/src/control.rs` | S01–S07 |
| OS-vault credentials; save, replace, test, delete; endpoint validation and masked fields | `crates/waku-credentials/src`, `settings.rs` | S05–S07; owner keys are never removed for QA |
| Executable detection, per-harness path, auth/model discovery, enabled harnesses, model selection and refresh | `model_catalog.rs`, `slash_command_catalog.rs`, `settings.rs`, `composer.rs` | S08–S09, T02/T10–T12 |
| General settings, math rendering, anonymous usage preference | `settings.rs` | S10; privacy preference is observed, not changed |
| Appearance, theme, fonts, sizes, editor/terminal appearance; durable preferences | `settings.rs`, core `persistence.rs`, `theme.rs` | S10, R03 |
| Skills catalog/install paths and agent skill availability | `settings.rs`, `crates/waku-core/src/skills.rs` | T14; no arbitrary skill installation during testing |
| Daemon exposure/listening settings, connection identity, personal gateway and pairing configuration | `settings.rs`, `gateway_owner.rs`, `packages/personal-gateway` | S07, U06, P05 |
| Native window controls, navigation/back/forward, task switching, sidebar, settings search, focus and shortcuts | `window_chrome.rs`, `render.rs`, `src/lib.rs`, `src/ui` | S11, R01–R04 |
| Existing profile migration, SQLite history, update checks/installer handoff and release notes | `persistence.rs`, `src/updater*`, `resources/windows/waku.iss` | S01, R03, R05 |
| Ion visible branding, helper descriptions and platform privacy strings | `build.rs`, `resources/windows/helper-build.rs`, platform plists, mobile app configuration | R05 and branding tests; actual macOS/iOS permission dialogs need those platforms |

## Projects, tasks and existing coding surfaces

| Implemented features | Owner / native source | Acceptance scenarios |
|---|---|---|
| Open folder, project selection, projectless workspace, recent projects and project task lists | `sessions.rs`, `sidebar.rs`, `workspace_control.rs`, `projectless.rs` | T01, T08–T09 |
| New task, provider/model/effort/access/environment selection and persisted drafts | `composer.rs`, `sessions.rs`, core `persistence.rs` | T02, T11, T13, R03 |
| Message submission, streamed text/reasoning, ordered tools/results, provider errors, retry and cancel | `runtime.rs`, `streaming.rs`, `transcript_view.rs`, core `driver/*` | T02–T06, T10–T12 |
| Human command/file approvals, denial, questions/answers and cross-project Decisions routing | `components.rs`, `workspace_control/inbox.rs`, core `driver/*` | T03, T05, W05 |
| Steering/follow-ups, slash commands, attachments/images, goals and provider options | `composer.rs`, `driver/*`, protocol `model.rs` | T13; actual supported paths depend on the harness |
| Reopen history, load earlier transcript, native provider histories/imports, session forks/rewind/checkpoints | `sessions.rs`, `transcript_view.rs`, `*_session.rs`, `checkpoint.rs` | T06, T13, R03–R04 |
| Task title/search, find/replace, text selection/copy, links, tool expansion and source context | `transcript_search.rs`, `components.rs`, `src/md` | T13, R04 |
| Delete confirmation/cancel, running-task protection, remove project while retaining source folder | `sessions.rs`, daemon `RemoveSession`/`RemoveProject` | T04, T07–T09 |
| Files tree, text editor/save, Markdown preview, file changes, Git diff/unified view and source links | `right_panel.rs`, `git_commit.rs`, `workspace.rs` | T14; use disposable source only |
| Terminal sessions and browser pane/navigation/visibility; Computer Use provider integration | `src/terminal.rs`, `src/browser.rs`, core `computer_use.rs`, `driver/*` | T14, U06; no terminal commands through UI automation |

## Teams and agent execution

| Implemented features | Owner / native source | Acceptance scenarios |
|---|---|---|
| Agent name, expertise, instructions, harness/model, local/remote environment and pinned definitions | `workspace_control.rs`, `team_apps.rs`, core `control.rs`, `teams/*` | A01–A03 |
| Workspace read/write, shell/network and exact connected-app grants | `team_apps.rs`, core `teams/dispatch.rs`, `teams/apps.rs` | A01, W02; no extra grants to owner accounts |
| Multiple saved teams, lead/member/parent hierarchy, cycle/depth validation, team objective | `workspace_control.rs`, core `teams/storage.rs`, `team_runtime.rs` | A03–A04 |
| `@team:{name}` parsing/native composer and ordinary-agent delegation into a saved team | `composer.rs`, core `driver/*`, `native_team_hook.rs` | A04–A05 |
| Prepared/pinned run, child assignments, delegation, bounded parallelism and model/tool receipts | `team_activity.rs`, core `team_runtime.rs` | A04, A06–A08, U02 |
| Read/review/reject/rework child evidence, output contracts, blocker/result propagation | `team_activity.rs`, core `teams/dispatch.rs`, `team_runtime.rs` | A06; live acceptance and component rework tests are distinct |
| Proposal approvals/decline, decision revision binding, Stop/descendant cancellation | `team_activity.rs`, `team_recovery.rs`, core `teams/*` | A07, T05, W05 |
| Retry/recovery choices, uncertain file/app/remote effects, no blind replay | `team_recovery.rs`, core `teams/recovery.rs` | A07, W06; do not create real uncertain external effects for QA |
| Local/Nebius routing, configured local fallback, host registration and capability matching | core `teams/dispatch.rs`, `team_runtime.rs`, `teams/remote.rs` | A08; a model key alone is not a remote host |
| Work-item, depth, attempts, rework, model-call/token, timeout and parallel budgets | `team_limits.rs`, core `teams/*` | A06–A08; actual runs use explicit finite limits |
| Ordinary-task source identity and approved code handoff, change review and integration decisions | `team_integration.rs`, core `native_team_hook.rs`, `teams/workspace.rs` | A05, W06 |

Harness scope is defined by the current Teams harness release ledger. Twelve bounded completion adapters and thirteen ordinary-task bridges are implemented. **Amp/Cursor bounded completion and Fx ordinary-task delegation are unfinished and excluded.** Kimi additionally requires its prepared native host. Claude is excluded by the owner. An installed catalog row, localhost fixture, real account task and physical remote host test are four different evidence classes.

## Apps, workflows, decisions and routines

| Implemented features | Owner / native source | Acceptance scenarios |
|---|---|---|
| Composio project key, live catalog, connected accounts, auth handoff and disconnect/error states | core `teams/apps.rs`, `workspace_control.rs` | S06, W01; account authentication is owner-operated |
| Per-agent app/action/connected-account grants, exact action approval, receipts and uncertainty | `team_apps.rs`, core `teams/dispatch.rs`, `teams/apps.rs` | W01–W02, A07 |
| Manual and prompt-generated workflow definitions; project, stage instructions/team and required formats | `workflow_ui.rs`, `automation_ui.rs`, core `control.rs` | W03–W04 |
| Stage gates, exact Decisions approval, preceding artifact inputs, sequential continuation | `workflow_ui.rs`, core `teams/automation.rs`, `teams/*` | W05 |
| Branch routes/conditions, bounded retries, code workspace/handoff settings | `workflow_ui.rs`, core `teams/automation.rs` | W06 |
| Run history/status/stage progress, task/artifact links, failure/cancel and restart recovery | `automation_ui.rs`, core `teams/automation.rs` | W05–W07, A07, R03 |
| Routine definition/project/prompt/provider/team/workflow target, interval and persisted next fire | `routine_ui.rs`, `automation_ui.rs`, core `control.rs` | W07 |
| Pause/resume, exact scheduled firing identity, manual run, completed/error history and no duplicates | `routine_ui.rs`, core `teams/automation.rs` | W07 |
| Decisions project/task/type filters, team proposals, workflow stage gates and ordinary task approvals | `inbox.rs`, `team_activity.rs`, core control/Teams | T03, T05, W05 |

## Documents and artifacts

| Implemented features | Owner / native source | Acceptance scenarios |
|---|---|---|
| Agent text originals: HTML, MD, TXT, CSV, JSON, SVG, XML, YAML | core `teams/documents.rs`, `waku-document` | D01–D02 |
| Inspect exact file evidence and publish original PDF/Office/image/document bytes with digest/provenance | core `teams/documents.rs`, `control.rs`, `waku-document` | D01–D02, D06 |
| Thirteen native publication exporter fixtures and shared phone readers | `waku-document`, `apps/mobile`, shared client | D02, D06; fixture success is not physical phone acceptance |
| Artifact cards, search, project/task/format filters, pagination/virtualized grid, task/project links | `artifacts_ui.rs` | D03, R01–R02 |
| Verified HTML Reading/Source views, private bounded resources, no authored external navigation | `document_preview.rs`, `src/document_html.rs`, `src/browser.rs` | D04a–b, D06 |
| PDF page/zoom, original text/source extraction, Unicode/large bounds, save/export exact originals | `document_preview.rs`, `waku-document` | D04c, D06 |
| Retain by default, explicit expiry/cleanup, active task/evidence protection and digest mismatch rejection | core `control/artifact_retention.rs`, `control.rs` | D05; owner cleanup is prohibited |

## Usage, billing, resources and remote execution

| Implemented features | Owner / native source | Acceptance scenarios |
|---|---|---|
| Settings-only Ion activity ledger, every implemented harness receipt, project/harness/time filters and cursor pages | `usage_ui.rs`, core `usage_ledger.rs` | U01–U03 |
| Reported/estimated/missing input/output/cost, finality, idempotent update and duplicate prevention | core `usage_ledger.rs`, provider adapters | U03 |
| Separate existing CLI daily/monthly/project history, raw-price estimates and quality/source labels | `usage_page.rs`, core `usage_history.rs` | U04; historical Claude records may be read, no Claude execution |
| Billing connection identity/IAM, report/export discovery, FOCUS snapshots, decimal cost allocation and refresh | `billing_ui.rs`, core `cloud_billing*` | U05; needs pre-existing read grants/export |
| Scope budgets, forecast/observed status, enforcement settings and resource associations | `budget_ui.rs`, core cloud budget/resource modules | U05; do not modify production policies |
| Command/GPU resource listing, collector configuration and receipts | `resource_ui.rs`, `collector_ui.rs`, core resource/collector modules | U06; configured hosts/export endpoint required |
| Remote target/run statuses, output/result, refresh, cancel/recovery and no resubmission on refresh | `remote_ui.rs`, core remote execution modules | U06, A08 |
| Gateway pairing/grants/reconnect, remote tasks/decisions/workflows/history/artifacts/billing controls | `packages/personal-gateway`, `apps/mobile`, core `gateway_owner.rs` | U06, P05; exact paired endpoint/device required |

## Speech, mobile and reliability

| Implemented features | Owner / native source | Acceptance scenarios |
|---|---|---|
| Visible keyboard-operable mic, preparation/recording/Stop/Cancel/error, live preview and draft ownership | `composer.rs`, `settings.rs`, `src/speech.rs` | P01–P02 |
| Cached/local NeMo-Speech runtime/model discovery, WAV transcription, language Auto/English/Hindi and insertion | `src/speech.rs`, `settings.rs` | P03 |
| Owner-configured hosted speech URL/token, authenticated realtime transport, finalization and cancellation | `waku-credentials/src/speech.rs`, shared client, speech service | P04; do not invent an endpoint or provision one |
| Phone microphone/dictation, exact speech service, paired native remote controls and device layout | `apps/mobile`, gateway/shared speech | P05; no connected phone is available |
| Shared native page alignment, narrow/wide resizing, long names, scrolling, keyboard and status text | `workspace_control*`, `src/ui`, `window_chrome.rs` | R01–R02 |
| Empty/loading/populated/error/disconnected states, readable theme/icon contrast and hidden Dashboard/Inbox | native shell/workspace/settings | R01–R02, U01 |
| Restart persistence, dropped-daemon reconnect without replaying effects, single dev watcher and update recovery | core persistence/client, `scripts/dev*.ts`, updater | R03, R05 |
| Long transcript virtualization, cached background data, bounded streaming/pulse cadence and reduce-motion behavior | `transcript_view.rs`, `runtime.rs`, `streaming.rs`, `src/ui/motion.rs`; `docs/performance.md` | R04; responsiveness observations do not establish a measured high-refresh frame budget |

## Deliberate exclusions

No unfinished adapter is executed merely because it appears in a catalog. Dashboard/Inbox remain hidden. No video was recorded. No owner credential, project, chat, source document or production resource is deleted. No real app message, publication, deployment or paid infrastructure provisioning is performed. Missing dependencies and unexecuted native variations remain explicit in the scenario report.
