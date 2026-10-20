# Ion serverless gateway

App publishing currently accepts only prebuilt static exports and HTML documents.
No Nebius VM, controller or hosting-worker token is required for these releases.
Build locally, review the output, then upload it through the authenticated
hosting API. Server runtimes, cloud build/start commands and runtime secrets
are rejected; old runtime releases cannot be activated or served. Bootstrap
`appDeployment` means static app publishing is configured when R2 is bound.

Workers + SQLite Durable Objects + private R2. Reuses the existing account/device/relay and hosting protocols. No speech VM is involved; phone audio goes to the paired desktop's local Parakeet runtime.

**Private R2 storage and model download deployed; live publishing/compute acceptance pending.** See [R2 migration](../../docs/ai-ide-plan/128-r2-speech-and-publishing.md). Owner permission switches default OFF and cannot enable an unconfigured service. The speech model is distributed by Ion from private R2 storage; the earlier Hugging Face revision remains provenance. The native downloader and CPU runtime have passed real acceptance against that identical artifact.

## Local checks

Requires installed workspace dependencies and Node 22+. From the repository root:

```sh
bun run --cwd packages/ion-cloud-worker check
bun run --cwd packages/ion-cloud-worker build
bun run --cwd packages/ion-cloud-worker test
```

`build` is a Wrangler dry run. `test` uses local workerd, persistent temporary SQLite/R2 and fixture JWT keys; it does not contact Auth0, Nebius, Expo Push or a paid account. The SQLite shim rejects accidental local file opening in Workers. `dev` starts the local Worker only when explicitly requested; it is separate from the app dev watcher.

## Deployment inputs

- Cloudflare zone/account operator access; SQLite Durable Objects and R2 enabled.
- Private bucket `ion-private-objects`, bound as `OBJECTS`; leave its public access disabled.
- `PUBLIC_ORIGIN` and `CONTENT_DOMAIN` are public values in `wrangler.jsonc`.
- `MEMBERS_JSON`: **secret** containing approved Auth0 subjects with `owner` or `judge` roles. An absent/empty list admits nobody. Set via `wrangler secret put MEMBERS_JSON`; never commit account passwords or private operator files.
- Optional existing isolated worker configuration: `HOSTING_WORKER_ORIGIN`, private `HOSTING_WORKER_TOKEN` and 64-hex `HOSTING_SECRETS_KEY`. This connects the existing app worker; it does not supply a VM lifecycle controller.

Create the bucket with `wrangler r2 bucket create ion-private-objects`. Store the approved list with `wrangler secret put MEMBERS_JSON`. Review the configuration and set DNS/routes, then `bun run --cwd packages/ion-cloud-worker deploy`. These are operator instructions, not actions already performed.

The full configuration uses the `api.build-with-ion.tech` custom domain plus a proxied `*.build-with-ion.tech/*` publishing route. The zone has an explicit `www.build-with-ion.tech/*` route with **no script**, installed before the wildcard, so the website is excluded. The wildcard does not match the root domain. Preserve this exclusion and add exact routes/exclusions before assigning other website subdomains. The gateway-only fallback reserves only the API custom domain. First-level 32-hex site hostnames are reserved for published documents/apps; all other hosts are rejected by the Worker. Root/`www` and API names cannot become site IDs.

### Pilot without R2 activation

`bun run --cwd packages/ion-cloud-worker deploy:gateway` uses
`wrangler.gateway.jsonc`. It deploys the same Worker and SQLite Durable Objects
without an R2 binding. Account login, device pairing, relays and configured shared
inference remain available. Publishing and speech binaries return 503; bootstrap
does not advertise HTML or app publishing. Compute cannot be enabled without
storage. Paid-service switches remain owner-controlled and default OFF.

After R2 activation, create/bind the private bucket and use the full configuration.
The script name, Durable Object classes, IDs and migration tag stay the same, so
adding storage does not replace account or spending records. Verify the model
and site routes before claiming first-use downloads or publishing work.

