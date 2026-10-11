# Gateway workflows and registered owners

Updated 2026-10-11. Development branch: `codex/ion-agent-teams`.
The personal gateway adds its own automation relay **version 1**; the direct daemon
RPC protocol stays **37**. Update the gateway service, desktop and phone together
for this feature. Older gateways continue receiving the original desktop task feed.

## Available behavior

- The daemon background worker publishes project names, saved workflow
  and routine summaries, recent runs and pending stage decisions. Each feed belongs
  to the owner's registered publisher ID. Linux services can use a private pairing
  file; temporary memory-only desktop pairings retain their desktop bridge. The phone can select the owner,
  project and Workflows / Routines / Runs & decisions view.
- A paired phone can start a saved definition, approve or decline its pending
  stage, stop a run, and enable or pause a routine. These use the owner's existing
  daemon commands, pinned teams, per-agent harness/environment, configured grants
  and budgets. The normal New task flow is unchanged.
- The owner console has separate **Read workflows** and **Control workflows**
  invite grants. **Publish desktop state** belongs on each publishing desktop.
  Existing task/review grants do not gain workflow authority automatically.
- The mirror has at most 64 definitions, 64 combined pending/recent runs and the
  projects present in that page per owner. Its UTF-8 JSON is capped at 60 KB.
  Pending decisions are considered before recent history. A truncation label
  explains when the desktop has more history. Reading is bounded to 64 enabled
  registered publishers.
- The feed excludes definition prompts, team objectives/profile bodies, local
  checkout paths, credentials, document bodies and provider output. A pending
  stage includes its own review instructions. Above 1,000 Unicode characters the
  view is partial and cannot approve that stage; desktop review is required.
- Owner publication older than 60 seconds or from the future disables controls.
  Queued requests expire after five minutes. Revoked/expired publishers are
  excluded from reads and new submissions; a revoked phone's queued request is
  rejected before the owner claims it.

## Effect and recovery contract

The phone saves the command nonce, supplied start-run UUID, owner identity,
resource fingerprint and intended action in device storage **before** sending.
Gateway HTTP reads/writes have a 20-second deadline, including response bodies;
a timeout remains ambiguous and keeps the original recovery identity.
It stores no bearer token in this recovery record. Concurrent taps share one
submission. Recovery is scoped to the exact gateway origin, phone, owner and target.

A gateway transaction checks the current resource fingerprint and revision, then
queues a durable command. Only that publisher can read its queue, claim or settle
it. Claiming commits `uncertain` before dispatch. The daemon actor performs its
own transactional definition/run revision check. Changed definitions or stages
cannot silently authorize the current version of the work.

The phone reads the existing receipt after a lost acknowledgement. A retry can
resend the **same** nonce and same intent if no receipt is available; it never
replaces an unknown request with a new run UUID. Changed saved intent is retained
for recovery and blocks another send. Foreground/credential changes while a
request is being prepared fence the consequential send.

Claimed commands never return to the queue after a restart. A lost start
acknowledgement can be reconciled by the owner from the exact persisted run UUID,
definition identity and definition revision; this is a read, not another start.
An absent receipt does not prove that execution failed. Other lost daemon
acknowledgements remain explicitly unknown for owner review. Known daemon
rejections settle as rejected. Repeated identical settlement is safe.

**Stop run** first fences the automation. Existing Teams cancellation/reconciliation
then confirms each child host's outcome. A canceled automation does not by itself
prove an operating-system process or remote effect was undone. Pausing a routine
prevents future firings and leaves existing runs under their own controls.

The original individual-task command queue also checks publisher ownership at
HTTP reads, claims and settlement. Registering a second desktop cannot consume
the first desktop's stop, steer, permission or answer commands. Ownership stays
with claimed commands after their task leaves the mirror. Legacy queued orphan
commands without a provable publisher are rejected during migration.

## Setup and later review

