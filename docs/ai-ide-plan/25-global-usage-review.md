# Global usage ledger and review

Implemented checkpoint: 2026-10-09, isolated `codex/ion-agent-teams` worktree,
protocol **36**. The complete Ion release still has development remaining.

## Available behavior

Usage in the desktop sidebar and phone drawer reads an owner-maintained ledger
across projects. Filters select project, task, agent, harness, exact model,
Local/Nebius environment, usage type and time range. All projects/tasks are the
default. Pages contain at most 32 rows; totals cover the complete filtered scope,
including older pages. Time filters use the original call start or first owner
observation, so a late receipt does not move usage into a different period.

The ledger captures:

- Team model reservations and actual token receipts for the already implemented
  harness transports. Identity comes from the pinned run and actual attempt,
  rather than the agent's current saved settings.
- Ordinary Nebius task calls and bounded researcher/reviewer specialist calls.
  The owner records each call before HTTP dispatch and settles only a complete
  validated provider token receipt. An incomplete/canceled stream keeps usage
  unavailable. These calls do not invent a Teams token reservation.
- Remote command state and validated command elapsed time, preserving failures
  and original task/project/harness identities.
- GPU job state, validated result elapsed time and the original approved spending
  allowance. Stop, runtime settlement and resource cleanup do not turn that
  allowance into a measured charge or release a billing reservation.
- Local and remote Claude Teams SDK cost totals as provisional estimates. Missing, invalid
  or out-of-bound cost fields remain unavailable.

