# Ion production branch integration checkpoint

20 October 2026. This checkpoint integrates the managed services, native/mobile
account admission, composer references and verified 110M local speech source into
`ion-prod`. Infrastructure and device acceptance remain release gates.

## Pre-merge verification

- Cloud, gateway, native-client and mobile-account regressions: **121 passed,
  1 existing external HTML-fixture case skipped, 0 failed** (810 assertions).
- Real local workerd / SQLite / R2 integration: **3 passed**. Provider calls and
  compute operations use fixtures; these do not prove a live deployment.
- Native credentials/account/speech regressions: **43 passed, 1 opt-in installed
  model test ignored**. The installed model was verified at checkpoint 116.
- Composer, daemon API, phone WAV and speech-readiness/client regressions:
  **53 passed** (151 assertions). Fixed slash-command argument mention detection;
  file, team and app completion now continue after command arguments.
- Desktop/daemon `cargo check --offline`, cloud/Worker/controller/hosting/mobile
  type checks, Worker deployment dry run and whitespace checks passed.
- No new paid compute was started. No visual or physical device test was run.

## Owner release checklist

- [ ] Authenticated Cloudflare/Nebius access and approved Auth0 owner/judge subjects.
- [ ] Private R2/DO, API/content DNS/TLS, secrets and reviewed price/budget policies.
- [ ] Exact speech model upload and public download digest verification.
- [ ] Separate supervised Linux controller and isolated app VM; real provider
  inference and wake/stop/outage tests, ending with confirmed stopped resources.
- [ ] Final release EXE/APK builds and fresh-install/mobile microphone acceptance.

See [117](117-managed-compute-and-inference.md) for operational prerequisites,
accounting limits and implementation boundaries. These are not prerequisites for
merging tested source, but must be satisfied before calling the release live.
