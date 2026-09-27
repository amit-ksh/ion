# Personal gateway

This package holds the authorization rules, HTTPS service, and owner console
for Milestone 4. It has not been deployed to a Nebius account or tested with
a real Composio connection yet.

`GatewayAuthority` accepts an explicit tool/account/scope allowlist. The
owner console at `/owner` creates single-use pairing invites lasting at most five minutes.
Paired device tokens last at most one day, are stored only as SHA-256 digests,
and can be revoked. A paired device can inspect or propose only according to
its grants. External effects have an immutable review snapshot and fingerprint.
The separate `review` grant permits a paired phone to see pending proposals
and decide an exact fingerprint, but never to execute it. A reviewer cannot
decide its own proposal.
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
requires both `WAKU_GATEWAY_TLS_KEY` and `WAKU_GATEWAY_TLS_CERT`. The private
key file and database directory must be owner-only on Linux. `WAKU_GATEWAY_HOST` and
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

The owner API and `/owner` console support device listing, proposal listing, one-time invites,
revocation, and exact-fingerprint decisions under `/v1/owner/`. Its bearer
token is separate from paired-device tokens. `/v1/pair` consumes an invite;
paired devices can inspect their session, list only allowlisted account
summaries, propose, and inspect only their own effects under `/v1/device/`.
The console keeps the owner token in tab memory and clears it when locked;
never put a token in a URL. Responses disable caching, and the service exposes no CORS
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

## Personal deployment preparation

The owner must first choose a Nebius VM, region, budget, public network policy,
and deletion plan. No VM or billable resource is created by this repository.
Use a trusted certificate for the gateway hostname. The desktop accepts HTTPS
with normal certificate validation, or HTTP only to loopback. The gateway's
local TLS path was smoke tested with a temporary self-signed certificate; a
real public deployment still needs a trusted certificate and network review.

On a Linux host, install Bun, checkout the reviewed source, and create a
dedicated unprivileged user. As that user, create private directories in its
home directory. Replace the example home path and domain with the actual
deployment. Do not paste tokens into the shell command line or check them
into Git.

```sh
umask 077
mkdir -p "$HOME/.local/share/ion-gateway" "$HOME/.config/ion-gateway"
chmod 700 "$HOME/.local/share/ion-gateway" "$HOME/.config/ion-gateway"
openssl rand -hex 32 > "$HOME/.config/ion-gateway/owner-token"
chmod 600 "$HOME/.config/ion-gateway/owner-token"
printf '[]\n' > "$HOME/.config/ion-gateway/allowlist.json"
```

The service environment contains **paths**, never secret values:

```sh
WAKU_GATEWAY_DB=/home/ion-gateway/.local/share/ion-gateway/state.sqlite
WAKU_GATEWAY_ALLOWLIST=/home/ion-gateway/.config/ion-gateway/allowlist.json
WAKU_GATEWAY_OWNER_TOKEN_FILE=/home/ion-gateway/.config/ion-gateway/owner-token
WAKU_GATEWAY_TLS_KEY=/home/ion-gateway/.config/ion-gateway/tls-key.pem
WAKU_GATEWAY_TLS_CERT=/home/ion-gateway/.config/ion-gateway/tls-cert.pem
WAKU_GATEWAY_HOST=0.0.0.0
WAKU_GATEWAY_PORT=8787
WAKU_GATEWAY_ENABLE_EXECUTION=0
```

Run `bun packages/personal-gateway/src/main.ts` under the dedicated user,
with the repository as its working directory. Put the process under a service
manager with automatic restart, read-only source files, and write access only
to the private database directory. An editable
[`systemd` unit example](deploy/ion-gateway.service.example) is included;
check the Bun path, service user, and checkout path before enabling it.
Restrict the firewall to the intended
clients or private network. Verify `https://<your-host>:8787/v1/health`
before opening `/owner`. The owner console asks for the owner token, creates
an invite, and lets you review devices and proposals. In ion, open **Settings
→ Models & Speech → Personal gateway**, enter the HTTPS base URL and one-use
invite, then select **Pair desktop** and **Test pairing**. **Remove pairing**
deletes the local device token; revoke that device in `/owner` as well.
For the later write review, create an invite with **Propose writes** enabled.
In the desktop **Personal gateway** section, open **Propose test action** and
enter the exact allowlisted toolkit, tool slug, pinned version, connected
account ID, target, JSON arguments object, and scopes. **Submit for owner
review** only queues the action. In `/owner`, inspect the full proposal, deny
it or approve its fingerprint, then use the separate **Execute once** control
if appropriate. The desktop's **Check proposal status** reads the result.

When a disposable Composio account is available, put its scoped key in a
private file and set `WAKU_GATEWAY_COMPOSIO_KEY_FILE` to that path. Configure
only exact pinned tools and connected account IDs in the allowlist. The scoped
key needs **Connected accounts: Read only** for listing. Enable
`WAKU_GATEWAY_ENABLE_EXECUTION=1` only for the reviewed write demonstration,
and include **Tool execution (Legacy): Write only** on the scoped key. A real
tool call has not been made yet; keep this setting off until the owner checks
the account, scopes, tool version, and write body in the console.

Stop the service, revoke paired devices, rotate or delete the Composio key,
and remove the VM and its storage when the test is over. Review any
`uncertain` proposal against the external account before making a new one.
The desktop Composio **Test** is a separate catalog check and does not
configure this gateway.
