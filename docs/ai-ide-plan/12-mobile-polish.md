# Milestone 5: mobile control and polish

The user authorized Milestone 5 development while Milestones 2–4 await live
review. Their unchecked acceptance items stay open. This milestone is **in
progress**. Phone pairing, exact external-action review, task mirroring,
controls, and opt-in notification code are implemented locally.

## Development checklist

- [x] Extend the personal gateway with a separate `review` device grant. An
      owner creates a one-use invite, and the paired phone can list pending
      proposals, inspect an exact fingerprint, approve or deny, and reconcile
      an ambiguous response. It cannot execute an approved action.
- [x] Store the phone device token in the native keychain and only gateway
      address, device ID, grant names, and expiry in ordinary app storage.
      Web pairing remains in tab memory.
- [x] Add a mobile gateway screen with HTTPS pairing, full tool/account/
      version/target/arguments/scope review, visible connection errors, and
      disabled decisions while disconnected.
- [x] Bridge desktop task summaries, ordered events, artifact metadata, diffs, and
      pending task approvals to the paired gateway with narrow, device-specific
      read grants. The existing mobile daemon connection supports task views,
      but uses a full daemon token and is not this gateway path.
- [x] Publish bounded desktop task summaries, bounded visible messages, tool
      activity, and diffs through a background bridge. Separate `publish`,
      `taskRead`, and `taskControl` grants isolate the desktop and phone. Task
      deletion reconciles against the publisher's latest snapshot.
- [x] Add gateway-mediated answer, steer, and stop commands with durable
      acknowledgements and idempotent replay rules. Show the active target and
      an emergency stop for any computer-control flow.
- [x] Add durable, single-claim steer and stop commands. A lost desktop
      acknowledgement stays `uncertain` and is never replayed as a new action.
      The phone disables controls when the owning desktop's heartbeat is stale.
- [x] Mirror exact provider permissions and questions with request fingerprints.
      The gateway checks the selected option or complete answers against the
      pending runtime before a single claim. The phone displays the current
      computer-control app and window without sending screenshots or blob data.
- [x] Add monotonic event cursors and a nonsecret offline cache for gateway
      task inspection. A stale view must never enable a write decision.
- [x] Add opt-in phone notifications for new provider questions and permissions.
      The push payload contains only a task ID and generic copy. Registration
      requires `taskRead`, revocation removes the subscription, and tickets and
      receipts are checked. Live delivery needs account-owned EAS, FCM/APNs,
      and phone build setup below.
- [ ] Optional phone microphone audio transfer for desktop local transcription.
      The desktop ASR accepts WAV, while the current Expo audio presets do not
      establish a cross-platform WAV capture path. Do not upload audio through
      the gateway until capture format, size limits, consent, and deletion are
      explicit. Phone keyboard dictation can enter editable text for steering.
- [x] Keep gateway activity in a virtualized, 300-event phone window, pause
      polling when screens lose focus, and provide text labels and 44-point
      controls for task actions. Static type/config checks pass.
- [x] Virtualize the phone's mirrored task list so tasks beyond the first 20
      remain reachable without mounting every task card at once.
- [x] Add an EAS `preview` profile for installable Android/iOS review builds.
- [x] Prepare a [submission draft](SUBMISSION-DRAFT.md) with a reviewer setup,
      narrated demo outline, and fields for links and live evidence.
- [ ] Verify accessibility and long-transcript performance on installed iOS
      and Android builds. Prepare signed platform builds and narrated
      demo/submission assets.

## Setup needed for live notification review

