# Cloudflare app deployment feasibility

21 October 2026. Research and proposed scope; no provider migration, paid plan
activation or infrastructure mutation was performed by this research.

## Selected scope: static app publishing only

The user subsequently chose **static app publishing only for now**. Implement
multi-file HTML/CSS/JavaScript assets through the existing authenticated Ion
Worker and R2 path. Build or export explicitly on the user's machine, then
upload the output; do not run untrusted build commands in the shared gateway.
Preserve per-site origins, private/public access, immutable releases and revoke /
delete. No new paid Cloudflare subscription is required for this implementation.

Dynamic Workers, backend APIs, SSR/server runtimes, Containers and cloud builds
below are researched **future options**, outside the selected implementation.
Do not enable or advertise those capabilities as part of static publishing.

## Future runtime options

Cloudflare can host Ion user-built websites and backend APIs. Keep the existing
authenticated gateway, R2 objects, isolated site hosts and publishing controls.
Add a Cloudflare execution provider behind the deployment contract. The gateway
and HTML publishing already run on Cloudflare; the remaining migration concerns
build execution, runtime routing, lifecycle and accounting.

If backend hosting is added later, Dynamic Workers is a lower-cost option for
Worker-compatible JavaScript / TypeScript. Containers are the closer replacement
for the planned Nebius Linux app host when full Node, Bun or Python processes are
required. Neither is included in the current static-only change.

## Verified hosting choices

| Application | Cloudflare target | Main limitation |
| --- | --- | --- |
| HTML artifact or compiled SPA | Existing Ion Worker + private R2 publishing, or Workers Static Assets | Preserve authorization, paths, content types and per-site origins. |
| Worker-compatible API / SSR bundle | Dynamic Worker loaded by Ion's trusted gateway | Worker runtime compatibility; no arbitrary Linux commands. |
| Full Node / Bun / Python server | Container behind a Worker and a per-app Durable Object | Build/image integration, resource costs and ephemeral disk. |
| Larger conventional multi-tenant Worker platform | Workers for Platforms | Separate $25/month plan exceeds the current pilot budget. |

