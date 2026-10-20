# 110M local speech and the serverless gateway

**Latest continuation:** [managed compute and inference implementation](117-managed-compute-and-inference.md) adds the controller, spending enforcement and shared proxy. This document retains the earlier checkpoint details.

Implementation record, 20 October 2026. Branch: `codex/ion-cloud-auth`. This record distinguishes tested source from a deployed service. Existing unrelated changes on `ion-prod` are preserved.

## Implemented

### Local speech

- Active model: NVIDIA `nvidia/parakeet-tdt_ctc-110m`, English, **TDT decoder**, blockwise Q8_0. The auxiliary CTC head is not selected.
- Runtime: existing NeMo-Speech.cpp **0.1.0 Windows x86_64 CPU** install; Ion passes the verified GGUF path and explicit CPU backend. The installer packaging copies the runtime plus Ion's manifest/notices. Users do not need Python or a separate CLI after installation. macOS packaging copies the same notices; macOS execution is not validated here.
- First-use cache: OS cache directory `/ion/speech`, or the operator/development `ION_SPEECH_MODEL_DIR` override. Existing old 0.6B caches are neither selected nor deleted.
- HTTPS download has a 10-second connection limit and a shared 10-minute preparation deadline covering lock wait, verification and download. Cancellation is checked while awaiting network operations, at 100 ms intervals; runtime teardown has a 250 ms bound for OS DNS lookups. An OS resolver call itself may finish later without continuing the canceled model transfer. Redirects are rejected. File length and SHA-256 must match before activation. Failed/canceled/short/oversized/corrupt transfers remove their isolated partial file and do not replace a valid cache entry.
- Preparation is serialized across desktop/daemon processes using an owned file lock. Hashing, transfer and inference run on background workers, never render paths. Progress is based on received bytes and limited to two updates per second, reducing extra native redraws while other tasks stream. Disk errors offer retry guidance.
- Phone cancellation/backgrounding discards stale results; the direct daemon request can still finish its bounded background preparation/inference. Remote job interruption and managed-gateway audio chunk relay are not claimed by the current direct-desktop flow.
- Canceling as preparation finishes now clears the busy state; changing the selected chat while downloading prevents a delayed microphone start in the wrong context. Late progress events cannot overwrite the completed/canceled status.
- Desktop capture is bounded at 120 seconds. Phone capture is bounded at 60 seconds/8 MiB and sent after Stop to the explicitly selected paired desktop. The desktop must stay online. Batch recognition returns editable text; task execution requires the user's Send action.
- Owned subprocesses are killed/reaped on cancellation/timeout. Normal temporary WAVs are removed on every exit; old Ion UUID-named temporary recordings are swept after restart, with a 24-hour threshold to avoid interfering with another process's first-use download.
- Active onboarding/settings/mobile help now says **110M, about 146 MB, English only**. Hosted speech is not advertised by the production gateway.

### Gateway and publishing

- New `packages/ion-cloud-worker` supplies a deployable Workers bundle. One account Durable Object retains the existing device-grant, task, review, automation, portfolio and notification protocol. A platform Durable Object retains tenant-scoped hosting metadata, access capabilities and shared owner permissions.
- Local SQLite and Durable Object SQL share an explicit synchronous transaction interface. Durable Object transactions use `storage.transactionSync()`; cursors are consumed before any await. Local Bun transactions finalize cleanly on the supported Windows runtime.
- Publication/cleanup mutations reject overlapping writes within an account while asynchronous object operations run, preventing deletion from racing a new publication. Different accounts remain independent. A focused test checks the conflict/retry boundary.
- Hosting uses a verified object-store interface. Private R2 replaces filesystem objects in Workers; the self-hosted Bun runtime keeps its filesystem backend. Source digests, tenant ownership, immutable versions and private-by-default publication are retained.
- Account admission checks verified issuer/subject against an approved owner/judge list; public signup configuration still belongs in Auth0. User-controlled role/email hints never grant owner permission.
- Shared Cloud and Token Factory permissions persist and default OFF. Paid-service activation remains unavailable until actual lifecycle/proxy and spending controls are connected; the API rejects attempts to turn an unconfigured service ON and the clients disable those controls. Only an owner can change them; individual field updates preserve the other switch. Desktop and mobile account settings display the state and restrict modification to the owner.
- Cloud OFF blocks new compute deployments/claims, suppresses desired running/building states and blocks dynamic content routing. Static HTML remains available. This does **not** claim that a remote VM has stopped.
- Shared Token Factory permission is stored/exposed; its shared inference proxy and monetary reservation enforcement remain pending. Personal provider keys remain a separate path.
- Workers push handling uses request work plus alarms only while notification jobs remain pending; it does not start a permanent per-account interval. Existing relay HTTP transports are preserved; this does not claim a new WebSocket implementation.
- API hostname and published 32-hex site hostnames can share the pilot domain while retaining different browser origins. API/website names never match a site ID. Published private content uses host-only access cookies; API tokens are never placed in a content URL.

