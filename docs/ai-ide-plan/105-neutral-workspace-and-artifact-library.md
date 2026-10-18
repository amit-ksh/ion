# Neutral workspace and artifact library

Desktop test build: **0.1.21**, based on `ion-prod` with the local changes recorded in its build manifest.

## Changes

- Hide Dashboard and Inbox in desktop navigation. Start in the existing New Task/chat surface; group Decisions with Tasks, Projects, Teams, Routines, Workflows and Artifacts under Work.
- Use neutral graphite surfaces in dark mode and neutral paper surfaces in light mode. Keep the green brand accent for icons and focus. Remove the full-page gradient and oversized tinted container from workspace pages.
- Align page headings, filters and lists on a shared 28 px inset, with a bounded content width. Apply the same inset to document and resource subpages. Definition forms take the available width in narrow windows.
- Add visible Remove actions to sidebar projects/chats and the Projects/Tasks pages. Chat context menus retain Remove. Confirm before removing history; block removal while the task is running or being prepared. Removing a project removes its chats from Ion and preserves the project folder and source files.
- Replace the artifact rows with a virtualized grid of document cards. Cards show document name, format, project, task and producer, with Open and Save copy actions.
- Place search, project, task and format filters directly in Artifacts. Remove the Manage storage entry and its unused desktop screen. Retention and cleanup backend policy is unchanged; this update does not clean documents.

## Review checklist

- [ ] Install 0.1.21 and confirm Dashboard/Inbox are hidden and Decisions is under Work.
- [ ] Check dark and light themes. Workspace surfaces should be neutral; green remains an accent. New Task and chat controls keep their existing layout.
- [ ] Open every workspace page at a wide and narrow window size. Check heading/filter/list alignment and editor width.
- [ ] Remove an unused chat from its sidebar action and from Tasks. Cancel the confirmation once; confirm once. Reopen Ion and verify the removed history stays removed.
- [ ] Remove a disposable project with two inactive chats. Verify the project and chats disappear from Ion, the selection remains valid, and the folder/files are still on disk. Reopen Ion to verify persistence.
- [ ] Try removal while a task is running; Ion should ask you to stop it first.
- [ ] Generate HTML/PDF/text documents. Inspect card metadata, combine filters, clear them, and verify Open and Save copy.
- [ ] Resize Artifacts through one, two and three columns and review keyboard navigation and long document names.

No visual acceptance or removal of owner data was performed during development. Native compile and package verification are separate from this owner review.
