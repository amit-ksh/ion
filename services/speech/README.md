# Speech runtime deployment

> Speech deployment is superseded by [local Parakeet dictation](../../docs/ai-ide-plan/116-110m-and-serverless-implementation.md). Current desktop and mobile dictation do not use these hosted STT paths.

Historical hosted implementation: the API supplied per-recording account tickets. The instructions below describe that dormant worker and are not required by current local dictation. See [managed implementation and deferred acceptance](../../docs/ai-ide-plan/111-managed-services-app-integration.md).

Status: prepared for a Nebius Linux VM; **not deployed**. No VM, cluster, DNS
name, or access target has been supplied. These files make no cloud changes.

Model: `nvidia/nemotron-3.5-asr-streaming-0.6b`. NeMo-Speech.cpp serves its
verified indexed conversion through authenticated HTTP and WebSocket APIs.
Desktop and native phone microphone dictation now stream PCM audio through
`/v1/audio/transcriptions/realtime`. Partial text is a preview; Stop waits for
final segments and the server's commit acknowledgment. Existing WAV imports
continue to use the HTTP upload path. Local desktop preview remains available.

## Provision and install

1. Create a Nebius Linux VM. Install a verified NeMo-Speech.cpp **server**
   build at `/opt/ion-speech`; CUDA acceleration requires its CUDA build and
   compatible NVIDIA drivers. Confirm `bin/nemo-speech --version` and
   `bin/nemo-speech serve --help`. Follow the upstream
   [install guide](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/main/docs/install.md).
2. Create the `ion-speech` system user and its writable model directory
   `/var/lib/ion-speech/models` (service-user ownership, mode `0750`). Download
   the model as that user with the same HOME and explicit cache directory:

   ```sh
   sudo -u ion-speech env HOME=/var/lib/ion-speech \
     NEMO_SPEECH_MODEL_DIR=/var/lib/ion-speech/models \
     /opt/ion-speech/bin/nemo-speech pull nvidia/nemotron-3.5-asr-streaming-0.6b
   ```

3. Save `speech.env.example` as `/etc/ion/speech.env`, replace its placeholder
   with a randomly generated speech token, and set root ownership and mode
   `0600`. Set `ION_SPEECH_MODEL_PATH` to the actual verified GGUF path from the
   pull. Set `ION_SPEECH_MODEL_SHA256` to the ASR artifact checksum in the
   installed, verified model index (see below). Keep that token private. Do not use the placeholder or the model
   provider's API key. Install `ion-speech-launch.sh` at
   `/opt/ion-speech/ion-speech-launch.sh` with root ownership and mode `0755`,
   and copy `ion-speech.service` to `/etc/systemd/system/`. Its preflight refuses
   an unset/placeholder token, missing binary, missing local model or checksum
   mismatch before the listener starts. Check the installed binary's `serve --help` against the flags
   in the launch script. Existing local GGUF selection avoids restart downloads.
4. Point your DNS hostname to the VM. Install Caddy and merge the provided
   Caddyfile into its configuration. Set `ION_SPEECH_DOMAIN` in Caddy's service
   environment. Permit inbound 80/443 for TLS issuance and HTTPS; keep 8080
   private. Validate with `caddy validate` before restarting Caddy.
5. Review the unit's host CPU/RAM/process limits against the chosen CPU/CUDA
   build and VM. They do not cap GPU device memory or guarantee ASR concurrency.
   Verify the unit with `systemd-analyze verify` on the target Linux host. Reload
   systemd, enable/start `ion-speech`, and check its private journal and local
   `/ready`. Verify the authenticated `/v1/models` inventory contains the
   Nemotron 3.5 model, then upload a short WAV. Check incorrect tokens and
   unauthenticated requests return a failure. Do not log request bodies,
   audio, or Authorization headers.

## Connect the clients

- Desktop: **Harnesses → Cloud speech** (also Settings → Models & Speech).
  Enter the HTTPS root URL and service token; Save, then Test. The OS vault
  holds the credential. Deleting it restores local speech.
- Phone: **Daemons → Cloud speech**. Enter the same URL and token; Test and
  save. The phone stores the token in SecureStore and streams directly to
  the service. Its selected task/daemon still determines where the resulting
  editable text belongs.
