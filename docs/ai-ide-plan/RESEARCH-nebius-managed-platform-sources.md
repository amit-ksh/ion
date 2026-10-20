# Nebius feasibility: managed Ion gateway, speech and app publishing

Checked **2026-10-18** against the official Nebius documentation and Kubernetes documentation. This is research and an infrastructure recommendation for an initial **10–50 Ion users**, including user-built websites/APIs and publishing HTML artifacts. No account credentials, cloud provisioning, paid operations or application-code changes were used.

## Findings that affect the product plan

1. **All three workloads can run on Nebius:** Ion's shared API/relay, a shared GPU speech worker, and user applications. Nebius supplies infrastructure; Ion still owns account authorization, per-user isolation, metering, deployment orchestration and the product experience.
2. **Serverless AI is a useful current option.** It runs custom CPU/GPU containers as endpoints and jobs. “Serverless” here does not establish per-request pricing, automatic scale-to-zero, replication or unlimited concurrency. Its documented billing follows allocated Compute resources. See [Serverless overview](https://docs.nebius.com/serverless/overview) and [pricing](https://docs.nebius.com/serverless/pricing-quotas).
3. **Object Storage is not a managed static-site host.** Nebius explicitly lists static website hosting as unsupported. Store HTML/assets there and put a static-serving service in front, or package them in an HTTP container. [S3 API compatibility](https://docs.nebius.com/object-storage/interfaces/s3-api-compatibility).
4. **Nebius DNS is private DNS.** It cannot alone provide public `api.<Ion domain>` or `*.apps.<Ion domain>`. Use the operator's external authoritative DNS provider for those records. [DNS overview](https://docs.nebius.com/vpc/dns/overview).

## What Nebius manages and what Ion must operate

| Component | Verified capability | Ion responsibility / boundary |
|---|---|---|
| Compute VMs | CPU and GPU VMs, including one-L40S presets | Guest OS/runtime, service updates, process health, TLS proxy if direct exposure |
| Managed Kubernetes | Managed control plane, CPU/GPU worker groups, Cilium, node scaling | Application manifests, ingress/gateway controller, certificates, workload policy, app health and rollout |
| Serverless AI endpoints | Custom containers, HTTPS URLs, bearer-token option, secret injection | Ion user auth, tenant authorization, service quotas, image supply chain and data persistence |
| Managed PostgreSQL | Provisioned database clusters, pooling, backups | Schema, tenant-safe queries, role design, restore drills, migrations |
| Object Storage | S3-compatible objects, policies, lifecycle/versioning | Object ownership and authorization, static-site HTTP serving, retention decisions |
| Container Registry | Private image storage and CLI/SDK authentication | Builds, scanning, signing, image immutability, per-app release history |
| SecretStash | Encrypted, versioned central secret storage | Least privilege, rotation, safe injection, avoiding secret exposure in build logs |
| Networking | Private networks, security groups, public/private addresses, Kubernetes load balancing | Explicit deny rules, public ingress design, external public DNS and application authorization |

The detailed source-backed limitations follow. The responsibility column is an architectural recommendation, not a promise of a separate managed Nebius PaaS/API-gateway product.

## Shared gateway and external access

### Direct VM or Kubernetes route

A CPU VM is sufficient as a deployment substrate for a pilot API/relay. A Kubernetes Service of type `LoadBalancer` can obtain a public or internal address; reusable IP allocations can preserve its address across recreation. The documented load-balancer examples are TCP services, so do not assume an included application-level gateway, custom-domain certificate lifecycle or Ion account authentication. [Kubernetes load balancing](https://docs.nebius.com/kubernetes/clusters/load-balancer).

**Recommendation:** own a stable `api.<Ion domain>` entry point. Operate a maintained HTTP reverse proxy/gateway for TLS termination, WebSocket relay, request limits and routing. Put the database and speech service on private networking. Use private addressing for the speech backend, or a private service credential if it must be reached through a managed URL. A public-facing token shared by every installation is not a tenant boundary.

### Serverless endpoint route

Endpoints accept custom images, CPU or GPU resources, private registry configuration and secret variables. Each HTTP port receives managed HTTPS; gRPC uses the managed URL too. Endpoint URLs are backed by Nebius Tunnels. Endpoint disk minimums are 20 GiB for CPU workloads and 50 GiB for GPU workloads, with larger images possibly requiring more. [Endpoint management](https://docs.nebius.com/serverless/endpoints/manage).

Nebius Tunnels explicitly supports HTTP/1.1 WebSocket upgrades, HTTP/2 gRPC and TCP. It establishes outbound TLS from the tunnel agent and does not add application user authentication. [Tunnels overview](https://docs.nebius.com/tunnels/overview).

**Inference:** because current managed endpoint URLs use Tunnels, WebSocket transport is feasible on the documented stack. Still run a compatibility check for Ion's real upgrade request, bearer-token behavior, streaming duration, reconnect and mobile background/resume. The inspected endpoint guide does not document a custom-domain configuration, request-duration guarantee or automatic replica/idle scaling policy. Obtain confirmation before making these product promises.

Current managed-URL limits are **120 Mbps combined inbound/outbound and 300 new connections/second per endpoint**; all endpoints share project/region pools. Tunnels documents **600 Mbps and 1,500 new connections/second per project**, **8 Gbps and 6,000/second per region**. These are shared limits, not reserved throughput; new connections/second is not concurrent sessions. [Endpoint limits](https://docs.nebius.com/serverless/endpoints/manage#traffic-and-connection-limits), [Tunnels limits](https://docs.nebius.com/tunnels/resources/quotas-limits).

**Pilot choice:** direct CPU VM with an operator-controlled domain/proxy is the clearest stable entry point; test Serverless AI as an alternative speech/app runtime. A managed URL can accelerate an internal pilot, but avoid embedding a disposable endpoint address in clients as the permanent public API contract.

## Shared STT compute

The current preset catalog includes `gpu-l40s-a`, `1gpu-8vcpu-32gb`, in `eu-north1`. CPU `cpu-d3` has a `2vcpu-8gb` preset in all listed regions. Region availability in documentation does not guarantee capacity or quota for a particular Ion project. [VM types and presets](https://docs.nebius.com/compute/virtual-machines/types).

**Recommendation:** start with one on-demand speech GPU worker, private to the gateway. Measure actual model/runtime memory, cold start, real-time factor, latency and concurrent streams. Admit streams through per-user and global limits; queue bounded batch uploads separately. Keep speech-session state isolated by user and request. A 10–50-user pilot is not a claim that one GPU supports 50 simultaneous audio streams.

If using Serverless AI, keep the low-latency worker warm during pilot service hours and persist model files independently of endpoint disk. Stopping an endpoint discards its local container disk; mounted storage survives and remains separately billable. Documented stop/start is not automatic request-driven scaling. [Serverless lifecycle/storage comparison](https://docs.nebius.com/serverless/overview), [pricing](https://docs.nebius.com/serverless/pricing-quotas).

**Production route:** multiple worker replicas, health-based admission, draining on deployment and independently scaled CPU ingress. Managed Kubernetes supports node-group autoscaling, but GPU scaling also needs CPU nodes for system networking: the guide recommends at least two CPU nodes or an autoscaling CPU group when a GPU group scales down. Pod count/service concurrency and node scaling are distinct controls. [Kubernetes autoscaling](https://docs.nebius.com/kubernetes/node-groups/autoscaling).

## PostgreSQL, identity, secrets and tenant boundaries

Managed PostgreSQL supports PostgreSQL 16, configurable host counts, public-access settings, connection-pool modes and scheduled daily backups with configurable retention. Keep public access disabled for Ion control-plane data. [Cluster management](https://docs.nebius.com/postgresql/clusters/manage). Validate point-in-time restore behavior and recovery objectives against the current [backup guide](https://docs.nebius.com/postgresql/backups) before promising a recovery SLA.

Nebius IAM organizes infrastructure into tenants/projects and grants users/service accounts permissions through groups. A service account's creation project does not inherently confine its effective access; group permissions determine the scope. [IAM model](https://docs.nebius.com/iam/overview).

**Recommendation:** Ion application accounts should be separate from Nebius infrastructure accounts. End users sign in to Ion and receive per-account/per-device tokens; they should not receive Ion's Nebius operator credentials. Create separate operator service accounts for release/deploy, runtime storage, billing read and cleanup. The [service-account auth guide](https://docs.nebius.com/iam/service-accounts/authentication) distinguishes authorized keys for CLI/Terraform from access keys for AWS-compatible APIs.

SecretStash stores encrypted, versioned secrets and can inject them into Serverless AI workloads. It is region-scoped; CLI/API/Terraform still use the identifier `mysterybox`. [SecretStash overview](https://docs.nebius.com/mysterybox/overview). Its current pricing page returned no usable tariff body during this check; do not quote an unverified permanent free tier or SLA.

The [registry auth guide](https://docs.nebius.com/container-registry/authentication) documents service accounts, short-lived access tokens and service-specific static keys. Prefer short-lived registry access in a trusted image-promotion step. Do not give a user-controlled build a global registry push credential.

Nebius security groups filter VM ingress/egress. Default groups allow all ingress and egress, and link-local `169.254.0.0/16` traffic is not evaluated by these groups. [Security group behavior](https://docs.nebius.com/vpc/security-groups/overview). Therefore “private IP” and “a security group exists” are insufficient evidence that malicious builds cannot reach credentials or internal services. Validate explicit workload, metadata and egress isolation in the actual runtime.

## Deploying user-built websites/APIs and HTML artifacts

### HTML artifact publishing

**Recommended implementation:** upload a selected artifact's immutable files to private Object Storage under owner/project/release keys. Serve them through an Ion static publisher on a separate untrusted-content origin. Add content types, asset resolution, SPA fallback only when configured, cache validators, size limits, unpublish and release rollback. Private previews require authorization; public publishing requires an explicit visibility decision. Do not share Ion login cookies with arbitrary HTML/JavaScript.

This is feasible using S3 object reads, but is **Ion functionality**. Nebius Object Storage supports `GetObject`/`PutObject`, bucket policies and CORS; its S3 compatibility guide explicitly excludes static website hosting, per-object ACLs and S3 event notifications. [S3 compatibility](https://docs.nebius.com/object-storage/interfaces/s3-api-compatibility).

### Containerized websites and APIs

**Pilot recommendation:** accept a constrained stateless HTTP app contract: build command, output/container image, listening port, health path and named runtime secrets. Build from an exact snapshot, then publish an immutable image digest to an isolated app runtime. Use release records, bounded resources, logs, readiness checks and rollback. Keep state in declared external services; do not promise endpoint-local persistence.

Serverless AI endpoints can host custom HTTP containers and provide managed URLs; they are not a source-to-production builder, managed per-user platform or demonstrated one-small-container-per-fractional-VM billing model. A separate endpoint allocates its chosen Compute preset. A fleet of mostly idle apps can therefore cost substantially more than a shared CPU node pool. [Endpoint guide](https://docs.nebius.com/serverless/endpoints/manage), [endpoint charging](https://docs.nebius.com/serverless/pricing-quotas).

**Production recommendation:** keep untrusted builds and apps away from the Ion gateway/database and speech credentials. Use ephemeral per-build VMs or an independently validated sandbox runtime; initially constrain app publishing to a reviewed pilot cohort. Kubernetes namespaces and ordinary containers alone are not a strong hostile-code boundary. Kubernetes documents weaker container isolation than VMs and recommends additional sandboxing for untrusted code. [Kubernetes multi-tenancy](https://kubernetes.io/docs/concepts/security/multi-tenancy/#sandboxing-containers).

A Nebius-managed customer cluster being VM-isolated from another Nebius customer does not establish isolation between Ion users sharing that same cluster. Separate projects/networks reduce blast radius but still require correctly scoped credentials and routes. [Nebius network isolation](https://docs.nebius.com/vpc/networking/isolation).

## Current price examples and budgeting

Prices were checked on **2026-10-18**, **USD excluding tax**, using **730 hours** only as a monthly illustration. These are resource calculations, not an Ion plan quotation, capacity reservation or model benchmark. Recheck actual project prices before provisioning.

| Resource example | Rate / calculation | 730-hour illustration |
|---|---|---|
| L40S Intel 1 GPU + 8 vCPU + 32 GiB RAM | `$1.35 + 8 × $0.012 + 32 × $0.0032 = $1.5484/hour` | `$1,130.33`, excluding disk |
| AMD CPU 2 vCPU + 8 GiB RAM | `2 × $0.015 + 8 × $0.0045 = $0.066/hour` | `$48.18`, excluding disk |
| Network SSD disk, 50 GiB | `50 × $0.071/GiB/730h` | `$3.55` |
| Managed PostgreSQL 2 vCPU + 8 GiB, per host | `2 × $0.034 + 8 × $0.009 = $0.14/hour` | `$102.20`, plus storage and any additional hosts |
| Standard Object Storage, 100 GiB | `$0.0147/GiB/730h` | `$1.47`, plus operations/egress |

Compute component rates and second-based metering: [Compute pricing](https://docs.nebius.com/compute/resources/pricing). PostgreSQL rates and per-host/disk charging: [PostgreSQL pricing](https://docs.nebius.com/postgresql/resources/pricing). Object Storage Standard also charges `$0.0034/1,000` class-A object requests and `$0.00028/1,000` class-B requests; internet/cross-region Standard egress is `$0.015/GiB`, same-region egress is free. [Object Storage pricing](https://docs.nebius.com/object-storage/resources/pricing).

For a continuously warm single-L40S pilot, the GPU is the dominant baseline; shared gateway CPU and database costs are additional. Do not offer unlimited free dictation/deployment before measured per-user costs and quotas are defined. On-demand speech capacity is preferable to interruptible capacity for a latency-sensitive default service. GPU spot prices changed in October 2026; do not reuse an old fixed-H100 discount as a current price.

Managed Kubernetes advertises no separate service fee, while worker compute/storage is charged. [Public price list](https://nebius.com/prices), [Kubernetes charging](https://docs.nebius.com/kubernetes/resources/pricing). Registry, SecretStash and Tunnels pricing pages did not expose useful current tariff text to this research tool; verify account/contract tariffs before finalizing a total.

Nebius budgets are delayed alerting, **not spending caps**; usage updates every few hours and a budget does not stop resources. Ion needs its own admission reservations, per-user quotas, per-app runtime limits and kill/cleanup controls. [Budgets](https://docs.nebius.com/signup-billing/budgets). Labels can help attribute infrastructure costs, but shared service user-level metering still belongs in Ion. [Cost allocation labels](https://docs.nebius.com/signup-billing/usage/cost-allocation-labels).

## Practical rollout and go/no-go checks

### Pilot: 10–50 invited users

1. Choose one region using actual latency, GPU capacity and required managed-service availability; `eu-north1` is a documented candidate for L40S and PostgreSQL, not an automatic selection for every geography.
2. Establish operator-owned public DNS/TLS and a stable Ion API address; provision one small CPU gateway, private managed PostgreSQL, private buckets and narrowly scoped service accounts.
3. Deploy one private STT worker; validate real model/runtime GPU compatibility, mobile codecs and streaming behavior before opening access.
4. Add per-user quotas, session limits, support diagnostics, backup/restore and feature kill switches before distributing the default endpoint in clients.
5. Enable HTML publishing first, then a limited stateless app/API runtime for the invited cohort, with builds executed separately from control-plane hosts.
6. Record real utilization, peak simultaneous speech sessions, ingress traffic, app idle/runtime hours, support failures and cost per active user. Scale only against those observations.

### Production gate

- Demonstrated two-user negative authorization tests across relay, speech, artifacts, secrets, deployment and logs.
- No long-lived infrastructure secret in desktop/mobile binaries, user source snapshots, static HTML or public containers.
- Tested relay reconnect/failover and speech draining; multiple replicas and database HA appropriate to the advertised availability objective.
- Strong untrusted-build/runtime isolation with metadata access and network egress tested; no broad Docker socket, host mounts or operator service account inside app builds.
- Enforced cost limits independent of delayed provider billing; confirmed quotas and capacity. Managed Kubernetes consumes Compute/IP quotas, and some regions have zero default cluster quota. [Kubernetes quotas](https://docs.nebius.com/kubernetes/resources/quotas-limits).
- Public-domain/TLS ownership and custom app-domain design verified. The endpoint API inspected here does not establish built-in custom-domain automation.
- Managed URL WebSocket/gRPC streaming test including auth, close codes, mobile resume and long stream; confirm undocumented timeouts with Nebius.
- Current account tariff and required regional service/preset availability verified. Public examples are planning inputs, not procurement commitments.

## Documentation caveats

Some current pages disagree in detail: PostgreSQL pricing lists AMD availability while the cluster guide still describes `cpu-e2` as the only platform. List presets against the selected project and confirm the actual console/API contract during deployment preparation. The empty tariff bodies noted above must not be interpreted as zero cost. No verified Nebius product claim here replaces Ion's own multi-user control plane, identity or deployment security work.
