# Graph teams and action-led onboarding

23 October 2026. Additional user requirements extend the existing UI queue.
The latest team reference is a hierarchy diagram, not authorization to add new
harness adapters or change the execution/access model.

## UI-09 — Graph-based team creation and editing

- Show each team as a connected hierarchy with a lead at the top and direct
  reports below, including deeper branches such as technical lead → frontend/
  backend agents. Support multiple separately configured teams.
- Nodes display the actual agent name/role, selected harness and environment.
  Preserve real member identities, instructions, expertise and access settings.
- Select a node to inspect its context and edit its reporting relationship.
  Offer adding a direct report and removing a membership without deleting the
  underlying agent profile. Keep keyboard-accessible editing controls.
- Reject cycles and self-reporting. Promoting an agent to lead must preserve
  the rest of the hierarchy. Removing a manager must preserve/reparent its
  direct reports instead of leaving broken parent IDs.
- Save through the existing Team/member `parentId` model and validation. These
  edges govern real higher-to-lower delegation; this is not a decorative diagram.
- Keep connected lines and nodes legible in both themes; scroll wide graphs
  within their panel. Cache geometry when definitions change, with no I/O in
  frame rendering. Do not copy example names or unsupported harnesses literally.

Status: source implementation is in progress on desktop and mobile. Desktop
modules `team_graph.rs`, `team_graph_layout.rs` and `team_hierarchy.rs` provide
cached graph rows, node selection and reporting-line edits. Snapshot refresh
updates cached agent labels without replacing an unsaved membership draft. The
old indented member rows are replaced by graph rows; keyboard member controls
remain in the editor. Both clients retain the existing 32-agent backend limit.

Mobile `components/team-graph.tsx` and `lib/team-graph.ts` are connected to the
Teams list and team editor. Nodes use existing harness logos and account-scoped
profile data. Selecting a saved-team node opens that editor/node. Direct-report
selection and membership removal use the same pure edit functions as the normal
form controls. In particular, removing a manager no longer turns all its reports
into separate leads. Existing revision-checked saves and draft conflict handling
remain in place. Graph geometry scales with the phone's text-size setting.

Local checks: **9 actual Windows Rust layout/hierarchy tests** and **11 mobile
layout/draft tests** pass. They cover the supplied branch structure, connector
bounds, invalid/disconnected cycles, lead promotion, manager/lead removal,
identity preservation, stale/duplicate choices, the 32-member boundary and larger
mobile text geometry. Four affected mobile TSX files pass syntax transformation;
tracked source diffs pass whitespace checks. This is not full GPUI/TypeScript
type checking, rendered/device acceptance or proof of live team execution.

Receipt: `.waku-cache/ion-team-graph-local-checks.json` records commands, terminal
outputs and hashes of the actual checked source modules. All five checks passed
with unchanged inputs. No provider request or app/device operation was performed.

The two guarded offline desktop checks did not launch. The latest attempt saw
2.38 GiB free physical memory and 3.45 GiB commit headroom, below the existing
2.5/4 GiB gate (`.waku-cache/ion-graph-refresh-native-check.json`). A later
read-only resource observation found 1.73/3.11 GiB; it did not start another
check. No existing process was stopped, build restarted, app installed or native
screenshot taken. Full desktop/mobile type checks and scoped native interaction
acceptance remain required before packaging these changes.

## UI-10 — Clean onboarding and personal Nebius configuration

- Replace the text-heavy flow with compact steps showing the current setup
  state and the next useful action. Use icons, short headings, labelled fields
  and concise inline help; remove long static explanation.
- Preserve mandatory desktop/mobile Ion login. Optional service setup can be
  skipped and completed later through the existing top-bar setup indicator.
- Clearly distinguish managed Ion services from connecting the user's own
  Nebius account. Do not overwrite shared operator credentials or change another
  account's service controls.
- Give personal Token Factory configuration clear API-key and project fields.
  Identify the appropriate AI project when needed for sandbox access. Keep
  Nebius Cloud project/region/credential setup separate where those features
  require it; never silently interchange AI-project and compute-project IDs.
- Keep secrets in the existing OS/platform secure storage. Mask key input,
  provide an accessible reveal control and never place keys in chat, ordinary
  settings files, logs or screenshots.
- Offer explicit connection testing, progress, inline errors and a clear saved
  state using the supported read-only checks. Do not automatically provision
  resources, start inference/sandboxes or enable paid services as part of setup.
- Keep the initial path useful for existing local harnesses. Missing optional
  setup should clearly affect only its dependent features, with a direct action
  to complete it later.

Acceptance: a first-time user can understand the next action, connect their own
key/project or skip it, return later without lost configuration, and distinguish
managed versus personal setup. Test field validation, secure storage, account
changes, narrow layouts, keyboard and clear error/disabled/loading states.

Status: desktop onboarding and personal-project access-check source is in
progress. No compiled, installed or personal-account acceptance is claimed.