Workers Static Assets serves matching assets without a script invocation; direct
asset requests and storage are free. Authentication or `run_worker_first` can
invoke billable Worker code. These free-static guarantees do **not** make Ion's
existing R2-backed authenticated publishing path unmetered. [Static asset billing](https://developers.cloudflare.com/workers/static-assets/billing-and-limitations/)

Workers supports JS, TS, Python and Rust/Wasm. Framework support includes React,
Vue, Svelte, Astro and Next.js; Next.js requires an adapter rather than assuming
a generic Node deployment. Node compatibility includes nonfunctional stub
modules, including `child_process`; package import success is insufficient
compatibility evidence. [Languages](https://developers.cloudflare.com/workers/languages/),
[Workers overview](https://developers.cloudflare.com/workers/),
[Node compatibility](https://developers.cloudflare.com/workers/runtime-apis/nodejs/)

Ordinary Workers have 128 MB memory; paid invocations allow up to five minutes
CPU. Static asset files are limited to 25 MiB each; account script and asset-count
limits also apply. An upload/build validator must check actual bundle sizes and
runtime requirements. [Workers limits](https://developers.cloudflare.com/workers/platform/limits/)

## Dynamic Workers: the lower-cost managed-code option

Current docs explicitly support uploaded user applications and AI-generated
applications in isolated runtime-created Workers. Ion can choose supplied
bindings, observe execution and configure resource limits. This is a documented
alternative to requiring the $25 Workers for Platforms subscription for every
managed-code prototype. [Dynamic Workers](https://developers.cloudflare.com/dynamic-workers/)

Dynamic Workers require Workers Paid. Their requests and CPU share Workers
Standard allowances. The plan includes 1,000 unique daily creations accumulated
over the month; additional worker-days cost $0.002. Stable IDs and code are
essential: `.get()` reuses identity; `.load()` without identity counts each
invocation as a creation. Startup CPU is billed as well as execution CPU.
[Dynamic Workers pricing](https://developers.cloudflare.com/dynamic-workers/pricing/)

Proposed identity: `workspace/site/release-digest`; authorize the workspace and
resolve immutable code before invoking. Do not accept a caller-selected tenant
binding. Give user code only narrow per-app capabilities. Block outbound access
with `globalOutbound: null` by default, or intercept it through an allowlisted
gateway. Provider credentials can be injected by that gateway without exposing
them to user code. [Egress controls](https://developers.cloudflare.com/dynamic-workers/usage/egress-control/)

Dynamic Workers can serve assets sourced at runtime from R2/KV using an asset
binding, fitting the existing object-store design. A Worker request can have
four distinct Dynamic Workers in flight; a Durable Object context can have ten.
These are per-context limits, not a four-app account limit.
[Dynamic assets](https://developers.cloudflare.com/dynamic-workers/usage/static-assets/),
[Dynamic limits](https://developers.cloudflare.com/dynamic-workers/platform/limits/)

Workers for Platforms remains a supported alternative with dispatch namespaces,
user Workers and default untrusted mode. Its separate plan starts at $25/month;
do not activate it under Ion's $10 monthly cap.
[Platform architecture](https://developers.cloudflare.com/cloudflare-for-platforms/workers-for-platforms/how-workers-for-platforms-works/),
[Isolation](https://developers.cloudflare.com/cloudflare-for-platforms/workers-for-platforms/reference/worker-isolation/),
[Platform pricing](https://developers.cloudflare.com/cloudflare-for-platforms/workers-for-platforms/reference/pricing/)

## Containers: broader compatibility, more migration work

Containers is available on Workers Paid and runs containerized languages and
Linux-style applications. Instances start on demand under Worker control; the
documented lifecycle includes inactivity timeout. This removes Ion's need to
start/stop a Nebius VM for app hosting, but does not remove Ion's deployment,
security or spending controls. [Containers overview](https://developers.cloudflare.com/containers/)

Disk is ephemeral after sleep/restart. Snapshots are immutable point-in-time
state, not a continuously durable filesystem; supported snapshots expire after
30 days unless restored. R2 FUSE is available with different performance from
native disk. Store durable data explicitly in R2/D1/external services, and reject
apps expecting a persistent local SQLite file until a supported persistence
path exists. [Container FAQ](https://developers.cloudflare.com/containers/faq/),
[Container limits](https://developers.cloudflare.com/containers/platform/limits/)

Required work for the broader runtime path:

1. Replace Docker/gVisor/XFS host execution with Cloudflare image deployment and
   container lifecycle integration; prove supported image/runtimes first.
2. Use a distinct runtime identity per workspace/app/release; never execute two
   tenants in a shared application process or leak shared operator credentials.
3. Build untrusted sources in isolation. An app container and a build executor
   are different security boundaries; npm install scripts cannot run on Ion's
   trusted controller. Bound network, CPU, memory, disk and build duration.
4. Reuse deployment metadata, immutable revisions, authorization and existing
   public/private/revoke semantics. Keep private authorization ahead of proxying.
5. Add bounded logs, readiness failures, cold starts, restarts, cancellation,
   rollback and terminal cleanup. Apply controlled outbound traffic and scoped
   per-app secrets. [Container outbound controls](https://developers.cloudflare.com/containers/configuration/outbound-traffic/)
6. Replace Nebius rate assumptions and reservations with Cloudflare dimensions;
   enforce request/concurrency/runtime quotas before dispatch and reconcile
   provider costs. Disabling deployment must also prevent runtime reactivation.

## Budget: verified prices versus estimates

Workers Paid has a **$5/month account minimum**, not $5 per app. It includes
10 million requests and 30 million CPU milliseconds; overages are $0.30/million
requests and $0.02/million CPU milliseconds. Other Cloudflare services can still
add charges. [Workers pricing](https://developers.cloudflare.com/workers/platform/pricing/)

Container allowances: 25 GiB-hours memory, 375 vCPU-minutes CPU and 200 GB-hours
disk monthly. Extra rates are $0.0000025/GiB-second memory,
$0.000020/vCPU-second and $0.00000007/GB-second disk. Memory/disk are provisioned
usage; CPU is actual usage. Charges end when the instance sleeps. Worker,
Durable Object, logs and egress charges must also be counted.
[Container pricing](https://developers.cloudflare.com/containers/platform/pricing/)

Illustrative calculations, **not quotes or guarantees**:

- Twenty stable Dynamic Worker apps used every day for 30 days create 600
  worker-days, inside the 1,000 allowance; 50 create 1,500, adding about $1 before
  requests/CPU/storage. Frequent releases create additional identities.
- A `basic` container (1 GiB, 0.25 vCPU, 4 GB disk) active for 20 hours totals
  20 GiB-hours, at most five CPU-hours and 80 GB-hours. These fit the included
  container dimensions if other usage has not consumed them.
- The same container continuously resident for 730 hours incurs about **$6.35
  memory overage alone**, in addition to the $5 plan and disk/CPU/other costs.

A tightly limited, mostly sleeping pilot can plausibly fit $5–$10/month.
Always-on apps cannot be promised within $10. A monthly estimate is not a
provider-enforced hard spending cap; retain Ion admission limits, conservative
reservations, runtime shutoff and a reserve for delayed usage reporting. Include
other existing account services and taxes when setting the available allowance.

## Scope and acceptance

Selected delivery: multi-file static bundles, authenticated deployments,
isolated public/private URLs, release replacement, revoke/delete and quota
errors. Reject backend entrypoints and unsupported bundle types with a clear
static-export requirement. JS/TS server bundles and full Bun/Node/Python
processes are deferred.

Verify two-account separation, malicious paths/archives, dependency build
scripts, denied outbound destinations, secret redaction, cold restart, sleep,
expiry, rollback, cancellation, failed health checks and revoked-site access.
Run live tiny paid checks only within the existing approved limit and reconcile
all started resources. No migration is complete solely because deployment API
calls succeed.
