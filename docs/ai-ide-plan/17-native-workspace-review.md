# Native ion workspace: implementation and review

Updated 2026-10-04. This is the status of the **native app**, separate from the
earlier HTML prototypes. The wider architecture in chapter 16 is still a
partially implemented proposal.

## Implemented in the native app

| Area | Available behavior |
| --- | --- |
| Navigation | Search directly below New Task; left-aligned, grouped navigation; a scrolling navigation section for short windows; keyboard-operable icon refresh. Existing task transcript, composer, files, browser, and settings remain connected. |
| Dashboard | Actual project, active task, agent profile, and waiting task counts; opens real tasks. Embedded ion and Dashboard icons are registered. |
| Decisions | Pending permissions, questions, computer approvals, proposed work, and issues across all projects. Review opens the task's actual interaction. Nebius can request a blocking decision. |
| Inbox | All projects/tasks by default, with project, task, type, and status filters. Shows live approval/question/issue/completion updates, generated documents, and durable proposed tasks with Accept/Decline. Accept prepares an editable task draft. Unread/acknowledgement history is not implemented yet. |
| Teams | Create/edit multiple teams, reusable agent profiles, expertise, instructions, harness/model selection, and reporting parents. Exactly one lead; cycle/missing-parent validation in the daemon. New task prepares the team's instructions for review. |
| Workflows | Project-specific, editable stages generated from a prompt using the Nebius key, or authored manually. Stage decision flags persist. New task prepares the ordered instructions. |
| Routines | Persist/edit recurring definitions and prepare a task manually. Definitions stay paused; automatic schedule execution is not implemented yet. |
| Artifacts | Agents add documents: settled successful edit activities collect HTML, PDF, Markdown, text, Word, and CSV from the project/worktree; Nebius can also publish HTML, Markdown, text, CSV, and JSON directly. Export stored bytes. No user upload control. Limit 16 MB per document. |
| Apps | Searchable Composio catalog, All apps/Connected tabs, Connect through hosted sign-in, and app settings. Uses the Composio project key from the OS vault. Custom authentication opens Composio configuration. Account connections do not automatically grant agent execution rights. |
| Token Factory sandbox | Settings toggle, off by default. Enables the Nebius `tokenfactory_exec` tool through an authenticated ConTree CLI session. Every command requires approval; the live switch is checked again before execution. Local files are not uploaded automatically. |
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
  execution-enforced stage gates. Nebius's explicit decision tool blocks for
  approval; saved workflow gate flags still rely on the agent invoking it.
- [ ] Add daemon-owned routine scheduler, durable firing IDs, overlap policy,
  restart/misfire behavior, and normal task/run creation.
- [ ] Add persistent unread/acknowledgement history and a daemon-owned portfolio
  projection. Current live notices cover the selected daemon's tasks; proposals
  and documents persist, while past approval/question events are not an archive.
- [ ] Extend automatic task/project document associations with run/stage
  provenance and hashes; add safe document preview.
- [ ] Add sandbox session selection, workspace sync, durable remote operation
  IDs/cancellation, and routing for other harnesses. The current adapter runs
  bounded, approved commands in the named `ion` ConTree session.
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

Protocol version is now 10. Desktop, daemon, web, and phone should be built
from this revision together; old clients need updating.

## Review without a Nebius key

- [ ] Confirm the title/icon is ion, Search is directly below New Task, sidebar
  labels/icons align to the left, and the Dashboard ion icon is visible.
- [ ] Tab to the refresh icon and activate it with Enter/Space. Check navigation
  remains reachable in a short window and existing task history still scrolls.
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
- [ ] Open Inbox and Decisions: default filters are All projects and All tasks.
  Change project, task, type, and Inbox status filters, then reopen the page.
- [ ] Inspect actual waiting tasks under Decisions; Review must open the exact
  task's permission, question, or computer approval. Failed tasks show an issue.
- [ ] Use an authenticated harness to create a document in the project. When the
  turn settles, refresh Artifacts/Inbox, export the document, and compare bytes.
  Existing unrelated files and files outside the project must not be collected.
  Artifact collection requires the harness to report successful file changes.
- [ ] Confirm Artifacts offers no Add document/upload action. Existing stored
  documents remain exportable after restart.
- [ ] Toggle Token Factory sandbox On/Off in Settings and restart; confirm the
  preference persists. Off must not advertise the sandbox tool to Nebius.
- [ ] Check light/dark text contrast, keyboard access, long task lists,
  scrolling, and a small window. No visual test was performed by the agent.

