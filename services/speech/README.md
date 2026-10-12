# Personal Nebius speech deployment

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
   `/var/lib/ion-speech`. Download the model as that user with the same HOME:

   ```sh
   sudo -u ion-speech env HOME=/var/lib/ion-speech \
     /opt/ion-speech/bin/nemo-speech pull nvidia/nemotron-3.5-asr-streaming-0.6b
   ```

3. Save `speech.env.example` as `/etc/ion/speech.env`, replace its placeholder
   with a randomly generated speech token, and set root ownership and mode
   `0600`. Keep that token private. Do not use the placeholder or the model
   provider's API key. Copy `ion-speech.service` to `/etc/systemd/system/`.
4. Point your DNS hostname to the VM. Install Caddy and merge the provided
   Caddyfile into its configuration. Set `ION_SPEECH_DOMAIN` in Caddy's service
   environment. Permit inbound 80/443 for TLS issuance and HTTPS; keep 8080
   private. Validate with `caddy validate` before restarting Caddy.
5. Reload systemd, enable/start `ion-speech`, and check its journal and local
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
