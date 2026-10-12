# Speech deployment and physical phone acceptance

## Status on 2026-10-12

The owner has an **iPhone 15** and **Moto G32**. Their OS versions, installed Ion
builds and USB/debug authorization have not been observed. There is **no Nebius
VM/SSH target or HTTPS hostname**. No cloud resource, DNS change, service
startup, microphone capture or physical-device result is claimed.

The installed Windows NeMo-Speech.cpp binary was inspected directly:

| Evidence                           | Observed result                                                                                                     |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| Help, serve help and native doctor | Version 0.1.0; ASR, HTTP and realtime WebSocket enabled                                                             |
| Backend                            | CPU only; CUDA, Metal and Vulkan absent                                                                             |
| Repository                         | `nvidia/nemotron-3.5-asr-streaming-0.6b`                                                                            |
| Model revision                     | `1c8deaecc64b91f034d73e08dd8b64625eb3395d`                                                                          |
| ASR file                           | `nemotron-3.5-asr-streaming-0.6b.q8_0.gguf`                                                                         |
| Complete bytes/hash                | 741,548,352 bytes; `a5c435f294eea8f88ce68dd27b8c3bfea7f777cb2fbba04fcd30eaa555f429ae`, equal to the installed index |
| Native model info                  | ASR architecture/role, runtime compatible, no errors; English/Hindi labels advertised                               |
| New inspection command             | Passed on the real installed binary/index/model; `deployment_host: false`                                           |

Binary/index hashes identify local files; they do **not authenticate the
upstream release**. Verified archive provenance remains an owner gate. Model
metadata/language labels do not prove recognition. The existing ignored
`.waku-cache/speech-smoke.wav` is zero bytes and unsuitable. Available RAM was
low during native compilation, so the model was not loaded. No recognition or
physical device acceptance has been inferred from these checks.

## Prepare a reviewed transfer package now

Node 22 or newer is required. These commands use only Node builtins, install
nothing, and neither download a model nor contact a target. Native inspection
runs only the explicit owner-selected executable's bounded diagnostics; it does
not load the model or open a listener. Keep the installation and selected files
protected from concurrent untrusted writes.

```powershell
node services/speech/deploy.mjs inspect `
  --binary "C:/Users/ksami/AppData/Local/Programs/NeMoSpeech/bin/nemo-speech.exe" `
  --index "C:/Users/ksami/AppData/Local/Programs/NeMoSpeech/share/nemo-speech/model-index.json" `
  --model "C:/Users/ksami/AppData/Local/NeMoSpeech/models/nvidia/nemotron-3.5-asr-streaming-0.6b/1c8deaecc64b91f034d73e08dd8b64625eb3395d/nemotron-3.5-asr-streaming-0.6b.q8_0.gguf"

