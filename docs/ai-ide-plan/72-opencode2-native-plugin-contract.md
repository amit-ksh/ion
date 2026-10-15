# OpenCode 2 native plugin contract

Updated 2026-10-15. Investigation for the remaining normal-task Teams bridge.

## Verified local evidence

The official Windows **2.0.29** package was downloaded into the ignored contract cache without global installation. Its npm SHA-512 integrity was verified before reading the archive; only the named regular executable was extracted. Archive SHA-256: `23b82cd749d63722c10f0364983c15d2bdab8651e5960c82c112607ac1ac3a95`. Native executable SHA-256: `2ddf0d3364aecac6a196c5508566713e837964eda6b19a152b8f5da98324949d`.

The matching `@opencode/plugin` package was independently checksum-verified. Archive SHA-256: `df8318286ffea206faad0262754585d84692a61172a15d6f6f0f3ad675455a0d`. Its Promise tool context extends the native session/assistant-message/call identity and adds an AbortSignal. The lightweight plugin definition helper returns the supplied object; a generated local plugin can avoid importing a separately installed dependency tree.

An actual native private-server fixture passed automatic `.opencode/plugins/` discovery, location-scoped plugin RPC, in-memory configuration and tool registration/reload. Status reported native version 2.0.29, the exact private workspace and initially unbound state. Configure returned ready with exactly the fixture tool. The server used isolated home/config/data/state/cache/temp paths, a fake private Basic credential and no real account/model call. Only its owned native process tree was stopped afterward.

## Transport choice

The older cached beta-17823 executable sends only tool name/arguments to MCP. The released 2.0.29 executable additionally forwards `ai.opencode/sessionID`, but its MCP call still does not forward native assistant-message/call IDs. Session metadata alone does not bind an owner approval to one exact native model tool call.

The supported plugin tool executor has those native IDs directly. **Ion implementation direction:** register the five existing team actions through an additive project plugin, keep callback endpoint/credential in native plugin memory via its location-scoped RPC, and independently witness each main-session call through Ion's adopted service stream before shared-worker dispatch. Preserve the selected native model/account, project plugins, adopted service ownership and existing grants/limits/replay/Stop behavior.

Native RPC at `/api/rpc/<rpcID>/<method>` accepts `{input: ...}` and returns `{output: ...}` for the probed methods. Released 2.0.29 requires explicit input/output schemas in this fixture; omitting a status input schema caused an internal validator error before its handler. The final explicit-schema fixture passed. Do not treat earlier RPC failures as service or account failures.

## Implementation still required

- [ ] Generalize the existing pinned project-plugin lifetime helper for `.opencode/plugins/` without weakening Amp cleanup/link/existing-file guarantees.
- [ ] Generate task-stable native tool names and a credential-free plugin; configure/revoke only this generation through the authenticated location RPC, with abort and stale-generation fencing.
- [ ] Add the ordinary OpenCode 2 workspace entry point and source bridge, matching live main-session/message/call/name/arguments; never authorize from history or reconnect snapshots.
- [ ] Route owner approvals and fence failed callbacks, Stop, execution end, disconnect and teardown without stopping the user's adopted service.
- [ ] Prove native tool execution and result return through a fake loopback provider, owner-approved invocation/replay/Stop and preserved native profile/plugins.
- [ ] Check release/version and executable-name compatibility. V2 stable distribution uses the `opencode` executable name, while the older Ion catalogue command is `opencode2`; do not silently reinterpret the V1 entry.
- [ ] Complete actual owner-account/platform acceptance. No full OpenCode 2 source-team implementation or authenticated model execution is claimed by this probe.

## Primary sources

- [OpenCode V2 native distribution](https://dev.opencode.ai/v2/docs)
- [Windows 2.0.29 package metadata](https://registry.npmjs.org/@opencode%2fcli-windows-x64/2.0.29)
- [Plugin 2.0.29 package metadata](https://registry.npmjs.org/@opencode%2fplugin/2.0.29)
- [Native plugin tools and lifecycle](https://dev.opencode.ai/v2/docs/build/plugins)
- [Native plugin RPC](https://dev.opencode.ai/v2/docs/build/plugins/rpc/)

These native probes change the next implementation seam; the coverage matrix remains unchanged until the bridge itself is implemented and verified.