- [ ] From `apps/mobile`, sign in to the owner's Expo account and run `eas
      init`. Confirm it wrote the real project UUID to `expo.extra.eas.projectId`
      in `app.json`.
- [ ] Configure Android FCM v1 and/or iOS APNs credentials for that project,
      then run `eas build --platform android --profile preview` or register
      the iOS phone with `eas device:create` and run `eas build --platform ios
      --profile preview`. Install the resulting build.
      Android push does not work in Expo Go. See the official
      [Expo setup guide](https://docs.expo.dev/push-notifications/push-notifications-setup/).
- [ ] Pair the phone with `taskRead`, press **Enable task notifications**, and
      grant OS permission. Trigger one provider question, confirm exactly one
      generic notice, tap it, and review the exact request inside ion. Disable
      notifications and repeat; no notice should arrive. Revoke the phone in
      `/owner` and repeat once more.
- [ ] Check the gateway's push ticket and receipt results. A missing or failed
      delivery does not replace the task feed or grant any action.

## User review checklist

- [ ] On `/owner`, create one desktop invite with **Publish desktop state** and a
      separate phone invite with **Read tasks** and **Control tasks**. Uncheck
      the default **Inspect own actions** grant if it is not needed. Pair the
      desktop in Settings → Models & Speech → Personal gateway;
      pair the phone in its Personal gateway screen. Confirm the desktop device
      cannot use phone read/control routes and the phone cannot publish.
- [ ] Start a desktop task, open it on the phone, and compare ordered messages,
      activity, and diffs. Confirm the phone shows the active project, provider,
      task, and runtime status. Steer the running task once, then stop another
      task and confirm computer control and the turn stop on the desktop.
- [ ] Disconnect the desktop or revoke its publisher grant. After a minute,
      confirm the phone marks that publisher's tasks stale and disables steer
      and stop. Reconnect and confirm the cursor resumes without duplicating
      events. Delete a desktop task and confirm it disappears from the phone.
- [ ] Disconnect the phone during a queued command. Reconnect and confirm the
      durable command status is shown. An `uncertain` outcome requires desktop
      inspection and must not replay the command automatically.
- [ ] Trigger a provider permission and a provider question on the desktop.
      On the phone, compare the full request and option labels, deny or answer
      one, and confirm the desktop advances once. Try an old fingerprint or
      changed runtime and confirm the gateway rejects it.
- [ ] Start computer control, verify the phone shows the correct app and window
      title, then use **Emergency stop** while another control is queued.
      Confirm the computer action and provider turn stop on the desktop.

- [ ] On the deployed gateway's `/owner` page, create a one-use invite with
      **Review approvals**. Pair an iOS or Android phone through **Personal
      gateway** in the mobile app. Check HTTPS certificate validation, expiry,
      and revocation.
- [ ] Submit an allowlisted test proposal from the desktop. On the phone,
      inspect the full JSON arguments, account, version, target, and scopes.
      Deny it and confirm no external call occurs.
- [ ] Submit a second harmless proposal. Approve it on the phone, confirm the
      desktop sees `approved`, and confirm execution still requires the owner
      console's separate **Execute once** action.
- [ ] Drop the phone connection during a decision, reconnect, and verify the
      durable status is shown without a duplicate effect. Revoke the phone and
      confirm future review requests fail.
- [ ] Review task control, offline recovery, performance, accessibility, and
      the demo items above on installed builds, then explicitly approve Milestone 5.

### Installed-build quality check

- [ ] On both iOS and Android, open a task below position 20 in a feed with at
      least 30 tasks. Scroll quickly through the list and a task with hundreds
      of events; check for missed taps, large pauses, or duplicated activity.
      Record the phone model, OS version, and approximate event count.
- [ ] Turn on the OS large-text setting. Check the pairing form, task cards,
      long JSON arguments, provider questions, and action buttons without
      clipped text or overlapping controls. Repeat in light and dark mode.
- [ ] With VoiceOver or TalkBack, navigate the pairing form, task list, exact
      approval, provider question, and emergency stop. Confirm each action has
      a spoken label and disabled controls are announced as unavailable.
- [ ] Enable Reduce Motion and confirm task updates and navigation remain
      understandable. Background and foreground the app during a pending
      command; confirm the displayed result matches the desktop and that the
      command is not sent twice.
- [ ] Install the Android preview APK and, if available, the registered iOS
      preview build on physical phones. Check launch, pairing, and generic
      notification delivery there; simulator or Expo Go results do not close
      this installed-build review.

## Demo and submission preparation

1. Record a short, fresh run with a real Nebius model: create a native task,
   show ordered tool activity, a local diff, and a reviewable approval. Keep
   tokens and personal account data out of the recording.
2. Record local desktop dictation turning speech into editable text, then
   show a paired phone reading the same task, answering a provider question,
   and stopping a separate computer-control run with its active target shown.
3. Show one Composio proposal denied on the phone and one approved there but
   executed only by the gateway owner. Then show offline stale state and a
   cursor resume after reconnection.
4. Prepare a signed desktop build, an installed phone build, public source and
   GPL license link, concise setup instructions, and the narrated video. The
   user supplies account credentials, records the live evidence, and verifies
   the current event submission rules before publishing.

### Evidence to save for review

- [ ] Save the tested desktop installer or archive and its SHA-256 hash, plus
      the installed phone build links and build IDs. Record the Git commit used
      for each build so the demo matches the submitted source.
- [ ] Capture a short screen recording of the real Nebius coding run, local
      speech, phone answer/stop, and separate gateway approval/execution. Blur
      or omit API keys, pairing invites, device tokens, account data, and
      private workspace paths before sharing it.
- [ ] Put the public source URL, GPL license URL, setup guide, build links,
      video link, and concise Nebius/NVIDIA implementation summary in the
      submission draft. Check the event's current form and deadline before
      submission; do not treat this list as a substitute for its rules.

The task mirror, exact questions and permissions, and controls passed local
gateway tests, TypeScript checks, and desktop compilation. A live phone,
deployed gateway, and provider task are still needed for the review items
above. Notification delivery and signed phone packaging need the user's
Expo/platform credentials. The optional phone audio transfer and live demo
recording remain open.