# Choose a NEW directory whose parent already exists.
node services/speech/deploy.mjs bundle --output "D:/private/ion-speech-package"
node services/speech/deploy.mjs verify --directory "D:/private/ion-speech-package"
```

The package preserves repository-relative paths and contains a strict allowlist
of launch/unit/proxy configurations, checkers and speech documents. It has
`manifest.json` and `SHA256SUMS`. Weights, binaries, real environment files,
tokens, dependencies and private reviews are excluded. Existing output is never
replaced. Retain the printed `manifest_sha256` through a trusted channel and
compare it after transfer. A self-consistent manifest received from an untrusted
party does not establish authenticity. Partial output is never called complete;
review it before removing only that newly selected directory and retrying.

## Install when a Linux target and hostname exist

Follow the [shared speech checklist](41-speech-service-acceptance.md) and
[service README](../../services/speech/README.md). No target is guessed.

1. Select the owner's Linux VM and CPU/CUDA build. Authenticate its release
   archive using published checksums/signatures. Retain a private receipt of
   exact archive SHA, backend, target identity and resource sizing.
2. Install `/opt/ion-speech/bin/nemo-speech`; create the `ion-speech` account and
   model directory. Pull the indexed conversion as that service user using the
   unit's explicit cache. On CUDA, verify NVIDIA driver/runtime and GPU readiness.
3. Run packaged inspection with actual absolute binary/index/model paths. The
   model path must be the service-user pull's real path, including cache folders.
   Add `--backend cuda` only on a working CUDA build. Inspection refuses missing
   reported accelerator/build readiness, but native doctor metadata is not an
   inference or CUDA device execution proof; verify the actual model on target. A Windows binary is not a Linux serving binary.
4. Make `/etc/ion/speech.env` root-owned mode 0600 from the example, with a
   private random token, actual model path and expected checksum from the verified
   index. Do not derive the expected checksum from an unchecked file. Keep token
   values out of arguments/chat. Review launch flags against installed serve help.
5. Install the reviewed launcher root-owned mode 0755 at
   `/opt/ion-speech/ion-speech-launch.sh` and unit under `/etc/systemd/system`.
   Run `systemd-analyze verify` on the actual host. Review effective CPU/RAM/task
   limits, restart, shutdown and reboot. The host limits do not cap GPU VRAM.
   Confirm only `127.0.0.1:8080` listens and that 8080 is inaccessible remotely.
6. Set the owner's HTTPS hostname and Caddy environment; validate the actual
   configuration before reload. Check trusted TLS and direct exact HTTP/socket
   paths. Do not enable token/body/audio logging or TLS verification bypasses.
7. Install isolated acceptance dependencies with the committed frozen lock and
   scripts disabled. Keep the service token only in a private invoking environment:

   ```sh
   bun install --cwd services/speech --frozen-lockfile --ignore-scripts
   node services/speech/accept.mjs check --url https://<owner-hostname>
   ```

8. After no-audio readiness, use owner-reviewed private 2-, 10- and 30-second
   mono PCM16 WAVs with accurate expected text in `accept`. Check two concurrent
   actual-model streams, all final words, acknowledgment and bounded latency;
   retain sanitized output and peak RAM/VRAM. Mock events cannot close this gate.
9. Execute both real-phone reviews below. Node transport checks cannot prove
   native socket TLS/redirect behavior or actual microphone cleanup.

This is a personal direct-integration server; assess upstream NIM guidance
before shared serving. [Serving contract](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/main/docs/server.md),
[install contract](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/main/docs/install.md).

## Install Ion on the two actual phones

See [device setup readiness](52-physical-device-review.md) for local tooling and
connected-device prerequisites. Its `node scripts/mobile-check.mjs check
--dependencies <installed-repository-root>` reports prerequisites only; optional
`--devices` counts authorized phones without printing serials. Custom
paste-input, Ghostty, SecureStore and
audio modules require an Ion native build. Expo Go and browser previews cannot
prove this acceptance. Device OS versions and actual USB permissions stay open.

The current `apps/mobile/eas.json` **preview** profile is an internal build.
Android produces an installable APK. iOS ad hoc builds require the owner's Apple
Developer account and the actual iPhone registered before the build. Account,
signing credentials and UDIDs stay private. [Expo internal distribution](https://docs.expo.dev/build/internal-distribution/).

These are owner setup commands once their build account is ready; none was run:

```sh
cd apps/mobile
# Moto G32: install the resulting preview APK from the authenticated build link.
bunx eas-cli build --platform android --profile preview
# iPhone 15: register the phone, then install its signed preview build.
bunx eas-cli device:create
bunx eas-cli build --platform ios --profile preview
```

Prefer a signed preview with bundled JavaScript for lifecycle review. For local
Android development, install Android SDK/platform tools, enable USB debugging on
Moto G32, accept computer authorization, then `bun run android --device` from
`apps/mobile`. iOS local builds require macOS/Xcode; Windows cannot run them.
Current source identifiers are `sh.waku.mobile`; record the installed Ion
build's exact bytes/source identity rather than changing identifiers for review.

## Retain private observations, never infer passes

Create a NEW private JSON review for the exact source SHA and canonical HTTPS
origin when known. Omitting the origin leaves endpoint-dependent cases open.
All **46 cases** start pending. The CLI does not connect to a device.

```powershell
$ionReviewRevision = git rev-parse HEAD
node services/speech/device-review.mjs init `
  --revision $ionReviewRevision `
  --output "D:/private/ion-speech-devices.json"
node services/speech/device-review.mjs evidence `
  --file "D:/private/sanitized-observation.json"
node services/speech/device-review.mjs audit `
  --file "D:/private/ion-speech-devices.json"
