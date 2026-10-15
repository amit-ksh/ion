# Grok source-task Teams bridge implementation

Updated 2026-10-15. Branch: `codex/ion-agent-teams`.

## Current work

The production ACP driver now carries an optional Grok source context and registers the five shared Teams tools through native session setup. The released Windows 1.0.46 integration passes actual driver acceptance and is included in [current source-level coverage](64-remaining-harness-integrations.md). Authenticated owner-account and other-platform acceptance remain open. [Released native contract evidence](75-grok-native-source-contract.md) remains distinct from actual driver acceptance.

The owner first observes the initial native tool call. The project's native command hook can then request a private per-call binding. A separate ACP pre-tool hook confirms the same session/call/name/arguments before the MCP callback reaches the existing bounded Teams worker. A callback credential alone cannot mint native authority. Original provider account/model and the native project-trust policy remain selected.

The adapter uses a unique `.grok/hooks/ion-teams-<source-task>.json` lease. Existing hooks and settings are retained; the last lease removes only unchanged owned bytes. Generated files contain the helper command and tool matcher; endpoint and bearer credentials stay in the native child environment and ACP MCP registration. The helper uses the existing shipped daemon executable.

Queued turns receive owner tickets. Stop invalidates tickets already queued, pending native bindings and shared-worker approvals. A later completion cannot close a newer active ticket. Source task/project identity, access intersection, durable invocation replay and document evidence checks remain owned by the shared worker/coordinator. Private binding fields and values are removed from native user-facing notifications and permission details. A changed command-hook request also revokes an already pending shared-worker approval. Retained native arguments have a 1 MiB aggregate limit per active turn; rejected identities remain bounded tombstones across cancellation and later turns.

## Verification

- [x] Seven production bridge regressions pass: approval/durable replay/catalogue, Stop/turn tickets, foreign/truncated/forged/changed requests, expiry/failure, hook leases/private-field scrubbing, changed-hook revocation during approval and aggregate argument retention/reset.
- [x] Rerun shared source-bridge and ACP regressions after the parser/lease and owner-lifecycle changes: 33 source-bridge and 21 ACP cases pass; excluded native/account cases are not counted.
- [x] Run actual released Grok 1.0.46 through the Ion ACP driver and shipped hook helper for approved durable invocation/result return, Stop at Ion approval and native resume without duplicate work.
- [x] After native resume/replay, change the team definition and send a fresh native call with its old revision. The shared worker refuses it before another approval, returns its refusal to the provider and leaves exactly one durable run.
- [x] Verify native project trust, existing hook preservation and final owned-file cleanup in the released Windows runtime fixtures.
- [x] Locked offline desktop/daemon checks (including the dev-binary feature), Rust formatting and diff checks. Existing compiler warnings remain.

## Native acceptance investigation

The three actual-driver fixtures pass: approved durable invocation/result return with native resume/replay; Stop at the separate Ion approval; and refusal in an untrusted project. No live account, paid provider request or owner file is used. Hook files survive driver handoff and are removed only after the last lease releases; existing owner hooks remain byte-for-byte intact. The approved/resumed case also changes the team definition and verifies the shared worker returns a refusal for the stale revision without a second run or approval. Across this checkpoint, **57 distinct cases pass**: 33 shared source-bridge, 21 ACP and three explicit actual-Grok driver fixtures. Ignored account/other-runtime tests are not counted.

The first positive fixture failed because its credential-isolation setup mishandled Windows environment keys. The fixture now normalizes names before clearing inherited values, restores the explicit system/path allowlist and sets a valid empty Git configuration count. Hook-discovery diagnostics then confirmed the private project source loaded. Production folder trust and directory pins were retained. Temporary standalone contract probes were removed after inspection.

## Owner review

- [ ] Configure the supported native Grok executable and your existing account/model. Open a disposable owner-trusted project and ask a normal source task to invoke a saved team.
- [ ] In Supervised mode, review the separate team access/limit approval. Confirm queued work is shown as queued and completed evidence is read before a final claim.
- [ ] Stop while the approval is pending, then send a new turn. The canceled request must not create work or become valid again.
- [ ] Resume the same source task and verify exact native invocation replay produces one durable run. Changed objective/revision must be refused.
- [ ] Review an untrusted project: Ion must not grant blanket Grok folder trust. Native hook trust remains your choice.
- [ ] Change access mode or close the task; late callbacks must be refused. Confirm existing `.grok` hooks/settings remain intact and unchanged generated files are eventually removed.

The remaining harnesses, wider CSS fidelity and authenticated/device acceptance remain part of the active full goal.

## Re-run released-runtime acceptance

Run from this implementation worktree with a built Ion daemon and the checksum-verified native 1.0.46 executable. Tests use private temporary accounts/repositories and localhost Responses; no live provider credential is required.

```powershell
$env:ION_TEST_GROK_BINARY = "D:/personal/apps/waku/.waku-cache/ion-team-harness-contracts/grok-contract/runtime/grok.exe"
$env:ION_TEST_ION_DAEMON = "D:/personal/apps/waku/target/debug/waku-daemon.exe"
cargo test --locked --offline -p waku-core --lib grok_source_team_native_driver_ -- --ignored --test-threads=1
```

All three native cases must execute. The normal unignored test run intentionally skips these executable-dependent cases. The shipped helper is unchanged from the separately verified native helper checkpoint. No visual test, watcher restart, deployment or merge to `ion-prod` occurred.
