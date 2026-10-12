# Inbox background notifications

Updated 2026-10-12. Implemented in `codex/ion-agent-teams`; not merged into `ion-prod`.

## Delivered

The daemon's outbound publisher now advances through the durable Inbox across
projects: proposed tasks, decisions, questions/approvals, issues, completed work,
agent documents and workflow/team updates. The temporary desktop pairing uses the
same owner feed through a background RPC. No render path reads the database or
contacts the gateway.

In Personal gateway on the native phone, **Enable Inbox notifications** is a
separate opt-in from the existing task-request notifications. It requires the
`inboxRead` grant and OS permission. Existing push subscriptions migrate as
**task requests only**, and old task/workflow grants do not gain Inbox access.
The new phone route `/v1/device/inbox/push` cannot be interpreted as a task opt-in
by an older gateway. New gateways advertise `inboxNotifications: 1`.

First owner publication establishes a silent baseline, so installing/upgrading a
publisher does not alert the phone about its entire past history. Subsequent
ascending batches scan up to 64 facts; their cursor commits with pending alerts
in one transaction. Exact lost-response replay returns that receipt. A changing
cursor refuses stale/foreign batches instead of resetting the history. When
restoring a different owner database, register a new publisher pairing.

Unread, unacknowledged and unarchived recent facts can alert. Superseded/resolved
requests are skipped. Owner **All / Issues only / Off** governs this Inbox stream;
Issues only includes pending human actions and issues. Old facts beyond 24 hours
and future-dated facts do not alert, but the cursor still advances. Initial task
history migration finishes before the baseline is established.

The gateway combines pending updates into one generic alert per owner/phone,
expires unsent alerts after 15 minutes and cancels unsent alerts when the owner
mutes them or either pairing loses the required grant. Phone toggles affect their
own topic, and task alerts retain their existing route. Enabling both topics may
produce both a task-request alert and an Inbox alert for a normal task request.

## Push content and destination

New Inbox alerts contain only a fixed title/body and `{version: 1, scope: "inbox"}`.
They transmit no project/task/title/content, device/publisher/event IDs or sequence
numbers to Expo. The Expo push token is required for the phone's explicit opt-in.
Tapping opens the **current** paired gateway Inbox; select the owner there when
multiple owners are registered. The alert cannot approve an action, mark a
notification read or authorize an agent. An expired/missing pairing or missing
Inbox read grant prevents navigation.

An earlier proposed payload with internal routing IDs was rejected by automatic
approval review. The implemented generic payload removes those IDs. The existing
task-request push payload and route are retained.

## Delivery boundaries

Alerts are best effort. The gateway records uncertainty before calling Expo, so a
lost send acknowledgment is not automatically resent; this avoids duplicate
alerts but can miss an alert. The durable Inbox remains available regardless.
Expo receipts are checked after 15 minutes and stop polling after 24 hours. A
DeviceNotRegistered result removes a subscription only if it still uses the
original token; a rotated token is preserved. Submitted alerts cannot be recalled.
A provider delivery receipt does not prove that the person saw a notification.
These boundaries follow the [Expo sending/receipt contract](https://docs.expo.dev/push-notifications/sending-notifications/).

No background polling by a suspended phone is required: the owner and gateway
workers publish/send. The owner must remain running, the gateway must be reachable,
and a configured native Expo development/production build must support push.
The browser and Expo Go are not substitutes for device acceptance; see the
[Expo notification setup](https://docs.expo.dev/push-notifications/push-notifications-setup/).

The direct feed RPC raises wire protocol to **40**. Update desktop, daemon/worker
and phone together, and update the gateway before enabling the new topic.

## Your setup and review checklist

- [ ] Upgrade the owner, phone and gateway from the reviewed worktree.
- [ ] Configure the mobile EAS project ID and native notification credentials.
  Install an Android/iOS development or production build and allow notifications.
  Keep service tokens and credentials in their normal setup, never in chat/docs.
- [ ] Pair a phone with Read owner Inbox history. Enable Inbox notifications
  explicitly. Verify a task-only phone cannot enable/read this topic.
- [ ] Confirm an existing task subscription remains enabled, and Inbox stays off
  after upgrade until explicitly enabled. Toggle either topic independently.
- [ ] With historical items present, start/register the owner and verify no alert
  storm. Create new actions/decisions/issues/completions/documents in multiple
  projects and teams, then verify current Inbox details after opening the alert.
- [ ] With multiple owners, open the generic alert and select its owner in the
  current gateway Inbox. Confirm the payload contains no owner/event identifiers.
- [ ] Check All / Issues only / Off on the owner, acknowledged/read/archived facts,
  resolved requests, offline backlog and two phone subscriptions.
- [ ] Restart gateway/owner or interrupt a publication acknowledgment. Confirm the
  cursor and one alert receipt survive and existing work does not execute again.
- [ ] Revoke/expire a phone or publisher while an alert is queued; no unsent alert
  should leave afterward. Already submitted alerts cannot be recalled.
- [ ] Background/switch pairing during OS permission/token registration. Confirm
  late results do not register another pairing or overwrite its UI status.
- [ ] Tap after unpairing or expiration; no authorized Inbox should be opened.
  Verify no action is approved and no notification marked read by tapping alone.
- [ ] Rotate the Expo token, disable notifications and remove the pairing. Check
  that old-token failure receipts cannot disable the newly registered token.
- [ ] Complete Android/iOS lock-screen, cold-start and foreground acceptance.
  Automated tests use fake transports and do not prove real push delivery.

This increment implements the Inbox delivery path. Authoritative billing, broader
remote/resource/egress controls, larger document formats/previews and shared speech
integration/acceptance remain open; unfinished adapters stay out of scope.