```

After actually executing a case, edit its private record: OS version, installed
APK/IPA SHA-256, UTC observation time, private evidence SHA-256 and listed Boolean
facts. A pass requires **all** facts true; a failure at least one false. Unobserved
facts stay null/pending. A blocked case has a reason (`endpoint_unavailable`,
`device_unavailable`, `build_unavailable`, `credentials_unavailable`,
`fixture_unavailable`, `instrumentation_unavailable`) without claimed evidence.

The bounded schema excludes freeform logs, transcripts, token/secret fields and
URL parameters. Audit prints statuses/digests, never private contents or paths.
Evidence SHA proves equality of retained bytes, **not authenticity of claims**.
Output is explicitly `owner_device_observations` and
`owner_reported_not_automatically_verified`. The owner/reviewer must inspect
retained evidence and actual behavior before accepting it; filled records alone
are not certification. Changed source/build/OS/origin requires a new review,
not relabeling old observations. Keep originals private. POSIX files are mode
0600; on Windows verify the containing directory's ACLs.

## Execute on each phone

Use a reachable paired daemon and editable task draft. Save cloud speech at
**Daemons → Cloud speech**. Keep tokens/test phrases/transcripts out of exports,
logs, screenshots and chat.

| Case                            | Required actual action/observation                                                                                                                                                 |
| ------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Install                         | Verify signed native build, exact source/build bytes and opening Ion.                                                                                                              |
| Readiness/wrong token           | Test works without microphone prompt/audio; wrong token gives sanitized refusal.                                                                                                   |
| Saved/changed origin            | Explicit Test reuses only the saved origin; another requires a new token. Instrument the controlled target to prove old token not delivered.                                       |
| Permission denied               | Deny mic permission; recording refuses, indicator off, draft unchanged. Restore OS permission afterward.                                                                           |
| 1/10/30/60 seconds              | Speak private expected phrase; partial preview, final words and Stop acknowledgment; editable unsent draft. Review automatic 60-second stop too.                                   |
| Hindi/English                   | Actual intended-language expected words, not advertised labels.                                                                                                                    |
| Cancel preparation/finalization | Capture stops, mic indicator off, no late text or replay.                                                                                                                          |
| Task/daemon change              | Capture stops; neither old nor new destination receives late text.                                                                                                                 |
| Background/headset              | Background/lock during capture/finalization and remove headset; capture stops, indicator off, no late text.                                                                        |
| Outage/no replay                | Controlled outage; visible error, no partial insertion, reconnect, re-upload, fallback or destination switch.                                                                      |
| Invalid TLS                     | Disposable token/staging certificate; native refusal, instrumented server sees no Authorization/audio.                                                                             |
| Socket redirect                 | Controlled exact socket path redirects to separate instrumented target; native refusal and no Authorization/audio at target. HTTP-only checks do not prove native socket behavior. |
| Stall                           | Controlled server withholds final/commit; deadline error, usable Cancel, mic off, no partial insertion/replay.                                                                     |
| Secure token storage            | Native SecureStore and no token in URL/logs/exports/task text.                                                                                                                     |

For failure cases use a controlled staging fixture and disposable credentials;
never repoint production secrets or disable TLS trust. Server evidence consists
only of counts and route/auth-delivery flags, never raw Authorization/audio/text.
Missing instrumentation leaves cases blocked rather than guessed.

## Gates

- [x] Real Windows runtime/model metadata and complete model SHA inspected.
- [x] Source-only bundle and strict complete verification implemented.
- [x] Private bounded actual-phone observation template/audit implemented.
- [ ] Owner-authenticated upstream archive/index release provenance.
- [ ] Nebius Linux target/hostname, backend and resource sizing supplied.
- [ ] Actual service/proxy/TLS/effective limits/reboot accepted.
- [ ] Real spoken ASR fixtures and two concurrent actual-model streams accepted.
- [ ] Native Ion installed on both phones; exact OS/build identities recorded.
- [ ] All 46 phone cases executed and retained evidence reviewed.

Tooling tests and native metadata inspection are separate from cloud/recognition
and physical-device evidence. No case passes merely because a script exists or
a mocked server responds.

## Implementation verification

59 Node tests passed: the existing 40 speech acceptance checks with real bounded
loopback HTTP/WebSocket/TLS transports and synthetic recognition events, plus
19 deployment/package/private-observation checks. New cases cover full-file
hashes and bounds, ambiguous/unsafe index metadata, model mismatch before child
execution, private native errors, exact package allowlists, tamper/missing/extra
files and junction refusal, cancellation/no overwrite, pending observation
counts, strict device/build/time/evidence/fact gates and sanitized output.

The real installed native binary/index/model separately passed `deploy inspect`
without model loading. A real source package was generated and verified from
its copied command, with the same manifest digest. Formatting, syntax and
whitespace checks passed. No device was accessed, no physical observation was
marked passed, and no live speech deployment or recognition was tested.
