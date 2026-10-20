# Ion isolated app worker

> Deferred by the owner's 21 October 2026 scope change. Ion currently publishes
> only static exports and HTML through Cloudflare Worker/R2. Do not provision
> or start this Nebius app-host path for static publishing. This runbook records
> the earlier server-runtime implementation for possible future use.

Runs reviewed source builds and stateless HTTP runtimes on a **dedicated disposable Linux VM**. Never install it on the user's computer or alongside shared account services.

See [implementation, operator setup and deferred acceptance](../../docs/ai-ide-plan/111-managed-services-app-integration.md).

The shared API now targets Cloudflare Workers; speech stays on the paired
desktop. This container worker does not provision or stop its own Nebius VM.
Production app deployment stays OFF until the implemented operator controller
and verified spending policy are configured. See the controller runbook and plan 117. See [current remaining gates](../../docs/ai-ide-plan/116-110m-and-serverless-implementation.md).

## Requirements

- Docker overlay2 on XFS with project quotas and installed gVisor/runsc.
- Reviewed runtime images pinned by actual SHA-256 digests.
- The provided internal Docker network/firewall, persisted across reboot.
- API HTTPS access and a worker HTTPS origin reachable only by the API host.
- Private state directory, matching operator-only token, explicit resource budget.
- No cloud instance credentials, unrelated data or mounted Docker socket inside app containers.

Public npm and PyPI dependency downloads use a constrained CONNECT proxy. Runtime applications have no unrestricted outbound access. The accepted subset is static frontends and stateless Node/Bun/Python HTTP apps.

## Configure and start

Copy and fill [worker.env.example](deploy/worker.env.example), [service unit](deploy/ion-hosting.service.example) and the private [proxy example](deploy/Caddyfile.example). Review and run [private-network.sh](deploy/private-network.sh) on the dedicated host. These files contain no live hostnames/image digests and were not deployed or exercised.

The service reads its operator environment then runs:

```sh
bun run --cwd services/hosting start
```

Only a local listener is opened. Install trusted private HTTPS termination and firewall it to the API host. Worker token/source/environment payloads must not enter access logs.

Keep until explicitly cleaned: inactive runtime/container source remains for rollback. Remove inactive versions through Ion to release retained slots. Stop prevents routing immediately and stops the runtime when the worker next connects. An offline worker cannot confirm process termination.

Compilation can be checked with:

```sh
bun run --cwd services/hosting check
```

Runtime/security acceptance is deferred. No Docker containers or paid infrastructure were started during implementation.
