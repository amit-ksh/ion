// Isolated released-source contract probe. This is not an installed-CLI adapter.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createServer } from "node:http";
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import net from "node:net";
import tls from "node:tls";

const source = process.env.ION_TEST_KIMI_SOURCE;
const root = process.env.ION_KIMI_CONTRACT_TEMP;
const mode = process.argv[2] ?? "allow";
assert(source && root, "Explicit isolated source and disposable home required");
assert(
  [
    "allow",
    "identical",
    "deny",
    "missing-witness",
    "changed-witness",
    "stop-execution",
    "collision",
    "profile-refresh",
    "native-read",
    "stop-approval",
    "resume",
  ].includes(mode),
  "Unknown fixture mode",
);
assert.equal(process.version, "v24.19.0", "Fixture runtime identity changed");
for (const [file, hash] of [
  [
    "pnpm-lock.yaml",
    "a30a1ad0bd2ca52e6f9e8d2116c0c4aa7a655e1a3c3a3fc309360294ea9e9639",
  ],
  [
    "packages/acp-server/src/start.ts",
    "b4a460764ce8e999cd7f6798bf07a1293cf353ac741aec5e7c243bd86f71805f",
  ],
  [
    "packages/agent-core-v2/src/agent/toolExecutor/toolExecutorService.ts",
    "c9b0a6f0ba91a9908dfceef482f1e76e98d8e69800a1921ce5869dbdd9e59b3b",
  ],
])
  assert.equal(
    createHash("sha256")
      .update(readFileSync(join(source, file)))
      .digest("hex"),
    hash,
    file,
  );

// No owner credentials enter this child. Refuse all non-loopback sockets too.
const blocked = [];
function networkCheck(raw) {
  let args = raw;
  if (Array.isArray(args[0])) args = args[0];
  let host;
  if (typeof args[0] === "object") {
    if (args[0].path) throw Error("Fixture named pipes are disabled");
    host = args[0].host ?? args[0].hostname ?? "localhost";
  } else if (typeof args[0] === "number")
    host = typeof args[1] === "string" ? args[1] : "localhost";
  else throw Error("Fixture network paths are disabled");
  if (!["localhost", "127.0.0.1", "::1"].includes(host)) {
    blocked.push(host);
    throw Error("Fixture blocked nonlocal network");
  }
}
const connect = net.Socket.prototype.connect;
net.Socket.prototype.connect = function (...args) {
  networkCheck(args);
  return connect.apply(this, args);
};
const tlsConnect = tls.connect;
tls.connect = function (...args) {
  networkCheck(args);
  return tlsConnect.apply(this, args);
};
const load = (relative) => import(pathToFileURL(join(source, relative)).href);
const core = await load("packages/agent-core-v2/src/index.ts");
const { createTestClient } = await load(
  "packages/acp-server/test/_helpers/acpClient.ts",
);
const { createDecorator } = await load(
  "packages/agent-core-v2/src/_base/di/instantiation.ts",
);
const { registerScopedService, ScopeActivation } = await load(
  "packages/agent-core-v2/src/_base/di/scope.ts",
);
const { LifecycleScope } = await load(
  "packages/agent-core-v2/src/app/scopes.ts",
);
const { ISessionEventBus } = await load(
  "packages/agent-core-v2/src/app/event/eventBus.ts",
);
const { ISessionContext } = await load(
  "packages/agent-core-v2/src/session/sessionContext/sessionContext.ts",
);
const { IAgentProfileService } = await load(
  "packages/agent-core-v2/src/agent/profile/profile.ts",
);
const { ToolCallStarted, ToolResultEvent } = await load(
  "packages/agent-core-v2/src/agent/toolExecutor/toolExecutorEvents.ts",
);
const { AgentStatusUpdated } = await load(
  "packages/agent-core-v2/src/agent/usage/usageEvents.ts",
);
const names = [
  "list_teams",
  "invoke_team",
  "get_team_runs",
  "await_team",
  "read_team_document",
];
const observations = [],
  callbacks = [],
  results = [],
  permissions = [],
  failures = [],
  registrations = [];
const requests = [],
  sessions = new Map(),
  revocations = [],
  promptResults = [];
let installIon = false;
let reachedCallback;
const callbackStarted = new Promise((resolve) => {
  reachedCallback = resolve;
});

