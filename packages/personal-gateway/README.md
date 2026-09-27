# Personal gateway authority core

This package holds the local authorization rules and a loopback HTTP service
for Milestone 4. It is not a deployed gateway. Composio calls require a key
supplied to the gateway host; none has been tested live yet.

`GatewayAuthority` accepts an explicit tool/account/scope allowlist. A trusted
owner surface creates single-use pairing invites lasting at most five minutes.
Paired device tokens last at most one day, are stored only as SHA-256 digests,
and can be revoked. A paired device can inspect or propose only according to
its grants. External effects have an immutable review snapshot and fingerprint.
The snapshot binds a concrete toolkit, tool, pinned version, connected account,
target, JSON arguments body, and declared scopes.
The owner must approve that exact fingerprint. Execution changes the proposal
to `uncertain` *before* an external call, so a lost response never triggers a
silent retry. A confirmed result may then be recorded. Trusted owner reads can
list devices and proposals without revealing token digests.

`DurableGatewayAuthority` persists the policy state in a Bun SQLite database.
Every mutation takes a `BEGIN IMMEDIATE` transaction and commits before returning.
In particular, `beginExecution` persists `uncertain` before its caller may make
the external request. Separate connections cannot consume the same invite or
approval. The database contains device token digests and proposed write bodies;
place it in an owner-only directory and back it up as sensitive application data.
No Composio or Nebius key belongs in this database.

## Run the local service

Create a private directory for the database and a JSON allowlist, for example:

```json
[
  {
    "toolkit": "github",
    "tool": "GITHUB_CREATE_ISSUE",
    "version": "replace-with-pinned-tool-version",
    "accountId": "replace-with-connected-account-id",
    "scopes": ["issues:write"]
  }
]
```

Set `WAKU_GATEWAY_DB` to a database path inside that directory,
`WAKU_GATEWAY_ALLOWLIST` to the JSON file, and
`WAKU_GATEWAY_OWNER_TOKEN_FILE` to a file containing a fresh random URL-safe
token of at least 32 characters. Keep the file owner-only. Then run
`bun packages/personal-gateway/src/main.ts` from the
repository root. The default listener is `127.0.0.1:8787`. Non-loopback binding
requires both `WAKU_GATEWAY_TLS_KEY` and `WAKU_GATEWAY_TLS_CERT`; this option has
not yet been reviewed for cloud deployment. `WAKU_GATEWAY_HOST` and
`WAKU_GATEWAY_PORT` override the bind address.

Optionally mount a scoped Composio project key file from the gateway host's
secret manager and set `WAKU_GATEWAY_COMPOSIO_KEY_FILE` to its path. Raw tokens
in environment values are rejected. Account
lookup uses `GET /api/v3.1/connected_accounts`, requiring **Connected accounts:
Read only**. The owner-only
`GET /v1/owner/connected-accounts` route returns only account ID, toolkit,
status, and pagination cursor. It strips Composio's credential-bearing account
state. Without the key, that route returns `503`; no other local gateway flow
requires it. To enable writes deliberately, also set
`WAKU_GATEWAY_ENABLE_EXECUTION=1`. Direct tool execution uses
`POST /api/v3.1/tools/execute/{tool_slug}`, requiring **Tool execution
(Legacy): Write only** on that scoped key. The gateway sends only structured
arguments, an explicit connected account, and the reviewed pinned version.
It never uses Composio's natural-language `text` or proxy execution paths.

The owner API supports device listing, proposal listing, one-time invites,
revocation, and exact-fingerprint decisions under `/v1/owner/`. Its bearer
token is separate from paired-device tokens. `/v1/pair` consumes an invite;
paired devices can propose and inspect only their own effects under
`/v1/device/`. Responses disable caching, and the service exposes no CORS
headers. After a separate exact-fingerprint approval, the owner can call
`POST /v1/owner/proposals/{id}/execute` once. The gateway commits `uncertain`
before calling Composio. A confirmed successful response records its log ID;
timeouts, missing confirmations, and queued results stay `uncertain` and need
manual reconciliation. A repeated execute request cannot replay the write.

State version 2 binds tool versions. When reading a version 1 local database,
unclaimed proposals are denied; paired devices remain available. Review any
previously `uncertain` proposal manually.

Run the local policy checks with:

```sh
bun test packages/personal-gateway/src
bun node_modules/typescript/bin/tsc --noEmit -p packages/personal-gateway/tsconfig.json
```

Before remote use, add an owner approval UI, validate TLS operation and real
Composio behavior, and prepare a deployment definition
reviewed against the user's Nebius project and budget. The deployed gateway
must load the Composio key from its own secret store, never from a client
request or model prompt. The current desktop Composio **Test** is a separate
development catalog check and does not configure this gateway.
