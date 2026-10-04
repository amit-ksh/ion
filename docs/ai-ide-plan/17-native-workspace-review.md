# Native ion workspace: implementation and review

Updated 2026-10-04. This is the status of the **native app**, separate from the
earlier HTML prototypes. The wider architecture in chapter 16 is still a
partially implemented proposal.

## Implemented in the native app

| Area | Available behavior |
| --- | --- |
| Navigation | Dashboard, Inbox, Decisions, Tasks, Projects, Teams, Routines, Workflows, Artifacts, Apps, Harnesses, Usage. Existing task transcript, composer, files, browser, and settings remain connected. |
| Dashboard | Actual project, active task, agent profile, and pending decision counts; opens real tasks. |
| Decisions | Tasks waiting for an existing provider permission or user question. Review opens that task's actual interaction. |
| Inbox | A task update view. Dedicated unread/acknowledgement semantics are not implemented yet. |
| Teams | Create/edit multiple teams, reusable agent profiles, expertise, instructions, harness/model selection, and reporting parents. Exactly one lead; cycle/missing-parent validation in the daemon. New task prepares the team's instructions for review. |
| Workflows | Project-specific, editable stages generated from a prompt using the Nebius key, or authored manually. Stage decision flags persist. New task prepares the ordered instructions. |
| Routines | Persist/edit recurring definitions and prepare a task manually. Definitions stay paused; automatic schedule execution is not implemented yet. |
| Artifacts | Import and export actual HTML, PDF, Markdown, text, Word, CSV, and JSON documents. Bytes live separately from temporary image blobs. Limit 16 MB per document. |
| Apps | Personal gateway connected accounts and existing connection/effect approval settings. Requires a configured personal gateway to show accounts. |
| Harnesses | All 14 supported provider entries, installation status, configuration, and existing model discovery. Missing CLIs remain visible and lead to setup. Antigravity's documented `agy` stream adapter is added. |
| Usage | Existing model/harness usage history view. Unreported/provider subscription costs are not fabricated. |
| Identity | ion app titles, localized copy, green palette/gradient, and new Windows/macOS/mobile icon assets. Installer output names and release asset paths align with ion. |
| Speech | Optional authenticated personal HTTPS ASR endpoint, OS vault/SecureStore storage, direct desktop/phone uploads, connection test, and local fallback when no cloud service is configured. |

Definitions are stored in daemon-owned `ion-control.db` next to the task DB.
Optimistic revision checks reject stale writes. Document payloads live in
`ion-documents`. Existing task data paths, bundle IDs, executable names, and
wire compatibility strings remain where migration would otherwise lose
access to existing tasks or break integrations. A project folder named `waku`
still displays its actual folder name; that is not an application title.

## Development still remaining

These are **implementation tasks**, not merely user testing:

- [ ] Pin profile/team versions and enforce app/resource grants at execution.
- [ ] Connect the saved hierarchy to actual child run dispatch, outputs,
  concurrency limits, and local/owner-cloud routing. The existing Nebius
  researcher/reviewer delegate tools work independently; supplying a team
  prompt does not implement arbitrary cross-harness delegation.
- [ ] Add durable workflow runs, dependencies, retries, app bindings, and
  execution-enforced decision gates. Saved gate text is an instruction;
  existing provider/tool permission checks remain authoritative today.
- [ ] Add daemon-owned routine scheduler, durable firing IDs, overlap policy,
  restart/misfire behavior, and normal task/run creation.
- [ ] Add persistent unread/acknowledgement inbox items and portfolio projection.
- [ ] Associate produced documents automatically with run/stage provenance and
  hashes; add safe document preview. The current collection imports/exports.
- [ ] Add per-agent budgets and detailed cost attribution across team runs.
- [ ] Complete Antigravity's native permission/question bridge and integration
  checks with an installed, authenticated `agy` version. The current adapter
  uses the CLI's default review policy unless the user chooses Full access;
  rollback/fork/session import are unsupported.
