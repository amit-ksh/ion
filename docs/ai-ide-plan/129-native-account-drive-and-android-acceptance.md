# Native account, managed Drive action and Android acceptance

21 October 2026. `ion-prod`; Windows, Android and shared backend only.

## Real native account boundary

The owner completed the actual Rust PKCE callback, not a transferred browser
token. Windows Credential Manager stored the session through the existing
chunked/atomic vault implementation. Reopening the vault and verifying the
account with production passed. The native session remains signed in.

An explicit opt-in test expired the local access-token timestamp, then used the
normal native authorization path to refresh and commit credentials. Production
identity remained bound to the original workspace; a request pinned to a
different workspace was rejected. No token, refresh credential or callback code
was logged or copied into source files.

Native production checks also passed managed catalog access, combined spending
readout, inference-OFF denial and app-action-OFF denial. These checks did not
change service switches or execute paid inference.

Evidence in `.waku-cache/`:

- `ion-native-rust-login.log`: actual callback/vault test passed.
- `ion-native-rust-refresh.log`: actual refresh/scope test passed.
- `ion-native-rust-managed-live.log`: managed catalog/controls test passed.

Final non-live native account regression in the actual Windows user context:
**19 passed / 6 opt-in live tests ignored / 0 failed**. The sandbox-context run
could not access Credential Manager (OS error 1312); the same isolated vault
roundtrip passed in the real user context. This was an execution-context limit,
not a reason to weaken or skip the real OS-vault check.

## Google Drive: real consent and one approved provider action

The owner selected Google Drive and approved the Composio Connect Link. The
native connection test discovered the toolkit from the production catalog,
prepared the account-scoped managed link and observed ACTIVE ownership.

An optional personal Composio project key already exists in the user's vault.
It was preserved. The live shared-service action acceptance therefore selected
the managed native account transport explicitly; it does not claim that the
personal credential facade or a chat/Teams UI was exercised in this test.

The first discovery query, `about`, returned an empty valid catalog. A subsequent
`list` query discovered the real `GOOGLEDRIVE_GET_ABOUT` tool, published version
`20261018_00`, and its actual schema. This was not an authentication failure.

One exact proposal was prepared and approved with arguments
`{"fields":"storageQuota"}`. The owner app-action switch was temporarily enabled,
one execution was sent and a successful provider result with a nonempty Composio
log ID was received. The result included `storageQuota`. Only result field names
and the receipt ID were retained as sanitized acceptance evidence; no file
contents, quota values or account profile data were logged in the report.

The prior app-action switch was restored. Cloud and Token Factory switches were
unchanged. The durable budget conservatively retains a **1000 micro-USD
(USD 0.001)** hold until actual billing is reconciled; a successful ordinary call
does not establish its final invoice cost. The opt-in test writes dispatch intent
before execution and refuses to repeat a previous attempt using that report path.

Evidence: `ion-native-drive-connection.json`, `ion-native-drive-tools.json`,
`ion-native-drive-quota-action.json` and their matching test logs in the cache.
The selected connection stays active for the owner's future use.

## Android package and physical Moto G32

The release-variant ARM64 APK build succeeded after 39 minutes. Verified:

- Package `sh.waku.mobile`, Ion display label, version 1.0.0/code 1.
- Minimum Android API 24; target/compile API 36; ARM64 only.
- `RECORD_AUDIO` is declared.
- APK Signature Scheme v2 verifies against the existing local Android debug
  signing identity. This is a sideloadable test package, not store signing.
- File length **53531504 bytes**, SHA-256
  **3df611ff92930468fc000d7b06a840c080eec6f796044208c6358f67fedfb4f0**.

Release copy:
`target/release/ion-android-1.0.0-20261021-arm64-v8a.apk`.

Build provenance remains the recorded `6bde270` snapshot plus the preserved
pre-existing QA patch/untracked build-source hashes in
`.waku-cache/ion-managed-release-inputs.json`. Later R2 manifest/docs and ignored
native acceptance tests do not change the phone's compiled code. Do not describe
this binary as reproducible from a clean commit alone.

`adb install -r` succeeded without uninstalling or clearing user data. After a
temporary USB disconnect, the user reconnected and confirmed owner login. A
nonvisual cold start succeeded in 2878 ms, process remained live and the bounded
startup log check found no fatal/native-JS startup signatures. The primary
Android user's microphone permission is granted; another device profile's
permission remains separate. No screenshots or native UI inspection were done.

Evidence: `ion-managed-android-package-final.log`, `ion-managed-android-signature.log`,
`ion-managed-android-badging.log`, `ion-managed-android-install.log`,
`ion-managed-android-launch.log` and `ion-managed-android-startup-errors.log`.

## Remaining gates

- Real recorded phone speech, selected-desktop pairing and transcript insertion.
- R2-enabled Windows packaging refresh, bundled runtime check and install/upgrade.
- Actual chat/Teams/workflow interactions, account switching, keyboard/UI review.
- Token Factory sandbox permissions/images and real lifecycle acceptance.
- Isolated app host/controller setup, real builds, sleep/wake/stop and invoices.
- Live second-account publishing denial; local workerd already covers this case.

No Nebius VM or sandbox was created or started for the checks above. The latest
IAM v2 project read reports the approved Compute project ACTIVE. The deprecated
v1 project's old suspension field must not be used as the current project state.
