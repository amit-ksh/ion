# Backend type-check acceptance

21 October 2026. Windows packaging was already running, so this milestone changes
only backend/test TypeScript annotations and backend budget test assertions.
No native source, mobile source, runtime spending policy or service switch changes.

## Fixes

- Declare the budget constructor's accepted envelope as readonly numeric monthly
  and lifetime limits. Previously inference from the frozen defaults incorrectly
  limited the TypeScript parameter to those exact two numeric literals, excluding
  valid smaller test envelopes. Existing runtime validation still rejects limits
  above the approved ceilings and freezes a defensive copy.
- Use `RequestInit["body"]` for the sandbox request body, which is available in
  both the Bun backend types and the Cloudflare Worker types.
- Declare fixture response shapes before accessing parsed JSON; remove fetch
  casts that incorrectly required Bun's unrelated `preconnect` static property.
- Add assertions rejecting even a one-micro-USD increase above either combined
  monthly or lifetime ceiling. Existing mutation, restart, unknown hold, debt,
  account and atomic rollback coverage remains.

## Evidence

- `bun run --cwd packages/ion-cloud check`: passed.
- `bun run --cwd packages/ion-cloud-worker check`: passed.
- Backend suite: **44 passed, 453 assertions, zero failures**.
- Real local workerd SQL/R2 suite: **7 passed, zero failures**.
- The emitted production Worker bundle before and after these fixes has the same
  SHA-256: **55fa7bf3ee67923af0d76b1454acb8bc39c0927e98aad7a3dc7b9ee58fd79045**.
  No new provider deployment or paid action was needed for type-only changes.

Logs: `.waku-cache/ion-backend-typecheck-service-tests.log`,
`ion-backend-typecheck-worker-tests.log`,
`ion-backend-typecheck-baseline.json` and
`ion-backend-typecheck-runtime-equivalence.json`.

The Windows package snapshot remains native source `7ffbbdd` plus the preserved
111-file QA patch. Its original automatic verifier deliberately rejects any
post-snapshot committed code change, including these non-native annotations.
If that guard stops verification, reconcile the exact changed paths and the
unchanged Worker bytes above before continuing package checks. Do not restart a
live native build or relabel a failed source guard as a passed package test.
No install, physical dictation, live sandbox or GUI acceptance is implied here.