class IonContractSession {
  constructor(lifecycle, session, bus) {
    this.disposables = [];
    this.states = [];
    this.disposables.push(
      lifecycle.onDidCreateScope(({ context, handle }) => {
        if (!installIon || context.agentId !== "main") return;
        try {
          const registry = handle.accessor.get(core.IAgentToolRegistryService);
          const profile = handle.accessor.get(IAgentProfileService);
          const baseline = registry.list();
          const state = {
            sessionId: session.sessionId,
            context,
            registry,
            profile,
            baseline,
            witnesses: new Map(),
            active: true,
            leases: [],
          };
          sessions.set(session.sessionId, state);
          this.states.push(state);
          // Detect all collisions before making the first additive mutation.
          if (mode === "collision")
            state.leases.push(
              registry.register(
                {
                  name: names[0],
                  description: "owner tool",
                  parameters: { type: "object" },
                  resolveExecution: () => ({
                    approvalRule: names[0],
                    execute: async () => ({ output: "owner preserved" }),
                  }),
                },
                { source: "user" },
              ),
            );
          if (names.some((name) => registry.resolve(name))) {
            failures.push("collision");
            return;
          }
          const activate = () => {
            if (state.active)
              for (const name of names) profile.addActiveTool(name);
          };
          state.leases.push(bus.onAgent(context, AgentStatusUpdated, activate));
          state.leases.push(
            bus.onAgent(context, ToolCallStarted, (event) => {
              if (
                !names.includes(event.name) ||
                bus.sourceOf(event) !== context ||
                !state.active
              )
                return;
              const item = {
                sessionId: session.sessionId,
                agentId: event.agentId,
                generation: context.generation,
                turnId: event.turnId,
                callId: event.toolCallId,
                name: event.name,
                args: event.args,
              };
              observations.push(item);
              if (mode !== "missing-witness")
                state.witnesses.set(
                  `${event.turnId}:${event.toolCallId}`,
                  mode === "changed-witness"
                    ? { ...item, args: { query: "changed" } }
                    : structuredClone(item),
                );
            }),
          );
          state.leases.push(
            bus.onAgent(context, ToolResultEvent, (event) => {
              if (bus.sourceOf(event) === context)
                results.push({
                  callId: event.toolCallId,
                  isError: event.isError === true,
                  output: event.output,
                });
            }),
          );
          for (const name of names)
            state.leases.push(
              registry.register(
                {
                  name,
                  description: `Ion native contract ${name}`,
                  parameters: {
                    type: "object",
                    properties: { query: { type: "string" } },
                    additionalProperties: false,
                  },
                  resolveExecution(args) {
                    return {
                      approvalRule: name,
                      async execute(ctx) {
                        const key = `${ctx.turnId}:${ctx.toolCallId}`;
                        const witness = state.witnesses.get(key);
                        state.witnesses.delete(key);
                        if (
                          !state.active ||
                          ctx.signal.aborted ||
                          !witness ||
                          witness.name !== name ||
                          JSON.stringify(witness.args) !== JSON.stringify(args)
                        )
                          return {
                            output:
                              "Ion contract refused missing or changed witness",
                            isError: true,
                          };
                        callbacks.push({ ...witness, aborted: false });
                        reachedCallback();
                        if (mode === "stop-execution") {
                          await new Promise((resolve) => {
                            if (ctx.signal.aborted) resolve();
                            else
                              ctx.signal.addEventListener("abort", resolve, {
                                once: true,
                              });
                          });
                          callbacks.at(-1).aborted = true;
                          return {
                            output: "Ion contract cancelled",
                            isError: true,
                          };
                        }
                        return {
                          output: JSON.stringify({
                            teams: [{ name: "Engineering" }],
                            nativeCallId: ctx.toolCallId,
                          }),
                        };
                      },
                    };
                  },
                },
                { source: "user", disclosure: "inline" },
              ),
            );
          activate();
          registrations.push({
            sessionId: session.sessionId,
            main: context.agentId,
            before: baseline.map((t) => t.name),
            added: registry
              .list()
              .filter((t) => !baseline.some((b) => b.name === t.name))
              .map((t) => t.name),
          });
        } catch (error) {
          failures.push(String(error));
        }
      }),
    );
    this.disposables.push(
      lifecycle.onWillClose((context) => {
        for (const state of sessions.values())
          if (state.context === context) {
            state.active = false;
            state.witnesses.clear();
            revocations.push({
              sessionId: state.sessionId,
              generation: context.generation,
            });
          }
      }),
    );
    this.disposables.push(
      lifecycle.onDidClose((context) => {
        for (const state of sessions.values())
          if (state.context === context)
            for (const lease of state.leases.splice(0).reverse())
              lease.dispose();
      }),
    );
  }
  dispose() {
    for (const state of this.states) {
      state.active = false;
      state.witnesses.clear();
      for (const lease of state.leases.splice(0).reverse()) lease.dispose();
    }
    for (const lease of this.disposables.splice(0)) lease.dispose();
  }
}
// Apply native DI parameter decorators without requiring a second TS compiler.
core.IAgentLifecycleService(IonContractSession, undefined, 0);
ISessionContext(IonContractSession, undefined, 1);
ISessionEventBus(IonContractSession, undefined, 2);
registerScopedService(
  LifecycleScope.Session,
  createDecorator("ionSourceContractSession"),
  IonContractSession,
  ScopeActivation.OnScopeCreated,
  "ionSourceContractSession",
);

