# waku-daemon

`waku-daemon` is the standalone process that hosts Waku's provider sessions.
It defaults to a loopback-only listener, authenticates clients with
`WAKU_DAEMON_TOKEN`, and
prints one JSON readiness record to stdout containing its address, protocol
version, and process ID.

```text
WAKU_DAEMON_TOKEN=<secret> waku-daemon --bind 127.0.0.1:0 [--parent-pid PID] [--allow-origin ORIGIN]...
```

An owner-managed Linux service can read a private token from
`WAKU_DAEMON_TOKEN_FILE` instead of placing its value in the environment.
For native Nebius tasks, it may read the provider key from
`WAKU_NEBIUS_KEY_FILE`. These paths are intended for a dedicated daemon user
and are validated as private regular files. The full [personal cloud setup
and review checklist](../../docs/ai-ide-plan/14-phone-asr-cloud-execution.md#personal-cloud-execution)
includes the service and TLS proxy examples.

Waku Desktop supervises this process. Debug builds use the feature-gated
`waku-debug-daemon` target at `target/debug/waku-debug-daemon`, so rebuilding
provider code replaces only the daemon. Release distributions place the signed
`waku-daemon` binary beside the desktop executable.

The token is a full-control capability for a trusted Waku client, not a user or
workspace-scoped credential. Browser handshakes are rejected unless their exact
Origin was supplied with `--allow-origin`; native clients send no Origin. A
non-loopback bind is refused unless `--allow-non-loopback` is also present.
Waku Desktop adds that flag only after the user enables exposure in Settings →
Daemon. The daemon does not terminate TLS itself. For access outside a private
network, put a trusted TLS proxy or tunnel in front of it and use `wss://`. Do
not give the daemon token to untrusted page JavaScript.

## Outbound workflow owner

The production daemon publishes workflow/routine/run metadata and consumes
revision-bound phone commands on a dedicated worker. It uses the same user OS
vault pairing as desktop gateway app actions. Desktop temporary (memory-only)
pairing retains its existing desktop bridge; it cannot keep operating after
that desktop process stops.

A Linux service without a credential vault can set the optional absolute path
`ION_GATEWAY_PAIRING_FILE`. Register this host as its own gateway device with
`publish` (and `propose` only if agents need app-action proposals). Store the
pairing as a private JSON file owned by the service user with mode `0600` and
fields `baseUrl`, `deviceId`, `token` and `expiresAt` (Unix **milliseconds**).
It must be a regular file, at most 16 KiB, without symlinks or hard links. The
service fails startup on invalid configuration, without printing the token.
It re-reads the file for rotation and does not fall back to a vault pairing
when that configured source is missing, expired or invalid. The path is removed
from the process environment before harness workers start.

See [headless owner setup and review](../../docs/ai-ide-plan/31-headless-gateway-owner.md).
No inbound daemon exposure is required for this outbound workflow relay. The
existing direct daemon connection still provides full transcripts, documents and
editing; gateway solo-task mirroring currently needs the desktop bridge.
