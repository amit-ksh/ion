# OpenCode 2 native plugin contract

Updated 2026-10-15. Implemented normal-task Teams bridge for the verified native 2.0.29 runtime.

## Verified local evidence

The official Windows **2.0.29** package was downloaded into the ignored contract cache without global installation. Its npm SHA-512 integrity was verified before reading the archive; only the named regular executable was extracted. Archive SHA-256: `23b82cd749d63722c10f0364983c15d2bdab8651e5960c82c112607ac1ac3a95`. Native executable SHA-256: `2ddf0d3364aecac6a196c5508566713e837964eda6b19a152b8f5da98324949d`.

The matching `@opencode/plugin` package was independently checksum-verified. Archive SHA-256: `df8318286ffea206faad0262754585d84692a61172a15d6f6f0f3ad675455a0d`. Its Promise tool context extends the native session/assistant-message/call identity and adds an AbortSignal. The lightweight plugin definition helper returns the supplied object; a generated local plugin can avoid importing a separately installed dependency tree.

An actual native private-server fixture passed automatic `.opencode/plugins/` discovery, location-scoped plugin RPC, in-memory configuration and tool registration/reload. Status reported native version 2.0.29, the exact private workspace and initially unbound state. Configure returned ready with exactly the fixture tool. The server used isolated home/config/data/state/cache/temp paths, a fake private Basic credential and no real account/model call. Only its owned native process tree was stopped afterward.

## Transport choice

The older cached beta-17823 executable sends only tool name/arguments to MCP. The released 2.0.29 executable additionally forwards `ai.opencode/sessionID`, but its MCP call still does not forward native assistant-message/call IDs. Session metadata alone does not bind an owner approval to one exact native model tool call.

The supported plugin tool executor has those native IDs directly. **Ion implementation direction:** register the five existing team actions through an additive project plugin, keep callback endpoint/credential in native plugin memory via its location-scoped RPC, and independently witness each main-session call through Ion's adopted service stream before shared-worker dispatch. Preserve the selected native model/account, project plugins, adopted service ownership and existing grants/limits/replay/Stop behavior.

Native RPC at `/api/rpc/<rpcID>/<method>` accepts `{input: ...}` and returns `{output: ...}` for the probed methods. Released 2.0.29 requires explicit input/output schemas in this fixture; omitting a status input schema caused an internal validator error before its handler. The final explicit-schema fixture passed. Do not treat earlier RPC failures as service or account failures.

## Implementation status

- [x] Generalize the existing pinned project-plugin lifetime helper for `.opencode/plugins/` without weakening Amp cleanup/link/existing-file guarantees.
- [x] Generate task-stable native tool names and a credential-free plugin; configure/revoke only this generation through the authenticated location RPC, with abort and stale-generation fencing.
- [x] Add the ordinary OpenCode 2 workspace entry point and source bridge, matching live main-session/message/call/name/arguments; never authorize from history or reconnect snapshots.
- [x] Route owner approvals and fence failed callbacks, Stop, execution end, disconnect and teardown without stopping the user's adopted service.
- [x] Prove native tool execution/result return and Stop through a fake loopback provider, with owner approval and preserved native plugins/service; verify durable replay separately through the exact source witness/worker path.
- [x] Check release/version and executable-name compatibility. V2 stable distribution uses the `opencode` executable name, while the older Ion catalogue command is `opencode2`; do not silently reinterpret the V1 entry.
- [ ] Complete actual owner-account/platform acceptance, including native resume/hot upgrade with the owner's configured model and projects. The private fixture does not certify authenticated execution.

## Primary sources

