# Requested sidebar, device and artifact UI changes

21 October 2026. User-requested additions to the active implementation/testing
queue. UI-03, UI-05 and UI-06 are now implemented in source in
[151](151-sidebar-and-composer-refinement.md), with native acceptance pending.
UI-01 and UI-04 are implemented in source in
[152](152-artifact-deploy-and-project-menu.md), with native acceptance pending.
UI-02 remains queued. Reference: the supplied project popover screenshot.

| ID | Requested change | Acceptance |
| --- | --- | --- |
| UI-01 | Remove deployment from Projects UI; show Deploy on Artifacts | Project menus/pages no longer offer deployment. Eligible HTML/static-app artifacts expose Deploy and open the existing reviewed static-publishing flow with the correct artifact/account. Keep the existing static-only scope, approvals, ownership and budget controls. Do not add backend runtime deployment or a deployment action to unsupported document types. |
| UI-02 | Improve the Android connected indicator and identify the device when possible | Present a readable device icon/status and verified device name/model where the authenticated connection provides one. Use an accurate Android fallback when identity is unavailable. Disconnect/reconnect updates correctly; do not invent a model or expose pairing tokens. |
| UI-03 | Move mention guidance into the task textbox placeholder | Remove the separate `@` information text below the task composer. Put concise `@` files/teams/apps, `@@` teams and `@@@` apps guidance in its empty-state placeholder. Preserve mention filtering/selection and draft text. No duplicate footer hint. |
| UI-04 | Restore the visible vertical three-dot project action icon; add left icons to Edit/Delete | The menu trigger visibly renders a vertical ellipsis with an accessible label and adequate hit area. Edit and Delete show their respective icons left of text. Keyboard focus, Enter/Escape, outside dismissal, rename persistence and delete confirmation/source preservation still work. |
| UI-05 | Remove Tasks and Projects sidebar entries from Work | Work no longer has those two navigation entries. Retain the separate project/chat hierarchy and existing data; this is navigation simplification, not deletion of tasks/projects or removal of their functionality. |
| UI-06 | Make the bottom Settings entry match the other sidebar buttons | Show a settings icon plus the text Settings, with matching alignment, spacing, hit area, focus and hover treatment. Keep it anchored at the bottom and usable in narrow windows. |

## Integration with existing tasks

- UI-02/UI-04/UI-05/UI-06 extend RD-01's header/project/sidebar review in
  [142](142-redesign-remaining-acceptance.md); they do not duplicate or erase its
  earlier passed build/persistence receipts.
- UI-03 extends the existing mention/composer checks in
  [132](132-physical-device-acceptance.md).
- UI-01 extends static publishing/native interaction acceptance in plans 120/132.
- Validate affected controls in Light/Dark/System, keyboard and narrow-window
  states when native visual testing is authorized. Existing no-visual-test,
  separate-worktree, package-build and physical-device limits still apply.
- This request adds tasks only. It does not automatically merge the redesign,
  run another APK build or change the deployment backend.
