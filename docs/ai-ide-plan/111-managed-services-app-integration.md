# Managed Nebius services: app integration and operator deployment

> Speech scope changed: [local Parakeet dictation](114-local-parakeet-dictation.md) replaces the shared STT deployment described below. Gateway and hosting remain in scope.

Implementation branch: `codex/ion-cloud-auth`. Updated **19 October 2026**.

**The API gateway, managed speech broker, HTML publishing and project deployment paths are implemented in source. Runtime, security, model-quality and device acceptance are deferred at the owner's request. No service has been deployed on Nebius, and this increment does not produce an installer or APK.**

## App entry points

| Feature | Desktop | Phone |
| --- | --- | --- |
| Account / gateway | Settings → Models & Speech → Ion Cloud; Remote access onboarding | Daemons → Ion Cloud |
| Shared speech | Existing task microphone; discovered from the signed-in account | Existing task microphone; discovered from the signed-in account |
| HTML publication | Artifacts → Publish HTML → source/visibility review | Open verified HTML artifact → Publish HTML → private/public confirmation |
| Project deployment | Projects → Deploy → review files, runtime, commands and settings | Deployments manages existing releases; source export/build submission starts on desktop |
| Deployment management | Projects → Deployments, or account card → Deployments | Account card → Deployments |
| Lifecycle | Open, Copy link, private/public, versions, rollback, Stop/Unpublish, logs, remove inactive versions | Same lifecycle controls |

Normal managed use requires sign-in and microphone consent, not a speech master key, gateway URL or user-owned Nebius VM. Personal gateway and speech endpoint settings remain explicit overrides.

HTML artifacts remain AI-produced. Publishing uploads the exact verified artifact revision and its hash-bound resource bundle. It does not add a manual artifact upload feature. Document mode disables scripts; the explicit static/interactive choice allows scripts on a separate website origin without Ion permissions.

Project deployment snapshots use Git's tracked and nonignored untracked files. Hidden paths, Git state, dependency/build directories, common credential extensions, links and obvious private keys are excluded/rejected. The review displays the exact included paths and byte count before upload. This is a bounded source export, not a guarantee that application source cannot contain a hardcoded secret; the user reviews the source.

## Implemented service contracts

### Managed gateway

The existing Auth0 account boundary, scoped device grants and outbound desktop/daemon publishers remain in use. Computers link explicitly with a publish grant; phone read/control grants remain separately selected. The API audience is still `https://ion-cloud-api`; it is **not** the deployment hostname.

### Shared Nemotron STT

- Uses the existing NeMo-Speech.cpp Nemotron 3.5 HTTP and transcription-WebSocket protocol. The operator installs and verifies the requested model once.
- Clients discover availability from the pinned bootstrap. A per-recording account request issues a one-use speech ticket; the upstream worker token stays server-side.
- Tickets travel in Authorization headers, never URLs or subprotocols. A speech ticket cannot access gateway or hosting routes.
- One active/reserved session per owner; initial global cap four; 3,600 reserved/metered seconds per owner/day; maximum 120 seconds per desktop session and 60 on phone. At most 120 session requests/hour. Limits are admission policy, not measured capacity.
- Reserved tickets expire after 30 seconds. Active leases, PCM rate/bytes, bounded messages, backpressure and finalization have server limits. Operational records expire after their accounting window.
- The broker forwards confirmed transcript/finalization events. It does not manufacture a final acknowledgment, replay audio, reconnect a failed session or switch the capture destination.
- PCM streams are transient; raw audio and transcripts are not stored in service state or routine logs. Bounded WAV uploads are checked for PCM16/mono/rate/duration.
- Manual personal endpoints retain priority. A failing selected managed capture errors; it does not silently fall back after audio has started.
- Sign-out cancels the active capture in the app.

The NIM protocol is not silently substituted for NeMo-Speech.cpp. A NIM migration requires a separate verified adapter. Real English/Hindi/Hinglish accuracy and shared capacity still need acceptance.

### HTML and static publication