- [OpenCode V2 native distribution](https://dev.opencode.ai/v2/docs)
- [Windows 2.0.29 package metadata](https://registry.npmjs.org/@opencode%2fcli-windows-x64/2.0.29)
- [Plugin 2.0.29 package metadata](https://registry.npmjs.org/@opencode%2fplugin/2.0.29)
- [Native plugin tools and lifecycle](https://dev.opencode.ai/v2/docs/build/plugins)
- [Native plugin RPC](https://dev.opencode.ai/v2/docs/build/plugins/rpc/)

The source bridge is now implemented and verified within the pinned native contract; the coverage matrix records this separately from owner-account acceptance.

## 2026-10-15 implementation increment

The ordinary driver now installs the additive native plugin and binds its callback only in memory through the adopted service's authenticated location RPC. The owner independently observes live native session/assistant-message/tool-call/name/arguments before routing the five actions through the existing Teams worker. The plugin requests native permission before callback dispatch. Native tool names are stable per Ion task; generation replacement aborts older callbacks and stale revocation cannot clear a newer binding.

The project plugin lifetime helper now shares a lease across concurrent/replacement drivers and checks original bytes before last-owner cleanup on a background worker. Pre-existing or owner-modified plugins remain untouched. The adopted native service is never stopped by the driver. Prompt rejection, Stop, disconnect, terminal execution, failed calls and source teardown cancel pending authority; reconnect snapshots do not create witnesses.

The bridge is intentionally gated to the verified **2.0.29** plugin runtime, rechecked at bind time. Other native versions retain ordinary execution with an explicit Teams-unavailable activity. Keep the OpenCode 2 binary override pointed at the owner's released V2 executable (`opencode`); the existing V1 entry is not reinterpreted or replaced.

A private native fixture found two release differences missed by the older beta integration: `/api/health` is absent (404), and inbox creation time is encoded as `time.created`. Discovery now falls back to `/api/info` only after 404, retains the descriptor PID check and refuses authentication/transport failures. Inbox decoding supports both native timestamp shapes while rejecting missing/conflicting values. Both final native fixtures pass through the actual Ion driver; registration success alone was not used as acceptance.

The isolated checks use a fake account, a localhost compatible model, private home/config/data/cache/state/temp and a privately owned foreground native server. They do not certify an authenticated owner account, mobile/device behavior, completion allowance contracts or other harnesses. No visual test is included.

The generated tool reached the shared owner callback during the native fixture. A subsequent fixture rejection exposed that `session.tool.called.executed` describes provider-hosted execution: local plugin tools use `false`. Ion now rejects provider-hosted calls as source authority, accepts the witnessed local path, and relies on separate failed/terminal events for cancellation. The final native fixtures assert this flag directly from an independent subscription to the actual native stream.

## Verified results

- Shared source-team suite: **36 passed**, 8 native/account fixtures skipped by default.
- OpenCode 2 API/service/driver suite: **66 passed**, 12 native/account fixtures skipped by default. Two source-bridge tests overlap the shared suite.
- Both new ignored native fixtures explicitly run and passed: approved invocation/result return and Stop at owner approval. **102 distinct checks passed** across these runs.
- The real native turn advertises the generated tool, creates one durable run after Ion approval and returns that run's actual ID to the second local model request. Stopping at approval creates no run.
- Native binding replacement/revocation, stale-generation refusal, credential-free project source, owner plugin preservation, final owned-file cleanup and adopted-server survival pass in both native fixtures.
- Shared tests cover changed/foreign/partial/provider-hosted/failed/terminal calls, durable exact replay, Stop and file handoff across replacement drivers. No past/history snapshot is accepted as a fresh call.

The private host retains an isolated profile and fake provider key. The actual Windows 2.0.29 executable is used, with the checksum recorded above. No live provider account, paid request, real desktop interaction or visual/device test is used.

Locked offline desktop/daemon binary checks, workspace Rust formatting, Markdown formatting and diff checks pass. Existing dead-code/future-compatibility warnings remain; no build failure was observed.

## Owner review

- [ ] Set the **OpenCode 2 binary override** to the installed V2 2.0.29 executable. Released V2 uses `opencode`; keep the V1 entry pointed at its own executable. Explicit binary overrides are already supported by provider discovery.
- [ ] Sign in/configure the model in native OpenCode, then confirm the same selected model/account works in an ordinary Ion task.
- [ ] Ask the ordinary task to invoke a saved team. Check its exact objective/access approval, one durable run and returned run status/documents.
- [ ] Stop before accepting the team request; verify no run is created. Reject an approval and verify the same result.
- [ ] Resume/restart the ordinary task and check existing run identity, without duplicate invocation or accepting stale callbacks. Test a mode change while the task is open.
- [ ] Keep an existing project plugin; verify it remains unchanged and native tools/account configuration remain usable after Ion task closure.
- [ ] Review native service restart/upgrade and explicit Teams-unavailable reporting on unverified versions. Hot-upgrade/account acceptance is not proven by the pinned fixture.

Re-run the private native checks with the verified V2 binary (PowerShell):

```powershell
$env:ION_NATIVE_OPENCODE2_BINARY = (Get-Command opencode).Source
cargo test --locked -p waku-core --lib opencode2_source_team_native -- --ignored --test-threads=1
```

The command starts only fixture-owned foreground servers with fake local model responses. It does not use the owner's service or account.
