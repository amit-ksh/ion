# Cursor native binding and hook transport

Updated 2026-10-16. Implementation branch: `codex/ion-agent-teams`. This is an implementation checkpoint for the remaining Cursor source-task Teams bridge. It does **not** enable the adapter or change the 23/28 implemented-path count.

## Verified native contract

The pinned native Windows CLI from [chapter 82](82-cursor-current-contract.md) now runs a working pre-tool hook. A private per-workspace cache hook can rewrite a field before the native permission request and MCP dispatch. Repeated calls with identical original arguments receive distinct hook nonces and distinct actual native call IDs. The permission's JSON content contains the rewritten arguments; the main stream independently retains the original arguments and native server/tool/call identity. A hook UUID remains distinct from the native call ID.

The exact staged [PowerShell hook template](../../crates/waku-core/src/driver/source_teams/cursor/hook.ps1) is exercised by the native fixture. It reads its private token from the native process environment and sends UTF-8 to an authenticated loopback endpoint with no proxy or redirects, a two-second client deadline, a 44 KiB request ceiling and a 64 KiB response ceiling. Denied, redirected, stalled, malformed and oversized responses cannot reach MCP. Native supplementary Unicode survives in the current fixture host. The native callback uses the ordinary selected fake Cursor account/model route, with built-ins and approval retained; this is not authenticated owner-account acceptance.

## Native Windows issues isolated

A Node helper waiting for standard input timed out. An inline PowerShell hook with explicit completion works. Cursor's shell executor normally captures state after a command; the working form exits its wrapper. This is an observed workaround for the pinned release, not a claim that every Windows shell or later native release behaves identically.

The pinned loader's `parseJSONC` removes line-comment markers without respecting strings. A literal URL in a hook command therefore invalidated the hook configuration and silently removed that boundary. The implementation constructs its URI with `UriBuilder`, retaining no literal comment markers in generated configuration. The generated command contains only the port and reads a validated 64-hex transport token from `ION_CURSOR_TEAM_HOOK_TOKEN`. Cursor can include a failed command in its native error, so the token is deliberately absent from the command/configuration text. Missing or invalid environment authority refuses the call.

Malformed-hook errors produced compressed native Connect envelopes. The fixture now honors the declared compression before decoding messages; the original failure was in the fixture decoder. [Connect streaming envelope semantics](https://connectrpc.com/docs/protocol/#streaming-request) explain those bits. No provider protocol code in Ion is changed by the fixture correction.

## Staged Rust binding state

The [binding module](../../crates/waku-core/src/driver/source_teams/cursor/binding.rs) now retains separate native-call and hook identities. It checks the observed MCP provider/tool/original arguments, current main session, hook name/identity, rewritten nonce, exact permission content and native approval before a callback can yield a worker invocation. Internal binding fields never become worker arguments.

Calls are one-use and cannot regain authority after Stop, terminal events, expiry or a new turn. Reused IDs remain tombstones. Input is bounded to 40 KiB per canonical call, 1 MiB of retained canonical arguments per active turn, and 2,048 identities of each kind per binding generation. Correlation expires after 30 seconds; a witnessed native owner-approval wait has a separate five-minute ceiling. These limits are staged code semantics and still need transport/worker integration acceptance.

## Remembered native approval

A real fixture demonstrates that choosing native **Allow always** skips the permission witness on the next rewritten call. The integration must expose one-time approval for its uniquely named Ion tools and preserve ordinary native permissions. It must refuse any callback without the exact owned permission witness, including a pre-approved/wildcard native route. It cannot infer an ID from arguments, use a hook UUID as an engine ID, or silently reset owner allowlists. The worker's own grants and approval remain required too.

## Verification

- [x] Final native fixture: **20 pass, zero failures/skips, 202 assertions in 79.79 seconds**. This includes the exact staged HTTP template with environment-bound authentication; preceding exploratory runs are not added to the total.
- [x] Twelve Rust binding-state cases pass, covering matching identity, identical calls, metadata forgery, changed/foreign/malformed input, native approval, denial, Stop, terminal events, repeated initial/hook IDs, replay, expiry, new-turn fences and retained-data/identity limits.
- [x] Strict standalone fixture TypeScript, Rust formatting and diff checks pass.
- [x] Final current-source Rust run: **14 pass, zero failures/ignored cases**. This includes all twelve binding cases and both updated hook-command checks.
- [x] Locked offline desktop/both-daemon binary compilation passes, including the dev daemon. Existing warnings remain, and the staged Cursor components add unused-code warnings until their transport integration is wired.

**34 distinct cases pass:** 20 native runtime cases plus 14 Rust cases. Earlier runs are not added. No visual/device test, watcher restart, owner key, paid inference, deployment or ion-prod merge is included.

## Source-turn ownership

The native hook's `generation_id` is the session ID in this release, not a prompt generation. Transport integration must retain Ion-owned queued/active-turn fences and invalidate authority before cancellation. Late hook/MCP callbacks, native observations and pending approvals must not regain authority after Stop or driver replacement. Actual native-driver Stop/recovery tests are required; the standalone state tests do not establish transport ordering or process termination.

## Next production integration

- [ ] Install an owned entry in the exact native workspace-cache hook file, preserving owner fields/other hooks, concurrent source drivers, edits during lifetime and final cleanup. Refuse linked/changed paths and incompatible native versions.
- [ ] Add uniquely named session MCP tools and the private bounded hook/MCP transport. Keep the selected native account/model/default tools and session workspace.
- [ ] Feed the actual ACP observation and permission messages into the binding module, remove remembered-approval options for Ion tools only, and record the selected native approval before callback consumption. Scrub binding data from product activity/transcripts.
- [ ] Connect accepted invocations to the existing shared worker and durable native call ID, retaining owner grants, changed-definition refusal, queue limits, result return and exact document reads.
- [ ] Fence queued prompts, Stop, failed/finished native calls, process replacement, resume and cleanup. Cancel already queued worker operations when native authority is revoked.
- [ ] Exercise the actual production driver against the native fixture, including approval/replay/changed team/Stop/resume and overlapping-driver preservation. Verify a route with remembered/wildcard permissions refuses cleanly.
- [ ] Repeat the supported transport on registered worker platforms; complete authenticated owner-account acceptance later.

Cursor bounded Teams completion remains a separate native output/spending/account/usage contract. Fx/Kimi source bridges, Amp completion and the remaining document CSS/font fidelity are still active goal work.

## Re-run

Use the native fixture environment and exact runtime from chapter 82:

```powershell
bun test scripts/fixtures/cursor-source-teams-contract.test.ts
```

Run the `driver::source_teams::cursor` Rust library filter with the existing locked/offline command/profile from the preceding implementation guides. Missing native runtime configuration skips the native cases and does not establish acceptance. The CLI account, localhost services, hooks and MCP server are disposable; the owner profile is not used. The final completed fixture run left no matching fake-profile process.