let turn = 0;
const server = createServer(async (req, res) => {
  try {
    const chunks = [];
    let bytes = 0;
    for await (const c of req) {
      bytes += c.length;
      assert(bytes <= 1024 * 1024);
      chunks.push(c);
    }
    const body = JSON.parse(Buffer.concat(chunks).toString("utf8"));
    assert.equal(req.url, "/v1/chat/completions");
    assert.equal(req.headers.authorization, "Bearer ion-disposable-native-key");
    assert.equal(body.model, "ion-native-model");
    requests.push(body);
    const last = body.messages.at(-1);
    const call = last.role !== "tool";
    const id = `chatcmpl-ion-${requests.length}`;
    const chunk = (delta, finish_reason = null) => ({
      id,
      object: "chat.completion.chunk",
      created: 1,
      model: "ion-native-model",
      choices: [{ index: 0, delta, finish_reason }],
    });
    const count = mode === "identical" ? 2 : 1;
    const calls = Array.from({ length: count }, (_, index) => ({
      index,
      id: `ion-call-${turn}-${index}`,
      type: "function",
      function:
        mode === "native-read" && turn === 1
          ? {
              name: "Read",
              arguments: JSON.stringify({
                path: join(root, "work", "evidence.txt"),
              }),
            }
          : { name: "list_teams", arguments: '{"query":"Engineering"}' },
    }));
    const events = call
      ? [
          chunk({ role: "assistant", tool_calls: calls }),
          chunk({}, "tool_calls"),
        ]
      : [
          chunk({ role: "assistant", content: "Native contract complete" }),
          chunk({}, "stop"),
        ];
    if (call) turn++;
    res.writeHead(200, { "content-type": "text/event-stream" });
    for (const event of events) res.write(`data: ${JSON.stringify(event)}\n\n`);
    res.end("data: [DONE]\n\n");
  } catch (error) {
    failures.push(String(error));
    res.writeHead(500);
    res.end("fixture error");
  }
});
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const homeDir = join(root, "home"),
  work = join(root, "work");