No Nebius key is required to pair devices or view this relay. A live stage still
needs its chosen harness/account and execution target configured on its owner.
Use the gateway [deployment instructions](../../packages/personal-gateway/README.md)
for the owner-token file and trusted HTTPS certificate; do not put credentials in
chat, project documents or command history.

- [ ] Redeploy the updated gateway and confirm `/v1/health` reports
  `features.automationRelay: 1`. Pair each desktop with Publish desktop state.
- [ ] Pair the native phone with Read workflows and Control workflows. Open
  Personal gateway → Workflows & routines. Confirm actual owner/project names,
  definitions and current run statuses, including a second registered desktop.
- [ ] Pair a read-only phone. Verify it can inspect workflows and decisions but
  cannot start, approve, stop or change a schedule. An old task-only grant must
  require a new explicitly scoped invite for workflow access.
- [ ] Create a workflow whose first stage requires a decision. Run from the phone
  and confirm one durable run with one ID, the selected team/harness/environment,
  and no stage dispatch before approval. Inspect the exact instructions first.
- [ ] Approve and decline separate stage decisions. Confirm later stages keep
  their own gates and app mutations still request exact action approval.
- [ ] Test a stage with over 1,000 characters of instructions: phone approval is
  disabled; declining/stopping stays available. Review full instructions on desktop.
- [ ] Enable and pause daily/weekly/monthly and interval routines from the phone.
  Confirm the owner's calendar/zone and next fire, and that pause does not stop an
  already active run. Edit the definition while a phone dialog is open and verify
  the stale action cannot apply to the edited version.
- [ ] Drop the phone response during Run now, reopen the app and recover the saved
  request. Confirm one run UUID. Drop the daemon start acknowledgement and verify
  owner recovery finds that exact run without starting it again.
- [ ] Disconnect a publisher for over 60 seconds, background the phone with a
  confirmation open, and remove/change pairing during a request. Controls must
  remain fenced until the current owner and credential are confirmed fresh.
- [ ] Revoke the phone before a queued command is claimed. Verify rejection and
  zero dispatch. Revoke a publisher and verify its feed is hidden and another
  publisher cannot claim or settle its pending commands.
- [ ] Stop a running workflow. Inspect actual child cancellation/reconciliation;
  confirm no later stage starts. Repeated taps/restarts must not replay an unknown
  gate, schedule or start action.
- [ ] Recheck individual-task stop/steer and pending permission/question responses
  with two publishing desktops. Verify delivery only to the owning runtime.
- [ ] Review small-screen layout, text scaling, screen-reader labels, keyboard
  selection and light/dark themes on real iOS/Android devices.

## Automated verification

- 40 gateway tests passed, including loopback HTTP grant/revocation/routing,
  durable restart/claim/settlement, existing external-action authority and task
  ownership migration. The changed migration/HTTP cases were rechecked after
  the final migration adjustment.
- 15 phone request/client tests passed, including storage-before-send, concurrent
  taps, lost acknowledgement, saved-intent mismatch, scope fencing and invalid feeds.
- 4 Rust shared-projection tests passed for metadata privacy, complete/partial
  Unicode review, whole-row UTF-8 bounds and missing/unsupported definitions.
- Full mobile, shared client and gateway TypeScript checks passed. Locked desktop,
  daemon and selected-harness worker Cargo checks passed. Protocol 37 binding
  equality, Rust formatting and whitespace checks passed.

These are 59 distinct automated tests. They do not establish live provider,
cloud deployment, TLS/device or visual acceptance.

## Remaining development and acceptance

Headless registered-owner publication is now implemented; see the separate
[service setup and review](31-headless-gateway-owner.md). Broader gateway Inbox/usage
projections, full paged historical workflow feeds and evidence/document transfer
remain separate development. The
phone currently reviews bounded pending-stage instructions; full documents,
profile access details and inherited code review remain in the direct owner UI.

Live HTTPS, native-device, chosen-harness and Nebius acceptance remain unchecked.
No visual test, live provider request, cloud deployment or billable provisioning
was performed for this increment. New deferred harness adapters remain out of scope.
