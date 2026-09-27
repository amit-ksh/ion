# Waku Mobile

Expo client for connecting to one or more remote Waku daemons from iOS,
Android.

## Run

From the repository root:

```sh
bun install
bun --filter @waku/mobile ios
bun --filter @waku/mobile android
```

## Connect

In Waku Desktop, enable the remote daemon and copy its WebSocket address and
token. Add those values in the mobile app. Use `wss://` outside a trusted LAN
or private tailnet; the token grants full control of the daemon host.

Saved profile metadata stays in app storage. On iOS and Android, daemon tokens
are stored separately in the device keychain through Expo SecureStore.

## Personal gateway approval review

Open **Personal gateway** from the start or Daemons screen. At the gateway's
`/owner` page, create a one-use invite with **Review approvals** and pair the
phone using the trusted HTTPS URL. A phone can see exact pending Composio
actions and approve or deny them; the gateway owner executes an approved
action separately. The phone token is kept in the native keychain. Removing
the local pairing does not revoke the device: revoke it in `/owner` too.

The existing direct daemon connection still provides task views and controls
using its full daemon token. Gateway-mediated task inspection and narrow
steering are later Milestone 5 development, tracked in
[`docs/ai-ide-plan/12-mobile-polish.md`](../../docs/ai-ide-plan/12-mobile-polish.md).

Waku is licensed under the repository's GNU GPL v3.0-only license.
