# Feature plans

The [README](README.md) is authoritative. Features below are staged after the
Milestone 1 distribution and credential foundation.

## 1. Projects and tasks

Keep Waku projects and durable sessions. A new task in the hackathon profile
selects Nebius regardless of a historical last-provider value; existing
legacy tasks remain readable. The user can pin and navigate projects, inspect
task state, and review work without losing history. Do not migrate internal
`waku-*` names, schema, storage paths, or wire identifiers during the event.

## 2. Native coding runtime and models

Milestone 2 adds Nemotron execution through Token Factory. The desktop accepts
a BYOK key into the typed vault service, verifies it against the official
`GET /v1/models` catalog, and shows only nonsecret model metadata. The
runtime validates tool proposals against permissions, streams events in
provider order, checkpoints turns, and can resume without replaying effects.
No generic legacy CLI is the hackathon execution fallback.

## 3. GitHub issues and pull requests

Issue reads can create a task with source attribution. Pull-request creation,
comments, merges, and other writes require task-scoped approval. The later
personal gateway uses a scoped Composio project key and user-connected
accounts. Tokens and provider OAuth credentials stay in the gateway's
credential boundary, not prompts, transcripts, or the desktop database.

## 4. App connections

Show the account, toolkit, requested scopes, and whether an action reads or
writes. Store only connection references in durable task state. A task receives
a narrow tool session and auditable result, never the upstream credential.
The user's gateway is provisioned under their own Nebius account through a
guided setup; it is not a shared hosted ion service.

## 5. Computer use

Cua remains local and permission based. Release enablement requires an
emergency stop, visible active target, explicit approval for consequential
actions, and an audit trail. It has no API-key field. Offscreen or unattended
computer use is deferred until its safety and recovery behavior are proven.

## 6. Local speech

Use only NVIDIA Nemotron 3.5 ASR Streaming 0.6B with NeMo-Speech.cpp.
The daemon supervises local streaming inference off the UI thread. The
composer receives editable partial/final text with a visible locale choice;
Hindi and English are acceptance cases. Audio remains local by default.
There is no speech credential or paid speech gate. Measure startup latency,
stream cadence, and memory on target hardware before shipping.

## 7. Usage and cost visibility

Show local runtime and user-borne Nebius usage as transparent informational
metrics where APIs provide it. Do not enforce a subscription, entitlement,
metered allowance, or paid feature gate. The product is free/BYOK; Nebius
charges the user's own account under its own terms.

## 8. Review and automation

A team run carries typed task contracts, dependencies, artifacts, approvals,
and bounded tool grants. Automations record trigger, action policy, resource
target, retry state, and stop conditions. First ship manual repeatable runs;
scheduled and cloud execution follow only after the native runtime is stable.