## Speech model distribution through private R2

The active native manifest downloads from Ion's HTTPS `/models/` endpoint. The
Worker streams the exact object from the private bucket; public `r2.dev` access
is disabled. Users need neither a Hugging Face account nor an R2 key. Speech
inference remains local on the desktop; this endpoint distributes model weights.

### Versioned model upload

The staged release directory is `D:/personal/apps/waku/dist/ion-speech-110m` on the current build machine. Verify the file against `resources/speech/parakeet-110m.json` before upload.

The pinned Wrangler CLI has **no custom-metadata upload flag**. Upload the exact checked file with the supported command:

```sh
wrangler r2 object put ion-private-objects/models/ion-parakeet-110m-tdt-q8-v1.gguf --file "D:/personal/apps/waku/dist/ion-speech-110m/ion-parakeet-110m-tdt-q8-v1.gguf" --content-type application/octet-stream --remote
```

The Worker checks the versioned key and exact size, rejects an incorrect SHA metadata value if one was supplied through S3, and streams the file without buffering it in Worker memory. **The client independently computes and verifies the full SHA-256 before activation.** The `X-Expected-SHA256` response header is the pinned expected digest, not a claim that the Worker rehashed the streamed body.

GET/HEAD `/models/ion-parakeet-110m-tdt-q8-v1.json` exposes provenance, attribution, license, changes and the exact expected digest. The binary response links to that public manifest and license. No public R2 bucket URL is needed. A missing file or incorrect size returns 503.

Confirm a real HTTPS GET returns exactly **146029600 bytes** and the expected SHA-256 before distributing installers. The manifest, CC-BY-4.0 attribution and conversion details are in `resources/speech/`.

## Limits and remaining work

- Persistent state remains scoped to verified accounts; device permissions are an additional authorization layer.
- Notification alarms stop when pending jobs reach terminal states. This preserves HTTP relays; a new WebSocket transport is not implemented.
- Cloud OFF blocks compute dispatch/routing and the separate controller requests provider stop; only provider-confirmed STOPPED settles a known session. Shared inference uses durable usage reservations and stays OFF until configured.
- Nebius Compute uses gRPC. Workers' `node:http2` is a nonfunctional stub; the Node gRPC SDK cannot simply be bundled here. The separate Linux operator controller uses the official gRPC SDK over a pull protocol. It must stay online; live SDK/provider/isolation acceptance remains a release gate.

See [implementation evidence and owner checklist](../../docs/ai-ide-plan/116-110m-and-serverless-implementation.md).

For policy examples, operator secrets and the latest verification/release checklist, see [plan 117](../../docs/ai-ide-plan/117-managed-compute-and-inference.md) and the [controller runbook](../../services/compute-controller/README.md).

## Managed app connections

`COMPOSIO_PROJECT_KEY` is an encrypted Worker secret, never a desktop/mobile key.
`/v1/apps/catalog`, `/connect` and `/disconnect` require the existing approved
Auth0 identity. Sessions use `ion_<verified-workspace-hash>`; caller-selected user
IDs/session IDs/credential overrides are rejected. Only account-checked summaries
and the user's Connect Link leave this boundary. Instant Tools, raw proxy calls
and Composio sandbox execution remain disabled. This is separate from the planned
Nebius Token Factory sandbox service.

Native Apps search/pagination/connect/revoke and connected app mentions use this
backend unless the user has retained an optional personal Composio project key.
Onboarding no longer requires that personal key. Account changes invalidate cached
app/grant data and queued operations remain pinned to the original workspace.

The app connection foundation is deployed; real app OAuth and read-only tool
execution acceptance still require a user's app authorization. Managed action
execution is a separate pending milestone, not implied by a successful catalog.

The shared spending ledger now enforces the [combined pilot envelope](../../docs/ai-ide-plan/121-combined-spending-envelope.md).