mkdirSync(homeDir);
mkdirSync(work);
writeFileSync(join(work, "evidence.txt"), "ion-native-builtin-read-marker\n");
writeFileSync(
  join(work, "AGENTS.md"),
  "# Native contract\nPreserve ion-workspace-instruction-marker.\n",
);
const config = `defaultModel = "ion-fixture"\n[models.ion-fixture]\nname = "ion-native-model"\nprotocol = "openai"\nbaseUrl = "http://127.0.0.1:${server.address().port}/v1"\napiKey = "ion-disposable-native-key"\nmaxContextSize = 32768\n`;
writeFileSync(join(homeDir, "config.toml"), config);
let client;
const deadline = setTimeout(() => {
  process.stderr.write("Kimi native contract exceeded deadline\n");
  process.exit(2);
}, 120_000);
try {
  // No registry/auth/model test seeds and native authentication stays enabled.
  client = await createTestClient({ homeDir, disableAuth: false });
  client.onRequest("session/request_permission", (params) => {
    permissions.push(params);
    if (mode === "stop-approval") {
      queueMicrotask(() =>
        client.notify("session/cancel", { sessionId: params.sessionId }),
      );
      return { outcome: { outcome: "cancelled" } };
    }
    return {
      outcome: {
        outcome: "selected",
        optionId: mode === "deny" ? "reject" : "approve_once",
      },
    };
  });
  await client.send("initialize", {
    protocolVersion: 1,
    clientCapabilities: {},
  });
  const control = await client.send("session/new", {
    cwd: work,
    mcpServers: [],
  });
  const controlHandle = core.getLiveSessionById(
    client.server.core.accessor,
    control.sessionId,
  );
  const controlAgent = controlHandle.accessor
    .get(core.IAgentLifecycleService)
    .handleOf("main");
  const nativeDefaults = controlAgent.accessor
    .get(core.IAgentToolRegistryService)
    .list();
  assert(
    nativeDefaults.length > 0,
    "Native default profile has no assembled tools",
  );
  installIon = true;
  const created = await client.send("session/new", {
    cwd: work,
    mcpServers: [],
  });
  const sessionId = created.sessionId;
  assert(sessions.has(sessionId), "Early lifecycle observer missed main agent");
  const state = sessions.get(sessionId);
  assert.equal(created.modes.currentModeId, "default");
  const selectedModel = state.profile.getModel();
  assert.equal(selectedModel, "ion-fixture");
  const baselineActive = state.profile.getActiveToolNames();
  if (mode === "collision") {
    assert.equal(state.registry.resolve(names[0]).description, "owner tool");
    assert.equal(
      state.registry.list().filter((t) => names.includes(t.name)).length,
      1,
    );
  } else {
    assert.equal(failures.length, 0, failures.join("; "));
    assert.deepEqual(registrations[0].added.toSorted(), names.toSorted());
    const nativeAfter = state.registry
      .list()
      .filter((t) => !names.includes(t.name));
    assert.deepEqual(
      nativeAfter,
      nativeDefaults,
      "Native built-in definitions changed",
    );
    registrations[0].before = nativeDefaults.map((t) => t.name);
    if (mode === "profile-refresh") {
      // Re-selecting the same model is a native no-op. Use the actual native
      // profile bind so this check exercises a real activation-overlay reset.
      const original = state.profile.data();
      state.profile.applyBindingSnapshot({
        ...original,
        activeToolNames: nativeDefaults.map((t) => t.name),
      });
      for (const name of names)
        assert(
          state.profile.getActiveToolNames().includes(name),
          `Native snapshot refresh lost ${name}`,
        );
      await state.profile.bind({
        profile: original.profileName,
        model: selectedModel,
      });
      assert.equal(state.profile.getModel(), selectedModel);
    }
    const pending = client.send("session/prompt", {
      sessionId,
      prompt: [{ type: "text", text: "Inspect the saved Engineering team" }],
    });
    if (mode === "stop-execution") {
      await callbackStarted;
      client.notify("session/cancel", { sessionId });
    }
    promptResults.push(await pending);
    if (["identical", "profile-refresh", "native-read"].includes(mode))
      promptResults.push(
        await client.send("session/prompt", {
          sessionId,
          prompt: [{ type: "text", text: "Inspect Engineering again" }],
        }),
      );
    if (mode === "resume") {
      const original = state.context;
      await client.send("session/close", { sessionId });
      assert.equal(
        state.active,
        false,
        "Closing native agent did not revoke authority",
      );
      await client.send("session/load", {
        sessionId,
        cwd: work,
        mcpServers: [],
      });
      const restored = sessions.get(sessionId);
      assert.notEqual(
        restored.context,
        original,
        "Resume reused stale lifecycle authority",
      );
      assert.deepEqual(
        restored.registry.list().filter((t) => !names.includes(t.name)),
        nativeDefaults,
      );
      assert.equal(restored.profile.getModel(), selectedModel);
      promptResults.push(
        await client.send("session/prompt", {
          sessionId,
          prompt: [
            { type: "text", text: "Inspect Engineering after native resume" },
          ],
        }),
      );
    }
    assert.equal(
      readFileSync(join(homeDir, "config.toml"), "utf8"),
      config,
      "Native profile rewritten",
    );
    const current = sessions.get(sessionId);
    for (const builtin of nativeDefaults.map((t) => t.name))
      assert(current.registry.resolve(builtin), `Built-in lost: ${builtin}`);
    for (const name of names)
      assert(
        current.profile.getActiveToolNames()?.includes(name) ?? true,
        `Inactive Ion tool ${name}`,
      );
    if (baselineActive)
      for (const name of baselineActive)
        assert(
          current.profile.getActiveToolNames().includes(name),
          `Native active tool lost: ${name}`,
        );
  }
  await client.close();
  client = undefined;
  assert.equal(
    failures.filter((x) => x !== "collision").length,
    0,
    failures.join("; "),
  );
  process.stdout.write(
    JSON.stringify({
      mode,
      node: process.version,
      registrations,
      observations,
      callbacks,
      results,
      permissions,
      requests,
      failures,
      selectedModel,
      blocked,
      revocations,
      promptResults,
    }) + "\n",
  );
} finally {
  clearTimeout(deadline);
  await client?.close();
  server.closeAllConnections();
  await new Promise((resolve) => server.close(resolve));
}
