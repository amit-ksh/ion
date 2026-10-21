# Relay recovery rollout and guarded release preparation

21 October 2026. Source: `fd9a36a0b9a6511230873e55fb11718d0ad83ef2` on
`ion-prod`, following scoped interaction recovery in bf2ce72.

## Deployed backend

- Worker: `ion-cloud` at `https://api.build-with-ion.tech`.
- New version: `8c731253-d440-4129-bf00-0c6da5ef6c2c` at 100%.
- Previous version: `d948eb00-1607-497d-9dd8-9e7dc75dbcbb`.
- Bundle SHA-256:
  `9d946b238a7702a242b419a12b473d86845a7071ab90c47d87acb06443c4e641`.
- All 61 mapped sources match the locally tested candidate; only the HTTP
  notification condition and task-relay resolution handling differ from the
  preceding deployed bundle.
- Before/after route semantics and secret names match. The wildcard still points
  to Ion, `www` remains excluded, and the existing API custom domain works.
- No secret, service-switch or permission change was performed; no Nebius paid
  workload, image import, VM or other app host was started. Existing Durable
  Object data was retained; the resolution table is an additive schema change.

Public health/bootstrap checks returned 200 before and after rollout. Anonymous
controls and both synchronization endpoints returned 401. The speech model HEAD
returned 200 with the expected 146,029,600-byte length and pinned SHA header.
Managed sandbox availability remains false until its existing image/pricing gates
are satisfied; static app publishing remains available.

The existing native account's authenticated acceptance case passed **1/1** after
rollout. It reads owner controls, the connected-app catalog and combined spending,
and checks disabled routes without dispatching a valid effect or inference.
The first invocation stopped at the sandbox-protected local account lock before
network access; the identical hash-verified binary passed through the permission
system. No browser or new login was needed.

This binary predates the new native queue scope helper. Its test source is
unchanged, but it does **not** establish new native queue/fingerprint acceptance.
The successful deployment likewise does not establish live idle traffic reduction
or physical-device behavior.

Authoritative local receipts:

- `.waku-cache/ion-scoped-worker-deployment.json` — original successful rollout.
- `.waku-cache/ion-scoped-worker-deployment-acceptance.json` — adds the native
  authenticated read result while preserving the original receipt.
- `.waku-cache/ion-scoped-worker-native-live-read-profile.log` — successful native
  case; the initial protected-lock failure is retained separately.
- A sanitized rollout receipt is saved alongside this document as
  `162-relay-worker-deployment-receipt.json`.

## Fresh release candidate

A new fd9a36a snapshot pins **1,935 tracked code files, 50 regenerated Android
native inputs, 63 external CPU speech-runtime inputs**, and three preserved
historical artifacts. It reuses hash-verified backups, without overwriting them.
Eight prior snapshots/verifiers/wrappers are themselves pinned so historical
evidence cannot silently change. The 51c17b8 draft and older release verifiers
remain intact and retain their original strictness.

Prepared local files (these have **not** built or installed packages):

- `ion-current-release-inputs.json` and `verify-ion-current-release-inputs.py`.
- `verify-ion-current-release.py` and `verify-ion-current-android-package.py`.
- `verify-ion-current-build-gates.py`.
- `measure-ion-current-release-resources.py`.
- `build-ion-current-packages.ps1` — sequential Windows and offline ARM64 build.

The source/input verifier passed. A negative gate check also passed: packaging is
refused specifically because the required native gates are unfinished. The
PowerShell wrapper parses successfully. Memory is measured through the Windows
memory API because CIM access is restricted here; the original headroom limits
are preserved (2.5 GiB physical, 4 GiB commit, 1.5 GiB C: and 4 GiB D: free, no
other Cargo/rustc/Java build). No process is stopped to meet these limits.

## Work still required

The original owner recovery build remains live as session **32910**. A separately
pinned follow-up, session **92797**, waits for that exact result and resource
headroom, then runs authenticated mobile presence, credential scope/transport
tests, combined desktop/core/client type checking and generated bindings. The
earlier bf2ce72 waiter exited on source drift before launching another build;
its receipt is preserved. Do not restart the original build because it is quiet.

New EXE/ZIP/APK output, upgrade/data retention acceptance, native UI/accessibility
and real Android recording are still unverified. Sandbox provider-image/kernel,
harness authentication, price ceilings and live lifecycle/cost acceptance remain
open. The original Cloudflare meter identity/plan and post-native-rollout
attribution are also open. iOS and deferred harness adapters remain excluded.

No final QA handoff has been sent and the persistent implementation goal remains
active. Plans 160–162 describe completed milestones without treating these open
gates as finished.
