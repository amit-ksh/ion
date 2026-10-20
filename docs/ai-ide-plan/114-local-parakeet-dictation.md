# Local Parakeet dictation

> **Model selection superseded:** the owner selected **Parakeet TDT-CTC 110M (Q8 target)** on 19 October 2026. See the [final end-to-end plan](115-final-e2e-platform-plan.md). This document records the earlier 0.6B source implementation and its checks; it is not evidence of 110M integration.

## Accepted scope — 2026-10-19

Use NVIDIA **Parakeet TDT 0.6B v3** locally, replacing the proposed hosted STT service. Mobile initially uses the explicitly paired desktop for recognition. Gateway, HTML publication and application deployment remain separate cloud features. No Nebius speech VM, public STT hostname, speech token or GPU is needed for this path.

## User flow

- The Windows installer bundles the existing NeMo-Speech.cpp CPU runtime; users do not install Python, CUDA or a separate CLI.
- On first microphone use, Ion downloads and verifies the model before starting desktop capture. Settings → Models & Speech and the onboarding Speech step also offer **Download / verify Parakeet**.
- The installed model index resolves `nvidia/parakeet-tdt-0.6b-v3` to its pinned GGUF, exact byte size and SHA-256. The inspected 0.1.0 index lists 713,975,456 bytes (about 714 MB), revision `541d1f99c6b0c3cd0b11a95167540bb8edefd82b`, SHA-256 `e3880d0aaaaf2c308ea2c35016b2b895c423eb3fda924c1b463d1c19b7f4d32e`. The bundled runtime performs verification; Ion does not silently replace its index.
- Model preparation runs on a blocking worker. Ion shows an indeterminate download/verification message, supports cancellation and retry, and reuses the verified cache. No invented percentage is displayed.
- Press the microphone to record, then Stop to transcribe. Parakeet is used in batch mode; the UI does not claim live transcript streaming.
- On mobile, consent identifies the selected desktop before recording. A bounded mono PCM16 WAV (up to 60 seconds / 8 MiB) is sent only to that paired desktop after Stop. Desktop capture remains bounded at 120 seconds.
- First phone use can wait for the model download on the desktop. The request deadline covers a bounded 10-minute preparation plus 2-minute transcription; preparing from desktop Settings first avoids that wait.
- Model files use NeMo's per-user cache, or the existing `NEMO_SPEECH_MODEL_DIR` override. Internet is needed for the first download; subsequent desktop recognition uses cached weights. Mobile still requires its paired desktop and transport to be online. Mandatory Ion sign-in remains a separate product requirement.

## Language, privacy and resource boundaries

NVIDIA's model card lists **25 European languages**, including English, with automatic language detection. **Hindi/Hinglish is not supported** and the old Hindi choice is removed; legacy Hindi requests receive a clear error. Speech is not translated.

Current dictation no longer discovers a managed speech endpoint or consults a saved personal STT token. Old service credentials are not deleted. The dormant broker/operator code remains historical reference; deployment of that service is out of scope.

Desktop microphone/WAV audio stays on that computer for recognition. Phone recordings cross the already selected paired-desktop transport; a remote gateway can relay that transport. No STT inference provider receives it. Temporary WAV files are deleted on normal completion/failure, and stale daemon recordings retain the existing cleanup policy. User approval to send the resulting text to an agent remains a separate action.

Both desktop and daemon share one local runtime adapter. It bounds output and runtime, reaps failed/canceled children, serializes preparation within each process and hides the runtime console on Windows. Preparation and inference never run on the render thread. Actual speed and memory use depend on hardware and are not yet measured for this model.

## Attribution and sources

- Model by NVIDIA, [Parakeet TDT 0.6B v3](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3), distributed under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). This integration changes the selected model and runtime options; it does not fine-tune the weights.
- [NVIDIA NeMo-Speech.cpp](https://github.com/NVIDIA/NeMo-Speech.cpp) supports local Parakeet inference and indexed model download/verification. Installer packaging already copies its runtime, model index and license materials.

## Verification and later acceptance

Completed source checks:

- `cargo check -p waku -p waku-daemon --offline` passed; pre-existing ts-rs/dead-code warnings remain.
- Mobile TypeScript checking passed.
- Ten focused mobile daemon/WAV cases passed (24 assertions).
- Four local subprocess cases passed: successful/failed execution, bounded output, timeout/cancel and their fixture.
- UI source detector returned no findings; `git diff --check` passed.

These process tests use a test fixture, not model weights or microphone input. No speech VM is provisioned. No model download, new installer/APK, microphone recording, visual review or physical device acceptance is implied by a passing compile.

- [ ] Install a new build on a clean machine; verify the bundled runtime and first-use download without a separate CLI installation.
- [ ] Verify cancel/retry during download, insufficient disk space and interrupted downloads.
- [ ] Verify cached Parakeet inference without STT network access, using a fixed English WAV and a human microphone recording; check mandatory account access separately.
- [ ] Confirm no legacy speech-service URL is contacted, even when old endpoint credentials remain saved.
- [ ] Confirm Stop, Cancel, time limits and edit-before-send behavior.
- [ ] Pair iPhone 15 and Moto G32; verify recording → desktop transcript, connection loss, app backgrounding and stale-result rejection.
- [ ] Measure Parakeet CPU time, memory, supported-language accuracy and acceptable latency on target hardware.
