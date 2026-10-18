# Workspace layout and Settings usage

Desktop build: **0.1.22**, continuing the changes on `ion-prod`.

## Corrections

- Workspace pages have one content heading. The native title bar keeps window controls and Finish setup without repeating the page name or showing an unrelated chat's change count.
- Decisions, Tasks, Projects, Teams, Routines, Workflows, Apps and Artifacts share the same content inset and heading alignment. View controls sit with actions; empty states use the full content area. Controls wrap in narrower windows.
- Teams/Runs moved into the Teams heading row. Routines and Workflows show a selected Definitions/Runs state.
- Neutral surfaces apply to workspace pages. The previous chat, New Task, sidebar and Settings palette is restored; this replaces the broader palette change in 0.1.21.
- Remove the standalone Usage page and sidebar entry. Settings → Usage opens **Ion activity**, showing recorded task/team usage for every harness, including Nebius, with project, harness and date filters. Rows retain reported/estimated and final/provisional cost labels; missing usage is unavailable.
- Keep the existing Claude/Codex transcript dashboard under **CLI history**. It is a separate source and is not added to Ion activity totals, avoiding duplicate charges or token counts. Other harnesses' work outside Ion is not imported by this change.
- Move GPU budgets, remote executions, billing and command resources into the Resources menu in Settings → Usage.
- Register the microphone SVG in the embedded asset table used by packaged builds. Dictation and onboarding use the same icon. Add a regression test for that registration.
- Use Ion in command-scope labels, computer-use presentation names, Codex's client title and desktop/daemon errors. Existing storage paths, application IDs and internal executable names remain compatibility identifiers.

## Owner review

- [ ] Install 0.1.22. Confirm the microphone is visible in the composer and Speech onboarding.
- [ ] Open all eight workspace pages at wide and narrow sizes. Confirm one content heading, aligned actions and centered empty states.
- [ ] Switch Teams/Runs and Definitions/Runs; verify the selected state and create/edit forms.
- [ ] Check New Task/chat and sidebar appearance against 0.1.20.
- [ ] Confirm Usage is absent from the sidebar and available under Settings.
- [ ] Run tasks with the installed harnesses, then check Settings → Usage → Ion activity. Filter by harness, project and date; check pagination and Open task.
- [ ] Compare CLI history separately; confirm its original daily/monthly/project views still work.
- [ ] Open each Resources view, refresh it and return to Usage.
- [ ] Check command scope, Computer Use settings and error messages for Ion branding. User project names and compatibility paths may still contain `waku`.

No owner data was removed and no visual acceptance was performed during development. Build/package checks and owner review are separate.