The desktop flow now has clickable setup choices, compact headings and shorter
copy. Personal key and self-hosted gateway fields are disclosed on demand.
Key reveal is an explicit keyboard-operable action; saving, moving between setup
steps and leaving setup restore masking. Skipping still clears unsaved secrets.
Provider status distinguishes a saved personal key, session-only storage and
Ion-managed availability. A missing personal key no longer offers a misleading
personal Test button that could actually exercise shared inference.

Personal Token Factory AI-project configuration is an optional access check
inside Models and the provider settings. The key keeps its existing vault entry;
project metadata uses a separate namespace per Ion workspace and contains only
the AI project ID, workspace binding, version and key digest. Key rotation or
account changes invalidate cached matches. Save/remove/check actions pin the
account that initiated them; key/project updates are serialized in the UI.
The read-only check uses the fixed HTTPS sandbox `whoami` endpoint, rejects
redirects, bounds the response and rechecks account/key/project after the reply.
Metadata has a labelled memory-only fallback when the OS vault is unavailable.

This is **not** personal sandbox execution activation. No field changes the
shared operator project, imports an image, enables a policy or starts paid work.
Inference uses the project associated with the supplied Token Factory key;
the optional explicit AI project is used for beta-access checks. Reviewed worker,
pricing/budget and actual sandbox lifecycle gates remain open. Nebius Cloud VM
project/region fields are not added to the current static-only publishing scope.

Validation so far: Rust formatting/syntax parsing, tracked diff whitespace and
all 14 static onboarding icon files/embedded entries pass. Five new project
binding/namespace tests are **declared but not executed**. Full credentials and
desktop type checks, unit tests, real vault save/restart/account switching,
read-only provider access and native onboarding interactions remain pending.
Receipt: `.waku-cache/ion-onboarding-project-source-observation.json`, including
hashes of all eight affected Rust files. Its zero request count refers to
authenticated application/provider API checks; public documentation was browsed.

The full desktop check was again deferred before launch at 2.47 GiB free physical
memory / 3.07 GiB commit headroom (`ion-onboarding-native-check.json`). Source
advanced after that guarded observation. A later read-only observation found
1.41/3.38 GiB and no Cargo/rustc process. No competing process was stopped, gate
lowered, native app restarted, package built or real credential changed.

Primary sources rechecked on 23 October:

- [Token Factory API authentication](https://docs.tokenfactory.nebius.com/api-reference/introduction)
  documents bearer-key authentication.
- [Nebius's project/key setup example](https://dev.nebius.com/cookbook/opencode-nebius-token-factory)
  identifies the selected project as the payer for its key.
- [Sandbox limits and usage](https://docs.tokenfactory.nebius.com/sandboxes/operate/limits-retention-and-usage)
  documents `whoami`, the Project header and permissions. It still requires
  confirmation of relevant resource units/retention behavior; raw operation cost
  remains diagnostic, not an invoice. This does not resolve the goal's pricing
  or real-provider acceptance gate.

## Follow-up source review — 23 October

The next build preflight found only 0.49 GiB free physical memory and 2.41 GiB
commit headroom, with no Cargo/rustc process observed. No compiler was launched.
The previous goal turn made concrete source progress; this is not a wait on a
live build or a claim that tests are still running.

Two additional source defects were corrected without operating the native app:

- Nonsecret AI-project drafts now survive validation/storage failures and key
  replacement. Key changes still invalidate the cached project binding; account
  switches still clear the draft. Secret key fields retain immediate clearing.
- A graph member whose profile is unavailable can still be selected and removed
  from a team. An explicit Remove unavailable agents action also permits repair
  when malformed legacy edges prevent drawing a graph. It rechecks the current
  cached snapshot on click, edits only the draft and uses the existing membership
  removal logic to preserve reports. An unavailable manager's label no longer
  incorrectly says Team lead.

Rust formatting/syntax parsing and tracked whitespace checks pass for these
four changed files. Static onboarding/starter/graph icons exist and are embedded.
Receipt: `.waku-cache/ion-ui-draft-and-missing-agent-review.json`. No new unit,
full application type, native visual, physical-device, credential or provider
test ran. Earlier 9 Rust / 11 mobile tests cover their recorded logic modules;
they do not verify the newly edited UI/controller paths. The five personal-project
tests remain unexecuted. Original source observations remain intact.

## Related work and release boundary

Plan 177 covers starter cards, app logos and the disappearing mention placeholder.
Starter-card source is now being implemented through the existing editable
definition forms, with no automatic execution. App-logo source and its remaining
gates are now tracked in plan 179.
The mobile idle/default composer label now reads Do anything… above the existing
mention placeholder; working/reconnecting/queued-message labels stay contextual.
The previously verified installed Windows/Android packages predate these new UI
source changes. Do not describe those packages as containing this graph or the
new starter cards; new compilation, package and scoped acceptance are required.
Follow-up: five actual production project-binding tests now pass in
[180](180-personal-project-binding-tests.md). Validation is split from its vault/
provider transport; whole-application and live acceptance remain open. Earlier
observations retain their recorded scope.

These source/document changes are uncommitted. Approval access returned for the
operations in plan 179, and the cancellation report was committed as eb7ee9b.
Memory still defers the new source checks; the installed daemon is currently
unavailable with saved exposure disabled.
