# Ion serverless gateway

Workers + SQLite Durable Objects + private R2. Reuses the existing account/device/relay and hosting protocols. No speech VM is involved; phone audio goes to the paired desktop's local Parakeet runtime.

**Source and local runtime checked; not deployed.** App VM wake/stop, conservative spending enforcement and shared inference are implemented in source; operator configuration and live acceptance are required. Owner permission switches default OFF and cannot enable an unconfigured service. The public model file is staged locally and must be uploaded before first-use download can succeed.

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

The source deliberately installs **no live routes**. Configure `api.build-with-ion.tech/*` plus the publishing site hosts, preserving the existing root/`www` website routes. A blanket wildcard Worker route without explicit website exclusions can intercept the download website; verify route precedence before enabling it. First-level 32-hex site hostnames are reserved for published documents/apps. Root/`www` and API names cannot become site IDs.

## Publish the exact speech model

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
