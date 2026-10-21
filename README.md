# ion

ion is an AI work IDE that coordinates code, connected apps, agent teams and
computers through reviewable tasks. The desktop is built in Rust with
[GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui).

## Install

Use the ion installer supplied with your release. On Windows, run
`ion-<version>-<arch>-Setup.exe`; the portable archive contains `ion.exe`.
Keep the portable folder intact so its daemon, computer-use and speech helpers
remain available. See [Windows setup](docs/windows.md).

The [ion website](https://build-with-ion.tech) provides download links when
release artifacts are published. Older upstream download links are not ion
releases.

Sign in with your approved ion account on desktop or mobile. Local tasks use
agent CLIs installed and authenticated on your computer; Nebius Token Factory
can use your personal key or the operator-configured managed service. Cloud
features need a deployed ion gateway and enabled operator policy.

## Supported agents

ion detects these agent CLIs (available execution modes vary by adapter):

- [Amp](https://ampcode.com/)
- Claude Code
- Codex CLI
- Cursor CLI
- [Fx](https://fx.sh/)
- Grok Build
- Kimi Code
- OpenCode
- Pi

Install and authenticate at least one supported agent CLI before starting ion.
ion detects available CLIs automatically and uses each provider's native
structured protocol and session continuity.

You can also use Nebius Token Factory with your own API key. Save and test the key under **Settings →
Models & Speech**, then create a Nebius task. See the
[native runtime check](docs/ai-ide-plan/09-native-nebius-runtime.md).

## Highlights

- Keep projects and independent agent sessions in one native app.
- Switch models, reasoning effort, and access modes from a shared interface.
- Queue or steer follow-up messages while an agent is working.
- Rewind Git-backed tasks with conversation-aware checkpoints.
- Keep projects and transcripts locally; sign in to access desktop and mobile.

## Architecture

The native desktop is an RPC client of the standalone `waku-daemon` process.
Provider sessions run in [`waku-core`](crates/waku-core), behind the
authenticated, versioned WebSocket contract in
[`waku-protocol`](crates/waku-protocol). ion Desktop depends on
[`waku-client`](crates/waku-client), not on the daemon implementation. The
daemon owns task SQLite data, uploaded attachments, provider-native session
forks, and all workspace filesystem and Git operations; paths returned by it
always refer to the daemon host. The desktop retains only presentation state
and a disposable preview cache.

The browser client lives at [`apps/web`](apps/web) and uses the generated
browser transport in [`packages/waku-client`](packages/waku-client). Its
checked-in types are generated directly from the Rust protocol, while its
WebSocket client implements the same handshake, request IDs, subscriptions,
sequence deduplication, and replay cursors as the Rust client. Run
`bun run protocol:generate` after changing a wire type and
`bun run protocol:check` to verify that generated files are current.

Projectless task workspaces live on the daemon host under
`~/.waku/projects/<date>/<slug>`. The daemon moves workspaces created by the
older `~/.waku/<date>/<slug>` layout on first load.

Configuration ownership is separate too: the Release desktop writes
`~/.waku/app.json`, while Debug stays isolated at `temp/app.json`. Daemon
provider and Computer Use settings live in `~/.waku/settings.json`. The
desktop's Settings → Daemon page can explicitly
expose the child daemon on a fixed port, configure exact browser origins, and
copy its stable authentication token. It remains loopback-only by default.

When connected to a daemon managed outside the desktop process, ion never
interprets daemon paths on the client machine. The local folder picker and PTY
are therefore unavailable until the protocol gains daemon-host picker and
terminal-stream endpoints; files, diffs, Git, skills, usage, task state, and
attachments already use daemon RPC.

Release apps bundle and sign `waku-daemon`. Development keeps the daemon at
`target/debug/waku-debug-daemon`, allowing provider-only edits to rebuild and
replace the daemon without relaunching ion Debug.

Internal Cargo/package names, bundle IDs, protocol keys and existing data
paths retain their legacy identifiers for upgrade compatibility. Product
labels and distributed Windows app names use ion.

## Development

Development is supported on macOS, Linux, and Windows and requires
[Rust 1.96 or newer](https://www.rust-lang.org/tools/install) and
[Bun](https://bun.sh/). Linux supports both Wayland and X11, and Windows needs
the MSVC toolchain; install the native build prerequisites listed in
[CONTRIBUTING.md](CONTRIBUTING.md) first.

```sh
bun install
bun run dev
```

For Windows desktop development, use
`bun install --frozen-lockfile --ignore-scripts` instead of `bun install`.
The workspace includes a mobile
dependency whose iOS postinstall invokes the Unix `unzip` command; the desktop
watcher does not need that postinstall.

Computer Use requires the bundled native helper and platform permissions. Agent sessions, projects, transcripts, skills, usage,
diffs, file editing, and the terminal run natively on Linux and Windows.
Phone dictation can use the selected desktop daemon's local ASR, and the same
daemon can run headlessly on an owner-managed Linux host for personal cloud
tasks. See the [Milestone 7 setup](docs/ai-ide-plan/14-phone-asr-cloud-execution.md).

See [CONTRIBUTING.md](CONTRIBUTING.md) for the development workflow and checks.
Release maintainers should also read [RELEASING.md](RELEASING.md).

## License

ion is licensed under the [GNU General Public License v3.0 only](LICENSE).
