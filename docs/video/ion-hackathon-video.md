# ion: Nebius hackathon video

Production draft, 2026-10-18. This is a script and recording plan, not a
rendered video or evidence that a feature has passed live acceptance.

## Agreed direction

- Explain ion's features and the problems they solve. Highlight the Nebius
  services behind the demonstrated work throughout the story.
- Main walkthrough: at most 5:00, including titles and closing credits.
- Companion submission cut: target 2:50. The official event rules say the
  demo should be under three minutes; judges need not watch beyond that.
- Launch cut: 30-45 seconds, using footage from the main walkthrough.
- Intended use: noncommercial hackathon presentation. No paid video service
  is necessary for the baseline production workflow.
- English narration and captions; 1920x1080 landscape master.
- Record real app behavior. Add legible callouts and restrained transitions.

## Core story

**ion brings AI work into one native workspace: create a task, delegate it,
review decisions, and keep the result. Nebius Token Factory supplies NVIDIA
Nemotron inference for the featured workflow.**

Use one small demo project throughout, such as a task-tracker app. Prepare a
bounded code change and a short review/report deliverable. Keep the viewer
oriented to the same project as the demonstration moves between features.

The narration below is a draft for footage matching these actions. Record and
verify each action before treating its wording as final. Do not narrate a
configuration screen as proof of a completed execution.

## Five-minute master

### 0:00-0:20 — What ion does

Show: ion title, then the actual project workspace with a completed task and
its resulting artifact. Introduce the outcome before touring navigation.

Narration:

> Meet ion, an AI work IDE that brings projects, agent tasks, teams, and
> reviewable results into one native workspace. For this hackathon, we're
> showing how NVIDIA Nemotron, served through Nebius Token Factory, powers
> real work inside ion.

On-screen title: `ion · AI work, in motion.`

### 0:20-0:45 — Projects and durable tasks

Show: select the sample project, open task history, and create a task.

Narration:

> Start with a project and give the work its own task. Instructions, activity,
> and results stay together, so you can return to the work with its context
> intact. The workspace also brings decisions, workflows, and artifacts into
> reach without losing the task you're working on.

Callout: `Projects → tasks → results`

### 0:45-1:50 — A real Nebius-powered coding task

Show: the selected live Nebius model; send the bounded change request; record
streamed text and tool activity; inspect an actual approval; approve the
specific edit; show the resulting diff and outcome. Trim waiting time visibly
if necessary, keeping the real interaction order.

Narration:

> Here, the task uses NVIDIA Nemotron through Nebius Token Factory. We ask it
> to inspect this project and make a specific change. As it works, ion shows
> the response and tool activity in the task.
>
> The model's proposal and the action are separate. When this edit needs a
> decision, ion shows what will change and waits for approval. After we allow
> it, we can inspect the actual diff and compare the result with the original
> request. Nebius supplies inference; ion's local daemon manages the workspace
> and the authorized tools.

Callouts: `Nebius Token Factory` / `NVIDIA Nemotron` / `Review the proposed edit`

Do not claim tests ran unless a real successful execution is shown. Use the
actual model identifier selected during recording, not a guessed model name.

### 1:50-2:30 — Agent teams

Show: one lead and two specialists, their assignments, child-task activity,
and a returned result. Select Nebius-backed agents where supported and tested.

Narration:

> For work with several responsibilities, ion has agent teams. A lead can
> delegate focused assignments to specialists and review what comes back.
> Each agent has its own harness, model, environment, and permissions. The
> hierarchy keeps those responsibilities visible, and the parent task brings
> their results back into the same piece of work.

Callout: `Delegate → inspect → review`

Include only a team execution that has been verified in the recording build.
Do not imply that every supported harness is a Nebius service.

### 2:30-3:10 — Workflows, routines, and connected apps

Show: an editable workflow; its assigned teams; a real approval gate and
completed next stage. Briefly show routine scheduling and Apps if configured.

Narration:

> Turn a repeatable process into a workflow with assigned teams, required
> outputs, and human review points. The next stage waits at its approval
> gate until you decide to continue. Routines can schedule that work, while
> connected apps extend the workflow to configured services. Decisions stay
> attached to the action and context you're reviewing.

Callout: `Repeatable work, with human decisions`

If showing prompt-generated stages, verify and identify the actual Nebius
generation request. Show a connected-app write only after an authorized,
successful run; otherwise limit this beat to configuration and label it so.

### 3:10-3:40 — Artifacts and visibility

Show: a genuine task or workflow artifact, its preview, original export, and
links back to the producing task. Briefly open Usage.

Narration:

> Results don't have to disappear into a conversation. ion keeps documents
> and reports as artifacts, linked to the work that produced them. Open the
> preview, inspect the source where available, or export the original. Usage
> views help you understand activity and the accounting available for a run.

Callout: `Keep the result and its context`

