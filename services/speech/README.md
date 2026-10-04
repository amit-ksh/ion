# Personal Nebius speech deployment

Status: prepared for a Nebius Linux VM; **not deployed**. No VM, cluster, DNS
name, or access target has been supplied. These files make no cloud changes.

Model: `nvidia/nemotron-3.5-asr-streaming-0.6b`. NeMo-Speech.cpp serves its
verified indexed conversion through authenticated HTTP and WebSocket APIs.
The desktop and phone currently use the HTTP upload path after recording
stops. Remote live partial transcription is a separate follow-up; local
desktop streaming preview remains available.

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
  save. The phone stores the token in SecureStore and uploads directly to
  the service. Its selected task/daemon still determines where the resulting
  editable text belongs.
- Both clients require an HTTPS root URL, and refuse redirects for requests
  carrying the token. A failing configured cloud endpoint does not silently
  switch to local speech. The desktop permits HTTP only for loopback testing.

No local NeMo install/model download is required for cloud dictation. Without
cloud configuration, the desktop and daemon keep their existing local mode.
Phone recordings are limited to 60 seconds; desktop recordings to 120 seconds.

## Review checklist

- [ ] Supply Nebius VM/SSH target, DNS hostname, and chosen CPU/CUDA server build.
- [ ] Provision VM and verified binary; download model; replace private token.
- [ ] Confirm service survives reboot, uses bounded workers, and 8080 is private.
- [ ] Verify TLS, model inventory, authentication, upload limit, and timeout.
- [ ] Test a short dictation from both desktop and phone; edit the result.
- [ ] Test Hindi/English, silence, service outage, cancel, and switching tasks.
- [ ] Check resource usage and latency with two simultaneous devices.

The upstream calls this server a direct integration/local serving path and
recommends NVIDIA NIM for production deployment. A larger shared installation
should assess NIM and an authenticated HTTP adapter before scaling. See the
[server contract](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/main/docs/server.md)
and [HTTP API](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/main/docs/api.md).