- Both clients require an HTTPS root URL. HTTP requests and the desktop
  WebSocket handshake reject redirects. Native phone sockets use the platform's
  TLS and redirect behavior; validate this with your deployed endpoint on both
  Android and iOS. Serve the exact WebSocket path directly, without redirects.
  Tokens are Authorization headers, never URL parameters or subprotocols.
- A failing configured cloud stream is stopped and discarded. Ion does not
  reconnect, replay audio, upload it again, or switch to local speech. The desktop
  permits HTTP/WS only for loopback testing.

No local NeMo install/model download is required for cloud dictation. Without
cloud configuration, the desktop and daemon keep their existing local mode.
Phone recordings are limited to 60 seconds; desktop recordings to 120 seconds.
Phone task/daemon changes, app backgrounding and Cancel stop capture and discard
pending text. Cancel also remains available during finalization. Final text is
editable and does not automatically submit a task. Cloud capture creates no WAV
on either client; phone offline capture still uses a temporary cache WAV.

The phone requires a native build with the installed Expo Audio 57 microphone
stream API; browser dictation is disabled. Test checks the bounded model inventory and an authenticated WebSocket session
with a PCM16 format acknowledgment. It sends no audio and requests no microphone
permission; successful recognition still requires a device check. Deployments must support the
explicit transcription WebSocket route and a trusted TLS certificate.

## Review checklist

- [ ] Supply Nebius VM/SSH target, DNS hostname, and chosen CPU/CUDA server build.
- [ ] Provision VM and verified binary; download model; replace private token.
- [ ] Confirm service survives reboot, uses bounded workers, and 8080 is private.
- [ ] Verify TLS, model inventory, authentication, upload limit, and timeout.
- [ ] Test 1-, 10-, and 30-second clips on desktop, Android and iOS: partial preview,
      all final words, Stop/acknowledgment, and editing before Send.
- [ ] Verify the finalization deadline gives an error for a stalled server; no
      partial result should be inserted as a complete transcript.
- [ ] Verify native TLS rejection of an invalid certificate and unexpected redirects;
      inspect only sanitized metadata, not the token or audio.
- [ ] Test Hindi/English, silence, service outage, Cancel during preparation and
      finalization, switching tasks/daemons, backgrounding, headset disconnect and
      microphone permission refusal.
- [ ] Verify no replay or destination switch after failure, and no leftover microphone
      indicator after Cancel or app backgrounding.
- [ ] Check resource usage and latency with two simultaneous devices.

The upstream calls this server a direct integration/local serving path and
recommends NVIDIA NIM for production deployment. A larger shared installation
should assess NIM and an authenticated HTTP adapter before scaling. See the
[server contract](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/main/docs/server.md)
and [HTTP API](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/main/docs/api.md).

## Streaming readiness checkpoint (2026-10-12)

Desktop Test and phone Test and save now verify both inventory and the direct
transcription socket. They require session creation and a 16 kHz/auto-language
format acknowledgment, then close without PCM or commit. Recording uses its
actual device sample rate; this check does not claim microphone or ASR quality.
The phone cancels an in-flight check on screen exit/backgrounding and refuses to
save its late result. Reusing a saved token is allowed only by an explicit Test
for its exact saved origin. A changed URL requires a new token.

- [ ] Confirm Test fails for inventory-only, missing WebSocket route and rejected
      format, with no microphone prompt/audio upload.
- [ ] Leave/background the phone during Test: no late save or confirmation.
- [ ] Test an existing saved origin without re-entering the token; change origins
      and verify the old secret is never sent to the new service.
- [ ] Complete the deployment, native TLS/redirect and real dictation checks above.

Automated checks cover native loopback inventory/socket acknowledgment, no PCM/
commit, sanitized errors, rejected inventory, cancellation and bounded payloads.
No Nebius speech service was deployed or tested live.

## Stable client identity and model provenance

The launch script supplies the supported `--asr.model.name nemotron-3.5` override.
This keeps the inventory ID and realtime session model compatible with desktop
and phone checks when serving an absolute GGUF path. It is an API label;
recognized text and that label do not establish model provenance.

