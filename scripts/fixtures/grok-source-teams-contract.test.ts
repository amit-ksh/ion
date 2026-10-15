// Native contract fixture only. No Ion Teams adapter is registered by this file.
import { expect, test } from "bun:test";
import { spawn, execFileSync } from "node:child_process";
import { randomUUID } from "node:crypto";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import {
  createServer,
  type IncomingMessage,
  type ServerResponse,
} from "node:http";
import { tmpdir } from "node:os";
import { join, resolve, sep } from "node:path";

type Json = Record<string, any>;
const binary = process.env.ION_TEST_GROK_BINARY;
const daemon = process.env.ION_TEST_ION_DAEMON;
const bindingField = "__ion_contract_binding";
const authoredArgs = { query: "Engineering" };
const nativeCalls = ["ion-contract-a", "ion-contract-b"];

function stream(items: Json[]): string {
  const id = `resp_${randomUUID()}`;
  const response = {
    id,
    object: "response",
    created_at: 1,
    model: "grok-4.6",
    status: "in_progress",
    output: [],
    usage: null,
  };
  const events: Json[] = [{ type: "response.created", response }];
  for (const [i, item] of items.entries()) {
    events.push({
      type: "response.output_item.added",
      output_index: i,
      item: {
        ...item,
        status: "in_progress",
        ...(item.type === "function_call"
          ? { arguments: "" }
          : { content: [] }),
      },
    });
    if (item.type === "function_call") {
      events.push({
        type: "response.function_call_arguments.delta",
        item_id: item.id,
        output_index: i,
        delta: item.arguments,
      });
      events.push({
        type: "response.function_call_arguments.done",
        item_id: item.id,
        output_index: i,
        arguments: item.arguments,
      });
    } else {
      events.push({
        type: "response.content_part.added",
        item_id: item.id,
        output_index: i,
        content_index: 0,
        part: { type: "output_text", text: "", annotations: [] },
      });
      events.push({
        type: "response.output_text.delta",
        item_id: item.id,
        output_index: i,
        content_index: 0,
        delta: "contract complete",
      });
      events.push({
        type: "response.output_text.done",
        item_id: item.id,
        output_index: i,
        content_index: 0,
        text: "contract complete",
      });
    }
    events.push({ type: "response.output_item.done", output_index: i, item });
  }
  events.push({
    type: "response.completed",
    response: {
      ...response,
      status: "completed",
      output: items,
      usage: {
        input_tokens: 100,
        output_tokens: 5,
        total_tokens: 105,
        input_tokens_details: { cached_tokens: 0 },
        output_tokens_details: { reasoning_tokens: 0 },
      },
    },
  });
  return events
    .map(
      (event, sequence_number) =>
        `event: ${event.type}\ndata: ${JSON.stringify({ ...event, sequence_number })}\n\n`,
    )
    .join("");
}
function json(response: ServerResponse, value: unknown, status = 200) {
  response.writeHead(status, { "content-type": "application/json" });
  response.end(JSON.stringify(value));
}
async function body(request: IncomingMessage): Promise<Json> {
  let bytes = 0;
  const chunks: Buffer[] = [];
  for await (const chunk of request) {
    bytes += chunk.length;
    if (bytes > 512 * 1024)
      throw new Error("Native fixture request exceeded 512 KiB");
    chunks.push(chunk);
  }
  return chunks.length
    ? JSON.parse(Buffer.concat(chunks).toString("utf8"))
    : {};
}
function environment(root: string): NodeJS.ProcessEnv {
  const env: NodeJS.ProcessEnv = {};
  for (const name of [
    "PATH",
    "PATHEXT",
    "SYSTEMROOT",
    "WINDIR",
    "COMSPEC",
    "TEMP",
    "TMP",
    "TMPDIR",
    "LANG",
    "LC_ALL",
  ])
    if (process.env[name] !== undefined) env[name] = process.env[name];
  Object.assign(env, {
    HOME: join(root, "home"),
    USERPROFILE: join(root, "home"),
    GROK_HOME: join(root, "home"),
    APPDATA: join(root, "home", "AppData", "Roaming"),
    LOCALAPPDATA: join(root, "home", "AppData", "Local"),
    ION_GROK_NATIVE_KEY: "fixture-only-not-real",
    GIT_CONFIG_NOSYSTEM: "1",
    GIT_CONFIG_GLOBAL: join(root, "empty-git-config"),
    GROK_SAMPLER_SHARED_CLIENT: "0",
    ...(process.platform === "win32" ? { GROK_SHELL: "powershell" } : {}),
    GROK_DISABLE_AUTOUPDATER: "1",
    GROK_TELEMETRY_ENABLED: "0",
    GROK_TELEMETRY_TRACE_UPLOAD: "0",
    GROK_WORKSPACE_UPLOAD_QUEUE_ENABLED: "false",
    GROK_MANAGED_CONFIG: "0",
    GROK_CRASH_HANDLER: "0",
    GROK_TURN_SUMMARY: "0",
    GROK_TITLE_REFRESH: "0",
    GROK_GOAL_SUMMARY: "0",
    GROK_PROMPT_SUGGESTIONS: "0",
    GROK_MEMORY: "0",
    GROK_SUBAGENTS: "0",
    GROK_WEB_FETCH: "0",
    GROK_WRITE_FILE: "0",
    GROK_TOOL_SEARCH: "0",
    GROK_LSP_TOOLS: "0",
    GROK_MANAGED_MCPS_ENABLED: "0",
    GROK_MANAGED_MCP_GATEWAY_TOOLS_ENABLED: "0",
    GROK_OFFICIAL_MARKETPLACE_AUTO_REGISTER: "0",
    GROK_AGENT_DASHBOARD: "0",
    OTEL_TRACES_EXPORTER: "none",
    RUST_LOG: "off",
  });
  for (const vendor of ["CURSOR", "CLAUDE", "CODEX"])
    for (const surface of [
      "SKILLS",
      "RULES",
      "AGENTS",
      "MCPS",
      "HOOKS",
      "SESSIONS",
    ])
      env[`GROK_${vendor}_${surface}_ENABLED`] = "0";
  return env;
}