- Owner-scoped immutable manifests and SHA-256 objects on a private persistent disk.
- Source upload and release activation are separate. Failed builds leave the previous release active; late completion cannot override a later Stop/rollback.
- Private by default. Each site has its own host under a separately configured content domain. Account API tokens are never sent to arbitrary HTML.
- Private Open uses a 30-second one-use fragment exchange, then a 15-minute host-only Secure/HttpOnly scoped cookie. It is bound to the owner/site/active release.
- Public access is an explicit choice. Unpublish/Stop immediately removes API routing and invalidates effective private access. Already downloaded copies cannot be recalled.
- Relative assets, MIME types, no-sniff/CSP and an explicit SPA fallback flag are supported. Document scripts/forms are disabled; interactive mode remains isolated.
- Limits: ten sites per account, twenty retained versions/site, five hundred files and 20 MB/export, 8 MB/file, and 200 MB of counted retained source/output per owner.
- Retention follows **keep until explicitly cleaned**. Active versions cannot be cleaned. Removing an inactive version queues worker resource deletion and removes unreferenced control-plane objects.

This pilot uses local private disk rather than S3. Nebius Object Storage does not itself serve websites; the content-serving layer remains necessary if storage is migrated later.

### Project app deployment

- Accepted runtimes: static output, Node, Bun, Python HTTP APIs. Runtime preparation installs declared public lockfile/requirements dependencies even when the optional build command is empty. Build/start commands are reviewed JSON argument arrays; HTTP port and readiness path are explicit.
- Untrusted build/runtime code runs only on a **separate dedicated Linux worker VM** with gVisor. The API and model hosts never run a project build.
- Pinned image digests, nonroot container users, dropped capabilities, no-new-privileges, process/CPU/RAM limits, read-only runtime source, bounded scratch, and quota-limited build layers.
- Build dependency downloads pass through a CONNECT-only registry proxy allowing the public npm/PyPI hosts. The internal Docker network and dedicated firewall rules block general outbound, metadata/control-plane access and peer initiation.
- The build worker requires gVisor plus Docker overlay2 on XFS with project quotas. It refuses incompatible configuration. Verify the actual gVisor/Docker/quota combination on the selected Linux image.
- Build commands have a ten-minute bound. Readiness must succeed before activation. Runtime ports bind only to worker loopback and are proxied through the authorized content route.
- Initial worker caps: one build, four running app containers, twenty retained worker jobs. Explicit inactive-version cleanup frees retained slots.
- Runtime secrets are encrypted with AES-256-GCM and owner/release associated data. They are injected only into the runtime, not builds; they are absent from release API responses and source bundles.
- Runtime logs are owner-read, bounded and redact configured environment values and common token patterns. Do not print secrets from application code; pattern-based redaction is not a guarantee for transformed secrets.
- Durable claims precede effects. Lost completion acknowledgments reconcile receipts; they do not rerun a build. Restarted ambiguous work is retained as uncertain.
- Stop gates public routing immediately and causes worker shutdown on its next connected poll. Rollback switches only application release/runtime state, not an external database.

The accepted pilot does not provision a database, persistent application disk, private package registry, arbitrary Dockerfile, cron worker, multi-service stack, custom domain or unrestricted runtime egress. Native EXE/APK execution in a browser is not provided.

## Operator setup: once for all users

1. Choose the Nebius project/region, explicit pilot budget and approved CPU/GPU resources. Create a CPU API/content host with private persistent disk. Put the hosting worker on a separate disposable VM with no instance credentials or unrelated data.
2. Supply a real API HTTPS hostname and a separate wildcard content domain; ideally use a different registrable domain. Configure authoritative DNS and trusted TLS. Wildcard Caddy certificates require an actual DNS plugin/configuration or an installed wildcard certificate.
3. Complete Auth0 Native callbacks/scopes/rotation from [desktop setup](110-desktop-cloud-login.md) and [phone setup](109-mobile-cloud-login.md).
4. Install the verified NeMo server/model using [speech operator instructions](../../services/speech/README.md), including the model checksum and API label. Keep its listener private; configure `ION_SPEECH_UPSTREAM_ORIGIN` and its private token file in the API.
5. On the worker VM, install Docker/gVisor, quota-enabled XFS/overlay2, pinned reviewed runtime images, and the provided dedicated internal-network/firewall policy. Persist firewall rules across reboot.
6. Configure the API's hosting worker HTTPS origin and matching random worker token. Restrict worker HTTPS ingress to the API host. Put a 32-byte hex hosting-secret encryption key in a private file; back it up separately.
7. Install the provided service units. Set root-owned configuration/secret files to mode 0600, service state to 0700. Review Caddy, image/runtime compatibility, resource limits and backup paths before enabling services.
8. Set `ION_CLOUD_PUBLIC_ORIGIN` consistently for desktop/daemon and `EXPO_PUBLIC_ION_CLOUD_ORIGIN` for the native phone build. Build updated binaries after configuration.