Get the expected checksum from the **verified installed index**, normally
`/opt/ion-speech/share/nemo-speech/model-index.json` for this prefix, or the
verified source checkout's `models/index.json`. Select the repository
`nvidia/nemotron-3.5-asr-streaming-0.6b` and its `role: asr` file; record its filename,
revision, size and `sha256`. Save that expected `sha256` as
`ION_SPEECH_MODEL_SHA256` in the private service environment. Do not set the
expected checksum by hashing an unchecked file. Preflight uses standard
`sha256sum` to compare the current local model bytes before every start; mismatch
refuses serving. Keep the model protected from concurrent untrusted writes.

The detailed acceptance document records the inspected upstream index revision
and naming chain. A successful service check proves the client-facing protocol;
a valid configured checksum proves equality to the owner-selected artifact.
The trust and correctness of the selected index/conversion remain an owner gate.

## Executable readiness and recognition acceptance

Install the isolated tooling dependencies from the Ion repository. This adds no
mobile/native runtime dependency and runs no package install scripts:

```sh
bun install --cwd services/speech --frozen-lockfile --ignore-scripts
node services/speech/accept.mjs --help
```

Use **Node 22 or newer** to execute the checker. Set `ION_SPEECH_TOKEN` privately
in the invoking environment; keep the token out of arguments, URLs and reports.
No deployment target is inferred and no infrastructure is provisioned.

```sh
# No audio or microphone access: authenticated inventory/format and auth refusal.
node services/speech/accept.mjs check --url https://speech.example.com

# Sends the selected WAV once per requested client, plus a 100 ms Cancel prefix.
# Both fixture files stay local; final/partial transcripts are never printed.
node services/speech/accept.mjs accept --url https://speech.example.com \
  --wav /private/english-10s.wav --expected-text-file /private/english-10s.txt \
  --clients 2 --max-wer 0.25
```

`accept` requires a 2..60 second mono PCM16 WAV by default, 8..96 kHz. The actual
WAV rate is negotiated; use `--max-audio-seconds 120` for the desktop upper bound.
All clients negotiate before simultaneous real-time paced streaming begins.
A partial preview, nonempty final text within the selected word-error tolerance,
and a following commit acknowledgment are required for every client. Cancel is
checked on a separate short prefix. Ctrl+C terminates all work; there is no
reconnect, upload retry, audio replay or alternate endpoint.

HTTP and socket redirects are refused, TLS verification stays enabled, and the
token is sent only as an Authorization header. A missing or incorrect token must
receive an explicit HTTP/socket auth refusal. HTTP/event messages are capped at
64 KiB; the socket transport also enforces fragmented-message limits before JSON
parsing. Total events, pending events, transcripts, fixtures and deadlines have
separate bounds. The checker uses exact pinned `ws` 8.22.0 on Node; running its
network commands through Bun is refused because a runtime shim can change the
receive-limit contract.

Stdout is compact TOON containing check names, status, byte/event counts,
word-error rate and latency. Exit 0 means the requested check passed; 1 means a
check failed/canceled; 2 means invalid configuration. Failure bodies, raw socket
errors, private fixture paths, audio, tokens and recognized text are omitted.
A successful `check` is labeled `readiness_only`; it cannot prove recognition.
A successful `accept` applies only to the selected fixtures, concurrency and
limits. It does not establish mobile microphone/TLS behavior or a capacity SLA.

```sh
# Explicit local endpoint fixture testing only; HTTP is otherwise refused.
node services/speech/accept.mjs check --url http://127.0.0.1:8080 --allow-loopback-http
node --test services/speech/acceptance/acceptance.test.mjs
```

For deployment evidence and client review, see
[the shared speech acceptance checklist](../../docs/ai-ide-plan/41-speech-service-acceptance.md).
No live endpoint, model, token or recognition was tested during implementation.

## Deployment package and the owner's devices

`node services/speech/deploy.mjs` offers bounded native install/model inspection,
source-only `bundle` and complete `verify` checks. No new dependency is needed;
no resource is provisioned or listener started.
`node services/speech/device-review.mjs plan` lists the iPhone 15/Moto G32 cases;
`init`/`audit` keep private owner observations separate from automatic proof. See
[deployment and physical acceptance](../../docs/ai-ide-plan/50-speech-deployment-device-review.md).

On 2026-10-12 the real installed Windows CPU binary and local 741,548,352-byte
GGUF were inspected. The full model SHA matches the installed pinned index. No
model was loaded, microphone used or Nebius service deployed. The owner has
iPhone 15 and Moto G32; actual device/build/TLS/microphone review remains pending.
