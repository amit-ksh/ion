# Product, brand, and user experience

## Positioning

**ion is an AI work IDE that coordinates code, apps, and computers through
reviewable tasks.**

The first audience is an individual developer or small product team that already
uses one or more coding-agent CLIs and wants one place to:

- work across several repositories and machines;
- turn issues into isolated implementation tasks;
- coordinate specialist agents;
- review changes and create pull requests;
- automate recurring engineering and business workflows;
- connect Gmail, GitHub, Slack, calendars, social platforms, and other apps;
- supervise the work safely from a phone.

The wedge is the coding IDE. The expansion is a personal and team agent control
plane. That sequencing matters: it gives the product a concrete, demonstrable
job before it tries to become a general automation platform.

## Brand direction

### Selected identity

- Display name: **ion**
- Styling: lowercase in product and marketing copy
- Working descriptor: **AI work IDE**
- Working line: **AI work, in motion.**
- Secondary line: **Plan, delegate, review, and automate.**

The visual identity should stay close to Waku's current neutral graphite theme.
The repository already reserves coral for meaning and action rather than large
decorative surfaces ([theme](../../src/theme.rs)). Keep that restraint. A rebrand
should not become a simultaneous visual rewrite during the hackathon.

### Safe rename sequence

1. Confirm trademark, domain, social-handle, package, and app-store availability.
2. Change public display name, icon, website, screenshots, and marketing copy.
3. Preserve attribution and the GPL license.
4. Keep internal `waku-*` crate names and the existing protocol version during
   the hackathon.
5. Add a data-directory migration only after backups, rollback, and side-by-side
   debug/release behavior are specified. The current identity constants are in
   [identity.rs](../../crates/waku-protocol/src/identity.rs).
6. Rename bundle IDs, updater channels, daemon executable names, environment
   variables, and storage paths in a separate release after migration tests.

## Information architecture

The sidebar should stop being a configurable rendering of chats and become a
stable product map. Remove the grouping and sort menus. Use one predictable
hierarchy on desktop, web, and mobile.

```text
ion
  New task
  Search
  Inbox / approvals
  Pull requests
  Issues
  Automations
  Apps
  Agent teams

Pinned
  Project: storefront
  Task: Fix checkout race

Projects
  storefront
    New task
    Review issue #128
    Add Apple Pay
  api
    New task
    Repair flaky tests
  Add project

Account
  Models & Speech
  Usage
  Devices
  Settings
```

### Fixed ordering rules

- Pinned items use explicit user order and support drag reorder.
- Projects use explicit user order; a newly added project appears immediately at
  the top until the user reorders it.
- Tasks inside a project use latest meaningful activity first.
- Archived projects and tasks live in searchable archive views, not mixed into
  the primary sidebar.
- Empty projects remain visible with an inline **New task** action.
- The same ordering is persisted by the daemon and synchronized to clients.

This directly fixes the current behavior where project groups are derived from
started sessions, so an added project can be absent until a chat exists. The
current behavior can be seen in `project_sidebar_groups` and `sidebar_rows` in
[sidebar.rs](../../src/app/sidebar.rs).

## Core product objects

### Project

A durable repository/workspace entry, independent of chats.

Required fields:

- stable ID, name, path or remote workspace reference;
- source-control remote and default branch;
- pin state and sidebar rank;
- last-opened and archived timestamps;
- device/daemon that owns the files;
- policy defaults for worktrees, approvals, and permitted teams.

### Task

A user-visible unit of work, not merely a chat transcript.

Required fields:

- objective and measurable completion condition;
- project and optional issue/PR/app-event source;
- state: draft, queued, running, waiting, blocked, review, complete, failed,
  cancelled, or archived;
- priority, labels, owner, assigned team, and budget;
- child work items and dependency graph;
- one or more agent sessions/runs;
- artifacts, evidence, approvals, and external links.

Existing Waku sessions migrate one-to-one into tasks. A simple task still has one
session. A team task becomes a parent task with several child sessions.

### Run

One execution attempt for a task or automation. Runs make retry, cost, logs, and
audit explicit instead of overwriting task state.

### Artifact

A diff, file, image, report, issue, pull request, email draft, social post draft,
calendar proposal, terminal log, or computer-use recording produced by a run.

### Approval

A typed request for human authority. Examples include command execution, file
access outside the workspace, app writes, email sending, PR creation/merge,
publishing, deletion, or computer input.

## Primary screens

### Home / task inbox

- active, waiting, blocked, review-ready, and recently completed tasks;
- approvals that need action;
- automation failures and expired app connections;
- device/daemon connectivity;
- compact usage status.

### Project

- task list and project activity;
- issues and pull requests linked to the repository;
- branches/worktrees and repository health;
- project-specific teams, skills, automations, and app policies.

### Task

- main transcript/activity timeline;
- plan and dependency view;
- child-agent/team status;
- changes, files, terminal, browser/computer, and artifacts;
- linked issue/PR and automation source;
- explicit completion criteria and final evidence.

### Pull requests

- assigned/requested reviews, authored PRs, CI state, and linked ion tasks;
- review diff with inline comments;
- actions to create a review task, request fixes, approve, or merge according to
  repository policy.

### Issues

- assigned, mentioned, recent, and filtered issues;
- create task from issue;
- task progress and generated PR shown back on the issue card.

### Agent teams

- reusable roles, members, models/providers, tool scopes, and delegation graph;
- live and previous team runs;
- budgets, concurrency, and outcome quality.

### Apps

- searchable connection catalog;
- connected accounts, scopes, status, and last use;
- read/write permissions per team or automation;
- audit log and disconnect/re-authenticate actions.

### Automations

- active, paused, failing, and recent schedules;
- trigger, prompt/workflow, execution environment, app scope, budget, and
  notification policy;
- run history with retry and approval status.

### Models & Speech and usage

- Nebius Token Factory BYOK setup with a fixed API base URL, vault status,
  key verification, and the live model catalog;
- local NVIDIA Nemotron 3.5 ASR setup through NeMo-Speech.cpp with no
  credential field;
- informational model and user-borne Nebius usage, with optional budget alerts;
- no checkout, subscription, entitlement, or paid feature controls.

## UX rules for agent work

- Queue is the default follow-up behavior; steering is explicit.
- Parent and child tasks are visually distinct. Do not flatten six agents into a
  single transcript.
- Every delegation shows the owner, objective, accepted inputs, completion test,
  budget, and current state.
- External writes are rendered as drafts until policy or a human authorizes the
  action.
- Computer control always shows target device/window, capture state, and a
  persistent stop control.
- Agent reasoning is not the audit log. The audit log records inputs, tool calls,
  approvals, outputs, artifacts, and state transitions.
- The mobile UI prioritizes approvals, status, diffs, attachments, and steering;
  it does not reproduce every desktop panel.

## Success measures

- Time from issue selection to isolated agent task.
- Percentage of added projects visible without first creating a chat: target 100%.
- Percentage of team runs whose final result includes verifiable evidence.
- Human touches from task start to PR creation.
- Automation success, retry, and approval-wait rates.
- App action reversal/error rate.
- Remote reconnect success and missed-event rate.
- Voice transcription latency and edit distance after user correction.
- Weekly active projects and tasks completed, not messages sent.
