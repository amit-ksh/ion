# Kimi source-task Teams bridge

Implementation worktree: `codex/ion-agent-teams`. This is a separately selected source host for the released Kimi Code **2.2.0** engine at commit `472192c34c1efad89a369870eff0da0b708ce9bb`. The three deferred harness paths remain outside scope.

## Compiled package follow-up

[Chapter 94](94-kimi-compiled-host.md) implements and verifies the relocated Windows/x64 compiled package, bundled runtime/native assets/notices and actual native Bash without source/loader fallbacks. The source execution path below remains useful for development; its historical startup/package limitations do not describe that compiled artifact. Signed release/update authenticity, other platforms, real owner accounts and interactive terminal acceptance remain open.

## Production path

`driver/acp.rs` can select a prepared host through **`ION_KIMI_SOURCE_HOST`** from the final configured process environment. The selection affects the ordinary ACP source session. The existing Kimi native executable selection, catalog and bounded completion adapter retain their own version/account contracts. If no compatible host is selected, the ordinary native session remains available and Ion reports that Teams tools are unavailable.

Before adding the bridge, Ion runs the selected host's `--ion-host-version` probe against the final executable, arguments and environment. It requires the exact host/source/protocol identity. It never downloads a replacement or treats a stock CLI as an unsupported plugin host.

The prepared host loads the native official ACP composition. It adds exactly the five production source-worker schemas to the main agent's own registry with the `ion_teams_` prefix. It checks all names for collisions before changing any registry entry. A scoped lifecycle service attaches before profile binding, preserves native tools and restores only its own activation overlay after profile refresh. Native global/workspace/session disabled-tool policy and native permission checks continue to apply.

The executable callback must match its exact live native lifecycle context and the original `ToolCallStarted` arguments. Synthetic/deduplicated native results do not create another callback. Independently, Ion's owned ACP reader must observe the same session, canonical native `${turnId}:${toolCallId}`, tool name and complete arguments in an executing tool notification. Pending streaming fragments, progress-only updates, history, foreign sessions and changed inputs are not authority.

Only then does the existing shared worker perform catalog/invocation/status/wait/document work. Project/task identity comes from the daemon. Existing owner approval, grant intersection, definition revisions, run/token limits, durable invocation replay and accepted document scope stay in that worker. The callback URL or credential alone cannot invoke a team.

## Lifecycle and bounds

- One native main context per source connection, bound to its session and native generation; close/revoke requires a new host connection.
- Active prompt generations fence queued turns. Stop, failed calls, native abort, close and changed call data revoke worker authority.
- Native call IDs retain bounded tombstones across prompts, preventing old events or callback replay from becoming fresh approval. Native turn IDs may advance monotonically within the active request envelope during steering; lower IDs cannot open another call.
- At most 2,048 call identities and 1 MiB of observed argument data per connection; at most 40 KiB for one argument object.
- Private authenticated loopback HTTP, strict JSON, exact Host, no Origin/chunked request, shared bounded parser and bounded replies/timeouts.
- Separate bounded control/read and invocation pools so owner-approval waits do not occupy the abort/revoke handlers.
- Host callback credentials are captured and removed from its process environment before the native engine starts, keeping them out of native shell-tool environments. Bridge errors scrub the credential.
- Socket work, source checks and worker calls run on provider/background threads; rendering performs none of this work.

## Explicit local preparation

Use the already verified source archive and frozen dependency installation described in [chapter 90](90-kimi-native-source-host-fixture.md). Node **24.15+ in the Node 24 line** and its locked `tsx` **4.21.0** loader are required. The preparation command does not download dependencies, alter native configuration or replace an existing output directory.

```powershell
node scripts/kimi-host/prepare-source-host.mjs --source D:/path/to/pinned-kimi-source --node C:/path/to/node.exe --native-command C:/path/to/kimi-2.2.0.exe --output D:/path/to/new-ion-kimi-host
$env:ION_KIMI_SOURCE_HOST='D:/path/to/new-ion-kimi-host/ion-kimi-host.cmd'
```

Set that environment before starting Ion through your normal launch flow. Keep the source checkout and its frozen dependencies available. For a native JavaScript CLI, use `--native-command <node executable> --native-entry <native CLI JavaScript entry>`. Paths are separate arguments; there is no shell-built native login command.

The generated launcher supports ACP terminal auth (`acp --login`) and the legacy `login` entry. Both delegate to the explicitly configured native CLI's real `acp --login`, with the same `KIMI_CODE_HOME` and optional native region. The source host neither implements a new login flow nor stores credentials. The native CLI must satisfy the pinned 2.2.0 version probe.

