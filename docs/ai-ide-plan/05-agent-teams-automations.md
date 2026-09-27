# Agent teams and automations

The [README](README.md) sets the product boundaries. This chapter designs
later milestones; Milestone 1 only establishes their provider and credential
seams.

## Agent teams

An ion team consists of a lead, specialist roles, typed task contracts,
dependencies, run events, artifacts, and human approvals. The daemon owns the
team graph and durable state. NVIDIA Nemotron through the user's Nebius Token
Factory key plans and reviews; ion validates every model-proposed delegation
and tool action. A specialist receives only its task context and narrowly
authorized tools. No model receives an API key, OAuth token, pairing token,
or hidden control marker.

Start sequentially. Parallel execution is admitted when task dependencies
and workspace ownership are explicit. Isolated Git worktrees protect code
changes. Each child reports evidence, changes, and blockers to its parent;
the parent can pause, reassign, or request approval. There is no paid team
tier or entitlement check. User-set budgets are safety controls for their
own Nebius account, not commercial gates.

## Execution adapters

The default hackathon path is the native Nemotron runtime in the Waku daemon.
Legacy provider implementations remain intact under the development-only
Full profile for later restoration. NemoClaw/OpenShell may be evaluated after
the core path works, behind an explicit feature flag and compatibility checks.
It is an execution adapter only and never owns team definitions or run state.
Eve is excluded.

## Approvals

A tool proposal declares actor, target, scope, expected effect, and
reversibility. Read-only work may proceed under an existing grant. Sending,
publishing, merging, deleting, changing account scope, or controlling a
computer requires a reviewable approval. Expired approval pauses the run;
it is never silently widened. The transcript preserves provider event order
while hiding private protocol markers.

## Automations

An automation stores trigger, typed action, run environment, notification
policy, retry policy, user-set budget, and owner. Local-project automation
requires a reachable daemon. Future personal Nebius gateway work can run
without a desktop only after explicit provisioning and pairing. A job or
sandbox is temporary compute, not the scheduler's source of truth. Durable
run records and idempotency keys prevent duplicate external writes after
retries. Notifications identify completion, failure, or required human action.

## First slice

After native execution, create one team with a lead and one specialist, one
typed delegation, one artifact, and one approval. Then make that same graph
repeatable as a manual automation. Add schedules, cloud execution, and more
connectors only after the run is observable and recoverable.
