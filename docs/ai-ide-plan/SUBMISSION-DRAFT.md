# ion hackathon submission draft

This is a working draft for the final Milestone 5 review. Fill the evidence
fields only after testing installed builds and a real Nebius task. Check the
event's current submission form and rules before publishing.

## Short description

ion is a native AI work IDE built on Waku. A user's Nebius Token Factory key
powers a Nemotron coding task while the local daemon owns the workspace,
tools, approvals, and transcript. NVIDIA Nemotron 3.5 ASR through
NeMo-Speech.cpp turns local microphone input into editable task text. An
optional personal gateway lets a paired phone inspect task activity, answer
provider questions, stop computer control, and review a scoped Composio action.
The gateway owner separately executes an approved external action. ion is
free and BYOK.

## Reviewer setup

1. Use a build made from the submitted commit. The current Windows packaging
   path is `bun scripts/bundle-windows.ts`; it needs the prerequisites in the
   [Milestone 4 guide](11-speech-gateway.md#milestone-4-review-checklist).
   Record the build commit and SHA-256 hash below. macOS and Linux packages
   require their platform release jobs and installed-build checks.
2. Open **Settings → Models & Speech**, save a personal Nebius Token Factory
   key, and press **Test**. Keep the key out of recordings and issue reports.
   Create a Nebius task, inspect streamed tool events and the local diff, and
   decide a permission. Use the [runtime checklist](09-native-nebius-runtime.md#milestone-2-acceptance-checklist).
3. For speech, press **Dictate** in the composer. The first use downloads the
   verified local model while online; later dictation can run offline. Review
   the transcript in the editable composer before sending it. The Windows
   release bundle includes the NeMo-Speech.cpp runtime, but the model is a
   separate first-use download. Use the [speech checklist](11-speech-gateway.md#local-speech).
4. For phone control, deploy the owner-managed personal gateway using its
   [deployment guide](../../packages/personal-gateway/README.md). Create
   separate desktop `publish` and phone `taskRead`, `taskControl`, and optional
   `review` grants. Install an EAS preview build on a physical phone and pair
   with the gateway's HTTPS URL. Use the [mobile checklist](12-mobile-polish.md#user-review-checklist).
5. For a connected-app demonstration, provide a scoped Composio project key
   and a disposable test account. Propose a harmless action, review its exact
   arguments on the phone, and execute only from the owner console after
   approval. Follow the [gateway checklist](11-speech-gateway.md#personal-gateway).

The native coding task can be reviewed without the phone or personal gateway.
Those components are needed to substantiate mobile-control and connected-app
claims in the demo.

## Narrated demo outline

| Beat | Show on screen | Evidence to capture |
| --- | --- | --- |
| Native coding | Nebius model, task prompt, ordered tool events, local diff, permission decision | Real model response and resulting workspace change |
| Local speech | Dictate, provisional/final text, editable composer, offline repeat | Local runtime and no cloud speech key |
| Phone control | Same task on phone, exact provider question, answer, active computer target, emergency stop | Desktop advances or stops once |
| External action | Exact Composio proposal, phone deny, second approval, owner-only Execute once | Denied action has no write; approved action executes once |
| Recovery | Disconnect publisher, stale phone controls, reconnect and resume | No duplicate command or event |

Use a fresh disposable workspace and test account. Capture a clean run rather
than presenting mocked gateway tests as live behavior. Do not show API keys,
pairing invites, device tokens, private account contents, or personal paths.

## Final evidence fields

- Submitted Git commit: **pending**
- Public source and GPL license URLs: **pending**
- Desktop build link, platform, version, SHA-256: **pending**
- Installed Android/iOS build links and EAS build IDs: **pending**
- Narrated video link and duration: **pending**
- Real Nebius model and task outcome: **pending**
- Local ASR hardware, language, and offline result: **pending**
- Gateway host region, paired-phone result, and Composio test-account result:
  **pending**
- Current event submission requirements checked on: **pending**

Do not mark a claim as verified until its matching live evidence is captured.
