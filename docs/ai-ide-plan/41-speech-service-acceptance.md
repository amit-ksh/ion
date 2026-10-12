# Shared speech service acceptance

Status: implementation and local transport checks prepared. **No Nebius target,
DNS name or credential has been supplied; no service was deployed or real speech
recognized.** The remaining client/deployment checks below are open.

## Scope and user experience

The same authenticated Nemotron 3.5 ASR endpoint serves native desktop and phone
PCM16 dictation. A user with a configured service needs no local NeMo installation
or downloaded model to use cloud microphone dictation. Desktop local speech stays
available when cloud speech is unconfigured. The phone needs its native build and
microphone permission. Dictation produces editable task text; it does not send the
task automatically.

Client Test checks inventory and socket format without audio. The new executable
service checker adds explicit auth refusal and a separate opt-in recorded-fixture
acceptance pass. It supports the owner's configured endpoint; it does not select
or create cloud resources, run on a presumed VM, or read a key from chat.

## Implemented files

- `services/speech/accept.mjs`: identification fast path, signal cancellation and
  sanitized CLI output; Node 22+.
- `services/speech/acceptance/`: strict flags/token/origin validation, private
  bounded fixture parsing, native HTTP and pinned WebSocket transport, protocol
  ordering, shared deadlines, concurrent recognition and Cancel acknowledgment.
- `services/speech/package.json` and `bun.lock`: isolated tooling package with
  exact `ws` 8.22.0; root/native/mobile dependencies are unchanged.
- `services/speech/ion-speech-launch.sh`: token/binary/local-model/checksum preflight,
  fixed loopback serving and supported native CLI flags; no token argument.
- `services/speech/ion-speech.service`: preflight-before-listener, bounded restart,
  process-group shutdown, host CPU/RAM/process limits and filesystem restrictions.
- `services/speech/speech.env.example`: intentionally unusable private token/model
  placeholders; the owner must select a verified local GGUF and expected SHA-256
  before starting.

## Run from an owner-controlled shell

Install the isolated tool with its lockfile:

```sh
bun install --cwd services/speech --frozen-lockfile --ignore-scripts
node services/speech/accept.mjs --help
```

Set `ION_SPEECH_TOKEN` privately in the command's environment. It must be the
speech service token, separate from Nebius Token Factory and IAM credentials.
The checker never accepts a token argument, query parameter or subprotocol.
Choose an exact HTTPS root origin and files that the owner consents to send.

```sh
node services/speech/accept.mjs check --url https://speech.example.com
node services/speech/accept.mjs accept --url https://speech.example.com \
  --wav /private/english-10s.wav --expected-text-file /private/english-10s.txt \
  --clients 2 --max-wer 0.25
```

`check` sends **no audio**. It validates readiness, a Nemotron 3.5 transcription
inventory item, missing/wrong-token refusal on HTTP and the explicit socket,
PCM16 session creation and the sample-rate/language acknowledgment. The health
route is intentionally unauthenticated; its success cannot stand in for API auth.

`accept` first validates both private files before contacting the endpoint, then
runs `check`. It opens 1..4 requested clients (default 2), waits until **all**
acknowledge the actual WAV rate, and streams the WAV simultaneously at real-time
pace. Each connection receives the WAV once. It checks at least one nonempty
partial, nonempty final segments, no unfinished partial at Stop, exact completion
of the sent bytes, expected-text word-error tolerance, and acknowledgment after
commit. A separate connection sends a 100 ms prefix and requires the server's
Cancel/clear acknowledgment. Recognized text remains in memory and is not printed
or persisted by the tool. This is an explicit audio-send operation.

Input is mono uncompressed PCM16 RIFF/WAVE at 8..96 kHz, 2..60 seconds by default;
`--max-audio-seconds` can increase the ceiling to 120. The expected UTF8 text file
is at most 4096 bytes/512 words. Word comparisons normalize case, compatibility
Unicode and punctuation; the threshold is configurable (default WER 0.25), and
the result reports the measured rate. It does not infer clinical or multilingual
accuracy from one fixture.

Useful controls are `--model` (exact Nemotron inventory ID), `--language`,
`--handshake-seconds`, `--timeout-seconds`, `--finalize-seconds`, `--clients`,
`--max-wer` and `--chunk-ms`. `--help` shows defaults and ranges. Unknown,
duplicate and command-inappropriate flags fail before dependency/network work.
Ctrl+C aborts all active requests/sockets and keeps the result failed. No audio
is reconnected, replayed, posted again or moved to an alternate destination.

## Resource, transport and privacy boundaries

- HTTP and socket redirects are refused. HTTPS/WSS certificate verification
  remains enabled. Loopback HTTP requires an explicit flag and a loopback IP.
