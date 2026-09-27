# Mobile remote control

The [README](README.md) is the decision record. Mobile is a later remote
control surface for work running on the desktop daemon or a user-owned Nebius
gateway; it does not run a coding agent on the phone.

## First useful flow

A paired device sees projects, tasks, ordered run events, artifacts, diffs,
and pending approvals. It can steer or stop a run, answer a question, and
approve a reviewable external effect. Offline navigation shows cached
nonsecret state and clearly marks it stale. A command is acknowledged only
after the execution authority records it.

## Pairing and authorization

Guided setup provisions a personal gateway in the user's Nebius account.
Desktop and mobile pair with short-lived tokens, device-specific grants,
expiry, and revocation. The gateway keeps its Composio scoped project key in
its own secret store. The desktop keeps its pairing token in the OS vault or,
when unavailable, memory only. Raw Nebius/Composio keys are never embedded in
mobile builds, logs, transcripts, settings, or model context.

A phone can request approval but cannot grant itself broader tool scope.
Review screens show actor, target, effect, and diff or message preview.
Computer control must expose the active target and emergency stop. Terminal
access is explicitly granted and audited.

## Voice

Phone dictation may send microphone audio only to the paired desktop for local
NVIDIA Nemotron 3.5 ASR Streaming 0.6B through NeMo-Speech.cpp, with user
consent and a clear transfer indicator. Offline phone capture can remain
queued until the paired desktop is reachable. The default desktop dictation
path is entirely local. No speech API key is shown.

## Sync and recovery

Use monotonic event cursors and idempotent commands. Reconnect resumes from
the last acknowledged cursor and reconciles pending actions before enabling
another write. A disconnected daemon or gateway pauses work that depends on
it, while preserving approvals and drafts. Device revocation invalidates
future commands without erasing task history.

## Delivery order

First prove desktop native Nemotron coding and typed credentials. Then pair
one phone to a personal gateway for task inspection and approval. Add diff
review, steering, notifications, and voice after the authority and replay
rules are stable. Do not add commerce or paid capability checks.
