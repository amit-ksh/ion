# Milestone 5: mobile control and polish

The user authorized Milestone 5 development while Milestones 2–4 await live
review. Their unchecked acceptance items stay open. This milestone is **in
progress**; the first implemented slice is phone pairing and exact gateway
approval review.

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
- [ ] Add gateway-mediated answer, steer, and stop commands with durable
      acknowledgements and idempotent replay rules. Show the active target and
      an emergency stop for any computer-control flow.
- [ ] Add monotonic event cursors and a nonsecret offline cache for gateway
      task inspection. A stale view must never enable a write decision.
- [ ] Add mobile notifications and optional phone dictation transfer to the
      paired desktop only after the connection and consent controls work.
- [ ] Review accessibility, long-transcript performance, platform packaging,
      and narrated demo/submission assets.

## User review checklist

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

No live phone or deployed gateway was available for this development pass.
