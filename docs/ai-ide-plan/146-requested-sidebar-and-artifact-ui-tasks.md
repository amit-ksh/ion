# Requested sidebar, device and artifact UI changes

21 October 2026. User-requested additions to the active implementation/testing
queue. UI-03, UI-05 and UI-06 are now implemented in source in
[151](151-sidebar-and-composer-refinement.md), with native acceptance pending.
UI-01 and UI-04 are implemented in source in
[152](152-artifact-deploy-and-project-menu.md), with native acceptance pending.
UI-02 is implemented in source in [153](153-authenticated-device-presence.md).
All six now await their applicable native acceptance; source implementation is
not an installed-package or visual pass. Reference: the supplied project popover screenshot.

23 October follow-up: UI-07/UI-08 and the clarified UI-03 placeholder behavior
are specified in
[177](177-starter-options-app-icons-and-composer-placeholder.md). Those additions
are queued; earlier source passes do not establish their implementation.

| ID | Requested change | Acceptance |
| --- | --- | --- |
| UI-01 | Remove deployment from Projects UI; show Deploy on Artifacts | Project menus/pages no longer offer deployment. Eligible HTML/static-app artifacts expose Deploy and open the existing reviewed static-publishing flow with the correct artifact/account. Keep the existing static-only scope, approvals, ownership and budget controls. Do not add backend runtime deployment or a deployment action to unsupported document types. |
| UI-02 | Improve the Android connected indicator and identify the device when possible | Present a readable device icon/status and verified device name/model where the authenticated connection provides one. Use an accurate Android fallback when identity is unavailable. Disconnect/reconnect updates correctly; do not invent a model or expose pairing tokens. |
| UI-03 | Move mention guidance into the task textbox placeholder | Include Do anything… and concise `@` files/teams/apps, `@@` teams and `@@@` apps guidance in the actual empty-input placeholder. Hide all of it on typing or draft restoration; clearing restores it. Remove the separate footer hint. Preserve mention filtering/selection and draft text. Refined acceptance is in plan 177. |
| UI-04 | Restore the visible vertical three-dot project action icon; add left icons to Edit/Delete | The menu trigger visibly renders a vertical ellipsis with an accessible label and adequate hit area. Edit and Delete show their respective icons left of text. Keyboard focus, Enter/Escape, outside dismissal, rename persistence and delete confirmation/source preservation still work. |
| UI-05 | Remove Tasks and Projects sidebar entries from Work | Work no longer has those two navigation entries. Retain the separate project/chat hierarchy and existing data; this is navigation simplification, not deletion of tasks/projects or removal of their functionality. |
| UI-06 | Make the bottom Settings entry match the other sidebar buttons | Show a settings icon plus the text Settings, with matching alignment, spacing, hit area, focus and hover treatment. Keep it anchored at the bottom and usable in narrow windows. |
| UI-07 | Starter options on Teams, Workflows and Routines | Icon-led cards open the correct editable creation draft, with consistent grid alignment and a custom path. Workflow cards prefill dynamic-generation prompts; no automatic run or access/spending change. See plan 177. |
| UI-08 | Recognizable app logos beside app names/actions | Use actual supported app identities with consistent icon-led rows, readable labels and connection states, validated/cached images and a stable missing-icon fallback. See plan 177. |
| UI-09 | Graph-based hierarchical teams | Connected agent nodes, visible harness/environment, node selection, direct reports and editable reporting lines; preserve members and reject cycles. Source in progress; see plan 178. |
| UI-10 | Clean action-led onboarding and personal Nebius setup | Source in progress: compact steps, optional own key/project setup, secure fields and a scoped read-only access check; preserve Ion login, Skip and Finish setup. Compilation and native/live acceptance remain open. See plan 178. |

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
