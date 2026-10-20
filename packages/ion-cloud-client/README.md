# Ion native account client

The shared client provides Authorization Code + PKCE S256 login, API-verified
account identity, vault-backed session persistence, serialized rotating refreshes,
crash/ambiguity recovery and local sign-out with best-effort remote revocation.

Platform code supplies cryptographic randomness/SHA-256, a secure vault and the
system browser. No client secret, password API, embedded login browser, unverified
ID-token identity or browser storage is used.

A refresh is marked in the vault before it is sent. If its response is lost or the
app restarts mid-refresh, the app clears that session and requires fresh login.
It never blindly retries a consumed refresh token. Token changes are committed
before access is returned to gateway callers.

The mobile native adapter is wired to Expo SecureStore and the system browser.
The desktop adapter is implemented in the native `waku-credentials` crate and shares its OS vault/refresh lock with the daemon. See [desktop integration](../../docs/ai-ide-plan/110-desktop-cloud-login.md), the
[mobile implementation and acceptance checklist](../../docs/ai-ide-plan/109-mobile-cloud-login.md)
and [cloud operator configuration](../ion-cloud/README.md).

```sh
bun test packages/ion-cloud-client/src
bun run --cwd packages/ion-cloud-client check
```
