# Milestone 5: mobile control and polish

The user authorized Milestone 5 development while Milestones 2–4 await live
review. Their unchecked acceptance items stay open. This milestone is **in
progress**. Phone pairing, exact external-action review, and the first task
mirror/control slice are implemented locally.

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
- [ ] Bridge desktop task summaries, ordered events, artifacts, diffs, and
      pending task approvals to the paired gateway with narrow, device-specific
      read grants. The existing mobile daemon connection supports task views,
      but uses a full daemon token and is not this gateway path.
- [x] Publish bounded desktop task summaries, complete visible messages, tool
      activity, and diffs through a background bridge. Separate `publish`,
      `taskRead`, and `taskControl` grants isolate the desktop and phone. Task
      deletion reconciles against the publisher's latest snapshot.
- [ ] Add gateway-mediated answer, steer, and stop commands with durable
      acknowledgements and idempotent replay rules. Show the active target and
      an emergency stop for any computer-control flow.
- [x] Add durable, single-claim steer and stop commands. A lost desktop
      acknowledgement stays `uncertain` and is never replayed as a new action.
      The phone disables controls when the owning desktop's heartbeat is stale.
- [x] Add monotonic event cursors and a nonsecret offline cache for gateway
      task inspection. A stale view must never enable a write decision.
- [ ] Add mobile notifications and optional phone dictation transfer to the
      paired desktop only after the connection and consent controls work.
- [ ] Review accessibility, long-transcript performance, platform packaging,
      and narrated demo/submission assets.

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
- [ ] Review the later task-control, offline, performance, accessibility, and
      demo items above when implemented, then explicitly approve Milestone 5.

The task mirror and controls passed local gateway tests, TypeScript checks,
and desktop compilation. A live phone, deployed gateway, and provider task
are still needed for the review items above. Pending task approvals, answer
commands, notifications, phone dictation, packaging, and demo assets remain
development work.
