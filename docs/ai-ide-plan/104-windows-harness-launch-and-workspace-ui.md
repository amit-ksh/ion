# Windows harness launch and workspace UI fixes

## Reproduced causes

- The failed Claude and Codex tasks referenced a project folder that no longer existed. Windows returned error 267 (invalid working directory), while the UI displayed only the outer launch error.
- PATH discovery also selected an unrelated `codex` Volta launcher reporting `0.2.3`. OpenAI's installed CLI reported `codex-cli 0.163.0-alpha.10.2` and supported the required app-server transport.

## Changes

- Validate the working folder on the execution worker before starting a local harness; report its path and recovery instructions. Never create an empty replacement project or silently execute in another folder.
- Preserve the underlying error through the daemon RPC response.
- Verify Codex's CLI identity and required `app-server --stdio` support with bounded local probes. Try other discovered launchers when a same-named program is incompatible, including the installed Windows Codex desktop CLI. Explicit binary overrides remain explicit and must pass the same checks.
- Replace the composer Dictate label with a microphone icon, stop icon while recording and a preparing icon while busy. Keep keyboard activation and state tooltips.
- Move Finish setup into the right side of the top bar, including Settings; remove the sidebar duplicate.
- Improve onboarding with grouped, full-height input fields, step icons, readable spacing and fewer competing actions on the last step.
- Hide Inbox from desktop navigation for now. Decisions and underlying task data remain available.
- Match the new workspace pages to the Harnesses surface: clear heading and description, grouped actions, inset list rows and compact usage controls. Keep virtualized lists and background data loading.
- Add Open folder to Projects so an existing workspace can be selected easily.

## Verification

- Original missing-folder probe reproduced Windows error 267 for both CLIs.
- Sixteen focused tests passed against the actual command environment and Codex discovery sources, including missing/file directories, PATH/Windows environment behavior and CLI identity.
- Live local startup check: compatible Codex discovery and app-server `initialize` succeeded; Claude stayed alive in streaming-input mode. No AI prompt, paid request or project edit was sent by this check.
- No visual test was performed, following the repository instructions. Installed-app UI acceptance remains for the owner.

## Owner review

- [ ] Install the updated 0.1.20 test build.
- [ ] Restore the missing project folder, or use Projects → Open folder to select an existing project. Create a new task there.
- [ ] Refresh Harnesses and confirm Codex resolves to an OpenAI Codex CLI, then send a small task through Claude and Codex using your existing accounts.
- [ ] Try an unavailable folder and confirm the error identifies the folder and recovery action.
- [ ] Verify the microphone control, recording stop action and keyboard activation.
- [ ] Open and skip onboarding; inspect field sizes, step navigation, credential save/test feedback and the right-side Finish setup indicator.
- [ ] Review Dashboard, Decisions, Projects, Teams, Routines, Workflows, Artifacts, Apps and Usage, including empty states and smaller windows. Inbox should be absent from navigation.
