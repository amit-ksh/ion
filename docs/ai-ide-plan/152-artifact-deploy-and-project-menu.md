# Artifact deployment and project actions

21 October 2026. Source implementation for UI-01 and UI-04 from plan 146.

## UI-01: deploy from Artifacts

- Projects no longer exposes a Deploy row action or the Deployments header
  action. The deployment library action is on Artifacts and opens that page.
- Eligible HTML artifact cards display **Deploy** with a cloud-upload icon.
  They use the existing account-bound artifact manifest/review/publishing flow.
- Eligibility remains HTML with captured evidence. PDF and other unsupported
  document types gain no deployment action. The existing static resource bundle
  may include its reviewed HTML assets; no backend runtime or cloud build is added.
- Project static-export internals remain for compatibility; this changes the
  requested UI entry points, not the hosting backend or old publication records.

## UI-04: project menu

- The visible project action trigger uses an embedded vertical-ellipsis SVG
  with explicit opaque fill, rendered through Ion's tinted alpha-mask primitive.
- The popover keeps project details and offers Edit project name and Delete
  project with pencil/trash icons to the left of their text.
- Reuses the reviewed `81827ff` project-menu/rename implementation selectively,
  without merging the redesign. Enter/outside click commits a nonempty bounded
  name; Escape cancels through the existing rename key context. Renaming also
  invalidates cached artifact labels. Switching to chat rename clears project
  rename state. Shared menu keyboard focus and outside dismissal are retained.
- Delete uses the existing confirmation, active-work checks and daemon removal;
  source folders/files are preserved. No new direct filesystem deletion exists.

## Evidence and limits

Five changed native files parsed successfully with installed Rustfmt 1.97.1,
edition 2024, without rewriting files or traversing child modules. This does not
prove Rust typechecking or native behavior. The SVG is a source asset; its
visibility still needs a freshly compiled native check in both themes.

No visual test, device interaction, provider operation, APK build or redesign
merge occurred. Existing build memory guards remain. Prior redesign persistence
receipts are design evidence, not acceptance of this separate source port.
Native build, project rename/delete, menu keyboard/focus, eligible static bundle
review and actual account-bound publishing interaction remain in plans 120/132.

Source edits are staged independently of the preserved QA overlay. UI-02
(Android connected status and device identity) is the remaining queued source
task from plan 146; all six still need the applicable native acceptance.