Claude documents its SDK `total_cost_usd` and `costUSD` as estimates computed from
client-side price tables; authoritative billing requires the Usage and Cost API.
The ledger therefore labels these **Estimated / Provisional**, including a known
zero estimate, and does not present them as invoices.
[Claude Agent SDK cost tracking](https://code.claude.com/docs/en/agent-sdk/cost-tracking).

Different currencies and estimate/finality classes have separate totals. The
ledger shows known input/output tokens, still-reserved tokens, unknown requests,
and costs unavailable. It does not replace missing amounts with zero.

## Owner and client boundaries

Source triggers copy whitelisted metadata in the source transaction. Existing
model/job rows receive a once-only current-source backfill. Ledger facts survive
source removal; edits to profiles cannot rewrite prior identities. Exact token
and cost receipt retries are harmless; conflicting receipts reject. Accounting
for an already reserved request can finish after Stop without granting another
model call or execution. Remote terminal observations, validated token receipts
and optional estimates commit together. Invalid accounting metadata rolls back
independently while the confirmed process stop and full unknown allowance remain.
A late retry must match the exact stored receipt.

The authenticated RPC is read-only. Clients cannot publish a cost, mint a call,
change a reservation or clear history. Pages omit prompts, instructions,
transcripts, credentials, app grants, command output and document bytes. Queries
bind client values to fixed indexed predicates and read one consistent snapshot.

Desktop queries run on background workers and cache bounded rows; render performs
no filesystem/network/subprocess work. Generation fences discard superseded
filter requests. Only the Usage page polls. Phone polling is foreground-only and
owner/connection/visibility fenced, with bounded FlatList pages. Opening a source
uses the existing task flow. No new harness connection is enabled by a filter.

## Native Claude and Codex receipts (2026-10-09)

Project-linked normal New task execution now records validated completed-message usage from the
existing Claude and Codex adapters. It does not enable any deferred adapter or
change task/team invocation. Metadata writes run on provider reader workers;
render and the streaming event pump perform no new work. A bounded receipt cache
avoids repeated writes while durable SQLite identities survive owner restart.
No native session ID, provider message ID, transcript or request body is published
in a usage row. First observation retains the original task/project/time. Native call start time
is unavailable, so its time filter uses the owner observation timestamp.

Claude uses the upstream message ID, even when a native fork has a different
session ID. Its uncached input, cache reads and cache creation are separate input
counts and are added once. Content-block repeats and resumed/copied messages do
not increment totals. Partial stream frames and cumulative SDK `result` totals
are excluded. The actual message model is retained. Missing, malformed, negative
or excessive token receipts are not replaced with zero; a valid zero is known.
The wire identity and usage fields follow the
[official Claude SDK parser](https://github.com/anthropics/claude-agent-sdk-python/blob/main/src/claude_agent_sdk/_internal/message_parser.py).

Codex records `last` usage once per native thread/observed turn/cumulative
input-output checkpoint. It never adds the cumulative `total` as a new call.
Historical replay before an observed turn, unrelated threads and synthetic
context-window fill events are excluded. Cached input and reasoning output are
already included in input/output, so are not added twice. Recently observed turns
are bounded and allow late receipts; durable identities suppress reconnect replay.
The usage notification omits the actual model: these rows retain model unavailable,
rather than presenting a configured/rerouted model as a verified receipt model.
See the official
[notification schema](https://github.com/openai/codex/blob/main/codex-rs/app-server-protocol/schema/typescript/v2/ThreadTokenUsageUpdatedNotification.ts),
[token breakdown](https://github.com/openai/codex/blob/main/codex-rs/app-server-protocol/schema/typescript/v2/TokenUsageBreakdown.ts)
and [resume replay implementation](https://github.com/openai/codex/blob/main/codex-rs/app-server/src/request_processors/token_usage_replay.rs).

Native CLI usage is observed accounting, not a pre-dispatch reservation or an
invoice. A dropped notification, old incompatible payload, receipt that cannot
be persisted, or a call never emitted by the CLI cannot be reconstructed here.
Costs remain unavailable for these normal CLI rows. This checkpoint does not
claim native provider billing reconciliation or live account acceptance.

Five focused tests passed for content-block repeats, restart and native forks,
malformed/partial/zero receipts, Codex historical replay/context-fill/foreign turns,
changed receipt rejection, privacy of stored identities and concurrent exact
settlement. No live provider or visual acceptance was performed.

## Coverage still requiring development

- Ordinary Claude and Codex usage observed after this upgrade is now captured by
  the native reader worker in this ledger, including its phone projection. This
  does not backfill old provider history. Other existing solo harnesses still
  need verified receipt capture and projectless tasks need an explicit ledger
  scope. Settings → Usage / Provider history remains a
  separate overlapping view; do not add its totals to ledger totals.
- Remote Claude SDK estimates are propagated by the updated native worker's
  optional `estimated_cost_usd_micros` field. Legacy workers omit it and remain
  usable, with costs unavailable. Use a matching owner and rebuild/re-register
  the immutable worker image before expecting remote estimates. Other existing
  harnesses still omit costs unless their trusted adapters supply a verified
  accounting source; no rate table or invoice is inferred.
- There is no provider invoice/credit/subscription reconciliation or Nebius cloud
  billing feed. Infrastructure elapsed time is not a price calculation.
- No reconstruction of already deleted pre-upgrade sources, unobserved provider
  calls or live account history is claimed.
- Live Nebius billing, provisioned GPU execution and phone/native visual acceptance
  need the user's later credentials, deployment target and review.

Unimplemented harness adapters remain outside the current scope.

## Verification

Ten targeted accounting/adapter cases passed: immutable identities through Stop
and profile edits; exact late receipt replay and restart; cursor/filter totals;
independent currencies; concurrent zero-cost settlement; ordinary Nebius slots;
Claude estimate classification; remote command receipt retention; and GPU allowance
versus actual result accounting; atomic remote settlement through Stop/restart;
invalid remote metadata rollback; and legacy/zero remote estimate contracts. The remote/GPU cases use the real owner reservation
and settlement functions, rather than client-written ledger rows.

Desktop and daemon compilation, generated binding equality, shared client and
full phone TypeScript checks passed. All 40 Nebius transport regressions and
21 remote lifecycle regressions passed, along with two exact reservation/Stop
cases and two native worker boundary tests. The desktop, daemon and native
worker binary check passed. Rust formatting and whitespace checks passed. No visual test, live account, device test, deployment or additional
dev watcher was performed.

## Your review checklist

- [ ] Open Usage on desktop and phone connected to the same protocol 36 owner.
  Confirm All projects/All tasks defaults and corresponding records/totals.
- [ ] Run one ordinary Nebius task and a researcher/reviewer call. Confirm separate
  request rows, actual model/token receipts, and unavailable costs when not reported.
- [ ] Run Claude locally and on an updated registered remote worker. Confirm
  provisional SDK estimates match each original receipt and old workers show
  unavailable costs. Check Stop/restart cannot duplicate or discard settlement.
- [ ] Run a team on an already configured local harness, then edit the agent's name,
  model and environment. Confirm prior usage keeps the original identity.
- [ ] Stop a team during a request, restart the owner and deliver a final receipt.
  Confirm it settles once, unknown/reserved counts update, and Stop prevents more work.
- [ ] Exercise a remote failing command and a GPU result received after Stop.
  Confirm real elapsed time, original host and allowance; no fabricated charge.
- [ ] Use project/task/agent/harness/model/environment/time filters and page through
  more than 32 rows. Confirm totals describe the filtered collection, not one page.
- [ ] Confirm USD/EUR and estimated/reported, provisional/final classes are not
  merged. Distinguish a verified zero estimate from a cost unavailable.
- [ ] Navigate away, background/reopen the phone and switch owner profiles during
  refresh. Confirm stale responses cannot replace the selected owner's page.
- [ ] Use every desktop control by keyboard and check readable text in both themes.
  No visual acceptance is claimed by compilation.

### Native usage review additions

- [ ] Run ordinary Claude and Codex tasks on the upgraded owner; inspect the same
  rows on desktop and phone. Confirm configured Teams invocation is unchanged.
- [ ] Resume and fork a Claude session. Confirm copied messages retain their first
  task/project and only newly observed requests add usage.
- [ ] Resume Codex, change its context window and stop a turn. Confirm historical
  replay/context adjustments do not count as calls and late known receipts settle
  once. Codex model and both native costs remain unavailable without exact receipts.
- [ ] Confirm old history is separate and is not added to the owner ledger totals.

Native usage integration verification: the five receipt tests and four existing
ledger regressions passed. Desktop, daemon and native worker compilation passed;
Rust formatting and whitespace checks passed. Protocol remains 35, with no shared
wire or phone source changes in this increment. No visual/live/device test was run.

### Native context-adjustment accounting follow-up

Codex cumulative totals can include a synthetic context-window offset. That
adjustment is never a model call, while later real calls retain distinct
checkpoint identities. Normal pre-upgrade receipt identities remain unchanged.
A sixth native regression verifies real calls after a context fill and exact
restart replay. Native receipts retain an observation timestamp and unavailable
call-start time rather than inventing a start time. The source behavior follows
[Codex TokenUsageInfo](https://github.com/openai/codex/blob/main/codex-rs/protocol/src/protocol.rs).
