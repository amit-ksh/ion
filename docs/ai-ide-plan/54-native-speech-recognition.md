# Actual local Nemotron speech recognition

Verified on 2026-10-12 from `codex/ion-agent-teams` source checkpoint `e8adb14`. This is an actual installed CPU model/service test using synthesized audio. It is not a Nebius deployment, human microphone, phone or TLS acceptance result.

## Exact input and runtime

- Installed NeMo-Speech.cpp **0.1.0**, Windows CPU build; native HTTP/realtime WebSocket enabled.
- Model: `nvidia/nemotron-3.5-asr-streaming-0.6b`, revision `1c8deaecc64b91f034d73e08dd8b64625eb3395d`.
- Complete 741,548,352-byte GGUF SHA: `a5c435f294eea8f88ce68dd27b8c3bfea7f777cb2fbba04fcd30eaa555f429ae`, rehashed against the installed index before each run. Upstream archive/index authenticity remains an owner gate.
- Offline Microsoft David Desktop synthesis: 16kHz, mono PCM16, 6.185 seconds/197,920 PCM bytes. No microphone or network synthesis.
- WAV SHA: `f97bc327bccfae547db34367156ab793c446f102ac457ad6fa2fcaf37fc7bb95`.
- The expected fixture sentence was fixed before recognition. Expected words were not changed to match output, and word-error allowance was 0.25; measured WER was 0.

The ephemeral service bound only to `127.0.0.1` on an unused high port, with a private generated token supplied through `NEMO_SPEECH_HTTP_API_KEY`, API label `nemotron-3.5`, CPU backend, no UI/warmup, two HTTP threads and two state-arena slots. The process was restricted to two available logical CPU cores. No token entered an argument, tracked file or reported output. A monitor sampled memory/exit status at 250 ms and would stop the owned processes at 512 MiB host free RAM, 1.5 GiB service working set or 360 seconds. These are test guardrails, not production-host enforcement or sizing guarantees.

After each completed run the acceptance process exited and the dedicated native service was terminated/reaped. No permanent listener, startup registration, system setting, paid resource or existing dev watcher was changed.

## Observed results

| Check                                       | Actual outcome                                                                   |
| ------------------------------------------- | -------------------------------------------------------------------------------- |
| Authenticated readiness and model inventory | Passed on the actual native service                                              |
| HTTP and socket wrong-token refusal         | Passed                                                                           |
| PCM16 session negotiation                   | Passed                                                                           |
| One-client recognition                      | 21 partials, 1 final, WER 0; commit 492 ms, total 6,692 ms                       |
| Two concurrent clients                      | Both 21 partials/1 final, WER 0; commits 4,021/4,020 ms; totals 10,211/10,210 ms |
| Cancel acknowledgment                       | Passed after prefix streaming on both acceptance runs                            |
| Sampled peak working set                    | 812,081,152 bytes single; 827,199,488 bytes concurrent                           |

The single probe completed in 9.067 seconds and the concurrent probe in 11.568 seconds including service start/readiness. Audio delivery/partial inference and finalization overlap; reported commit time is not a microphone end-to-end metric. This short English fixture does not establish latency/accuracy for Hindi, 30/60-second speech, noisy voices, arbitrary hardware or larger concurrency.

Private local evidence remains in ignored `.waku-cache/speech-native-20261012` and `.waku-cache/speech-native-concurrent-20261012`: pre-recorded expected text/WAV, sanitized `check.log`, `accept.log`, full model/source digests and `report.json`. No raw transcript/audio is included in this tracked report.

## Reproduce against your actual service

Start the reviewed local native server with the verified model and private token environment. Use the stable API label and native flags from installed `serve --help`; do not expose the loopback port publicly. Choose your own reviewed nonempty fixture and expected text before testing.

```powershell
# ION_SPEECH_TOKEN must match your privately configured service token.
node services/speech/accept.mjs check --url http://127.0.0.1:<port> --allow-loopback-http --model nemotron-3.5
node services/speech/accept.mjs accept --url http://127.0.0.1:<port> --allow-loopback-http --model nemotron-3.5 --wav <private.wav> --expected-text-file <private.txt> --clients 2 --language en --finalize-seconds 90 --max-wer 0.25
```

`--allow-loopback-http` is limited to exact loopback origins. Phone/cloud clients need the documented reachable service and trusted TLS. Local synthetic acceptance does not change the 46 pending physical observations.

## Remaining acceptance

- [x] Actual installed local CPU model readiness and real synthetic-WAV recognition.
- [x] Two concurrent actual-model streams, final text equality and cancel acknowledgment on the same local service.
- [ ] Human recordings/microphone, Hindi and 2/10/30/60-second fixtures with fixed expected text.
- [ ] Native desktop/phone stream cancellation/background and actual iPhone 15/Moto G32 observations.
- [ ] Real Nebius Linux/HTTPS deployment, target CPU/CUDA/resource/latency limits and restart.
- [ ] Native TLS/socket redirect rejection and controlled target auth/audio non-delivery evidence.

Deployment and device instructions remain in [speech preparation](50-speech-deployment-device-review.md) and [physical phone review](52-physical-device-review.md). The full goal remains active.
