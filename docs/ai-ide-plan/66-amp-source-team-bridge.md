# Amp normal-task team invocation

Updated 2026-10-14. Implementation branch: `codex/ion-agent-teams`.

## Implemented path

Amp remains the native source conversation. Its temporary project plugin registers the five existing Ion team actions. A native `tool.call` request carries the thread ID, tool-use ID and exact arguments to the owner loopback bridge. The owner independently witnesses the same call on Amp's main streaming session before dispatching to the shared source-team worker.

The source task/project and approved access come from the owner context. Model arguments cannot select another task, project, thread or source authorization. Invocation uses the existing definition revision, approval/scope intersection, durable run identity and configured agent limits. Amp's account, mode/model and original plugins/settings remain selected. Teams invoked by a normal Amp conversation may use any already-supported configured harness; this does not enable Amp as a Teams completion worker.

## Native lifecycle and installation

The supported Amp loading location is `.amp/plugins/` in the task workspace. Ion adds a uniquely task-named generated plugin. Parent components are pinned without following links. Existing files are never overwritten. A byte-identical stale generated file can be reused; a changed file blocks setup for owner review. Only the unchanged file created by this generation is removed, on a background cleanup thread. Matching preexisting files and all other project plugin/settings files are retained.

Tool names are stable for the source task across restart. Bridge endpoint and credential come from the native process environment, not the project file. A plugin registers only when its prefix matches that process's Ion generation configuration. An ordinary Amp session loading a leftover file with no Ion environment does not register these tools. Empty generated directories and crash-leftover inactive files may remain; cleanup does not recursively delete project directories.

The driver waits up to 30 seconds for plugin readiness. Owner callbacks bind only after the real native thread initialization. Other threads, nested native agents, other plugins and changed call arguments cannot authorize dispatch. Native failed tool results, end-turn, Stop, EOF and process exit revoke pending calls. Old call tombstones remain fenced when another turn begins; exact durable invocation replay across restart returns the original run.

On Windows, Amp is now contained in the existing native process-tree job and Stop closes it; the previous transport only signaled Unix. Unix Stop signals the native PID directly. Owner approval replies are routed from the driver to the shared worker.

## Verified checkpoint

Seventeen shared source-bridge regressions pass on the final executable, including the three Amp plugin file-lifetime cases and two Amp native identity/approval/replay/Stop/foreign-call cases. The generated hook also passes through the actual installed Amp plugin runtime with an isolated fake profile and localhost owner: five tools register, the native request returns the saved team, exact replay matches, and a stopped call is rejected. The test verifies the project file contains neither the live endpoint nor its credential.

The offline `amp plugins exec ... tool.call` runner emits a generic event. Request hooks use a distinct native RPC method, so that command's successful exit does not prove callback dispatch. The installed runtime request method is exercised explicitly in the committed ignored fixture using `ION_TEST_AMP_BINARY` and `ION_TEST_AMP_RUNTIME`.

Rust formatting and diff checks pass. The final locked desktop/daemon binary checks pass. These fixtures do not claim authenticated full CLI/plugin discovery, owner-account execution or live provider completion acceptance.

## Owner acceptance

The isolated installed CLI `plugins list` probe could not load global plugins without connecting to Amp; native project discovery and authenticated execution remain acceptance gates, not offline fixture claims.

- [ ] With an authenticated supported Amp CLI, start a normal task in a project and ask it to list saved Ion teams. Verify the native tool name and returned saved team.
- [ ] Ask Amp to invoke a supported team, review configured access and limits, and confirm the real team run appears under the same source project/task.
- [ ] Stop while approval is pending and while a team callback is waiting. Confirm no late callback starts a new run, and that the native Windows process tree exits.
- [ ] Resume a saved native Amp thread; inspect run identity and avoid repeating a completed invocation.
- [ ] Check existing personal/project plugins and settings still load. On close, verify only the unchanged Ion-created plugin is removed; preserve changed or preexisting files.
- [ ] Check a subdirectory workspace uses the expected native project plugin discovery root and reports setup failure clearly if discovery is unavailable.

## Remaining harness work

Amp Teams completion still needs a supported bounded completion and metered-usage contract. Cursor Teams completion still needs a native output/turn ceiling. Broader normal-task bridges and live account acceptance remain open.

## Primary references

- [Amp plugin loading and lifecycle](https://ampcode.com/docs/customize/plugins)
- [Amp plugin API](https://ampcode.com/docs/plugin-api)
- [Amp streaming JSON schema](https://ampcode.com/docs/cli/streaming-json)
