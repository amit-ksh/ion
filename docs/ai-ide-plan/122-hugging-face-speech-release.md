# Public speech model release and native acceptance

> Historical Hugging Face release acceptance. The user's later direction moved
> the default model download to private R2 through Ion's HTTPS API; see
> [128](128-r2-speech-and-publishing.md). Source attribution and the HF immutable
> revision remain provenance. References below to HF as the active download
> origin or R2 as unavailable describe that earlier milestone.

20 October 2026. The operator supplied `amit-ksh/ion-stt` and local upload access.

## Published artifact

- Public repository: https://huggingface.co/amit-ksh/ion-stt
- Immutable revision: `e2d275da00383be64c0ee95f340ff37e3329c407`
- Artifact: `ion-parakeet-110m-tdt-q8-v1.gguf`
- Size: 146029600 bytes.
- SHA-256: `21ca92b8f0299c6e7d28eb9caa25235bf0a55eaa6511e8e46fbbc27e96900876`.
- Model card and NOTICE include NVIDIA/Suno attribution, CC-BY-4.0, pinned source
  and converter revisions, Q8 conversion details, runtime compatibility and
  English-only limitations. Model weights were not fine-tuned.

The bundled manifest points directly to the immutable Hugging Face URL. End users
do not supply Hugging Face credentials. The uploader used the operator's existing
local login; its token is not in the app, repository, model card or manifest.

## Downloader

- Initial Hub URLs must match this repository, exact filename and a 40-character
  revision; branch URLs and URL credentials are rejected.
- Anonymous HTTPS redirects are bounded to five hops and an explicit Hub/CDN
  hostname allowlist. The real public download redirects to `us.aws.cdn.hf.co`.
- The accepted hostname is not proof of model integrity: exact size and SHA-256
  are still checked before replacing the model cache.
- Partial/corrupt/oversized downloads cannot replace a valid model. Preparation
  remains cancellable, cross-process locked and bounded by its existing deadline.

## Actual acceptance

1. The first real native download failed closed on an unrecognized CDN hostname.
   Read-only header inspection identified the actual CDN; its exact hostname was
   added and covered by the trusted-host regression. No broad wildcard was added.
2. `speech::download::tests::live_public_model_download_and_cache_reuse` passed
   against the published file using a fresh isolated cache and **no access token**.
   Full bytes/hash were verified, and a second preparation reused the cache
   without entering the download path. Duration after compilation: 13.03 seconds.
3. `speech::local::tests::verified_110m_runtime_smoke` passed with that downloaded
   model and installed CPU NeMo-Speech.cpp 0.1.0. The fixed synthetic speech was
   transcribed as expected; the silent fixture was rejected as no speech.
   Duration after compilation: 2.16 seconds.
4. The full credential regression suite passed: 49 tests, two explicitly ignored
   live tests. The two ignored speech acceptance cases above were run separately.
   A restricted-shell vault probe initially failed with OS error 1312; the full
   suite with Windows Credential Manager access passed, including the vault test.

Logs: `.waku-cache/ion-hf-native-acceptance.log`,
`.waku-cache/ion-hf-local-runtime-acceptance.log`,
`.waku-cache/ion-managed-credentials-full.log`.

## Remaining acceptance

Physical desktop microphone permission/recording, paired Android dictation,
first-use behavior in a new installer and its bundled runtime still need device
acceptance. Synthetic fixture tests do not prove those interactions. iOS is out
of scope. R2 is no longer a dependency for this speech model download, but remains
a separate prerequisite for the planned HTML/app publishing storage.

No speech VM or hosted inference endpoint was created. Dictation runs locally on
desktop; Android sends its bounded recording to the paired desktop initially.
