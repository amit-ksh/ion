# Ion Cloud: shared gateway and hosting

This package starts the shared platform described in
[plan 107](../../docs/ai-ide-plan/107-managed-nebius-services-and-deployments.md).
The managed service and app integrations are implemented in source; runtime acceptance is deferred. This is not a deployed Nebius service. See [implementation and deferred checklist 111](../../docs/ai-ide-plan/111-managed-services-app-integration.md).

## Current pilot deployment

Use the [Workers runbook](../ion-cloud-worker/README.md) for the serverless
pilot. This package supplies the shared protocol and self-hosted Bun runtime;
its local SQLite backend is replaced by SQLite Durable Objects and private R2
in Workers. Speech now runs on the paired desktop using Parakeet TDT-CTC 110M,
not on a hosted GPU. See [current implementation and remaining gates](../../docs/ai-ide-plan/116-110m-and-serverless-implementation.md).

Owner/judge subject admission is closed by default. Shared paid-service
activation is unavailable until lifecycle/proxy and spending controls are
connected. A source bundle or ON permission is not evidence of a running VM or
working shared inference service.

## Implemented

- Auth0 RS256 access-token verification against a pinned issuer, audience and
  native client allowlist. Remote JWKS keys are cached with bounded refresh.
- Required API scope `ion:gateway`; rejection of foreign clients, ID-token
  audiences, machine credentials, unsigned/expired/future/tampered tokens.
- Stable personal workspace identity from SHA-256 of JSON `[issuer, subject]`.
  Email, request bodies and caller-supplied workspace headers cannot choose it.
- Account-isolated device pairing, revocation, tasks, commands, approvals,
  routines, workflow projections and document relay through the existing gateway.
  Each account has its own SQLite database, including every relay table/cursor.
- Persistent pilot workspace admission cap (default 64), one listener, graceful
  request draining and separate device-grant enforcement.
- Public bootstrap reports configured speech/publishing/deployment capabilities.
  Missing operator services stay disabled. Source implementation does not prove readiness.

No authentication database, password handler or Auth0 client secret is needed.
The databases here store application state and device grants.

## Operator configuration

The public client configuration in `src/config.ts` is:

| Field             | Value                                |
| ----------------- | ------------------------------------ |
| Issuer            | `https://dev-amit-ksh.au.auth0.com/` |
| API audience      | `https://ion-cloud-api`              |
| Desktop client ID | `M7iCZjpo5bzOVO2E5nrvKzDJ00p3ZWy0`   |
| Mobile client ID  | `bZ4cOhwaVD4N9A1g882surgTm2kHaSyq`   |

The audience is an identifier, **not the public hostname of the deployed API**.
OIDC discovery was reachable and matched this issuer on 2026-10-19. This does
not verify the API registration, client types, connections, grants or callbacks.

Before real login:

1. Confirm both applications are Native public clients. Enable Authorization
   Code and Refresh Token grants; never ship a client secret.
2. Confirm API identifier `https://ion-cloud-api`, RS256 signing, Allow Offline
   Access and the `ion:gateway` permission. Grant/request that scope.
3. Enable the intended email login connection on both applications. A preferred
   email login does not specify whether the owner wants password or passwordless.
4. Register the desktop callback `http://127.0.0.1:43891/ion/callback` and mobile
   callback `ion://auth/callback` in their respective Native clients. The mobile
   app also retains `waku` as a compatibility scheme. These source settings are
   not evidence of Auth0 dashboard registration. No browser logout URL is used.
5. Configure rotating refresh tokens and an API access-token lifetime (proposed
   15 minutes). JWT verification alone cannot immediately revoke a valid account
   access token; device revocation remains a separate control.

See [Auth0 API token validation](https://auth0.com/docs/secure/tokens/access-tokens/validate-access-tokens)
and [Auth0 native PKCE](https://auth0.com/docs/get-started/authentication-and-authorization-flow/authorization-code-flow-with-pkce/call-your-api-using-the-authorization-code-flow-with-pkce).

## Run and check

From the worktree root, install with your normal workspace dependency setup,
then:

```powershell
$env:ION_CLOUD_PUBLIC_ORIGIN = "https://YOUR-ACTUAL-CLOUD-HOSTNAME"
$env:ION_CLOUD_STATE_DIRECTORY = "$PWD/.waku-cache/ion-cloud-state"
$env:ION_CLOUD_PORT = "8790"
bun run --cwd packages/ion-cloud start
```

The process binds **only** to `127.0.0.1:8790`. Put the HTTPS proxy on the same
host. Do not expose a plain HTTP listener through a tunnel or rely on forwarded
identity headers. The process does not read `.env` secrets from a user's app.

```powershell
Invoke-RestMethod http://127.0.0.1:8790/v1/bootstrap
bun test packages/ion-cloud/src packages/ion-cloud-client/src
bun test packages/personal-gateway/src
bun run --cwd packages/ion-cloud check
bun run --cwd packages/ion-cloud-client check
bun run --cwd packages/personal-gateway check
```

The bootstrap contains public Auth0 metadata only. Account endpoints require a
real API access token. Never put tokens into a command line, chat, URL or logs.

## HTTP contract

- `GET /v1/health`: readiness of this API process only, not upstream speech.
- `GET /v1/bootstrap`: public login configuration, gateway URL, feature flags.
- `GET /v1/session`: verified personal `workspaceId`.
- `GET /v1/gateway/v1/health`: authenticated relay capability discovery.
- `/v1/gateway/v1/owner/*`: account owner's gateway management endpoints.
- `/v1/gateway/v1/pair`: consumes an invite within the signed-in account.
- `/v1/gateway/v1/device/*`: paired-device relay endpoints.

Every gateway call uses `Authorization: Bearer <Auth0 API access token>`.
Device endpoints additionally require `X-Waku-Device-Id` and
`X-Ion-Device-Token`. Device credentials alone cannot access the managed API.
The owner token used by the existing personal handler is generated inside the
server and never returned. The user cannot select another workspace in a URL.

The existing personal gateway contract stays available for self-hosted users.
Native phone and desktop publishers send the managed contract for cloud pairings. Desktop task and daemon automation/document publishers recheck the saved account and computer grant before dispatch.

## Pilot deployment boundary

One process on one CPU host with a **private persistent disk** is supported.
The optional systemd/Caddy examples do not provision resources or configure DNS.
On Linux the state directory/files must be private to the service user; keep
Windows test directories within the user's protected profile/workspace.

Do not horizontally replicate this SQLite layout or put it on shared object
storage. Back up all workspace databases consistently; restore the same
issuer/subject mapping. A larger/multi-host deployment needs a tenant-aware
application store and admission coordination. The 64-account cap is an initial
admission guard, not a speech, request-rate, storage or spending quota.

No operator-wide Composio account discovery or execution is wired into managed
tenants. Per-user OAuth/account ownership must be added first. The standalone
personal gateway's owner-configured Composio path is unaffected.

## Runtime acceptance and operator deployment

- Native account integration is implemented: [desktop setup and acceptance](../../docs/ai-ide-plan/110-desktop-cloud-login.md) and [phone setup and acceptance](../../docs/ai-ide-plan/109-mobile-cloud-login.md). Live acceptance requires the hostname and Auth0 callbacks.
The shared STT broker, HTML publication, isolated builds and selected stateless HTTP runtimes are now implemented. Configure and accept them using [implementation 111](../../docs/ai-ide-plan/111-managed-services-app-integration.md). All testing of this service increment is deferred at the owner's request. No live resources, new installer or APK were created.