- [ ] Add remote speech partial results while recording. Current cloud
  dictation uploads after Stop; the selected model is a streaming ASR model.

## Run the current app

From PowerShell in `D:\personal\apps\waku`:

```powershell
bun ./scripts/dev.ts
```

Use one watcher. If it is already running, leave it running; it rebuilds and
relaunches the native debug app. Do not launch an older installed app to check
source changes. The process binary is still `waku.exe`; its app title is
`ion Debug`. Unset `ION_DISTRIBUTION` for the full provider catalogue; the
explicit `nebius-only` profile intentionally limits a packaged distribution.

Provider authentication belongs to each harness's CLI. Install/login to the
harnesses you want to run, then refresh Harnesses. Seeing an entry is not proof
its CLI is installed or authenticated. Nebius uses your key in Models & Speech.

Protocol version is now 9. Desktop, daemon, web, and phone should be built
from this revision together; old clients need updating.

## Review without a Nebius key

- [ ] Confirm the title/icon is ion and the new sidebar destinations appear.
- [ ] Open Harnesses: Claude, Codex, Antigravity, OpenCode, Nebius, and all
  other provider entries appear with truthful installation status.
- [ ] Open the model picker. Missing CLI entries should lead to setup;
  installed/enabled harnesses should expose their discovered models.
- [ ] Create three agents with different instructions, then two teams.
  Choose a lead and reporting parents. Save; restart; confirm persistence.
- [ ] Try a hierarchy cycle and a parent outside the team; saving must fail.
- [ ] Prepare a team task in an existing project. Confirm the chosen harness
  and instructions are editable before sending it.
- [ ] Create a workflow manually. Edit/add/remove stages and decision flags;
  save and reopen. Confirm it stays tied to the chosen project.
- [ ] Create/edit a routine. Confirm it is paused and no unattended run starts.
- [ ] Import an HTML/PDF document, restart, export it, and compare bytes.
  Unsupported formats and files above 16 MB must be rejected.
- [ ] Inspect actual waiting tasks under Decisions and updates under Inbox.
- [ ] Check light/dark text contrast, keyboard access, long task lists,
  scrolling, and a small window. No visual test was performed by the agent.

## Review when resources are available

- [ ] Nebius key: save/test in Harnesses; verify the real model catalogue,
  run a short task, and generate/edit a workflow draft from a prompt.
- [ ] Installed/authenticated CLIs: run a short task in each selected harness.
- [ ] Personal gateway: pair desktop/phone, confirm Apps accounts, and review
  an actual external effect before accepting it.
- [ ] Speech: provide a Nebius VM/SSH target and DNS hostname, then follow the
  [speech deployment checklist](../../services/speech/README.md). No deployment
  target was supplied and no cloud resource was provisioned.
- [ ] Configure the same speech service on desktop and phone; test short
  audio, silence, cancellation, task switching, timeout, and service failure.
- [ ] Confirm cloud dictation works without a local NeMo installation. Delete
  cloud configuration and confirm the existing local mode still works.

## Verification performed

- Native app and daemon `cargo check` passed; the dev watcher built both
  binaries and launched the freshly rebuilt debug app.
- Provider distribution regression reproduced before the fix, then passed.
- Team/store tests passed for hierarchy cycles, persistence, stale revisions,
  document byte round trips, and failed-import cleanup.
- Speech tests passed for HTTPS URL validation, authenticated WAV request,
  transcript parsing, and redirect refusal.
- Phone and shared client TypeScript checks passed, including the Expo HTTP
  transport. Generated protocol bindings passed their consistency check.
- Live CLI authentication, provider calls, cloud deployment, mobile build,
  installer execution, and visual review remain unverified.

Antigravity contract: [official headless CLI documentation](https://www.antigravity.google/docs/cli/headless/).
Speech contract: [NVIDIA NeMo-Speech.cpp HTTP API](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/main/docs/api.md).
