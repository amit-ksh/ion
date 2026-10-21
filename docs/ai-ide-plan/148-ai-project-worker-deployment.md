# AI-project sandbox compatibility: deployed gateway

21 October 2026. Completes deployment of the sandbox project-ID compatibility
fix from `660d735`. It does not configure or pass live sandbox execution.

## Regression and emitted-source verification

The existing real workerd sandbox lifecycle test now uses
`aiproject-fixture123` and checks that the returned project and provider header
retain that exact ID. The old bundle failed with HTTP 500 and the policy
validation error. Rebuilding from the corrected source passed all **seven**
workerd integration tests, including authenticated bridge/isolation, durable
restart, single dispatch and owner-stop alarm behavior. Worker TypeScript passed.
All provider requests in these tests are intercepted fixtures, not live effects.

The production bundle changed from
`55fa7bf3ee67923af0d76b1454acb8bc39c0927e98aad7a3dc7b9ee58fd79045`
to `ad725c8c15ccad0357d6c646fc49dcdd8fd23086fc82f6189d7d0934879f0b17`.
Both source maps have 61 entries; only `../../ion-cloud/src/sandbox.ts` changed.
The previous bundle/map are preserved in the ignored cache.

## Live preflight and deployment

Authenticated Wrangler reads confirmed the prior version
`0f0f8849-6b69-4881-8e50-247d4bad145b`, four existing required secrets, and no
`SANDBOX_POLICY_JSON`. Route reads confirmed the publishing wildcard and the
separate `www` exclusion. The deployment helper rechecked the version immediately
before writing, so an unexpected deployment by another session would stop it.

The first preflight stopped before uploading because Python urllib received
Cloudflare HTTP 403/error 1010 on public health/bootstrap. Node's default client
and Ion's explicit application identity both returned healthy 200 responses.
Cloudflare documents 1010 as client-signature blocking; no browser identity was
impersonated and no security setting was changed. See
[Cloudflare error 1010](https://developers.cloudflare.com/support/troubleshooting/http-status-codes/cloudflare-1xxx-errors/error-1010/).
The failed preflight report is preserved. A separate continuation used Node's
normal client for public checks.

At **18:26 IST**, the tested gateway was deployed to
`https://api.build-with-ion.tech` as version
**`b20ab52f-1155-424b-af51-eedc9957cd0f`**, using existing variables, secrets,
SQL Durable Object classes/migration and the private R2 binding. Deployment
compilation retained the exact reviewed bundle hash. The deployment helper did
not write secrets, service switches, spending policies or authentication settings,
and did not clear or replace the existing Durable Objects.

Before and after:

- Health and bootstrap returned 200. Static HTML/app publishing remains available.
- Bootstrap reports `managedSandbox: false` because no policy is installed.
- Unauthenticated service controls returned 401.
- Speech model HEAD returned 200, 146,029,600 bytes and the expected pinned hash.
- Secret names are unchanged; no sandbox policy was installed.

At **18:27 IST**, both zone routes had the same configuration hash as before,
including the wildcard to `ion-cloud` and the null-script `www` exclusion.
No paid compute, model request, app tool action, sandbox import or workload was
started. These checks do not replace authenticated native/device acceptance,
full model-byte verification already recorded elsewhere, or provider invoices.

## Evidence and remaining release gate

Ignored-cache receipts: `ion-ai-project-worker-red-status.log`,
`ion-ai-project-worker-green.log`, `ion-ai-project-worker-bundle-comparison.json`,
`ion-ai-project-worker-remote-preflight.json`,
`ion-ai-project-worker-deployment.json` (stopped before upload),
`ion-public-client-identity-20261021.json`,
`ion-ai-project-worker-client-verified-deployment.json`,
`ion-ai-project-public-before-retry1.json`,
`ion-ai-project-public-after-retry1.json`, and the before/after route reports.

SBX-02/03 still need provider-compatible workers, harness bootstrap and confirmed
pricing before installing a managed policy. The new repository test change is
backend-only; Windows/mobile runtime source remains at the recorded candidate.
Reconcile that exact test file against the frozen candidate's source guard before
packaging. Never bypass source/QA guards just because these tests passed.