Do not present usage estimates as reconciled Nebius invoices. Do not promise
full Office page/slide fidelity from an extracted-text preview.

### 3:40-4:05 — Voice and mobile supervision

Show: dictate into the editable composer. If a paired physical phone has
passed acceptance, show the same task and an actual review decision there.

Narration:

> You can also dictate instructions and review the text before sending.
> NVIDIA speech recognition can run locally, separate from Token Factory
> inference. With a configured, paired phone, you can inspect task activity
> and respond to supported decisions away from the desktop.

Callout: `Voice input · mobile review`

Mobile is conditional on a verified native device run. If unavailable,
replace its time with task continuation and artifact inspection. Do not use a
browser mockup as evidence of native phone behavior.

### 4:05-4:40 — Explain Nebius's role

Show: a simple architecture overlay on real footage:

`ion → local daemon → Nebius Token Factory → NVIDIA Nemotron`

Place local workspace/tools/approvals beneath the daemon. Add any cloud
execution branch only after its exact service and live deployment are verified.

Narration:

> Here's where Nebius fits. ion sends model requests to Token Factory and
> receives the responses and tool proposals that drive this workflow. The
> daemon keeps task state, workspace access, and execution decisions under
> the user's control. That gives the Nebius-backed model a practical product
> around it: durable work, visible activity, human decisions, and useful
> outputs.

If live Nebius cloud execution is ready, replace part of this explanation with
the actual submission, running job, and returned result. Name the verified
service precisely. An intended deployment is not an executed job.

### 4:40-5:00 — Outcome and closing

Show: final diff and artifact, then ion closing card and repository/demo URL.

Narration:

> From a request to a reviewed result, ion brings the work together. Projects,
> agent teams, workflows, and artifacts give the AI a place to work and give
> you a place to stay involved. Built for the Nebius x NVIDIA hackathon, with
> Nemotron inference through Nebius Token Factory.

Leave the closing card readable. The total includes all end cards; never
extend the master beyond 5:00 to fit attribution or URLs.

## Submission and launch edits

### 2:50 submission cut

- 0:00-0:15: product and audience.
- 0:15-1:25: real Nebius task, approval, and resulting change.
- 1:25-1:55: verified team/workflow action.
- 1:55-2:15: actual artifact and result.
- 2:15-2:40: precise Nebius architecture and demonstrated value.
- 2:40-2:50: closing card.

Keep the strongest functioning-app evidence in the first three minutes of
the master as well. The short cut has its own concise narration.

### 30-45 second launch cut

- Hook: one native workspace for AI work.
- Show: a Nebius task, a review decision, team activity, and the final artifact.
- Close: ion, powered by NVIDIA Nemotron through Nebius Token Factory.

Reuse actual accepted footage. Do not introduce additional feature claims in
the faster edit.

## Free production route

- Desktop capture: OBS Studio or another existing recorder.
- Composition: local Remotion project and official Remotion skills, subject
  to its free-license eligibility.
- Media processing: FFmpeg or the renderer's supported bundled tooling.
- Audio baseline: recorded human narration, or readable captions without voice.
- Optional ElevenLabs free narration: use only within its noncommercial-use
  conditions and credit allowance, with the required attribution in the
  published title. Planning to delete the video later does not alter its
  conditions while published. No ElevenLabs calls are authorized by this draft.
- Music: omit by default; add only an appropriately licensed track.
- No paid render hosting, avatar subscription, or generated-video service is
  required for this production route. Nebius runtime usage is separate and
  depends on the user's credits and configured resources.

## Recording prerequisites

- Configured recording build and a real successful Nebius task.
- Clean sample project, rehearsed prompt, and repeatable bounded outcome.
- Live acceptance for each optional team/workflow/app/phone/cloud beat used.
- Recording path for the actual native app; a browser screenshot is insufficient.
- Final narration choice, pronunciation of ion, and closing destination URL.

Repository docs currently leave owner credential/device checks open. That is
not proof those checks are still absent on the user's machine. Establish the
actual setup before recording; do not ask for API keys in chat.

## Evidence and sources

- Product identity and positioning: `docs/ai-ide-plan/01-product-brand-ux.md`.
- Integrated feature inventory and open live checks:
  `docs/ai-ide-plan/103-ion-prod-testing-checkpoint.md`.
- Workflow behavior: `docs/ai-ide-plan/21-workflows-routines.md`.
- Existing demo evidence fields: `docs/ai-ide-plan/SUBMISSION-DRAFT.md`.
- Event rules: https://nebiusglobalaihackathon.devpost.com/rules
- ElevenLabs publication conditions:
  https://help.elevenlabs.io/hc/en-us/articles/13313564601361-Can-I-publish-the-content-I-generate-on-the-platform
- Remotion license: https://www.remotion.dev/docs/license/faq
- OBS Studio: https://obsproject.com/
