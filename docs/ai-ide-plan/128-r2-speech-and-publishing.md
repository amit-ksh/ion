# R2 speech distribution and publishing storage

21 October 2026. User-directed change: R2 is now active and replaces Hugging
Face as Ion's default model download origin. Speech inference remains local;
mobile dictation still uses the paired desktop. No speech VM is needed.

## Verified production storage

- Created private Standard bucket `ion-private-objects` in the approved account.
- Verified public `r2.dev` access is disabled. No public bucket domain or R2
  credentials are distributed to desktop/mobile clients.
- Uploaded `models/ion-parakeet-110m-tdt-q8-v1.gguf` after checking its local
  digest. The production Worker streams it through
  `https://api.build-with-ion.tech/models/ion-parakeet-110m-tdt-q8-v1.gguf`.
- Production HEAD returned 200 and the expected length/digest header. A complete
  HTTPS GET independently measured **146029600 bytes** and SHA-256
  **21ca92b8f0299c6e7d28eb9caa25235bf0a55eaa6511e8e46fbbc27e96900876**.
- The full Worker configuration was deployed with the `OBJECTS` R2 binding as
  version `f07fa321-ca4b-45ee-bbdf-38e986947118`. Health/bootstrap remained 200;
  HTML publishing became configured, while app compute and managed sandboxes
  remained unavailable. Owner spending switches were not changed.

Sanitized evidence: `.waku-cache/ion-r2-create.log`,
`ion-r2-public-access.log`, `ion-r2-speech-upload.log`,
`ion-r2-worker-deploy.log` and `ion-r2-speech-acceptance.json`.

## App changes

The bundled manifest now uses Ion's HTTPS endpoint. Model identity, runtime,
filename, byte count, SHA-256, cancellation, partial-file handling, bounded
downloads and verified cache reuse remain unchanged. The existing downloader
already accepts this exact Ion origin/path. Existing valid cached model files
remain usable without another download.

Native download regressions passed four tests (the opt-in network test was
initially skipped). Then the opt-in test passed separately against production
R2 from an empty isolated cache: first-use download, complete checksum validation
and reuse without another download completed in 11.56 seconds. Evidence:
`.waku-cache/ion-r2-native-regressions.log` and
`.waku-cache/ion-r2-native-download.log`.

NVIDIA/Suno attribution and CC-BY-4.0 notices remain bundled. The previous
Hugging Face distribution revision is historical provenance for the identical
bytes; it is not the active download URL. The earlier public repository was not
deleted. No Hugging Face user token is needed.

The desktop packaging job already running when this change was requested is a
pre-migration candidate. Refresh its packaging after completion before calling
it the R2-enabled release. Keep the original source/QA-patch record and record
the final rebuilt input hashes separately. Android downloads no model itself;
its dictation continues to use the paired desktop.

## Publishing routes

Before enabling the full publishing route, inspected all existing DNS and
Worker route records. Only the API DNS record existed. Added an explicit
`www.build-with-ion.tech/*` exclusion with no Worker script and a proxied
wildcard DNS record. The full config reserves `*.build-with-ion.tech/*` for
publishing; root DNS is unchanged and the wildcard route does not match root.
The Worker accepts only first-level 32-hex site IDs or the exact API origin.

Preserve the website exclusion and add a specific route or exclusion before
assigning any future non-publication subdomain. Exact routes take precedence;
see [Cloudflare routing](https://developers.cloudflare.com/workers/configuration/routing/routes/)
and [wildcard DNS](https://developers.cloudflare.com/dns/manage-dns-records/reference/wildcard-dns-records/).

The route configuration and production model manifest were deployed after all
seven local Worker tests and TypeScript passed. Final Worker version:
`898aab00-4669-4478-99fc-89d17de50d5c`. Live checks verified the descriptor's R2
URL, hosting API denial without login (401), a missing 32-hex site over real
DNS/TLS (404), rejection of an unassigned non-site hostname (404), and retention
of the explicit `www` exclusion. Evidence: `ion-r2-routing-deploy.log` and
`ion-r2-routes-acceptance.json` in the local cache.

Private/public publication, ticket exchange, revocation and cross-account denial
must also be exercised with a real admitted user session. The disposable HTML
acceptance runner is prepared and awaiting Auth0 login. Storage activation alone
does not satisfy that live acceptance gate.

## Remaining dependencies

R2 activation and model upload access are complete. Real desktop/phone login,
one user-authorized Composio connection, physical microphone/dictation checks,
Token Factory sandbox permission and compatible image verification, and an
isolated Nebius app host remain separate gates. Nothing in this milestone
starts Nebius compute or changes the USD 10 monthly / USD 15 cumulative limits.