- Native HTTP consumes at most 64 KiB of successful JSON, never failed response
  bodies; response headers are bounded. Every request has a deadline.
- The Node `ws` transport limits each message to 64 KiB, 128 fragments and 256
  buffered chunks before application JSON parsing; compression is disabled.
  Pending events are bounded to 32/256 KiB, with 4096 events/2 MiB over a session.
  Recognized/partial text is bounded to 16 KiB.
- WAV files are read into a fixed-size allocation capped at 24 MiB. RIFF sizes,
  format/rate/block alignment, duplicate chunks and duration are validated.
- Structured TOON stdout includes only static check names, status, counts, WER
  and latency. Error bodies, tokens, provider diagnostic strings, audio,
  recognized text, endpoint origins and private fixture paths are not printed.
  No report/audio/transcript is saved automatically; the owner may retain the
  sanitized stdout as deployment evidence.
- Service host limits are CPUQuota 200%, MemoryHigh 6 GiB, MemoryMax 8 GiB,
  TasksMax 128 and LimitNOFILE 4096. These are reviewable template choices and
  must fit the selected VM/build. They do not limit GPU VRAM or establish model
  capacity/concurrency. GPU startup/stream memory still needs real measurement.
- The loopback listener uses four workers, bounded upload/socket settings and
  explicit local GGUF selection. Preflight checks credential shape, executable availability, readable local
  model and exact owner-configured expected SHA-256 before launch. It verifies
  current file equality, not binary provenance, index trust or model validity.

The service is the upstream direct-integration server. Its own documentation
recommends NVIDIA NIM for supported production serving. This work does not make a
personal VM a supported shared-scale deployment. [Upstream serving contract](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/main/docs/server.md).

## Client-facing identity and pinned model bytes

