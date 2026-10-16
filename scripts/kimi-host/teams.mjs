// Additive native Session service for the pinned Kimi engine; no provider loop.
import { request } from "node:http";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { isDeepStrictEqual } from "node:util";
const names = [
  "list_teams",
  "invoke_team",
  "get_team_runs",
  "await_team",
  "read_team_document",
];
const prefix = "ion_teams_";
const refused =
  "Ion Teams could not verify this active native call. Retry from the task; no invocation is automatically retried.";
function id(value) {
  return (
    typeof value === "string" &&
    value.length > 0 &&
    Buffer.byteLength(value) <= 256 &&
    !/[\u0000-\u001f\u007f-\u009f]/u.test(value)
  );
}
function jsonArgs(args) {
  return (
    args !== null &&
    typeof args === "object" &&
    !Array.isArray(args) &&
    Buffer.byteLength(JSON.stringify(args)) <= 40 * 1024
  );
}
export function connection(endpoint, token) {
  const base = new URL(endpoint);
  if (
    base.protocol !== "http:" ||
    base.hostname !== "127.0.0.1" ||
    !base.port ||
    base.pathname !== "/" ||
    base.username ||
    base.password ||
    base.search ||
    base.hash ||
    !/^[a-f0-9]{64}$/.test(token)
  )
    throw Error(refused);
  return (route, value, signal, timeout = 340000) =>
    new Promise((resolve, reject) => {
      const bytes = Buffer.from(JSON.stringify(value));
      if (bytes.length > 44 * 1024) {
        reject(Error(refused));
        return;
      }
      const timerSignal = AbortSignal.timeout(timeout);
      const req = request(
        new URL(route, base),
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            "Content-Length": bytes.length,
            Authorization: `Bearer ${token}`,
          },
          signal: signal ? AbortSignal.any([signal, timerSignal]) : timerSignal,
        },
        (res) => {
          let size = 0;
          const chunks = [];
          res.on("data", (chunk) => {
            size += chunk.length;
            if (size > 512 * 1024) req.destroy(Error(refused));
            else chunks.push(chunk);
          });
          res.on("error", () => reject(Error(refused)));
          res.on("end", () => {
            try {
              if (res.statusCode !== 200) throw Error(refused);
              resolve(JSON.parse(Buffer.concat(chunks).toString("utf8")));
            } catch {
              reject(Error(refused));
            }
          });
        },
      );
      req.on("error", () => reject(Error(refused)));
      req.end(bytes);
    });
}
export async function installTeams(source, post) {
  const load = (relative) =>
    import(
      pathToFileURL(
        join(source, "packages/agent-core-v2/src", relative + ".ts"),
      ).href
    );
  return installNativeTeams(post, load);
}
export async function installNativeTeams(post, load) {
  const definitions = await post("/tools", {}, undefined, 10000);
  if (
    !Array.isArray(definitions) ||
    definitions.length !== names.length ||
    definitions.some(
      (d, i) =>
        d?.type !== "function" ||
        d.function?.name !== names[i] ||
        d.function?.parameters?.type !== "object" ||
        typeof d.function?.description !== "string",
    )
  )
    throw Error(refused);
  const core = await load("index");
  const { createDecorator } = await load("_base/di/instantiation");
  const { registerScopedService, ScopeActivation } =
    await load("_base/di/scope");
  const { LifecycleScope } = await load("app/scopes");
  const { ISessionEventBus } = await load("app/event/eventBus");
  const { ISessionContext } = await load(
    "session/sessionContext/sessionContext",
  );
  const { IAgentProfileService } = await load("agent/profile/profile");
  const { ToolCallStarted } = await load(
    "agent/toolExecutor/toolExecutorEvents",
  );
  const { AgentStatusUpdated } = await load("agent/usage/usageEvents");
  class IonTeamsSession {
    constructor(lifecycle, session, bus) {
      this.states = [];
      this.leases = [];
      const revoke = (state) => {
        if (!state.active) return;
        state.active = false;
        state.witnesses.clear();
        for (const controller of state.pending) controller.abort();
        state.pending.clear();
        void post("/revoke", state.identity, undefined, 5000).catch(() => {});
      };
      this.revoke = revoke;
      this.leases.push(
        lifecycle.onDidCreateScope(({ context, handle }) => {
          if (context.agentId !== "main") return;
          const registry = handle.accessor.get(core.IAgentToolRegistryService);
          const profile = handle.accessor.get(IAgentProfileService);
          // The native registry replaces names: check every collision before any write.
          if (names.some((name) => registry.resolve(prefix + name)))
            throw Error(
              "Ion Teams tool name already exists; reconnect after resolving the collision.",
            );
          if (
            !id(session.sessionId) ||
            !Number.isSafeInteger(context.generation) ||
            context.generation < 0
          )
            throw Error(refused);
          const state = {
            context,
            identity: {
              session_id: session.sessionId,
              agent_id: "main",
              generation: context.generation,
            },
            active: true,
            witnesses: new Map(),
            pending: new Set(),
            leases: [],
            bytes: 0,
          };
          this.states.push(state);
          // Registration is synchronous before native profile binding/activation.
          // Execution awaits this handshake; normal Kimi authentication stays enabled.
          state.ready = post("/context", state.identity, undefined, 10000).then(
            (reply) => {
              if (reply?.ok !== true) throw Error(refused);
            },
          );
          state.ready.catch(() => revoke(state));
          const activate = () => {
            if (state.active)
              for (const name of names) profile.addActiveTool(prefix + name);
          };
          state.leases.push(bus.onAgent(context, AgentStatusUpdated, activate));
          state.leases.push(
            bus.onAgent(context, ToolCallStarted, (event) => {
              if (
                !state.active ||
                bus.sourceOf(event) !== context ||
                event.agentId !== "main" ||
                !names.includes(event.name.slice(prefix.length)) ||
                !event.name.startsWith(prefix)
              )
                return;
              if (
                !Number.isSafeInteger(event.turnId) ||
                event.turnId < 0 ||
                !id(event.toolCallId) ||
                !jsonArgs(event.args)
              )
                return;
              const key = `${event.turnId}:${event.toolCallId}`;
              const witness = {
                name: event.name,
                args: structuredClone(event.args),
                used: false,
              };
              const previous = state.witnesses.get(key);
              if (state.witnesses.has(key)) {
                if (
                  !previous ||
                  previous.name !== witness.name ||
                  !isDeepStrictEqual(previous.args, witness.args)
                ) {
                  state.witnesses.set(key, null);
                  void post(
                    "/cancel",
                    {
                      ...state.identity,
                      turn_id: event.turnId,
                      call_id: event.toolCallId,
                      name: event.name.slice(prefix.length),
                      arguments: event.args,
                    },
                    undefined,
                    5000,
                  ).catch(() => {});
                }
                return;
              }
              const bytes = Buffer.byteLength(JSON.stringify(event.args));
              if (
                state.witnesses.size >= 2048 ||
                state.bytes + bytes > 1024 * 1024
              ) {
                revoke(state);
                return;
              }
              state.bytes += bytes;
              state.witnesses.set(key, witness);
            }),
          );
          try {
            for (const definition of definitions) {
              const def = definition.function;
              const name = prefix + def.name;
              state.leases.push(
                registry.register(
                  {
                    name,
                    description: def.description,
                    parameters: def.parameters,
                    resolveExecution(args) {
                      const original = structuredClone(args);
                      return {
                        approvalRule: name,
                        async execute(ctx) {
                          const key = `${ctx.turnId}:${ctx.toolCallId}`;
                          const witness = state.witnesses.get(key);
                          // Consume only the executable callback. Native synthetic/deduped
                          // results never create a second worker request.
                          const consumed = witness?.used;
                          if (witness) witness.used = true;
                          if (
                            !state.active ||
                            ctx.signal.aborted ||
                            !witness ||
                            consumed ||
                            witness.name !== name ||
                            !isDeepStrictEqual(witness.args, original) ||
                            !jsonArgs(original)
                          )
                            return { output: refused, isError: true };
                          const body = {
                            ...state.identity,
                            turn_id: ctx.turnId,
                            call_id: ctx.toolCallId,
                            name: def.name,
                            arguments: original,
                          };
                          const controller = new AbortController();
                          state.pending.add(controller);
                          const cancel = () => {
                            controller.abort();
                            void post("/cancel", body, undefined, 5000).catch(
                              () => {},
                            );
                          };
                          ctx.signal.addEventListener("abort", cancel, {
                            once: true,
                          });
                          try {
                            await state.ready;
                            if (
                              !state.active ||
                              ctx.signal.aborted ||
                              controller.signal.aborted
                            )
                              throw Error(refused);
                            const reply = await post(
                              "/call",
                              body,
                              controller.signal,
                            );
                            if (
                              !state.active ||
                              ctx.signal.aborted ||
                              controller.signal.aborted ||
                              typeof reply?.ok !== "boolean" ||
                              typeof reply?.text !== "string"
                            )
                              throw Error(refused);
                            return { output: reply.text, isError: !reply.ok };
                          } catch {
                            cancel();
                            return { output: refused, isError: true };
                          } finally {
                            ctx.signal.removeEventListener("abort", cancel);
                            state.pending.delete(controller);
                          }
                        },
                      };
                    },
                  },
                  { source: "user", disclosure: "inline" },
                ),
              );
            }
            activate();
          } catch (error) {
            revoke(state);
            for (const lease of state.leases.splice(0).reverse())
              lease.dispose();
            throw error;
          }
        }),
      );
      this.leases.push(
        lifecycle.onWillClose((context) => {
          for (const state of this.states)
            if (state.context === context) revoke(state);
        }),
      );
      this.leases.push(
        lifecycle.onDidClose((context) => {
          for (const state of this.states)
            if (state.context === context)
              for (const lease of state.leases.splice(0).reverse())
                lease.dispose();
        }),
      );
    }
    dispose() {
      for (const state of this.states) {
        this.revoke(state);
        for (const lease of state.leases.splice(0).reverse()) lease.dispose();
      }
      for (const lease of this.leases.splice(0)) lease.dispose();
    }
  }
  core.IAgentLifecycleService(IonTeamsSession, undefined, 0);
  ISessionContext(IonTeamsSession, undefined, 1);
  ISessionEventBus(IonTeamsSession, undefined, 2);
  registerScopedService(
    LifecycleScope.Session,
    createDecorator("ionSourceTeamsSession"),
    IonTeamsSession,
    ScopeActivation.OnScopeCreated,
    "ionSourceTeamsSession",
  );
}
