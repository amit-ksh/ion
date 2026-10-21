# Sidebar and composer refinements

21 October 2026. Implements the source changes for UI-03, UI-05 and UI-06 from
plan 146 directly in `ion-prod`, without merging the separate redesign.

- **UI-03:** removes the repeated mention caption below the desktop and mobile
  composer. Empty inputs show the task prompt followed by `@` files/teams/apps,
  `@@` teams and `@@@` apps guidance. Existing draft content, completion handlers,
  connection/busy prompt text and submission behavior are retained.
- **UI-05:** removes Tasks and Projects from the Work navigation list. Their
  page implementations and task/project data remain; the project/chat hierarchy
  remains available. The Workspace group boundary moves with the shorter list.
- **UI-06:** uses the existing sidebar action-row primitive for bottom Settings:
  icon plus localized text, matching spacing and hover/focus treatment, filling
  the remaining footer width beside the updater. Enter and Space activate it.

## Verification

Mobile TypeScript passed. The four changed Rust files parsed successfully with
installed Rustfmt 1.97.1, edition 2024, without rewriting their formatting or
walking child modules. This is syntax evidence, **not Rust typechecking**.
No new tests mirror these low-impact presentation edits. No network, file I/O,
new timer or provider effect is added to a render path.

No native visual test, microphone use, APK build or separate-worktree edit was
performed. Light/Dark/System, narrow-window placeholder wrapping, keyboard and
freshly built native acceptance remain pending. The separate redesign source
must reconcile these changes during a later authorized integration.

Five source edits are staged independently from the QA overlay using preserved
committed/working inputs. Unrelated QA edits stay unstaged. Historical candidate
receipts are preserved; the prior MCP candidate now intentionally fails its
source guard because this milestone changes native source. Prepare a new
reviewed package snapshot after the remaining queued UI tasks, rather than
weakening that guard or shipping an old package as current.

UI-01 (artifact-only Deploy), UI-02 (device identity/status), and UI-04
(project ellipsis/menu icons) remain queued. This milestone does not complete
those tasks or the full release goal.
