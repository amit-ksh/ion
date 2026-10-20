# Chatbox file, team and app selection

## Shortcuts

| Typed prefix | Suggestions | Inserted tag |
| --- | --- | --- |
| `@` | Project files and folders, saved AI teams, connected apps | The selected category's tag |
| `@@` | Saved AI teams only | `@@{Team name}` |
| `@@@` | Active connected apps only | `@@@toolkit_slug` |

- Match the longest prefix first. App tags never start a team run.
- File selections insert `@path`; native selections use `@./path` when needed to avoid a legacy team-name collision.
- Team selections use a stable UUID for duplicate names or names containing braces/newlines. Braced names support spaces and Unicode.
- Mixed suggestions reserve space for teams and apps even in large checkouts. File matching stays memoized on desktop and batched/cancelable on phone.
- Start a fresh task with a team tag and objective to enter the existing durable Teams queue, with its saved lead, environment, access, delegation and budget limits.
- Within an existing solo chat, team tags reference the harness's existing team tools. Tags do not grant app permissions or change an agent's environment.
- Earlier `@teamname`, `@team:{name}` and `#team:{name}` recipients remain readable in stored tasks. New selection uses `@@`.
- App suggestions use active connected toolkit summaries from the Composio credential boundary. Credentials and account IDs never enter prompt text.
- Desktop and web retain arrows, Enter/Tab and Escape. Phone supports the same shortcut categories and direct + menu selections. Sheet presentation briefly waits for a completed prefix so typing `@@@` is possible.
- Bottom hint: `@ files, teams & apps · @@ AI teams · @@@ apps`.

## Owner deployment note

The owner reports Auth0 configured. Dashboard settings have not been independently verified. Live services still require Nebius VM instances, DNS/TLS and operator setup in document 111. No infrastructure was provisioned by this change.

## Review after updated builds

- [ ] Type `@`: verify files, AI teams and connected apps all appear; search by name.
- [ ] Type `@@`: verify only AI teams appear, including names with spaces.
- [ ] Type `@@@`: verify only connected apps appear and no team is invoked.
- [ ] Select each category mid-text; verify trailing draft text remains intact.
- [ ] Start a fresh task with a team and objective; verify the correct lead starts.
- [ ] Mention a team in an existing solo chat; verify the existing runtime stays active.
- [ ] Check duplicate and Unicode team names and legacy stored recipients.
- [ ] Check keyboard selection, Escape dismissal and bottom hints in both themes.
- [ ] Repeat on phone using typing and the + menu; switch daemon while loading.
- [ ] Disconnect an app and refresh suggestions; verify credentials never appear.
- [ ] Confirm app tagging respects the agent's existing permissions and approvals.

Feature, visual and device acceptance remain deferred at the owner's request. Compilation/type checks are recorded separately.

## Compile/type verification (2026-10-19)

- After the shortcut revision, `cargo check -p waku -p waku-daemon --offline` passed.
- Mobile, web and shared-client TypeScript checks passed.
- `git diff --check` passed.
- Feature, device and visual acceptance remain deferred; no updated installer/APK was built in this increment.
