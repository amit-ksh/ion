# Gateway Inbox notification controls

## Implemented

The phone's **Personal gateway → Inbox & usage → Inbox** now supports **Mark
read / Mark unread**, **Acknowledge**, **Archive / Restore** for an observed
historical notification. All projects remain the default and existing filters,
counts and paged history are preserved. The owner applies the change to the same
durable Inbox state used by the desktop and directly connected phone.

The gateway queues a typed notification command separately from read queries and
workflow controls. Each request pins the registered owner, phone, notification
sequence, Inbox revision, exact action, nonce and two-minute deadline. The phone
persists that identity before sending, without its credential. Duplicate taps or
an interrupted acknowledgment recover that request; another action cannot silently
replace a saved uncertain request. Only an explicit terminal result, or a confirmed
absent expired/rejected request, clears saved intent.

On the owner, the state change and exact receipt commit in one SQLite transaction.
A lost gateway/RPC acknowledgment can recover the original receipt after restart,
even if another client changed the notification afterward. A stale revision,
missing notification or expired operation is rejected without changing state.
Expired queued requests become terminal even while their owner is offline; claimed
requests retain uncertainty until the owner receipt is recovered. Receipt commit
failure rolls back the state change. Acknowledgment updates only
the notification; its task, decision, pending approval and workflow keep their
existing execution and authorization state.

## Upgrade and pairing

Upgrade the desktop, daemon and phone together to **wire protocol 38**. The new
owner receipt RPC is included in regenerated bindings; protocol 37 clients cannot
connect to a version 38 daemon. Restart/rebuild the usual development runtime using
its existing watcher workflow. This development did not launch a second watcher.

Upgrade the personal gateway as well. Its health response advertises
`inboxControl: 1`. Create a fresh explicit invite with both:

- **Read owner Inbox history** (`inboxRead`).
- **Change Inbox notification state** (`inboxControl`).

Existing read-only and workflow/task control invites do not gain notification
write access. The requesting phone and registered publisher are checked on submit,
lookup and claim. Revoked/expired phone grants prevent a queued or uncertain claim
from reaching the owner. Publisher isolation prevents another registered owner
from claiming or settling the command. A completed owner receipt can be settled
without exposing it to a revoked phone.

Persisted/private Linux pairing uses the outbound headless daemon owner. Memory-only
pairing uses the existing desktop bridge and the same daemon receipt RPC. No inbound
daemon port or model/provider credentials are transferred. Scope, navigation,
background, grant and pairing changes fence phone submission/result updates. The
page refreshes owner data after a terminal receipt rather than inventing local state.

The queue returns at most 16 pending commands per owner pass. It retains at most
512 command receipts per publisher; the owner retains at most 10,000 notification
receipts. Subsequent writes prune receipts whose pinned deadline is over seven
days old. A fixed expired deadline stays inert after
tombstone pruning. Capacity errors preserve existing receipts and do not erase
uncertain intent.

## Verification

Focused tests cover durable state/receipt rollback, concurrent revisions, restart
and intervening updates, changed nonce bodies, stale/missing/expired entries;
separate grants and revocation, exact publisher/device routing, queue capacity and
expiry; phone persistence-before-send, lost acknowledgments, status failure,
changed scope/intent, foreign receipts, terminal cleanup and request headers.
The final build/test counts are recorded in the progress log.

## Your review checklist

- [ ] Upgrade app, daemon, phone and gateway together. Pair read-only, control-only,
      workflow-control and Inbox read+control phones. Only read+control can write.
- [ ] Read/mark unread, acknowledge, archive and restore across projects and older
      pages. Compare counts and state with the desktop/direct daemon Inbox.
- [ ] Acknowledge a pending decision or failed task notification. Verify the
      underlying task/decision is still awaiting its own action.
- [ ] Change a notification from the desktop while the phone has an older revision;
      a stale phone command must reject and refresh owner state.
- [ ] Interrupt a submit/owner settlement. Restart the owner and tap the same action
      to recover its original receipt. Change state from another client before
      recovery; the old request must never apply a second mutation.
- [ ] Background/navigate away/change pairing or owner during a request. Revoke the
      phone and publisher separately; old results must stay fenced.
- [ ] Stop the owner past a request's deadline and reconnect. The expired operation
      must not apply later. Verify offline and capacity messages retain saved intent.
- [ ] Review physical Android/iOS accessibility, text sizes, themes and scrolling;
      test the headless Linux owner and actual gateway TLS.

No visual/device test, live provider, billable resource or service deployment was
started. Background push delivery, broader resource/billing enforcement, other
existing solo-harness receipts and speech/device deployment acceptance remain
separate work. Deferred harness adapters remain outside scope.
