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

## Merge and restored workspace

- Feature commit: `41eb1b1`; merge into `ion-prod`: `ccfd272`.
- The existing website commit remains in history. Its animation dependency was
  reconciled into the combined lockfile.
- All 731 saved QA paths are present. The original QA work remains recoverable
  in stash `805bed6a6d4459bd4b927c0cc69ed6a8783318cf`; it was not dropped.
- Eight restoration conflicts were reconciled. Shared inference now retains the
  QA live-catalog workflow model selection and explicit output bound. The 110M
  Parakeet path retains microphone interruption handling; obsolete Nemotron
  preview code was superseded by the requested local model architecture.
- Restored `ion-prod` desktop/daemon compile and mobile/web/Worker/controller/
  hosting type checks pass. Focused post-merge regressions: **47 passed, 246
  assertions**. Real local workerd/SQLite/R2 integration: **3 passed**.
- Worker tests now build their generated bundle before execution, so a fresh
  checkout cannot silently rely on an old worktree's `dist` directory.
- Existing unrelated QA edits and artifacts remain uncommitted in the primary
  checkout. No cloud resource was started and no deployment was performed.