## Exact model provenance

| Item | Value |
| --- | --- |
| Original model revision | `431a349f3051ab85c22b9b7a2741b5fe77065665` |
| Original `.nemo` bytes | `459243520` |
| Original SHA-256 | `1e6b5c3bcaa390b479bd8112512ede515d2ef5b61495568f2c266d6d5e680c3a` |
| NVIDIA converter revision | `caa3f47c7c75fa2e9e6340272ebad57c1647dd44` |
| Runtime decoder / quantization | `tdt` / `q8_0`, blockwise |
| Release filename | `ion-parakeet-110m-tdt-q8-v1.gguf` |
| Release bytes | **146029600** |
| Release SHA-256 | `21ca92b8f0299c6e7d28eb9caa25235bf0a55eaa6511e8e46fbbc27e96900876` |
| First-use URL | `https://api.build-with-ion.tech/models/ion-parakeet-110m-tdt-q8-v1.gguf` — **not published** |
| Local staged release directory | `D:/personal/apps/waku/dist/ion-speech-110m` |

Model conversion uses the pinned NVIDIA `conversion/asr.py`, `source.py` and `quantization.py`, CPU PyTorch, NumPy, sentencepiece, PyYAML, gguf and librosa. The conversion call is `asr.convert(checkpoint, outfile, "tdt", "q8_0", "block")`. librosa is present so the converter emits its actual mel filterbank. The installed build-time package versions are recorded in `resources/speech/conversion-environment.txt`. The isolated conversion environment and original weights remain in the ignored `.waku-cache/parakeet-110m` build cache, not in the app source or installer. Attribution/modification details ship in `resources/speech/NOTICE.md`; weights use CC-BY-4.0.

## Checks completed

- Actual Windows CPU CLI: fixed synthetic 16 kHz mono sentence produced exactly: “Create a new project and run the tests before deploying the application.” Runtime diagnostics explicitly confirmed `head=tdt`, `backend=CPU`, offline/full-context mode. A 1-second PCM silence fixture produced empty output. These are compatibility checks, not microphone/device accuracy or performance benchmarks.
- Final Rust speech suite: **17 passed, 0 failed**; the installed-model smoke case is intentionally excluded from the generic run, then explicitly run with the staged cache/fixtures: **1 passed, 0 failed**. This checks actual 110M transcription and empty-silence handling through the shared adapter. Cache verification timeout/cancel, stalled-network cancellation, corrupt/short/oversized transfer cleanup and child timeout/reaping are covered.
- Gateway regression suite: **88 passed, 1 existing skipped case, 0 failed**, 598 assertions before the final two unavailable-service guard assertions; the focused controls rerun passes all 10 assertions. The existing skipped HTML-source HTTP case remains a review limitation.
- Mobile account/dictation cases: **16 passed**, 80 assertions; mobile typecheck passed.
- Workers, Bun gateway and personal-gateway typechecks passed.
- Final Wrangler **dry run**, with pinned tools: bundle built successfully (275.30 KiB, gzip 61.80 KiB). This never uploaded a Worker or created cloud resources.
- Final real local workerd/SQLite/R2 integration: **2 passed, 0 failed**. Covered: signed native token admission, unapproved account/client rejection, forged identity-header denial, cross-account pairing denial, single-use invites, device authorization after restart, owner-only controls, R2 publication, private/public access, operation idempotency, static serving with Cloud OFF, stop and durable settings after restart all passed.
- Full Windows native desktop/daemon check: **passed**, `cargo check -p waku -p waku-daemon --offline`, with one build job and the shared target cache. Existing unused-code/ts-rs warnings and a dependency future-compatibility warning remain; no native compiler errors. This is compile verification, not visual or installer acceptance.
- No visual app, physical microphone, phone, installer or live cloud acceptance was performed in this increment.