Current source at upstream commit
[`e59dfb0b5c22655620f41391cc9a000d6f56b802`](https://github.com/NVIDIA/NeMo-Speech.cpp/tree/e59dfb0b5c22655620f41391cc9a000d6f56b802)
resolves both an indexed alias and an absolute path into `asr_config.model.path`.
The recognizer's name comes from an explicit `asr.model.name`, otherwise the ASR
model's GGUF `general.name`. The HTTP inventory ID and socket session model both
read that recognizer name. The filename itself is not a reliable API identifier.
See [serve resolution](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/e59dfb0b5c22655620f41391cc9a000d6f56b802/app/serve.cpp),
[recognizer naming](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/e59dfb0b5c22655620f41391cc9a000d6f56b802/src/asr/recognizer.cpp),
[GGUF naming](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/e59dfb0b5c22655620f41391cc9a000d6f56b802/src/asr/model.cpp),
and the [supported display-name key](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/e59dfb0b5c22655620f41391cc9a000d6f56b802/docs/asr/configuration.md).

The launcher now specifies `--asr.model.name nemotron-3.5`. That lowercase label
matches desktop `SpeechService::test_inventory` in
`crates/waku-credentials/src/speech.rs` and phone `verifySpeechService` in
`apps/mobile/src/lib/speech-readiness.ts`: both accept lowercase
`nemotron-3.5`/`nemotron-3_5` substrings. The acceptance CLI now follows the same
case-sensitive match and refuses an uppercase-only match. Native serving with an
absolute path therefore uses a stable ID consistent with both actual clients.
This label does **not** prove the bytes are Nemotron 3.5.

The separate launch gate requires `ION_SPEECH_MODEL_SHA256` (64 hex digits) and
compares the local model with standard `sha256sum`, refusing missing, malformed
or mismatched values before starting the native server. Obtain the expected
value from `/opt/ion-speech/share/nemo-speech/model-index.json` in a verified
installed archive, or the verified source revision's `models/index.json`; select
the ASR file for the Nemotron 3.5 repository. `nemo-speech --json model list` can
identify repository/revision/roles but does not expose the artifact hash at the
inspected revision. Use the actual index, rather than inventing a checksum or
promoting an unchecked file's computed checksum into the expected value.

The [inspected pinned index](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/e59dfb0b5c22655620f41391cc9a000d6f56b802/models/index.json)
records the following artifact metadata; no model weights were downloaded here:

- Repository: `nvidia/nemotron-3.5-asr-streaming-0.6b`
- Model revision: `1c8deaecc64b91f034d73e08dd8b64625eb3395d`
- ASR file: `nemotron-3.5-asr-streaming-0.6b.q8_0.gguf`
- Size: `741548352` bytes
- SHA-256: `a5c435f294eea8f88ce68dd27b8c3bfea7f777cb2fbba04fcd30eaa555f429ae`

If the owner's verified installed release/index differs, use its exact trusted
conversion metadata and record that source. Protect the selected model from
concurrent untrusted writes. The runtime acceptance CLI cannot infer provenance
from a display label, text match or stored checksum configuration.

## Authoritative contracts checked on 2026-10-12

- [Install](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/main/docs/install.md):
  verified release archive checksums, backend/build selection and separately
  downloaded models.
- [Model/cache CLI](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/main/docs/cli.md):
  indexed Nemotron 3.5 pull, explicit model directory and local-GGUF serving.
- [HTTP API](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/main/docs/api.md) and
  [server implementation](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/main/server/http/http_server.cpp):
  exact transcription route, Bearer auth, session/PCM16 creation, update echo,
  partial/final events, commit-after-finish, clear/cancel acknowledgment.
- [Pinned ws 8.22.0 contract](https://github.com/websockets/ws/blob/8.22.0/doc/ws.md):
  explicit no-follow redirects, payload/fragment/chunk and handshake limits.
- [Reported finalization stall #49](https://github.com/NVIDIA/NeMo-Speech.cpp/issues/49):
  one reported release/build stalls after longer-than-subsecond audio. This is
  why a successful no-audio readiness check or a tiny clip cannot close the real
  finalization gate. The checker requires a fixture of at least two seconds and
  fails within a configured finalization deadline.

## Deployment and owner review checklist

- [ ] Supply Nebius VM/SSH or chosen deployment target, DNS, backend and resource
      sizing. No VM/cluster provisioning is performed by this feature.
- [ ] Verify the native binary/archive SHA and installed `serve --help`; record
      exact version/build/backend and the model artifact SHA/revision.
- [ ] Create the service account/model directory; pull as that user using the
      unit's explicit cache directory; select the actual verified GGUF path.
- [ ] Install the root-owned launch script, private mode-0600 environment file
      with random token/model path/expected artifact SHA-256, and systemd unit. Confirm placeholders and
      missing models and wrong checksums refuse startup before exposing a listener.
- [ ] Review CPU/RAM/process limits and CUDA device access on the target host.
      Run `systemd-analyze verify`, confirm effective limits and restart/shutdown
      behavior. Reboot and confirm the same model is ready without a new download.
- [ ] Validate Caddy configuration, DNS/TLS and loopback-only 8080. Verify
      unauthenticated/wrong-token requests fail and no auth/audio body logging is
      enabled. Check actual upload/time limits on the server and proxy.
- [ ] Run `check` against the exact HTTPS origin; retain sanitized stdout.
- [ ] Run `accept` with 2-, 10- and 30-second private spoken fixtures and accurate
      expected text. Choose a meaningful WER threshold. Confirm two simultaneous
      devices/streams meet latency and memory expectations; record peak RAM/VRAM.
- [ ] Check the selected model's supported languages using real intended-language
      fixtures. Do not assume Hindi or another language from the English model name.
- [ ] Test Ctrl+C, outage, a stalled finalization, and Cancel; confirm no commit of
      partial text, replay, request retry or alternate endpoint.
- [ ] Desktop: saved origin Test, real microphone, 1/10/30/120-second dictation,
      final Stop acknowledgment, editable text, permission denial and headset removal.
- [ ] Android and iOS native builds: saved origin Test, microphone permissions,
      1/10/30/60-second dictation, final Stop acknowledgment and editable text.
- [ ] Both phone platforms: backgrounding, task/daemon change, Cancel during
      preparation/finalization, service outage, invalid certificate and redirect
      refusal. No late text/save, wrong task destination or lingering mic indicator.
- [ ] Verify tokens use desktop OS vault / native phone SecureStore and never
      appear in URLs, logs, exports, inbox notifications or task transcripts.

## Implementation verification

40 Node tests passed on Windows with real loopback HTTP/WebSocket transports and
synthetic/fake recognition events. They cover flags, token/origin/fixture bounds,
auth refusal, redirect isolation, untrusted TLS refusal before auth, transport payload/fragment/pending-event limits, model/format echoes,
concurrent PCM pacing, partial/final/commit ordering, expected-text mismatch,
finalization/global deadlines and external cancellation. The identification path
is compared to the Node startup floor. Git's POSIX shell checks launch syntax and
negative preflight cases, local SHA equality/mismatch and actual launcher argument
selection using a synthetic artifact/native stub. The stable served-model label
passes the actual desktop/phone inventory predicates; uppercase-only matches
are refused. No test recognizes real speech or runs the native model/systemd/CUDA.

The installed default WSL Ubuntu distribution cannot start because its VHD file
is missing; no distribution was repaired or recreated. Linux systemd enforcement
and real native server launch remain target-host verification gates.
