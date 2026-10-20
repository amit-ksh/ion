# Static app publishing on Cloudflare

21 October 2026. The owner narrowed app deployment to **static apps only**.
This supersedes Nebius app VM/controller setup as a current release gate.
Token Factory inference and remote sandbox execution remain separate features.

## User flow

1. Build/export the website locally using its normal project instructions.
2. Open project publishing. Ion detects `dist`, `out`, `build`, or a plain
   `index.html` website in the project root. A different relative output folder
   can be entered and explicitly reviewed.
3. Review the exact exported files, entry page, optional SPA routing and
   private/public visibility. No command runs when opening or publishing.
4. Upload the reviewed immutable snapshot to the existing authenticated
   Cloudflare Worker/private R2 publisher and open its isolated HTTPS site.
5. Republish, change visibility, roll back, unpublish and remove inactive
   versions using the existing owner-scoped publication controls.

Static frontends such as exported React/Vite/Vue/Svelte apps are supported.
SSR, backend APIs, Node/Bun/Python servers, cloud builds and runtime secrets
are unavailable. Frontend JavaScript remains visible to site visitors; secrets
must never be embedded in an export. No new Cloudflare paid subscription is
activated by this change; existing Worker/R2 usage remains metered.

## Enforcement

- The hosting API accepts only document/static runtimes with empty build/start
  commands and an existing HTML entry. Runtime secrets are rejected.
- Static publication is available with the storage binding and does not depend
  on Nebius Cloud being enabled or on a compute controller being online.
- Legacy server releases cannot start, roll back into service or serve content.
  Old hosting workers receive no claimed jobs and an empty desired runtime set.
- Native export traversal stays inside the project, rejects links/reparse
  points, omits hidden/dependency/server-source files, bounds traversal/reads
  and enforces 500 files, 20 MB total and 8 MB per file. Uploaded files retain
  hashes and the server's existing path/content/ownership validation.
- Export reads and HTTP calls stay on background workers. Changing the output
  folder requires a fresh review before publishing the retained snapshot.
- HTML artifact document mode keeps scripts disabled; interactive static mode
  retains the existing separate-origin/content policy.

## Verification

- Cloud service suite: **44 passed**, including legacy-runtime activation,
  serving and worker-claim denial without consulting or waking compute.
- Real local Worker SQL/R2 suite: **7 passed**, including multi-file JS assets,
  SPA fallback, missing asset/HTTP method behavior, cloud-build/server/secret
  rejection and publication with Cloud OFF. Existing account isolation,
  private access, idempotency, restart, unpublish and inference tests also pass.
- Native static export tests: **3 passed** on Windows. Unix link-specific test
  is conditional and was not run on this host.
- Worker and mobile TypeScript checks passed. Final desktop
  `cargo check --offline -p waku` passed, including the reparse-point guard.
  The native suite passed three tests with the explicit live test ignored.
- The standalone cloud-package TypeScript check remains red in unrelated
  app/sandbox test typings, budget fixture literal types and `BodyInit` typing.
  Its executable service tests and the production Worker's type check pass;
  this is not recorded as a clean repository-wide type check.

Evidence is retained under `.waku-cache/ion-static-*`. No native visual test,
new VM, container, paid subscription or change to owner spending switches was
performed for this implementation.

Production rollout, native account publication and refreshed release artifacts
are separate acceptance steps; do not infer them from local tests.


## Production rollout in progress

Initial static-only Worker version `02cd55d3-b808-4322-b73d-6544ea62cf1b`
was deployed. A real native-account check confirmed private denial, idempotency,
HTML/JS asset integrity and SPA fallback. Its server-denial assertion incorrectly
expected the HTTP response body in the native error, while that transport
intentionally exposes only the status. The test was corrected to check 403.
The first disposable site was deleted and all switches were verified unchanged;
this first run is not a passed complete acceptance. Evidence:
`.waku-cache/ion-static-live-publication.json`.

The final source additionally removes unreachable server-build/proxy logic.
Both service (44) and Worker (7) suites passed again. Deploy this final source,
rerun the corrected opt-in live test and refresh the Windows/Android artifacts
before claiming final release acceptance. The existing EXE/APK predate this UI.


## Final backend acceptance

The committed publisher was deployed successfully as Worker version
`0f0f8849-6b69-4881-8e50-247d4bad145b`. The corrected opt-in native test then
passed against the actual service using Windows Credential Manager's existing
owner session: private denial, immutable idempotency, exact HTML/JS content,
SPA fallback, server publishing denial, unpublish and republish. The temporary
site was deleted successfully; Cloud, Token Factory and app-action switches
were unchanged. No compute started. Evidence:
`.waku-cache/ion-static-live-publication-final.json` and matching log.

The backend static-publishing gate is passed. The rebuilt desktop/mobile visual
interaction and refreshed EXE/APK gates remain separate; this check used the
real native account transport and public DNS/TLS rather than a GUI click.