Configuration files:

- [API operator environment](../../packages/ion-cloud/deploy/operator.env.example)
- [API/content proxy](../../packages/ion-cloud/deploy/Caddyfile.example)
- [API service](../../packages/ion-cloud/deploy/ion-cloud.service.example)
- [Worker environment](../../services/hosting/deploy/worker.env.example)
- [Worker service](../../services/hosting/deploy/ion-hosting.service.example)
- [Worker private proxy](../../services/hosting/deploy/Caddyfile.example)
- [Worker network policy](../../services/hosting/deploy/private-network.sh)

No operator credentials belong in app settings, source exports or chat. The examples intentionally contain placeholders. They do not provision or start resources.

## Verification completed in this increment

Only source formatting, Rust compilation and TypeScript checks were run. **No feature, unit, integration, security, visual, model or device test was run after these service changes**, following the owner's request to test later. Earlier account-test counts in documents 109/110 describe their earlier increment, not acceptance of this one.

```powershell
cargo check -p waku -p waku-daemon --offline
bun run --cwd packages/ion-cloud check
bun run --cwd services/hosting check
bun x tsc --noEmit -p apps/mobile/tsconfig.json
```

A passing compile does not prove TLS, gVisor, speech finalization, tenant isolation, dependency installation or production readiness.

## Deferred acceptance checklist

### Configuration and gateway

- [ ] Complete operator DNS/TLS, Auth0 callbacks, model/runtime images, budget and private worker setup.
- [ ] Rebuild desktop and phone with the same real managed origin.
- [ ] Run account/gateway regression suites and two-owner isolation checks.
- [ ] Same account on desktop, Moto G32 and iPhone 15: link/see tasks/review; other account is denied.
- [ ] Test expired/revoked grants, refresh, restart, offline sign-out and unchanged self-hosted mode.

### Speech

- [ ] Authenticated inventory contains the verified model; unauthorized worker and API requests fail.
- [ ] Desktop and phone mic works without a personal speech token.
- [ ] English/Hindi/Hinglish, silence/noise, 1/10/30/60/120 seconds as applicable.
- [ ] Preview/final words and Stop acknowledgment, Cancel/background/sign-out and no replay.
- [ ] Reject expired/reused/foreign tickets, bad format, excessive bytes/duration, quota and concurrent overflow.
- [ ] Disconnect upstream, stall finalization and restart API; verify leases/counters recover.
- [ ] Measure latency and memory at 1/2/4 concurrent streams before accepting the global cap.

### Publication

- [ ] Verified HTML plus nested CSS/images/SVG/fonts; modified source/hash is rejected.
- [ ] Document scripts/forms blocked; interactive scripts isolated from account/desktop APIs.
- [ ] Owner private Open works; copied private base URL and another owner cannot access it.
- [ ] Public link works without login only after explicit promotion.
- [ ] New version, failed upload, duplicate request, rollback and Stop/revoke behave correctly across restart.
- [ ] Active objects survive source cleanup; removing inactive versions reclaims only unreferenced objects.

### Project deployment

- [ ] Plain static site, built frontend with public dependencies and declared SPA routing.
- [ ] Node/Bun/Python stateless HTTP app listens on PORT and passes declared readiness.
- [ ] Encrypted secrets reach runtime only; API responses/builds/logs contain no configured secret.
- [ ] Reject source links, excluded files, oversized exports/output, unsupported images and exhausted caps.
- [ ] Verify gVisor/XFS quotas, Docker firewall/metadata/peer isolation and public-registry-only egress.
- [ ] Build timeout/unhealthy version leaves previous deployment active.
- [ ] Lost claim/receipt, worker/API restart, Stop during build, late completion and repeated cleanup.
- [ ] Test HTTP reads/writes, private/public access, logs, rollback, Stop and retained resource removal.
- [ ] Confirm actual container state, bound ports, disk/memory and Nebius cost before opening the pilot.

### Packaging

- [ ] Run the updated automated suites after these changes.
- [ ] Build a new Windows installer and native phone package.
- [ ] Complete app layout/keyboard/accessibility and real-device review.
- [ ] Do not publish a production-readiness claim until the above acceptance is recorded.
