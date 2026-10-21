# Hackathon roadmap and approval gates

The [README](README.md) is the canonical plan. The project extends Waku
directly and ships as ion. Coding and Agentic Engineering is the primary
Nebius x NVIDIA hackathon track.

## Demonstrable path

A user supplies their own Nebius Token Factory key, verifies the live model
catalog, creates a native Nemotron coding task, reviews ordered tool events
and a local diff, and approves a consequential action. Local NVIDIA
Nemotron 3.5 ASR Streaming 0.6B via NeMo-Speech.cpp can turn speech into
editable task text. A later personal Nebius gateway supports scoped Composio
connections and mobile control. The product is free/BYOK.

## Milestones

| Gate | Scope | Required evidence |
| --- | --- | --- |
| 1. Foundation | Plan, public ion identity, distribution policy, Nebius protocol/catalog seam, typed desktop vault, Models & Speech setup | Existing build/static checks; user manually checks rebuilt app and explicitly approves |
| 2. Native runtime | Nemotron Token Factory execution, ordered streaming, validated tools, checkpoint/resume | One end-to-end coding task against real provider payload and user approval |
| 3. Workflows | Typed lead/specialist delegation, artifact and approval, GitHub issue/PR path | Observable parent-child run and reviewable external write |
| 4. Speech and gateway | Local Nemotron 3.5 ASR; guided personal Nebius gateway with scoped Composio key | Offline local dictation and paired gateway demonstration |
| 5. Mobile and polish | Task inspection, steering, approvals, recovery, submission assets | Paired mobile proof, accessibility/performance review, demo build |
| 6. Optional NemoClaw adapter | Approval-gated one-off execution in a user-managed sandbox behind an explicit flag | Exact command approval, bounded output, uncertain-outcome recovery, live sandbox review |
| 7. Phone ASR and personal cloud execution | Phone microphone to selected desktop's local ASR; headless owner-managed Linux daemon for cloud tasks | Installed phone dictation and independent cloud task with reconnect and recovery |

At every gate the agent implements only that milestone, preserves unrelated
work, runs proportional checks, reports changes and limitations with a manual
checklist, and stops. The user tests and explicitly approves before the next
milestone starts. A requested fix stays in the current milestone.

The user authorized Milestone 3 implementation while deferring the Milestone 2
live Nebius check. This is an explicit exception to the implementation order,
not a passed Milestone 2 acceptance result. Both live gates remain open until
the user reviews the real provider runs.
The user also authorized Milestone 4 implementation while deferring those
reviews until a Nebius key is available. This does not pass any live gate;
use the [Milestone 2](09-native-nebius-runtime.md#milestone-2-acceptance-checklist),
[Milestone 3](10-workflows-github.md#milestone-3-review-checklist), and
[Milestone 4](11-speech-gateway.md#milestone-4-review-checklist) checklists
for later review.
The user later authorized Milestone 5 development with those reviews deferred.
Its [installed-build and mobile review checklist](12-mobile-polish.md#user-review-checklist)
remains open. The user then authorized Milestone 6's optional NemoClaw adapter
while those live checks remain deferred. Its [review checklist](13-nemoclaw-adapter.md#review-checklist)
requires a user-managed sandbox and a live Nebius key.
The user then requested phone dictation to desktop ASR and personal cloud
execution. Milestone 7 implementation is tracked in its [review checklist](14-phone-asr-cloud-execution.md#review-checklist);
the installed phone and owner cloud live checks remain open.

## Milestone 1 acceptance

**21 October follow-up:** remaining redesign identity/navigation/theme and
keyboard acceptance is tracked in [142](142-redesign-remaining-acceptance.md),
RD-01–05, linked to the current completion checklist (120) and interaction
checklist (132). This adds review tasks, not a claim that the separate redesign
was merged or permission for another build/device test. Historical scope below
does not override the later agreed Windows/Android managed-access release scope.

- The README and chapters contain the free/BYOK, native Nemotron, local ASR,
  personal gateway, and approval decisions; the developer resource chapter
  links primary sources.
- Visible customer-facing identity says ion where safe, while internal
  compatibility names remain stable.
- NebiusHackathon hides legacy providers from new-task, model, and Settings
  surfaces and rejects new starts and resume imports. Old tasks remain readable.
  Full is a debug-only override; disabled-providers preferences are untouched.
- The typed service stores three credential classes in each OS vault or
  memory only, and exposes set/test/delete/status without returning secrets.
  Nebius verification uses the fixed Token Factory base URL and holds the
  catalog only in memory.
- Cua and local ASR have no credential field. All vault/network/filesystem
  work stays off GPUI render paths. Keyboard focus and activation work.
- No new tests or test files and no test-suite run. No visual testing by the
  agent; the user validates the watcher-rebuilt app.

## Later scope and cuts

The native runtime, agent teams, automation scheduling, mobile apps, and
gateway provisioning are not Milestone 1 behavior. NemoClaw is only a later
optional feature-flagged adapter. Eve is excluded. There is no commerce,
subscription, entitlement, or paid feature roadmap. User-borne Nebius charges
are disclosed by the guided setup, never collected by ion.

## Submission

Provide a working build, public source and license, concise setup guide,
evidence of substantive Nebius/NVIDIA use, and a short narrated demo showing
real model inference and agentic coding. Verify current event rules and
deadlines from the official hackathon page before submission.