## Operator/deployment checklist

### Gateway/model hosting

- [x] Public DNS on 20 October confirms the assigned `matias`/`nicole` Cloudflare delegation.
- [ ] Configure API DNS/routes and HTTPS: the fresh query for `api.build-with-ion.tech` still returns NXDOMAIN.
- [ ] Authenticate the operator CLI using scoped local credentials (`wrangler whoami` currently reports no authenticated session); confirm Workers/SQLite Durable Objects and private R2 are enabled and their plan limits accepted.
- [ ] Create the private R2 bucket and approve the exact Auth0 owner/judge subjects. Disable public Auth0 signup and check native callback/refresh-token settings. No native client secret is needed.
- [ ] Upload the staged 146 MB model using the supported `wrangler r2 object put --file ... --remote` command in the Worker runbook. The Worker serves only the pinned filename with the correct size (and rejects incorrect SHA metadata if supplied through S3); the client independently verifies the complete hash. The pinned CLI does not offer a custom-metadata upload flag. Until uploaded, its public model route fails with 503 rather than returning another model.
- [ ] Configure API and wildcard publishing routes without intercepting the Ion website/root/`www` traffic. Reserved hostnames need explicit routing/exclusion; no live wildcard route is installed by the source config.
- [ ] Deploy, then verify public model size/hash, authenticated native login, pairing and private/public HTML over real HTTPS. Gate EXE/APK distribution on a working first-use URL.

### App compute / shared inference

- [ ] **Resolve and validate the Nebius lifecycle transport:** Compute exposes gRPC; Workers' `node:http2` is a stub. The Node gRPC SDK is not a direct Worker solution. Use a supported authenticated control path outside the sleeping VM, with verified status and bounded start/stop reconciliation. Do not keep a paid Nebius API VM running to paper over this requirement, and do not add another paid control plane without an accepted budget.
- [ ] Provision the isolated Linux app worker in the supplied Nebius project; verify region, CPU pricing, XFS/Docker quotas, gVisor, private management origin and actual pinned runtime-image digests.
- [ ] Implement/verify wake coalescing, bounded cold starts, active app/build leases, idle stop, maximum runtime, failed operation reconciliation and provider-confirmed stopped state. The current container worker's start/stop loop is not a VM controller.
- [ ] Add per-request/runtime reservations and conservative reconciliation for USD 10 monthly and USD 25 lifetime Cloud envelopes; retain a storage/tax/safety allowance. Provider billing lag prevents an absolute invoice guarantee from an estimate alone.
- [ ] Configure the separate shared Token Factory secret and approved model prices, then enforce per-judge/global token/cost reservations at the proxy. ON permission alone does not make that proxy available.

### Release and review

- [ ] Build final Windows EXE/portable distribution after the public model and gateway are reachable. Verify fresh install without a separate NeMo installation, model cancel/retry/corruption recovery and cached recognition.
- [ ] Rebuild APK with the actual gateway origin; check Moto G32 permissions, desktop pairing, Stop/cancel, account/task switching and backgrounding. iPhone 15 native acceptance needs a compatible iOS build and Apple signing/Mac access.
- [ ] Review real mic input, transcript quality, silence, long utterances, low-end CPU responsiveness and accessibility; no visual review was requested in this increment.
- [x] Pass native desktop/daemon compilation, focused speech tests and the explicit staged 110M runtime smoke check in the implementation worktree.
- [ ] Finish the remaining platform source/runtime and live release gates before merging the candidate into `ion-prod`. Do not overwrite its unrelated pending QA changes.

## Primary implementation references

- [NVIDIA model card](https://huggingface.co/nvidia/parakeet-tdt_ctc-110m).
- [NVIDIA converter at the pinned revision](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/caa3f47c7c75fa2e9e6340272ebad57c1647dd44/conversion/asr.py).
- [Cloudflare SQLite transactions/cursors](https://developers.cloudflare.com/durable-objects/api/sqlite-storage-api/).
- [Cloudflare nonfunctional Node stubs](https://developers.cloudflare.com/workers/runtime-apis/nodejs/#non-functional-stub-modules).
- [Nebius API protocol and authentication](https://github.com/nebius/api), [official Node gRPC SDK](https://github.com/nebius/js-sdk).
