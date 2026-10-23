# Requested starter options, app icons and composer placeholder

23 October 2026. User-requested task additions based on the two supplied
screenshots. This records requested work; it does not claim implementation,
native visual acceptance or a new package. Preserve existing Ion styling.

## UI-07 — Starter options on Teams, Workflows and Routines

- Show relevant, clickable starter cards in each screen's empty/start state,
  using the reference's icon, short title, concise description and orderly grid.
- Keep headings compact. Use consistent gutters, alignment and readable text;
  adapt the card grid to narrow windows instead of clipping content.
- Teams options open editable team creation with the existing hierarchy,
  instructions, per-agent harness/environment and access settings.
- Workflow options prefill prompts for the existing dynamic workflow generator.
  Keep generated steps, connected apps and instructions editable; do not replace
  prompt-generated workflows with fixed pipeline types.
- Routine options open editable scheduled-task drafts with the existing agent/
  team, project and schedule controls.
- Selecting a card prepares a draft; it does not start execution, connect an app,
  grant access or enable spending. Keep a custom creation path available.
- Cards must work with keyboard focus/activation and have readable icon/text
  contrast in both themes. Populated lists retain their normal navigation.

Acceptance: all three screens offer appropriate starter choices; each opens the
correct editable creation flow without losing project/account context. Existing
definitions, scheduling, dynamic generation and permissions continue to work.

## UI-08 — Recognizable app icons

- Show each supported integration's actual app logo beside its name and short
  description, matching the reference's compact icon-led rows and aligned action.
- Use the actual Composio/catalog app identity and existing connection status.
  The screenshot is a visual reference, not a request to add unsupported apps.
- Preserve connection/reconnection actions and their labels; show a clear
  Connect/add icon where applicable and an accurate connected state.
- Prefer existing approved assets or validated catalog icons. Load/cache remote
  assets off the render path, preserve layout while loading and use a stable
  fallback for missing/failed icons. Do not invent a logo or show broken images.
- Keep app names visible; logos alone must not convey identity or status.

Acceptance: available app logos render at consistent size/alignment, missing
icons degrade cleanly and connection actions still target the correct app/account.

## UI-03 refinement — Mention guidance inside the empty input

Extend the existing UI-03 in plan 146; do not create another duplicate task.

- Include **Do anything…** and concise mention guidance in the textbox's actual
  placeholder: `@` files/teams/apps, `@@` teams only, `@@@` apps only.
- Hide the entire placeholder, including mention guidance, as soon as any text
  is entered or an existing draft is restored. Clearing the input restores it.
- Remove the separate helper beneath the composer. Do not implement the hint as
  persistent input content, a floating overlay or text that can be submitted.
- Keep narrow layouts readable without overlapping the microphone, picker or
  send controls; preserve existing mention suggestions and selection behavior.

Acceptance: empty, typing, restored-draft and cleared-input states behave as
specified, and the three mention filters retain their existing semantics.

## Integration and status

- UI-07 starter-card source is in progress; compilation and native acceptance
  remain pending. Each card opens an existing editable form and pre-populates
  its title/instructions; it does not run or generate work automatically.
- UI-08 local app-logo source now covers desktop/mobile catalog identities and
  adaptive desktop columns. Plan 179 records validation and remaining type/native
  gates. UI-03's desktop placeholder already uses
  the actual empty-input path; mobile idle/default wording has now been refined
  to Do anything… alongside the mention guidance. Four affected mobile TSX files
  pass syntax checks in plan 178, which does not establish native placeholder
  behavior or a full application type check.
- Link the three items to the existing native layout/keyboard, Apps and composer
  acceptance gates in plans 120, 132 and 142.
- Existing scope, budget, separate-worktree preservation and no-visual-test rule
  continue to apply. No new provider, deployment type or paid resource is added.
