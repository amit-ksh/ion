# Milestone 7: phone dictation and personal cloud execution

The user requested both additions with earlier live reviews deferred. The
phone sends microphone audio only after an explicit action to the selected,
authenticated ion daemon. A desktop daemon transcribes it locally through
NeMo-Speech.cpp. An owner-managed Linux VM can run the same daemon headlessly,
so a phone or web client can start and monitor tasks while the desktop is off.
The personal gateway continues to handle scoped connected-app proposals; it
is not the coding runtime or the speech relay.

## Implementation checklist

- [x] Add native phone microphone capture with a clear recording/transfer
      indicator, cancel control, and user consent before audio transfer.
- [x] Convert mono 16-bit PCM from Expo's audio stream to a canonical WAV.
      Cap recordings at 60 seconds and 8 MiB; the host validates the same
      limits, sample rate, WAV length, and language before starting ASR.
- [x] Send audio only over the selected authenticated daemon WebSocket. Never
      send it to the gateway or a speech API. Reject a task or daemon change
      during capture or transcription. Insert the final text into the editable
      composer, leaving Send under the user's control.
- [x] Run NeMo-Speech.cpp on a daemon worker, cap its run time and in-memory
      output, and delete its temporary audio on normal completion. Sweep stale
      recording files after a daemon crash when it next starts.
- [x] Support a headless Linux daemon with separate private files for the
      daemon token and Nebius key, persistent task and provider state, and a
      loopback listener behind a trusted WebSocket TLS reverse proxy.
- [x] Provide an owner-editable systemd unit and Caddy example. The existing
      mobile daemon selector, remote project picker, task creation, ordered
      events, approvals, and reconnect path work against this host.
- [ ] Verify an installed phone with a real desktop ASR run and a live cloud
      daemon with an owner-provided Nebius key.

## Owner setup

### Phone to desktop ASR

1. Install NeMo-Speech.cpp and pull `nemotron-3.5` on the **desktop daemon
   host**, as in the [speech guide](11-speech-gateway.md#local-speech). Keep the
   desktop daemon reachable from the phone over a private network or trusted
   `wss://` endpoint. In the phone's **Daemons** screen, select that desktop.
2. Build/install the mobile app with `expo-audio` included. Open a task or
   **New task**, tap the microphone, approve OS microphone access and the
   in-app transfer notice, speak, then tap Stop. Review and edit the text
   before sending.
3. The direct daemon token is a full-control capability. Keep it in the
   phone's SecureStore and use a trusted private network or TLS endpoint.
   Gateway pairing alone does not authorize audio transfer.

### Personal cloud execution

1. Provision an owner-controlled Linux host, choose a region and spending
   limit, and arrange SSH access, a DNS name, and a trusted TLS endpoint. No
   VM or billable resource is created by this repository. Install the Rust
   build prerequisites and build `waku-daemon` in release mode:

   ```sh
   cargo build --locked --release -p waku-daemon
   ```

2. Create a dedicated `ion-runner` system user and private directories for
   its workspaces and secrets. Put the reviewed binary at
   `/opt/ion/bin/waku-daemon`. Create `/etc/ion-daemon/daemon-token` with a
   fresh random 32–256 character URL-safe token, and
   `/etc/ion-daemon/nebius-key` with one Nebius Token Factory API key. Both
   files must be regular files readable by `ion-runner` with mode `0600`.
   The key is read on the host at task start; it is never sent to the phone.
3. Copy and edit the [systemd unit](../../crates/waku-daemon/deploy/ion-cloud-daemon.service.example)
   and [Caddyfile](../../crates/waku-daemon/deploy/Caddyfile.example). Ensure
   `/var/lib/ion-runner/workspaces` exists and belongs to `ion-runner`. The
   daemon listens only on loopback; Caddy forwards its `/v1` WebSocket path
   over `wss://` using its documented [WebSocket reverse proxy](https://caddyserver.com/docs/caddyfile/directives/reverse_proxy).
   Limit network access to trusted devices when possible.
4. Start the service. Add its `wss://` address and daemon token in the phone's
   **Daemons** screen. Choose a project path on the cloud host (or create a
   projectless task), select Nebius, and start a task. The daemon and its
   persistent state stay on that host while the desktop is off. The phone's
   **Personal gateway** screen is a different, scoped service and is not
   required for this direct cloud task path.

## Review checklist

- [ ] On an installed iOS/Android build, grant microphone permission once.
      Check the recording label, Stop, Discard, and transfer state. Verify
      denial and Discard send no audio.
- [ ] Dictate a short sentence to a reachable desktop. Confirm NeMo-Speech.cpp
      runs on that host and inserts editable text into the current composer.
      Check that nothing is sent to the agent until Send is pressed.
- [ ] Try a disconnected desktop, a daemon/task switch during recording, an
      overlong capture, and an ASR error. Confirm an error appears and audio
      cannot be delivered to a different host or task.
- [ ] Confirm the audio is absent from the personal gateway and that ordinary
      temporary recordings are removed. After a forced desktop daemon crash,
      restart it after ten minutes and confirm stale recording files are swept.
      The cloud systemd unit's private temporary directory is removed when
      its service stops.
- [ ] With the desktop off, connect the phone to the owner-managed cloud
      daemon over `wss://`. Create a Nebius task against a checkout on that
      host; inspect ordered activity, approve one bounded action, disconnect
      and reconnect the phone, and confirm the task continues without a
      second execution of that action.
- [ ] Restart the cloud daemon and verify task state and a resumable Nebius
      journal. Revoke/rotate the phone's full-control daemon token and confirm
      the old token no longer connects. Check the VM budget, storage, logs,
      firewall, and TLS certificate.
- [ ] Review the live evidence and approve Milestone 7 or request fixes.

Local compile, type, and protocol tests do not prove microphone behavior on
an installed phone, a specific ASR installation, or an owner cloud deployment.
