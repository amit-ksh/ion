# Global usage ledger and review

Implemented checkpoint: 2026-10-08, isolated `codex/ion-agent-teams` worktree,
protocol **35**. The complete Ion release still has development remaining.

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

## Coverage still requiring development

- Ordinary non-Nebius CLI session usage is still available through the existing
  desktop Settings → Usage / Provider history scanner for Claude and Codex. It
  has not been imported into this owner ledger or the phone projection. Do not
  combine that scanner's totals with ledger totals: their scopes can overlap.
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

- [ ] Open Usage on desktop and phone connected to the same protocol 35 owner.
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