This prepares a **local source host**, not a self-contained redistributable application. Critical ACP/executor/lockfile/compiler-configuration hashes and release/loader versions are checked; this is not a full source/dependency/native-asset integrity audit. Native PTY/platform assets, redistribution notices and complete release packaging remain open.

## Verification

**77 distinct executed cases pass**, with zero failures in the final runs:

| Gate                                           | Executed passes |
| ---------------------------------------------- | --------------: |
| Kimi production shared-worker/state/HTTP cases |              16 |
| Actual native-engine production-driver cases   |               6 |
| Existing ACP regressions                       |              21 |
| Existing Grok bridge regressions               |               7 |
| Existing Cursor bridge regressions             |              21 |
| Source-host CLI/setup/Windows-PTY cases        |               6 |

The CLI/Windows-PTY cases have 46 Bun assertions. The six native cases explicitly execute approval/result return, owner denial, Stop at approval, stale definition refusal, actual native built-in Read and a new driver's load/resume. They select the host through `ION_KIMI_SOURCE_HOST` while a distinct native executable remains configured, assert all five exact production parameter schemas and verify native model/configuration/workspace instructions and file bytes. The user's project compiler configuration intentionally disables decorators; native source loading uses the verified Kimi configuration and preserves that project file.

Desktop and both daemon binaries pass locked offline compiler checks. Strict standalone fixture TypeScript, Node syntax, Rust/Prettier formatting and diff checks pass. Existing compiler warnings remain. The ten optional owner/native cases ignored by the ACP/Grok/Cursor regression filters are not counted as passes.

The complete six-case native suite passed before the final monotonic-turn follow-up; that adds one policy case, and actual invocation/resume was rerun successfully against the final host shutdown behavior (41.90 seconds, connections at 24.696 and 12.969 seconds). The same test case is counted once.

Seven source-host connections measured **13.273–33.271 seconds** in the final native run (six cases, including a second connection for resume); the native suite took 177.06 seconds. This source-execution startup is a known release limitation and motivates the remaining compiled distribution work.

Earlier investigation exposed the workspace-selected TypeScript configuration disabling native decorators; pinning and verifying Kimi's own configuration fixes that defect. An intermediate 50-second cold-start fixture deadline expired; the final explicit source fixture allows 150 seconds for connection without changing production ACP deadlines. An initial Windows malformed-header test saw a connection reset because the parser rejects unauthorized headers before consuming their body; it now accepts transport closure or a negative reply for those rejected requests, while the valid authenticated route must return the exact production schemas. Intermediate failures are not counted as passes.

To rerun the six real-engine cases after setting the explicit source/runtime environment from chapter 90:

```powershell
cargo test --locked --offline -p waku-core --lib kimi_source_native_driver_ -- --ignored --nocapture --test-threads=1
bun test scripts/fixtures/kimi-source-host-cli.test.ts --timeout 60000
```

The new Rust fixtures exercise the actual shared worker against disposable durable control stores. Ignored native driver fixtures explicitly require `ION_TEST_KIMI_SOURCE` and `ION_TEST_KIMI_NODE`; an ignored case is not a pass. Native fixtures use a private home/config/workspace, an HTTP model on loopback and a nonlocal-socket guard. The fake login only records argument/home forwarding and does not prove owner authentication.

The Windows asset probe loads the pinned `node-pty` 1.1.0 prebuild under Node 24.19.0/x64, runs a literal echo in a private headless `cmd /d /q /c`, verifies output and exit code, then closes its disposable process. This proves the cached Windows module/ABI path without running install scripts or a native build. A first probe returned the correct output/exit but remained alive on a native worker until its outer deadline. The source host now follows the released CLI: await `runAcpServer` (including its ordered core/storage close), then explicitly exit. This asset probe does not establish full Kimi Bash permissions, deployed asset paths or other-platform acceptance.

## Remaining acceptance and packaging

- [x] Execute all six real-engine driver cases: approval/result, denial, Stop, changed definition, native built-in Read and new-driver resume.
- [x] Verify the cached Windows/x64 PTY prebuild loads, executes and returns under the selected Node runtime.
- [ ] Complete self-contained host distribution, fast startup, deployed native asset paths, full Kimi Bash/PTY and additional-platform acceptance, full integrity/version verification and license notices.
- [ ] Review actual native model/account catalog and configuration migration compatibility, OAuth/device login and project permissions using an owner account. Source execution of a fake HTTP model does not establish these contracts.
- [ ] Verify additional host platforms and actual owner/device review.
- [ ] Complete the remaining document/browser/device work in the [current checklist](20-agent-teams-release-checklist.md).

No live cloud resource, owner account, visual validation, watcher restart or `ion-prod` integration is part of this increment.
