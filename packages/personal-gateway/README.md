# Personal gateway authority core

This package holds the local authorization rules for Milestone 4. It is not a
deployed gateway. It has no HTTP listener, Composio calls, or persistent store.

`GatewayAuthority` accepts an explicit tool/account/scope allowlist. A trusted
owner surface creates single-use pairing invites lasting at most five minutes.
Paired device tokens last at most one day, are stored only as SHA-256 digests,
and can be revoked. A paired device can inspect or propose only according to
its grants. External effects have an immutable review snapshot and fingerprint.
The owner must approve that exact fingerprint. Execution changes the proposal
to `uncertain` *before* an external call, so a lost response never triggers a
silent retry. A confirmed result may then be recorded.

Run the local policy checks with:

```sh
bun test packages/personal-gateway/src/authority.test.ts
bun node_modules/typescript/bin/tsc --noEmit -p packages/personal-gateway/tsconfig.json
```

Before remote use, add durable transaction storage for devices and proposals,
an authenticated owner control surface, TLS transport, a restricted Composio
adapter, and a deployment definition reviewed against the user's Nebius project
and budget. In particular, `beginExecution` and the durable `uncertain` state
must commit atomically before the adapter calls Composio. The deployed gateway
must load the Composio key from its own secret store, never from a client
request or model prompt. The current desktop Composio **Test** is a separate
development catalog check and does not configure this gateway.