## Review when resources are available

- [ ] Nebius key: save/test in Harnesses; verify the real model catalogue,
  run a short task, and generate/edit a workflow draft from a prompt.
- [ ] Ask Nebius to propose a follow-up task for Inbox. Refresh Inbox; Accept
  must save acceptance and prepare an editable draft without running it.
  Decline must persist after restart. A stale second client must not overwrite
  the first client's choice.
- [ ] Ask Nebius to save an HTML report directly to Artifacts and request a
  decision before continuing. Check both project filters and the actual
  Approve/Decline interaction in the task.
- [ ] Installed/authenticated CLIs: run a short task in each selected harness.
- [ ] Composio: save a scoped project key in Settings → Models & Speech. Open
  Apps, search for an app, choose Connect, complete hosted sign-in, refresh,
  and confirm the Connected tab. Check expired/failed accounts and invalid-key
  errors. Deleting/replacing the key must clear the old catalog/account cache.
- [ ] Personal gateway: pair desktop/phone, configure exact account/tool grants
  in the gateway, and review an actual external effect before accepting it.
  Connecting an account in Apps alone must not authorize its external effects.
- [ ] Speech: provide a Nebius VM/SSH target and DNS hostname, then follow the
  [speech deployment checklist](../../services/speech/README.md). No deployment
  target was supplied and no cloud resource was provisioned.
- [ ] Configure the same speech service on desktop and phone; test short
  audio, silence, cancellation, task switching, timeout, and service failure.
- [ ] Confirm cloud dictation works without a local NeMo installation. Delete
  cloud configuration and confirm the existing local mode still works.

### Token Factory sandbox setup and review

Requires Python 3.10+, a ConTree project with sandbox access, and its token and
project ID. In PowerShell:

```powershell
uv tool install contree-cli
contree --help
contree auth
contree images --prefix=ubuntu
```

Enter credentials only in the secure authentication prompt. Select an image
available to your project, then initialize ion's named session, for example:

```powershell
contree -S ion use tag:ubuntu:latest
```

These installation, authentication, and session commands follow Nebius's
[installation guide](https://docs.tokenfactory.nebius.com/sandboxes/cli/tutorial/installation)
and [first sandbox guide](https://docs.tokenfactory.nebius.com/sandboxes/cli/tutorial/first-steps).
An inference key working in ion does not establish sandbox entitlement.

- [ ] Make sure `contree` is on the daemon's PATH; enable Token Factory sandbox
  in Settings → Models & Speech, then start a Nebius task.
- [ ] Ask it to use `tokenfactory_exec` with `argv: ["uname", "-a"]`. Review
  the session and exact arguments before approving. Confirm remote output.
- [ ] Decline another command; no process should launch. Turn the switch Off
  while a command is awaiting approval; approving must then be refused.
- [ ] Cancel a running command or interrupt the daemon. Inspect the remote
  ConTree session before continuing. ion stops the local CLI and treats the
  remote outcome as uncertain; it does not automatically replay the command
  or claim that the remote operation has been canceled.

Composio contracts: [toolkit catalog](https://docs.composio.dev/reference/api-reference/toolkits/getToolkits),
[auth configurations](https://docs.composio.dev/reference/api-reference/auth-configs/postAuthConfigs),
and [hosted sign-in](https://next.docs.composio.dev/reference/api-reference/connected-accounts/postConnectedAccountsLink).

## Verification performed

- Native app and daemon `cargo check` passed; the dev watcher built both
  binaries and launched the freshly rebuilt debug app.
- Provider distribution regression reproduced before the fix, then passed.
- Team/store tests passed for hierarchy cycles, persistence, stale revisions,
  document byte round trips, failed-import cleanup, proposal deduplication,
  durable acceptance, and automatic document capture with unrelated/outside
  files excluded. Capture was also checked with existing workspace records.
- Sidebar history scroll regression passed. Sandbox default/live-switch and
  hosted Composio HTTPS sign-in URL validation tests passed.
- Speech tests passed for HTTPS URL validation, authenticated WAV request,
  transcript parsing, and redirect refusal.
- Phone and shared client TypeScript checks passed, including the Expo HTTP
  transport. Generated protocol bindings passed their consistency check.
- Live CLI authentication, provider calls, cloud deployment, mobile build,
  installer execution, and visual review remain unverified.

Antigravity contract: [official headless CLI documentation](https://www.antigravity.google/docs/cli/headless/).
Speech contract: [NVIDIA NeMo-Speech.cpp HTTP API](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/main/docs/api.md).
