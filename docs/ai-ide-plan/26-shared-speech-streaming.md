# Shared speech streaming implementation and review

Updated 2026-10-09. Worktree branch: `codex/ion-agent-teams`. Protocol stays 36.
This is client implementation; the Nebius speech service has **not** been deployed.
The user has not supplied a deployment target or credentials.

## Implemented behavior

- Desktop and native phone use the same configured personal NeMo speech endpoint.
  Desktop tokens stay in the OS credential vault; phone tokens in SecureStore.
- The explicit transcription WebSocket route accepts mono little-endian PCM16.
  The clients configure the microphone's actual sample rate before sending audio.
  This route is separate from NeMo VoiceChat and the OpenAI Realtime protocol.
- Partial transcript previews are bounded and coalesced to at most eight updates
  per second. They do not modify the draft. Confirmed segments replace partials.
- Stop sends one commit, then waits for the server's acknowledgment after its
  final segments. Earlier endpointed segments are retained, including when the
  last final segment is empty. Unacknowledged or unfinished text is never final.
- Cloud microphone sessions do not create or upload a WAV. Existing imported WAVs
  retain HTTP transcription. No reconnect, audio replay, automatic upload fallback
  or change to a different speech destination follows a failed cloud stream.
- Phone capture is bounded to 60 seconds and desktop to 120 seconds. Events and
  transcript text have fixed limits; phone preconfiguration buffering is bounded
  to two seconds. Finalization has a 20-second deadline and static error messages.
- Phone Cancel, backgrounding, task/daemon changes and disabled composer controls
  stop capture and suppress late results. A native socket that opens after Cancel
  is closed again. Capture cleanup cannot clear a newer capture's PCM buffers.
- Desktop Stop retains the command sender so Cancel can interrupt finalization.
  A canceled result cannot become insertable if the worker completes late.
  Local preview failure stays local even if speech settings change mid-capture.
- Desktop socket work stays on the existing blocking worker. Its TLS connection
  explicitly selects Ring without modifying the process-wide crypto provider;
  system roots and standard certificate/hostname verification are retained.
- Browser microphone streaming remains disabled. Normal New task, harness and
  team execution flows are unchanged by dictation.

## Verified source contracts

The [NeMo HTTP API](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/main/docs/api.md)
and [server implementation](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/main/server/http/http_server.cpp)
define session creation/update, binary PCM, partial/final segments and the commit
acknowledgment. The current server emits its final recognition result before that
acknowledgment. Client completion waits for both, rather than guessing from silence
or assuming the next segment must contain text.

The [Expo Audio API](https://docs.expo.dev/versions/latest/sdk/audio/#useaudiostreamoptions)
and installed `expo-audio` 57.0.5 provide native microphone PCM buffers through
`useAudioStream`. Playback waveform sampling is not used for dictation. React
Native's installed WebSocket implementation supports Authorization request headers;
the native phone TLS/redirect behavior must still be reviewed on each device.

## Automated verification

Nine Rust speech boundary tests and thirteen phone stream/WAV tests passed.
Desktop, daemon and native worker compilation and full phone TypeScript passed.
Rust formatting and whitespace checks passed. Tests use local/fake transports;
no visual test, live service deployment or physical microphone review was run.

## Remaining operator and device checks

Use the [deployment and review checklist](../../services/speech/README.md).
The model-inventory Test confirms HTTP readiness only. A working microphone
stream requires a deployed server build, direct WSS route and trusted certificate.

- [ ] Supply and configure VM/DNS/private service token, then verify the server.
- [ ] Test desktop, Android and iOS previews, complete final text and editing.
- [ ] Test 1-, 10-, and 30-second clips and both simultaneous clients.
- [ ] Check supported English/Hindi behavior against the exact installed model.
- [ ] Check silence, missing commit, timeout, disconnect and no audio replay.
- [ ] Check Cancel before connection, while recording and while finalizing.
- [ ] Check task/daemon change, backgrounding, permission denial and headset removal.
- [ ] Check invalid TLS and redirects without logging audio or credentials.
- [ ] Check resource use, native build packaging and temporary-file cleanup for the
  remaining offline phone mode.

These are pending live reviews, not evidence of a deployed or released service.
