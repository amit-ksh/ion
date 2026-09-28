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
