// Native Antigravity contract only. Production source-task registration is separate.
import { expect, test } from "bun:test";
import { spawn, execFileSync } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import {
  mkdtempSync,
  mkdirSync,
  writeFileSync,
  readFileSync,
  rmSync,
} from "node:fs";
import {
  createServer,
  type IncomingMessage,
  type ServerResponse,
} from "node:http";
import { tmpdir } from "node:os";
import { isDeepStrictEqual } from "node:util";
import { join, resolve, sep } from "node:path";

type Json = Record<string, any>;
type Mode =
  | "normal"
  | "forged-arguments"
  | "missing-witness"
  | "changed-witness"
  | "review"
  | "workspace"
  | "plugin";
const binary = process.env.ION_TEST_ANTIGRAVITY_BINARY;
const uuidPattern =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const windowsDigest =
  "f346d2cd9d68e7e5395cb94ff95e7e8ab2e0c65cf2ad835d0eb671fb41f12f5fc713d7a26ceb4510678eb9baa62c6cd9ab8205211e3661c6cd1e23ca7bc69781";

function environment(root: string): NodeJS.ProcessEnv {
  const result: NodeJS.ProcessEnv = {};
  const retained = new Set([
    "PATH",
    "PATHEXT",
    "SYSTEMROOT",
    "WINDIR",
    "COMSPEC",
    "LANG",
    "LC_ALL",
  ]);
  for (const [key, value] of Object.entries(process.env))
    if (retained.has(key.toUpperCase()) && value !== undefined)
      result[key.toUpperCase()] = value;
  return Object.assign(result, {
    HOME: join(root, "home"),
    USERPROFILE: join(root, "home"),
    APPDATA: join(root, "home", "AppData", "Roaming"),
    LOCALAPPDATA: join(root, "home", "AppData", "Local"),
    TEMP: join(root, "tmp"),
    TMP: join(root, "tmp"),
    TMPDIR: join(root, "tmp"),
    AGY_CLI_DISABLE_AUTO_UPDATE: "true",
    GEMINI_API_KEY: "fixture-only",
    GIT_CONFIG_COUNT: "0",
    GIT_CONFIG_NOSYSTEM: "1",
    GIT_CONFIG_GLOBAL: join(root, "empty-git-config"),
  });
}
function reply(response: ServerResponse, value: unknown, status = 200) {
  response.writeHead(status, { "content-type": "application/json" });
  response.end(JSON.stringify(value));
}
async function body(request: IncomingMessage): Promise<Json> {
  const chunks: Buffer[] = [];
  let size = 0;
  for await (const chunk of request) {
    size += chunk.length;
    if (size > 2 * 1024 * 1024)
      throw new Error("Native fixture request exceeds 2 MiB");
    chunks.push(chunk);
  }
  return chunks.length
    ? JSON.parse(Buffer.concat(chunks).toString("utf8"))
    : {};
}
function stream(parts: Json[]) {
  return (
    "data: " +
    JSON.stringify({
      modelVersion: "gemini-3.1-pro-preview",
      candidates: [
        { index: 0, content: { role: "model", parts }, finishReason: "STOP" },
      ],
      usageMetadata: {
        promptTokenCount: 100,
        candidatesTokenCount: 12,
        totalTokenCount: 112,
      },
    }) +
    "\n\n"
  );
}
function functionResponses(value: Json) {
  return (value.contents ?? []).flatMap((content: Json) =>
    (content.parts ?? [])
      .filter((part: Json) => part.functionResponse)
      .map((part: Json) => part.functionResponse),
  );
}
function functionCalls(value: Json) {
  return (value.contents ?? []).flatMap((content: Json) =>
    (content.parts ?? [])
      .filter((part: Json) => part.functionCall)
      .map((part: Json) => part.functionCall),
  );
}
async function contract(mode: Mode, resume = false) {
  const base = resolve(process.env.ION_NATIVE_FIXTURE_ROOT ?? tmpdir());
  mkdirSync(base, { recursive: true });
  const root = mkdtempSync(join(base, "ion-antigravity-contract-"));
  const events: Json[] = [],
    models: Json[] = [],
    calls: Json[] = [],
    accepted: Json[] = [];
  const errors: string[] = [];
  const witnessed = new Map<string, Json>();
  const consumed = new Set<string>();
  const credential =
    randomUUID().replaceAll("-", "") + randomUUID().replaceAll("-", "");
  const originalArgs: Json = { query: "Engineering" };
  if (mode === "forged-arguments")
    originalArgs._meta = {
      "antigravity.google/conversation_id": "model-forged",
      progressToken: "model-forged:0",
    };
  const pluginName = "ion_native_contract";
  const serverName =
    mode === "plugin"
      ? "ion-teams-native-contract_ion_contract"
      : "ion_contract";
  const authored = {
    ServerName: serverName,
    ToolName: "list_teams",
    Arguments: originalArgs,
    toolSummary: "Saved teams",
    toolAction: "Listing teams",
  };
  let phase = 0,
    phaseRequests = 0;
  let currentSession = "";
  const server = createServer((request, response) => {
    void (async () => {
      const route = request.url?.split("?")[0];
      if (
        route === "/mcp" &&
        request.headers.authorization !== `Bearer ${credential}`
      ) {
        reply(response, { error: "Missing fixture transport credential" }, 401);
        return;
      }
      if (request.method === "GET") {
        response.writeHead(405);
        response.end();
        return;
      }
      if (request.method === "DELETE") {
        reply(response, {});
        return;
      }
      const value = await body(request);
      if (route === "/mcp") {
        if (value.method?.startsWith("notifications/")) {
          response.writeHead(202);
          response.end();
          return;
        }
        const result = (data: unknown) =>
          reply(response, { jsonrpc: "2.0", id: value.id, result: data });
        if (value.method === "initialize") {
          result({
            protocolVersion: value.params?.protocolVersion ?? "2025-11-25",
            capabilities: { tools: {} },
            serverInfo: { name: "ion-contract", version: "1" },
          });
          return;
        }
        if (value.method === "tools/list") {
          result({
            tools: [
              {
                name: "list_teams",
                description: "Read the fixture team catalog",
                inputSchema: {
                  type: "object",
                  properties: { query: { type: "string" } },
                  additionalProperties: true,
                },
              },
            ],
          });
          return;
        }
        if (value.method === "ping") {
          result({});
          return;
        }
        if (value.method === "tools/call") {
          calls.push(value);
          const meta = value.params?._meta;
          const session = meta?.["antigravity.google/conversation_id"];
          const progress =
            typeof meta?.progressToken === "string" ? meta.progressToken : "";
          const index = progress.lastIndexOf(":");
          const nativeRun = progress.slice(0, index);
          const step = progress.slice(index + 1);
          if (
            !uuidPattern.test(session ?? "") ||
            !uuidPattern.test(nativeRun) ||
            !/^(0|[1-9][0-9]*)$/.test(step)
          )
            throw new Error("Missing native conversation/step metadata");
          const id = `${session}:${step}`;
          // The real stdout must attest this call before a callback is accepted.
          // This also verifies that pipe buffering does not invert the contract.
          const until = Date.now() + (mode === "missing-witness" ? 80 : 3000);
          while (!witnessed.has(id) && Date.now() < until) await Bun.sleep(5);
          const witness = witnessed.get(id);
          if (
            !witness ||
            consumed.has(id) ||
            session !== currentSession ||
            witness.tool_name !== "call_mcp_tool" ||
            witness.tool_info?.parameters?.ServerName !== serverName ||
            witness.tool_info?.parameters?.ToolName !== value.params?.name ||
            !isDeepStrictEqual(
              witness.tool_info?.parameters?.Arguments,
              value.params?.arguments,
            )
          ) {
            result({
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
          consumed.add(id);
          accepted.push({
            session,
            step: Number(step),
            nativeRun,
            arguments: value.params.arguments,
            phase,
          });
          result({
            content: [
              {
                type: "text",
                text: JSON.stringify({
                  native_step: Number(step),
                  teams: [{ name: "Engineering" }],
                }),
              },
            ],
          });
          return;
        }
        reply(response, {
          jsonrpc: "2.0",
          id: value.id,
          error: { code: -32601, message: "Fixture method unavailable" },
        });
        return;
      }
      if (
        !route?.includes(":streamGenerateContent") &&
        !route?.includes(":generateContent")
      )
        throw new Error("Unexpected native model route");
      const definitions = (value.tools ?? []).flatMap(
        (tool: Json) => tool.functionDeclarations ?? [],
      );
      const isMain = definitions.some(
        (tool: Json) =>
          tool.name === "view_file" || tool.name === "call_mcp_tool",
      );
      let parts: Json[];
      if (!isMain)
        parts = [{ text: "Ion fixture" }]; // Native title/auxiliary request stays local.
      else {
        models.push({ phase, request: value });
        phaseRequests++;
        if (phaseRequests > 2)
          throw new Error("Unexpected extra native model call");
        if (phaseRequests === 1) {
          const available = definitions.some(
            (tool: Json) => tool.name === "call_mcp_tool",
          );
          expect(available).toBe(true);
          parts = [0, 1].map(() => ({
            functionCall: { name: "call_mcp_tool", args: authored },
          }));
        } else parts = [{ text: "Contract complete" }];
      }
      response.writeHead(200, { "content-type": "text/event-stream" });
      response.end(stream(parts));
    })().catch((error) => {
      errors.push(String(error));
      if (!response.headersSent)
        reply(response, { error: "Native fixture failed" }, 500);
      else response.end();
    });
  });
  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const address = server.address();
  if (!address || typeof address === "string")
    throw new Error("No fixture address");
  const url = `http://127.0.0.1:${address.port}`;
  let child: ReturnType<typeof spawn> | undefined;
  let stderr = "";
  try {
    for (const path of [
      "home/.gemini/antigravity-cli",
      "home/.gemini/config",
      "work/.agents",
      "tmp",
      "home/AppData/Roaming",
      "home/AppData/Local",
    ])
      mkdirSync(join(root, path), { recursive: true });
    writeFileSync(join(root, "empty-git-config"), "");
    writeFileSync(
      join(root, "home/.gemini/antigravity-cli/settings.json"),
      JSON.stringify({
        modelProvider: "gemini",
        enableTelemetry: false,
        notifications: false,
        toolPermission: "strict",
        showTips: false,
        enableTerminalSandbox: false,
      }),
    );
    const mcpConfig =
      JSON.stringify(
        {
          ownerSentinel: { retained: "including unknown native fields" },
          mcpServers: {
            owner_disabled: { disabled: true, command: "owner-fixture-unused" },
            ion_contract: {
              serverUrl: `${url}/mcp`,
              headers: { Authorization: `Bearer ${credential}` },
            },
          },
        },
        null,
        2,
      ) + "\n";
    const configPath = join(
      root,
      mode === "workspace"
        ? "work/.agents/mcp_config.json"
        : "home/.gemini/config/mcp_config.json",
    );
    let activeConfigPath = configPath;
    if (mode === "plugin") {
      const pluginPath = join(
        root,
        "work/.agents/plugins",
        "ion-teams-native-contract",
      );
      mkdirSync(pluginPath, { recursive: true });
      writeFileSync(
        join(pluginPath, "plugin.json"),
        JSON.stringify({
          name: pluginName,
          description: "Isolated Ion native contract",
        }),
      );
      activeConfigPath = join(pluginPath, "mcp_config.json");
    }
    writeFileSync(activeConfigPath, mcpConfig);
    const env = environment(root);
    env.GOOGLE_GEMINI_BASE_URL = url;
    const version = execFileSync(binary!, ["--version"], {
      cwd: join(root, "work"),
      env,
      encoding: "utf8",
      timeout: 15000,
      windowsHide: true,
    });
    expect(version.trim()).toBe("1.3.1");
    if (process.platform === "win32")
      expect(
        createHash("sha512").update(readFileSync(binary!)).digest("hex"),
      ).toBe(windowsDigest);
    const runs: { session: string; result: Json; phase: number }[] = [];
    for (phase = 0; phase < (resume ? 2 : 1); phase++) {
      phaseRequests = 0;
      const args = [
        "--model",
        "gemini-3.1-pro-low",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--disable-slash-commands",
        "--print-timeout",
        "35s",
        ...(mode === "review" ? [] : ["--dangerously-skip-permissions"]),
        ...(phase ? ["--conversation", runs[0]!.session] : []),
      ];
      child = spawn(binary!, args, {
        cwd: join(root, "work"),
        env,
        windowsHide: true,
        stdio: ["pipe", "pipe", "pipe"],
      });
      let buffer = "",
        total = 0;
      let runResult: Json | undefined;
      const streamErrors: string[] = [];
      child.stdout!.on("data", (chunk: Buffer) => {
        total += chunk.length;
        if (total > 2 * 1024 * 1024) {
          streamErrors.push("Native output exceeds 2 MiB");
          child?.kill();
          return;
        }
        buffer += chunk.toString("utf8");
        let end: number;
        while ((end = buffer.indexOf("\n")) >= 0) {
          const line = buffer.slice(0, end);
          buffer = buffer.slice(end + 1);
          if (!line.trim()) continue;
          try {
            const event = JSON.parse(line);
            events.push({ ...event, phase });
            if (event.event === "init") currentSession = event.conversation_id;
            if (event.event === "result") runResult = event.result;
            const update = event.step_update;
            if (
              event.event === "step_update" &&
              update?.step_type === "tool" &&
              update.state === "ACTIVE" &&
              mode !== "missing-witness"
            ) {
              const witness = structuredClone(update);
              if (mode === "changed-witness")
                witness.tool_info.parameters.Arguments = {
                  query: "Changed after observation",
                };
              witnessed.set(
                `${update.conversation_id}:${update.step_index}`,
                witness,
              );
            }
          } catch (error) {
            streamErrors.push(String(error));
          }
        }
      });
      child.stderr!.on("data", (chunk: Buffer) => {
        stderr += chunk.toString("utf8");
        if (stderr.length > 65536) {
          streamErrors.push("Native stderr exceeds its bound");
          child?.kill();
        }
      });
      const exit = await new Promise<number | null>((resolve, reject) => {
        const timer = setTimeout(() => {
          child?.kill();
          reject(new Error("Native fixture timed out"));
        }, 50000);
        child!.once("error", (error) => {
          clearTimeout(timer);
          reject(error);
        });
        child!.once("exit", (code) => {
          clearTimeout(timer);
          resolve(code);
        });
        child!.stdin!.end(
          JSON.stringify({
            event: "user",
            message: {
              content:
                "Read the Engineering fixture catalog twice. No files or external services are needed.",
            },
          }) + "\n",
        );
      });
      expect(streamErrors).toEqual([]);
      expect(exit).toBe(0);
      expect(runResult).toBeDefined();
      expect(uuidPattern.test(currentSession)).toBe(true);
      runs.push({ session: currentSession, result: runResult!, phase });
      expect(readFileSync(activeConfigPath, "utf8")).toBe(mcpConfig);
    }
    expect(errors).toEqual([]);
    if (mode === "review") {
      expect(calls).toHaveLength(0);
      expect(accepted).toHaveLength(0);
      expect(runs[0]!.result.denied_actions).toContainEqual({
        action: "mcp",
        display_name: "CallMcpTool",
      });
      expect(stderr).toContain("headless mode cannot prompt");
      return;
    }
    expect(calls).toHaveLength(resume ? 4 : 2);
    if (mode === "missing-witness" || mode === "changed-witness") {
      expect(accepted).toHaveLength(0);
      const followup = models.find(
        (item) => functionResponses(item.request).length,
      );
      expect(followup).toBeDefined();
      expect(JSON.stringify(functionResponses(followup!.request))).toContain(
        "Unbound native call",
      );
      return;
    }
    expect(accepted).toHaveLength(resume ? 4 : 2);
    expect(
      new Set(accepted.map((item) => `${item.session}:${item.step}`)).size,
    ).toBe(accepted.length);
    expect(new Set(accepted.map((item) => item.session)).size).toBe(1);
    for (let p = 0; p < (resume ? 2 : 1); p++) {
      const these = accepted.filter((item) => item.phase === p);
      expect(these).toHaveLength(2);
      expect(new Set(these.map((item) => item.nativeRun)).size).toBe(1);
      expect(
        these.every((item) => isDeepStrictEqual(item.arguments, originalArgs)),
      ).toBe(true);
      const followup = models.find(
        (item) =>
          item.phase === p &&
          these.every((call) =>
            functionResponses(item.request).some((response: Json) =>
              response.response.output.includes(
                JSON.stringify({
                  native_step: call.step,
                  teams: [{ name: "Engineering" }],
                }),
              ),
            ),
          ),
      );
      expect(followup).toBeDefined();
      const returned = functionResponses(followup!.request);
      const invoked = functionCalls(followup!.request);
      for (const item of these) {
        expect(
          returned.some((response: Json) =>
            response.response.output.includes(
              JSON.stringify({
                native_step: item.step,
                teams: [{ name: "Engineering" }],
              }),
            ),
          ),
        ).toBe(true);
        expect(JSON.stringify(returned)).toContain("Engineering");
      }
      // In a resumed request the native history includes the earlier batch too.
      expect(invoked.length).toBeGreaterThanOrEqual(2);
      expect(new Set(invoked.map((item: Json) => item.id)).size).toBe(
        invoked.length,
      );
      expect(
        invoked.every((item: Json) =>
          isDeepStrictEqual(item.args.Arguments, originalArgs),
        ),
      ).toBe(true);
      expect(runs[p]!.result.status).toBe("SUCCESS");
      expect(runs[p]!.result.response).toContain("Contract complete");
    }
    if (resume)
      expect(
        Math.min(
          ...accepted
            .filter((item) => item.phase === 1)
            .map((item) => item.step),
        ),
      ).toBeGreaterThan(
        Math.max(
          ...accepted
            .filter((item) => item.phase === 0)
            .map((item) => item.step),
        ),
      );
    if (mode === "normal" && !resume) {
      const acceptedCount = accepted.length;
      const invoke = async (value: Json, token = credential) => {
        const response = await fetch(`${url}/mcp`, {
          method: "POST",
          headers: {
            "content-type": "application/json",
            Authorization: `Bearer ${token}`,
          },
          body: JSON.stringify(value),
        });
        return {
          status: response.status,
          value: (await response.json()) as Json,
        };
      };
      const replay = await invoke(calls[0]!);
      expect(replay.status).toBe(200);
      expect(replay.value.result.isError).toBe(true);
      const foreign = structuredClone(calls[0]!);
      foreign.params._meta["antigravity.google/conversation_id"] = randomUUID();
      const unrelated = await invoke(foreign);
      expect(unrelated.value.result.isError).toBe(true);
      const unauthorized = await invoke(
        calls[0]!,
        "not-the-fixture-credential",
      );
      expect(unauthorized.status).toBe(401);
      expect(accepted).toHaveLength(acceptedCount);
      expect(errors).toEqual([]);
    }
  } finally {
    if (child && child.exitCode === null) {
      child.kill();
      await new Promise<void>((resolve) => {
        child!.once("exit", () => resolve());
        setTimeout(resolve, 3000);
      });
    }
    server.closeAllConnections();
    await new Promise<void>((resolve) => server.close(() => resolve()));
    const absolute = resolve(root);
    if (
      !absolute.startsWith(base + sep) ||
      !absolute.slice(base.length + 1).startsWith("ion-antigravity-contract-")
    )
      throw new Error("Refusing unexpected native fixture cleanup target");
    rmSync(absolute, { recursive: true, force: true });
  }
}

const native = binary ? test : test.skip;
native(
  "antigravity native metadata binds identical calls and preserves provider arguments/results",
  () => contract("normal"),
  90000,
);
native(
  "antigravity native resume retains conversation identity and advances durable step identities",
  () => contract("normal", true),
  150000,
);
native(
  "antigravity model-authored argument metadata cannot replace native MCP identity",
  () => contract("forged-arguments"),
  90000,
);
native(
  "antigravity native callback without owner stream witnessing is refused",
  () => contract("missing-witness"),
  90000,
);
native(
  "antigravity changed owner arguments refuse native callback",
  () => contract("changed-witness"),
  90000,
);
native(
  "antigravity native review policy refuses ungranted headless MCP calls",
  () => contract("review"),
  90000,
);
native(
  "antigravity workspace-only MCP discovery preserves unrelated app configuration",
  () => contract("workspace"),
  90000,
);

native(
  "antigravity additive workspace plugin discovers namespaced MCP tools",
  () => contract("plugin"),
  90000,
);
