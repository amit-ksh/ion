# Target architecture

The [README](README.md) is the canonical decision record. This chapter describes
the intended system, including work after Milestone 1.

## Boundaries

```text
ion desktop (GPUI) ── typed client protocol ── Waku daemon ── local workspace/Git/tools
       │                                        │
       │ typed CredentialService                └─ native Nemotron runtime
       │  ├─ OS vault or memory only                  └─ Nebius Token Factory
       │  ├─ Nebius Token Factory API key
       │  ├─ Composio scoped project key
       │  └─ gateway pairing token
       └─ future paired mobile client ── personal Nebius gateway
```

The daemon remains the authority for sessions, workspaces, permissions, and
durable state. Nebius Token Factory supplies model inference for new hackathon
work. The daemon receives authorized operations, never raw credentials. The
desktop credential service performs key checks and passes an opaque authorized
connection to the later native runtime. The gateway, when added, is provisioned
under the user's Nebius account.

## Distribution policy

`DistributionProfile` has `NebiusHackathon` and `Full` values.
`ProviderPolicy` is applied at new-task creation, model choice, resume
discovery/import, and daemon start. The hackathon profile lists only Nebius.
Old sessions and provider code remain readable and restorable. The policy is
separate from persisted `disabled_providers`; changing profiles never rewrites
a user's preferences. `Full` is a debug-only environment override, not a
Settings toggle or command-palette route.

## Native Nemotron execution

Milestone 2 adds the native provider driver. It sends a typed session request
to Token Factory's fixed HTTPS base URL, streams ordered model/tool events,
validates every proposed tool action, and checkpoints turns before irreversible
effects. Prompt context includes task-relevant source and tool results only.
Credentials, internal provider markers, and gateway tokens never become
transcript text or model context. Failure and retry preserve the same durable
task identity and avoid duplicate tool effects.

The first model is selected from the verified live catalog, preferring NVIDIA
Nemotron 3 Super when present. Model IDs must be verified against the user's
catalog; a display label is not treated as an API ID. Cloud sandbox execution is
optional and explicit after local coding works. Serverless Jobs serve finite
asynchronous work; Endpoints serve long-lived APIs.

## Personal gateway and connected apps

A guided setup provisions a gateway under the user's Nebius account and pairs
it with a short-lived token. The gateway holds the Composio scoped project
key and brokers narrowly authorized tool sessions. The desktop vault holds
only its typed local secrets; a paired mobile device gets scoped grants and
revocation. No ion-operated identity, billing, or entitlement service is needed.
GitHub reads and consequential writes pass through task-scoped authorization.
The gateway cannot silently broaden account scopes, send messages, publish,
merge, or delete without the applicable user approval.

## Local speech and computer use

Only NVIDIA Nemotron 3.5 ASR Streaming 0.6B via NeMo-Speech.cpp is in scope
for local dictation. ASR work runs off the GPUI render path and returns editable
text. Cua is permission- and approval-based; it has no credential setting.
NemoClaw is an optional later feature-flagged execution adapter and never owns
task/team state.

## State, performance, and failure

SQLite stores tasks, run state, references, approvals, and nonsecret metadata.
The OS vault or process memory stores credentials. GPUI rows read cached state;
network, filesystem, subprocess, and vault work run on background executors.
Use generation counters for superseded collection reads and notify the entity
when a result lands. A missing vault degrades to memory-only with an explicit
status. A missing Nebius key or failed catalog check blocks new execution but
does not make old transcripts unreadable. A disconnected personal gateway
pauses remote work and preserves queued decisions.