async function contract(failHook: boolean, acp = false) {
  const base = resolve(process.env.ION_NATIVE_FIXTURE_ROOT ?? tmpdir());
  mkdirSync(base, { recursive: true });
  const root = mkdtempSync(join(base, "ion-grok-contract-"));
  const hooks: Json[] = [],
    calls: Json[] = [],
    models: Json[] = [],
    updates: Json[] = [],
    errors: string[] = [];
  const bindings = new Map<string, Json>();
  const hookToken =
    randomUUID().replaceAll("-", "") + randomUUID().replaceAll("-", "");
  const methods: string[] = [];
  const gates: Json[] = [];
  const witnessed = new Set<string>();
  const accepted: { session: string; call: string; arguments: Json }[] = [];
  const server = createServer((request, response) => {
    void (async () => {
      const route = request.url?.split("?")[0];
      if (request.method === "GET") {
        response.writeHead(405);
        response.end();
        return;
      }
      const value = await body(request);
      if (route === "/hook") {
        if (daemon && request.headers.authorization !== `Bearer ${hookToken}`) {
          json(
            response,
            { error: "Missing native helper authentication" },
            401,
          );
          return;
        }
        hooks.push(value);
        if (failHook) {
          json(response, { error: "deliberate hook failure" }, 500);
          return;
        }
        if (
          typeof value.sessionId !== "string" ||
          typeof value.toolUseId !== "string" ||
          value.toolInputTruncated
        )
          throw new Error("Missing complete native hook identity");
        const binding = randomUUID();
        bindings.set(binding, {
          ...value,
          canonicalArguments: value.toolInput?.tool_input ?? value.toolInput,
        });
        json(response, {
          hookSpecificOutput: {
            hookEventName: "PreToolUse",
            updatedInput: value.toolInput?.tool_input
              ? {
                  ...value.toolInput,
                  tool_input: {
                    ...value.toolInput.tool_input,
                    [bindingField]: binding,
                  },
                }
              : { ...value.toolInput, [bindingField]: binding },
          },
        });
        return;
      }
      if (route === "/mcp") {
        methods.push(value.method);
        if (value.method?.startsWith("notifications/")) {
          response.writeHead(202);
          response.end();
          return;
        }
        const reply = (result: unknown) =>
          json(response, { jsonrpc: "2.0", id: value.id, result });
        switch (value.method) {
          case "initialize":
            reply({
              protocolVersion: value.params?.protocolVersion ?? "2025-03-26",
              capabilities: { tools: {} },
              serverInfo: { name: "ion-contract", version: "1" },
            });
            return;
          case "ping":
            reply({});
            return;
          case "tools/list":
            reply({
              tools: [
                {
                  name: "list_teams",
                  description: "Read the fixture team catalog",
                  inputSchema: {
                    type: "object",
                    properties: {
                      query: { type: "string" },
                      [bindingField]: { type: "string" },
                    },
                    additionalProperties: false,
                  },
                },
              ],
            });
            return;
          case "tools/call": {
            calls.push(value);
            const args = { ...value.params?.arguments };
            const binding = args[bindingField];
            delete args[bindingField];
            const witness = bindings.get(binding);
            if (
              !witness ||
              JSON.stringify(args) !==
                JSON.stringify(witness.canonicalArguments)
            ) {
              reply({
                isError: true,
                content: [
                  {
                    type: "text",
                    text: "Unbound native call; no operation performed",
                  },
                ],
              });
              return;
            }
            // This probe has no effectful tool or durable Ion state. Acceptance
            // records only which independently observed native call reached MCP.
            bindings.delete(binding);
            accepted.push({
              session: witness.sessionId,
              call: witness.toolUseId,
              arguments: args,
            });
            reply({
              content: [
                {
                  type: "text",
                  text: JSON.stringify({
                    native_call: witness.toolUseId,
                    teams: [{ name: "Engineering" }],
                  }),
                },
              ],
            });
            return;
          }
          default:
            json(response, {
              jsonrpc: "2.0",
              id: value.id,
              error: { code: -32601, message: "fixture method unavailable" },
            });
            return;
        }
      }
      if (route === "/responses") {
        // Normal native sessions can create an initial title independently
        // of the tool loop. Keep that request on the same fake account/host.
        if (
          value.tools?.length === 1 &&
          (value.tools[0].name ?? value.tools[0].function?.name) ===
            "session_title"
        ) {
          response.writeHead(200, { "content-type": "text/event-stream" });
          response.end(
            stream([
              {
                type: "function_call",
                id: "fc_title",
                call_id: "title_fixture",
                name: "session_title",
                status: "completed",
                arguments: JSON.stringify({ session_title: "Ion fixture" }),
              },
            ]),
          );
          return;
        }
        models.push(value);
        if (models.length > 2)
          throw new Error("Unexpected auxiliary native model request");
        let items: Json[];
        if (models.length === 1) {
          const tool =
            value.tools?.find((tool: Json) =>
              String(tool.name ?? tool.function?.name).endsWith("list_teams"),
            ) ??
            value.tools?.find(
              (tool: Json) => (tool.name ?? tool.function?.name) === "use_tool",
            );
          if (!tool)
            throw new Error(
              `Native MCP tool not advertised: ${JSON.stringify(value.tools?.map((t: Json) => t.name))}`,
            );
          const name = tool.name ?? tool.function.name;
          items = nativeCalls.map((call_id, i) => ({
            type: "function_call",
            id: `fc_contract_${i}`,
            call_id,
            name,
            status: "completed",
            arguments: JSON.stringify(
              name === "use_tool"
                ? {
                    tool_name: "ion_contract__list_teams",
                    tool_input: {
                      ...authoredArgs,
                      ...(failHook
                        ? { [bindingField]: "forged-from-model" }
                        : {}),
                    },
                  }
                : {
                    ...authoredArgs,
                    ...(failHook
                      ? { [bindingField]: "forged-from-model" }
                      : {}),
                  },
            ),
          }));
        } else
          items = [
            {
              type: "message",
              id: "msg_contract",
              status: "completed",
              role: "assistant",
              content: [
                {
                  type: "output_text",
                  text: "contract complete",
                  annotations: [],
                },
              ],
            },
          ];
        response.writeHead(200, { "content-type": "text/event-stream" });
        response.end(stream(items));
        return;
      }
      throw new Error(
        `Unexpected native fixture route ${request.method} ${route}`,
      );
    })().catch((error) => {
      errors.push(String(error));
      json(response, { error: String(error) }, 500);
    });
  });
  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const address = server.address();
  if (!address || typeof address === "string")
    throw new Error("Missing loopback address");
  const url = `http://127.0.0.1:${address.port}`;
  let child: ReturnType<typeof spawn> | undefined;
  let stderr = "",
    stdout = "";
  try {
    for (const name of [
      "home",
      "home/hooks",
      "work/.git/objects",
      "work/.git/refs",
    ])
      mkdirSync(join(root, name), { recursive: true });
    writeFileSync(join(root, "empty-git-config"), "");
    writeFileSync(join(root, "work/.git/HEAD"), "ref: refs/heads/main\n");
    writeFileSync(
      join(root, "work/.git/config"),
      "[core]\nrepositoryformatversion=0\nbare=false\n",
    );
    writeFileSync(
      join(root, "input.txt"),
      "Read the Engineering fixture catalog twice. No local files or external services are needed.",
    );
    writeFileSync(
      join(root, "agent.md"),
      "---\nname: ion-contract\ndescription: Isolated native contract\ntool_config:\n  tools: []\ninject_default_tools: false\ndiscover_skills: false\ninherit_skills: false\nagents_md: false\nmax_turns: 2\n---\nUse the fixture catalog.\n",
    );
    const hookScript = join(root, "hook.mjs");
    writeFileSync(
      hookScript,
      `const chunks=[]; for await(const chunk of process.stdin) chunks.push(chunk); const response=await fetch(process.env.ION_GROK_CONTRACT_HOOK_URL,{method:"POST",headers:{"content-type":"application/json"},body:Buffer.concat(chunks).toString("utf8")}); if(!response.ok) process.exit(1); process.stdout.write(await response.text());`,
    );
    const quote = (value: string) =>
      process.platform === "win32"
        ? "'" + value.replaceAll("'", "''") + "'"
        : "'" + value.replaceAll("'", "'\\''") + "'";
    const command = `${process.platform === "win32" ? "& " : ""}${quote(daemon ?? process.execPath)} ${daemon ? "--source-team-hook" : quote(hookScript)}`;
    writeFileSync(
      join(root, "home/hooks/ion-contract.json"),
      JSON.stringify({
        hooks: {
          PreToolUse: [
            {
              matcher: "*",
              hooks: [{ type: "command", command, timeout: 5 }],
            },
          ],
        },
      }),
    );
    writeFileSync(
      join(root, "home/config.toml"),
      `[models]\ndefault="grok-4.6"\nmax_retries=0\n[model."grok-4.6"]\nmodel="grok-4.6"\nbase_url="${url}"\nenv_key="ION_GROK_NATIVE_KEY"\napi_backend="responses"\ncontext_window=500000\nmax_completion_tokens=32768\nmax_retries=0\nsupports_backend_search=false\n[features]\nremote_fetch=false\nmanaged_config=false\ntelemetry="off"\ncodebase_indexing=false\ntitle_refresh=false\n[telemetry]\ntrace_upload=false\n[cli]\nauto_update=false\n[endpoints]\ncli_chat_proxy_base_url="http://127.0.0.1:1"\nxai_api_base_url="http://127.0.0.1:1"\n[mcp_servers.ion_contract]\nurl="${url}/mcp"\ntype="http"\nenabled=true\nstartup_timeout_sec=10\ntool_timeout_sec=10\n`,
    );
    const env = environment(root);
    env.ION_GROK_CONTRACT_HOOK_URL = `${url}/hook`;
    if (daemon) {
      env.ION_SOURCE_TEAM_HOOK_ENDPOINT = `${url}/hook`;
      env.ION_SOURCE_TEAM_HOOK_TOKEN = hookToken;
    }
    const version = execFileSync(binary!, ["--version"], {
      cwd: join(root, "work"),
      env,
      encoding: "utf8",
      timeout: 30000,
      windowsHide: true,
    });
    expect(version.trim()).toBe("grok 1.0.46 (2765805b9442)");
    child = spawn(
      binary!,
      acp
        ? ["agent", "stdio"]
        : [
            "--cwd",
            join(root, "work"),
            "--prompt-file",
            join(root, "input.txt"),
            "--agent",
            join(root, "agent.md"),
            "--model",
            "grok-4.6",
            "--no-memory",
            "--no-subagents",
            "--disable-web-search",
            "--permission-mode",
            "bypassPermissions",
            "--max-turns",
            "2",
            "--no-wait-for-background",
            "--output-format",
            "streaming-json",
          ],
      {
        cwd: join(root, "work"),
        env,
        windowsHide: true,
        stdio: ["pipe", "pipe", "pipe"],
      },
    );
    let rpcSequence = 0;
    const pending = new Map<
      string,
      { resolve: (value: Json) => void; reject: (error: Error) => void }
    >();
    const send = (value: Json) =>
      child!.stdin!.write(JSON.stringify(value) + "\n");
    const rpc = (method: string, params: Json) =>
      new Promise<Json>((resolve, reject) => {
        const id = `ion-contract-${++rpcSequence}`;
        pending.set(id, { resolve, reject });
        send({ jsonrpc: "2.0", id, method, params });
      });
    if (!acp) child.stdin!.end();
    let received = "";
    child.stdout!.setEncoding("utf8");
    child.stdout!.on("data", (data) => {
      stdout += data.toString();
      if (stdout.length > 1024 * 1024) child?.kill();
      if (!acp) return;
      received += data;
      let offset;
      while ((offset = received.indexOf("\n")) !== -1) {
        const line = received.slice(0, offset);
        received = received.slice(offset + 1);
        if (!line.trim()) continue;
        try {
          const value = JSON.parse(line) as Json;
          if (value.method?.endsWith("x.ai/hooks/run")) {
            const gate = value.params;
            gates.push(gate);
            const input = gate.toolInput?.tool_input ?? gate.toolInput;
            const binding = input?.[bindingField];
            const witness = bindings.get(binding);
            const allowed =
              witness &&
              witness.sessionId === gate.sessionId &&
              witness.toolUseId === gate.toolUseId &&
              JSON.stringify(witness.canonicalArguments) ===
                JSON.stringify(
                  Object.fromEntries(
                    Object.entries(input).filter(
                      ([key]) => key !== bindingField,
                    ),
                  ),
                );
            if (allowed) witnessed.add(binding);
            send({
              jsonrpc: "2.0",
              id: value.id,
              result: {
                decision: allowed ? "continue" : "deny",
                systemMessage: allowed ? undefined : "Unbound native call",
              },
            });
          } else if (value.method === "session/request_permission") {
            const option = value.params.options.find(
              (o: Json) => o.kind === "allow_once",
            );
            send({
              jsonrpc: "2.0",
              id: value.id,
              result: {
                outcome: option
                  ? { outcome: "selected", optionId: option.optionId }
                  : { outcome: "cancelled" },
              },
            });
          } else if (value.id !== undefined && !value.method) {
            const entry = pending.get(String(value.id));
            pending.delete(String(value.id));
            if (value.error)
              entry?.reject(new Error(JSON.stringify(value.error)));
            else entry?.resolve(value.result);
          } else if (value.method && value.id !== undefined) {
            send({
              jsonrpc: "2.0",
              id: value.id,
              error: { code: -32601, message: "Unsupported fixture request" },
            });
          }
        } catch (error) {
          errors.push(String(error));
          child?.kill();
        }
      }
    });
    child.stderr!.on("data", (data) => {
      stderr += data.toString();
      if (stderr.length > 64 * 1024) child?.kill();
    });
    const exited = new Promise<number | null>((resolve, reject) => {
      const timeout = setTimeout(() => {
        child?.kill();
        reject(
          new Error(
            `Native Grok fixture timeout; model requests=${models.length}, hooks=${hooks.length}, MCP calls=${calls.length}; ${stderr.slice(-3000)}`,
          ),
        );
      }, 60000);
      child!.once("error", (error) => {
        clearTimeout(timeout);
        reject(error);
      });
      child!.once("close", (code) => {
        clearTimeout(timeout);
        resolve(code);
      });
    });
    if (acp) {
      const work = async () => {
        await rpc("initialize", {
          protocolVersion: 1,
          clientCapabilities: {},
          clientInfo: { name: "ion-contract", version: "1" },
        });
        const session = await rpc("session/new", {
          cwd: join(root, "work"),
          mcpServers: [],
          _meta: {
            "x.ai/hooks": {
              PreToolUse: [
                {
                  matcher: "*",
                  hookCallbackIds: ["ion-contract-witness"],
                  timeout: 5,
                },
              ],
            },
          },
        });
        await rpc("session/prompt", {
          sessionId: session.sessionId,
          prompt: [
            {
              type: "text",
              text: "Read the Engineering fixture catalog twice.",
            },
          ],
          _meta: { promptId: "ion-contract-prompt" },
        });
        child!.stdin!.end();
      };
      await Promise.race([
        work(),
        exited.then(() => {
          throw new Error("Native Grok closed during ACP fixture");
        }),
      ]);
    }
    const exitCode = await exited;
    for (const line of stdout.split(/\r?\n/).filter(Boolean)) {
      try {
        updates.push(JSON.parse(line));
      } catch {
        errors.push(`Non-JSON native stdout: ${line.slice(0, 500)}`);
      }
    }
    expect(errors).toEqual([]);
    if (exitCode !== 0)
      throw new Error(
        `Native exit ${exitCode}: ${stderr.slice(-5000)}; updates=${JSON.stringify(updates).slice(-3000)}`,
      );
    expect(models).toHaveLength(2);
    const expectedCalls = failHook && (acp || daemon) ? 0 : 2;
    if (hooks.length !== 2 || calls.length !== expectedCalls)
      throw new Error(
        `Missing native hook/MCP calls: methods=${JSON.stringify(methods)}; hook_count=${hooks.length}; tool_outputs=${JSON.stringify(models[1]?.input?.filter((item: Json) => item.type === "function_call_output")).slice(-3000)}; stderr=${stderr.slice(-2000)}`,
      );
    expect(hooks).toHaveLength(2);
    expect(calls).toHaveLength(expectedCalls);
    expect(new Set(hooks.map((hook) => hook.sessionId)).size).toBe(1);
    expect(hooks.map((hook) => hook.toolUseId).sort()).toEqual(
      [...nativeCalls].sort(),
    );
    if (acp) {
      expect(gates).toHaveLength(failHook && daemon ? 0 : 2);
      if (!failHook || !daemon)
        expect(gates.map((gate) => gate.toolUseId).sort()).toEqual(
          [...nativeCalls].sort(),
        );
      expect(new Set(gates.map((gate) => gate.sessionId)).size).toBe(
        failHook && daemon ? 0 : 1,
      );
      // Released 1.0.46 omits promptId in both hook paths despite the
      // session/prompt metadata. Do not invent an on-wire turn identifier.
      for (const gate of gates) {
        const hook = hooks.find((hook) => hook.toolUseId === gate.toolUseId)!;
        expect(gate.promptId).toBeUndefined();
        expect(gate.sessionId).toBe(hook.sessionId);
        expect(gate.promptId).toBe(hook.promptId);
      }
      expect(witnessed.size).toBe(failHook ? 0 : 2);
    }
    const modelInput = JSON.stringify(models[1]!.input);
    if (failHook) {
      expect(accepted).toHaveLength(0);
      expect(modelInput).toContain(
        daemon
          ? "Ion Teams hook unavailable"
          : acp
            ? "Unbound native call"
            : "Unbound native call; no operation performed",
      );
    } else {
      expect(accepted.map((entry) => entry.call).sort()).toEqual(
        [...nativeCalls].sort(),
      );
      for (const entry of accepted)
        expect(entry.arguments).toEqual(authoredArgs);
      const original = calls[0]!;
      for (const request of [
        { ...original, id: "consumed-replay" },
        {
          ...original,
          id: "changed-arguments",
          params: {
            ...original.params,
            arguments: { ...original.params.arguments, query: "Changed" },
          },
        },
        {
          ...original,
          id: "forged-binding",
          params: {
            ...original.params,
            arguments: {
              ...original.params.arguments,
              [bindingField]: "forged-from-model",
            },
          },
        },
        {
          ...original,
          id: "missing-binding",
          params: { ...original.params, arguments: authoredArgs },
        },
      ]) {
        const replay = (await fetch(`${url}/mcp`, {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify(request),
        }).then((r) => r.json())) as Json;
        expect(replay.result.isError).toBe(true);
        expect(accepted).toHaveLength(2);
      }
      expect(modelInput).toContain("Engineering");
      for (const call of nativeCalls) expect(modelInput).toContain(call);
      // The hook's binding must stay out of the following provider input.
      // Native wire presentation is inspected separately during integration.
      for (const call of calls.slice(0, nativeCalls.length))
        expect(modelInput).not.toContain(call.params.arguments[bindingField]);
      const authored = models[1]!.input.filter(
        (item: Json) => item.type === "function_call",
      );
      expect(authored).toHaveLength(2);
      for (const item of authored) {
        const args = JSON.parse(item.arguments);
        expect(args.tool_input ?? args).toEqual(authoredArgs);
      }
    }
    return { hooks, calls, updates, models, accepted };
  } finally {
    child?.kill();
    server.closeAllConnections();
    await new Promise<void>((resolve) => server.close(() => resolve()));
    // Only the exact disposable root created above may be removed.
    if (
      !resolve(root).startsWith(base + sep) ||
      !root.split(sep).at(-1)?.startsWith("ion-grok-contract-")
    )
      throw new Error(
        "Refusing native fixture cleanup outside its disposable root",
      );
    rmSync(root, {
      recursive: true,
      force: true,
      maxRetries: 5,
      retryDelay: 100,
    });
  }
}

test.skipIf(!binary)(
  "Grok native identical tool calls in one batch retain distinct hook/MCP identity and unmodified model arguments",
  async () => {
    await contract(false);
  },
  90000,
);
test.skipIf(!binary)(
  "Grok native failed hooks and model-forged bindings execute no fixture operation",
  async () => {
    await contract(true);
  },
  90000,
);

test.skipIf(!binary)(
  "Grok native ACP client gate independently witnesses the rewritten session and call before MCP dispatch",
  async () => {
    await contract(false, true);
  },
  90000,
);

test.skipIf(!binary)(
  "Grok native ACP gate or fail-closed helper rejects failed-hook model-forged bindings before MCP dispatch",
  async () => {
    await contract(true, true);
  },
  90000,
);
