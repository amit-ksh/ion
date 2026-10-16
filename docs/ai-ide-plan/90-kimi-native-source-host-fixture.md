# Kimi released-source host fixture

Updated 2026-10-16. Development branch: `codex/ion-agent-teams`.

## Scope

The released Kimi 2.2.0 engine now executes in an isolated source host with the official ACP composition, native protocol adapter and a localhost HTTP model server. The fixture adds exactly five Ion-shaped tools before the main agent's first prompt. It uses the native default profile and authentication gate, with no replacement model/authentication/permission services and no `extraSeeds`.

This proves a native extension contract for a separately packaged source host. It does not install an adapter in the owner's selected `kimi.exe`, wire the shared Ion worker, create real Teams work, or establish owner-account acceptance. The fixture tools return a disposable Engineering catalog result; their schemas/results are contract probes, not the production Teams API. Kimi's existing ACP path and bounded completion adapter are unchanged. Coverage remains **24/28 integration paths (about 86% by count)**.

[Chapter 89](89-kimi-source-host-contract.md) records the exact lifecycle interfaces and primary source links. [Chapter 83](83-kimi-current-contract.md) records why installed external hooks/MCP alone do not establish this contract.

## Pinned inputs

| Input                   | Identity                                                                                                                                      |
| ----------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| Official release commit | `472192c34c1efad89a369870eff0da0b708ce9bb`                                                                                                    |
| Full source archive     | [Official versioned archive](https://codeload.github.com/MoonshotAI/kimi-code/zip/472192c34c1efad89a369870eff0da0b708ce9bb), 28,151,669 bytes |
| Archive SHA-256         | `20e2c7920ddf2bcff986b118b3534961ecc5fe85e48809203201e03182fc6df8`                                                                            |
| Frozen lockfile SHA-256 | `a30a1ad0bd2ca52e6f9e8d2116c0c4aa7a655e1a3c3a3fc309360294ea9e9639`                                                                            |
| Node used               | `24.19.0`; released minimum is `24.15.0`                                                                                                      |
| Package manager         | `pnpm 10.33.0`, as pinned by the release                                                                                                      |
| pnpm archive integrity  | `sha512-EFaLtKavtYyes2MNqQzJUWQXq+vT+rvmc58K55VyjaFJHp21pUTHatjrdXD1xLs9bGN7LLQb/c20f6gjyGSTGQ==`                                             |
| Source loader           | Locked `tsx 4.21.0` plus the upstream `build/register-raw-text-loader.mjs`                                                                    |

Full source and the package manager are kept under `D:/personal/apps/waku/.waku-cache/ion-team-harness-contracts/kimi-2.2.0-source-host`. The earlier 2.1.1 research subset is not a runnable build input. Archive entries were checked to remain within the explicit extraction directory. The sole `CLAUDE.md -> AGENTS.md` symlink was materialized as a regular file containing its relative target, matching a Windows checkout convention; runtime source was not changed. `source-identity.json` and `toolchain/pnpm-identity.json` record the cache provenance.

The ACP workspace dependency graph was installed with the released lockfile:

```powershell
# Run in the extracted released source root.
$nativeNode='C:/Users/ksami/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin/node.exe'
$env:PATH=(Split-Path $nativeNode)+';'+$env:PATH
& $nativeNode '../../toolchain/pnpm/package/bin/pnpm.cjs' --filter '@moonshot-ai/acp-server...' install --frozen-lockfile --ignore-scripts --store-dir '../../pnpm-store' --reporter append-only
```

This selected nine of 22 workspace projects and 732 dependency packages. Lifecycle scripts were disabled. Successful source loading and the native file reader are verified; a distributable bundle, all native PTY/default-tool operations, platform assets and license packaging remain separate shipping gates. No global Node, pnpm or Volta configuration was changed.

## Native behavior covered

- Early registration through an additive Session-scoped service, with main-agent-only callbacks and no replaced native service.
- A native control session's complete assembled tool definitions equal those retained in the extended session. The selected model, project instruction marker, default manual mode and exact configuration file remain intact.
- Exactly the five additional Ion names are advertised. Profile/model refresh reactivates their in-memory overlays without removing native active tools.
- `ToolCallStarted` from the exact lifecycle context independently witnesses native session/agent/generation/turn/call identity and original arguments before the executable callback. Missing or changed witness data returns a native tool error.
- The native approval request precedes successful execution; denial or Stop at approval starts no callback. Stop during the callback propagates through the native AbortSignal.
- Results travel through the native executor into the next real HTTP model request under the native call ID.
- Identical calls have distinct native IDs. Released Kimi deliberately deduplicates equal name/argument calls within a step, so only the original executes and the native synthetic call reuses its result. A later turn executes again. The fixture preserves this behavior and never treats a synthetic event as another Ion worker invocation.
- The real native `Read` tool reads a disposable local file and returns its contents alongside the added Ion tools.
- Native close revokes the old context; load/resume creates a different lifecycle context, reattaches the tools once and returns subsequent native results in the same session. Cleanup is scoped to each Session service.
- A preexisting same-name tool prevents all Ion registration and remains intact.

The first identical-call expectation assumed four callbacks. Investigation of the released `AgentToolDedupeService` and actual results showed four independent observed native IDs but two original executable callbacks over two turns. The regression now asserts both counts and all successful native results; native deduplication was not disabled or patched. [Released deduplication service](https://github.com/MoonshotAI/kimi-code/blob/472192c34c1efad89a369870eff0da0b708ce9bb/packages/agent-core-v2/src/agent/toolDedupe/toolDedupeService.ts).

## Verification

**11 distinct native cases pass.** The complete suite passed with 130 Bun assertions, zero failures and zero skips. A subsequent focused rerun of the strengthened profile-refresh case passed (nine Bun assertions; the other ten cases were intentionally filtered). It now applies an actual native restrictive binding snapshot and rebinds the same native profile, rather than relying on re-selecting the same model, which Kimi treats as a no-op. The controller additionally checks native definitions, policy, configuration bytes, runtime/source identities and lifecycle state.

Strict standalone TypeScript for the test runner, Node syntax validation, Prettier and Git diff checks pass. No Rust/mobile production sources changed, so their unchanged build/test suites were not repeated for this fixture-only increment.

The fixture launches a child with an allowlist of environment fields and a disposable home, app-data paths, temporary directory and workspace. The fake key is fixture-only. Non-loopback sockets/named pipes are refused; each successful case also checks that no nonlocal connection was attempted. Request bodies are bounded. Native host/storage/connection shutdown is awaited, and recursive cleanup verifies the exact temporary directory remains within its explicit parent. No owner credential, paid provider call, visual test, watcher restart, cloud deployment or installed CLI/configuration change is included.

Run after preparing the pinned cache (replace the Node path only with the required verified runtime):

```powershell
$env:ION_TEST_KIMI_SOURCE='D:/personal/apps/waku/.waku-cache/ion-team-harness-contracts/kimi-2.2.0-source-host/source/kimi-code-472192c34c1efad89a369870eff0da0b708ce9bb'
$env:ION_TEST_KIMI_NODE='C:/Users/ksami/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin/node.exe'
$env:ION_NATIVE_FIXTURE_ROOT='D:/personal/apps/waku/.waku-cache/ion-team-harness-contracts/kimi-2.2.0-source-host'
bun test scripts/fixtures/kimi-source-host-contract.test.ts --timeout 180000
```

Without explicit source/runtime paths these native fixtures are skipped, not counted as passes. The controller verifies lockfile, ACP composition and executor hashes before loading source.

## Remaining implementation and acceptance

- [ ] Connect the witnessed callback to the existing shared Ion worker and real production tool schemas; retain owner approval, intersected grants, durable request identities and run/token limits.
- [ ] Verify changed-team refusal, worker-side Stop/replay/queued-turn fencing, stale callbacks and real invocation/result review through that shared worker.
- [ ] Produce and explicitly select a separately versioned source-host distribution, preserving configured Kimi home, model/account routing, native tools/permissions and supported login delegation. Do not silently replace the selected stock binary or claim it loads an unsupported plugin.
- [ ] Verify native PTY/platform assets, artifact integrity/version gates and license notices for a distributable host.
- [ ] Review authenticated owner accounts, actual project permissions and additional platforms. The localhost fake-account fixture does not satisfy these checks.

These remain part of the active implementation goal. Fx source-task support, native Amp/Cursor bounded completion contracts, dynamic document CSS names and owner/browser/device/infrastructure acceptance remain open in the [release checklist](20-agent-teams-release-checklist.md).
