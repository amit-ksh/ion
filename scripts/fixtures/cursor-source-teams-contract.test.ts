// Native Cursor protocol research fixture. This does not register a Teams adapter.
import { expect, test } from "bun:test";
import { createHash } from "node:crypto";
import {
  mkdtempSync,
  writeFileSync,
  readFileSync,
  rmSync,
  realpathSync,
} from "node:fs";
import { join, resolve, dirname, sep } from "node:path";
import { tmpdir } from "node:os";
type Json = Record<string, any>;
const nativeHookTemplate = readFileSync(
  new URL(
    "../../crates/waku-core/src/driver/source_teams/cursor/hook.ps1",
    import.meta.url,
  ),
  "utf8",
);
const runtime = process.env.ION_TEST_CURSOR_RUNTIME;
const native = test.skipIf(!runtime || process.platform !== "win32");
const indexHash =
  "0e7e28c34e050c6b83fe574e1fefa8d95d7c80bea6c4a4d3ae2726e5191243ef";

async function run(mode: string): Promise<Json> {
  const dir = resolve(runtime!);
  expect(
    createHash("sha256")
      .update(readFileSync(join(dir, "index.js")))
      .digest("hex"),
  ).toBe(indexHash);
  const base = realpathSync(process.env.ION_NATIVE_FIXTURE_ROOT ?? tmpdir());
  const root = mkdtempSync(join(base, "cursor-source-contract-"));
  const host = join(root, "host.mjs");
  writeFileSync(host, controller);
  try {
    const p = Bun.spawn([join(dir, "node.exe"), host, mode], {
      cwd: root,
      env: {
        SystemRoot: process.env.SystemRoot,
        WINDIR: process.env.WINDIR,
        PATH: process.env.PATH,
        ION_TEST_CURSOR_RUNTIME: dir,
        ION_CURSOR_CONTRACT_TEMP: root,
        ION_CURSOR_HOOK_TEMPLATE: nativeHookTemplate,
      },
      stdout: "pipe",
      stderr: "pipe",
    });
    const [out, err, code] = await Promise.all([
      new Response(p.stdout).text(),
      new Response(p.stderr).text(),
      p.exited,
    ]);
    expect(err).toBe("");
    expect(code).toBe(0);
    const result = JSON.parse(out);
    expect(result.timedOut).toBe(false);
    expect(result.modelSelections).toEqual(["ion-native-fixture-model"]);
    return result;
  } finally {
    const target = realpathSync(root);
    if (dirname(target) !== base || !target.startsWith(base + sep))
      throw new Error("Fixture cleanup escaped its explicit parent");
    rmSync(target, { recursive: true, force: true });
  }
}
const calls = (r: Json) => r.mcp.filter((x: Json) => x.method === "tools/call");
const permissions = (r: Json) =>
  r.messages.filter((x: Json) => x.method === "session/request_permission");

native(
  "Cursor native ACP retains separate IDs for identical MCP calls and returns successful results",
  async () => {
    const r = await run("allow");
    const pc = permissions(r);
    const mc = calls(r);
    expect(pc.map((x: Json) => x.params.toolCall.toolCallId)).toEqual([
      "ion-native-call-1",
      "ion-native-call-2",
    ]);
    expect(mc).toHaveLength(2);
    expect(mc[0].params).toEqual(mc[1].params);
    expect(mc.every((x: Json) => x.params._meta === undefined)).toBe(true);
    expect(
      mc.every(
        (x: Json) => x.params.arguments.ion_native_token === "model-forged",
      ),
    ).toBe(true);
    expect(r.nativeResults).toHaveLength(2);
    expect(
      r.nativeResults.every((x: string) => x.includes("native-mcp-success")),
    ).toBe(true);
    const source = r.messages.filter(
      (x: Json) => x.params?.update?.sessionUpdate === "tool_call",
    );
    expect(source.map((x: Json) => x.params.update.toolCallId)).toEqual(
      pc.map((x: Json) => x.params.toolCall.toolCallId),
    );
    expect(
      source.every((x: Json) => x.params.sessionId === pc[0].params.sessionId),
    ).toBe(true);
  },
  40000,
);

native(
  "Cursor native permission denial prevents the MCP invocation",
  async () => {
    const r = await run("deny");
    expect(permissions(r)).toHaveLength(1);
    expect(calls(r)).toHaveLength(0);
    expect(r.nativeResults[0]).toContain("User rejected");
  },
  40000,
);

native(
  "Cursor native Stop while approval is pending prevents the MCP invocation",
  async () => {
    const r = await run("stop");
    expect(permissions(r)).toHaveLength(1);
    expect(calls(r)).toHaveLength(0);
    const result = r.messages.find(
      (x: Json) => x.id === 3 && (x.result || x.error),
    );
    expect(result).toBeDefined();
    expect(result.result?.stopReason).toBe("cancelled");
  },
  40000,
);

native(
  "Cursor native private-profile missing hook fails closed before MCP execution",
  async () => {
    const r = await run("missing-hook");
    expect(calls(r)).toHaveLength(0);
    expect(permissions(r)).toHaveLength(0);
    expect(r.nativeResults[0]).toContain("fail closed");
  },
  40000,
);

native(
  "Cursor cached ACP plugin flag does not make its plugin hook an authority boundary",
  async () => {
    const r = await run("plugin");
    expect(calls(r)).toHaveLength(1);
    expect(permissions(r)).toHaveLength(1);
    expect(r.nativeResults[0]).toContain("native-mcp-success");
  },
  40000,
);

native(
  "Cursor native hook overwrites model input before the witnessed permission and MCP call",
  async () => {
    const r = await run("hook-update");
    expect(r.hooks).toHaveLength(1);
    expect(r.hooks[0].tool_input.ion_native_token).toBe("model-forged");
    expect(r.hooks[0].conversation_id).toBe(permissions(r)[0].params.sessionId);
    expect(r.hooks[0].tool_use_id).not.toBe(
      permissions(r)[0].params.toolCall.toolCallId,
    );
    expect(calls(r)).toHaveLength(1);
    expect(calls(r)[0].params.arguments.ion_native_token).toBe(
      r.hooks[0].ion_hook_nonce,
    );
    expect(permissions(r)).toHaveLength(1);
    expect(JSON.stringify(permissions(r)[0])).toContain(
      r.hooks[0].ion_hook_nonce,
    );
    expect(r.nativeResults[0]).toContain("native-mcp-success");
  },
  40000,
);

native(
  "Cursor cached ACP loads the workspace-cache hook without changing its user profile",
  async () => {
    const r = await run("project-hook");
    expect(r.hooks).toHaveLength(1);
    expect(calls(r)).toHaveLength(1);
    expect(calls(r)[0].params.arguments.ion_native_token).toBe(
      r.hooks[0].ion_hook_nonce,
    );
    expect(r.nativeResults[0]).toContain("native-mcp-success");
  },
  40000,
);

native(
  "Cursor rewrites identical calls with separate bindings witnessed under distinct native call IDs",
  async () => {
    const r = await run("project-hook-pair");
    expect(r.hooks).toHaveLength(2);
    expect(calls(r)).toHaveLength(2);
    const ids = permissions(r).map((p: Json) => p.params.toolCall.toolCallId);
    expect(ids).toEqual(["ion-native-call-1", "ion-native-call-2"]);
    expect(new Set(r.hooks.map((h: Json) => h.ion_hook_nonce)).size).toBe(2);
    expect(new Set(r.hooks.map((h: Json) => h.tool_use_id)).size).toBe(2);
    const source = r.messages.filter(
      (m: Json) => m.params?.update?.sessionUpdate === "tool_call",
    );
    for (let i = 0; i < 2; i++) {
      const hook = r.hooks[i];
      const request = permissions(r)[i];
      const rewritten = calls(r)[i].params.arguments;
      const text = request.params.toolCall.content[0].content.text;
      const nativeArgs = JSON.parse(
        text.slice(text.indexOf("\n") + 1, text.lastIndexOf("\n")),
      );
      expect(rewritten).toEqual({
        team: "ion-team-fixture",
        ion_native_token: hook.ion_hook_nonce,
      });
      expect(nativeArgs).toEqual(rewritten);
      expect(hook.tool_input).toEqual({
        team: "ion-team-fixture",
        ion_native_token: "model-forged",
      });
      expect(hook.conversation_id).toBe(request.params.sessionId);
      expect(hook.tool_use_id).not.toBe(ids[i]);
      expect(source[i].params.update.toolCallId).toBe(ids[i]);
      expect(JSON.stringify(source[i].params.update.rawInput)).toContain(
        "model-forged",
      );
    }
    expect(
      r.nativeResults.every((x: string) => x.includes("native-mcp-success")),
    ).toBe(true);
  },
  40000,
);
for (const mode of ["project-hook-deny", "project-hook-stop"]) {
  native(
    "Cursor rewritten call remains stopped by native " + mode,
    async () => {
      const r = await run(mode);
      expect(r.hooks).toHaveLength(1);
      expect(permissions(r)).toHaveLength(1);
      expect(calls(r)).toHaveLength(0);
      if (mode.endsWith("stop"))
        expect(
          r.messages.find((m: Json) => m.id === 3 && m.result)?.result
            .stopReason,
        ).toBe("cancelled");
      else expect(r.nativeResults[0]).toContain("User rejected");
    },
    40000,
  );
}

native(
  "Cursor native allow-always can bypass the permission witness on a later rewritten call",
  async () => {
    const r = await run("project-hook-always");
    expect(r.hooks).toHaveLength(2);
    expect(calls(r)).toHaveLength(2);
    expect(permissions(r)).toHaveLength(1);
    expect(r.nativeResults).toHaveLength(2);
  },
  40000,
);

native(
  "Cursor native PowerShell hook can use an authenticated loopback callback and return rewritten input",
  async () => {
    const r = await run("project-hook-http");
    expect(r.hookRequests).toHaveLength(1);
    expect(r.hookRequests[0].tool_input.ion_native_token).toBe("model-forged");
    expect(calls(r)).toHaveLength(1);
    expect(calls(r)[0].params.arguments.ion_native_token).not.toBe(
      "model-forged",
    );
    expect(JSON.stringify(permissions(r)[0])).toContain(
      calls(r)[0].params.arguments.ion_native_token,
    );
    expect(r.nativeResults[0]).toContain("native-mcp-success");
  },
  40000,
);

native(
  "Cursor native loopback hook preserves UTF8 arguments and supplementary Unicode",
  async () => {
    const r = await run("project-hook-http-utf8");
    expect(r.hookRequests).toHaveLength(1);
    expect(r.hookRequests[0].tool_input.team).toBe(
      "ion-team-fixture — 研究 🚀",
    );
    expect(calls(r)[0].params.arguments.team).toBe(
      "ion-team-fixture — 研究 🚀",
    );
  },
  40000,
);
for (const mode of [
  "project-hook-http-deny",
  "project-hook-http-redirect",
  "project-hook-http-stall",
  "project-hook-http-malformed",
  "project-hook-http-oversized",
]) {
  native(
    "Cursor native hook refuses unsuccessful loopback response " + mode,
    async () => {
      const r = await run(mode);
      expect(r.hookRequests).toHaveLength(1);
      expect(calls(r)).toHaveLength(0);
      expect(permissions(r)).toHaveLength(0);
      expect(r.requestPaths).not.toContain("/_ion_unexpected_redirect");
    },
    40000,
  );
}

for (const mode of [
  "project-hook-http-missing-token",
  "project-hook-http-bad-token",
]) {
  native(
    "Cursor native hook refuses unavailable or invalid transport authority " +
      mode,
    async () => {
      const r = await run(mode);
      expect(r.hookRequests).toHaveLength(0);
      expect(calls(r)).toHaveLength(0);
      expect(permissions(r)).toHaveLength(0);
    },
    40000,
  );
}

native(
  "Cursor native Claude-compatible local hook coexists with existing Cursor project configuration",
  async () => {
    const r = await run("project-hook-claude-local");
    expect(r.hooks[0].hook_event_name).toBe("preToolUse");
    expect(r.hooks[0].conversation_id).toBe(permissions(r)[0].params.sessionId);
    expect(r.hooks[0].session_id).toBe(permissions(r)[0].params.sessionId);
    expect(r.hooks).toHaveLength(1);
    expect(calls(r)).toHaveLength(1);
    expect(calls(r)[0].params.arguments.ion_native_token).toBe(
      r.hooks[0].ion_hook_nonce,
    );
    expect(r.nativeResults[0]).toContain("native-mcp-success");
    expect(r.ownerHookPreserved).toBe(true);
  },
  40000,
);

// Runs the pinned CLI's real Connect HTTP/2 agent protocol. No alternative local-agent provider.
native(
  "Cursor native separate Claude settings hook preserves owner Cursor hooks",
  async () => {
    const r = await run("project-hook-claude-settings");
    expect(r.ownerHookPreserved).toBe(true);
    expect(calls(r)).toHaveLength(1);
    expect(permissions(r)).toHaveLength(1);
    expect(calls(r)[0].params.arguments.ion_native_token).toBe(
      r.hooks[0].ion_hook_nonce,
    );
    expect(r.nativeResults[0]).toContain("native-mcp-success");
  },
  40000,
);

native(
  "Cursor native grouped private-tool matcher rewrites input",
  async () => {
    const r = await run("project-hook-claude-group");
    expect(calls(r)).toHaveLength(1);
    expect(r.hooks).toHaveLength(1);
    expect(calls(r)[0].params.arguments.ion_native_token).toBe(
      r.hooks[0].ion_hook_nonce,
    );
  },
  40000,
);

native(
  "Cursor native Claude hook uses authenticated HTTP rewrite",
  async () => {
    const r = await run("project-hook-claude-http");
    expect(r.hookRequests).toHaveLength(1);
    expect(calls(r)).toHaveLength(1);
    expect(calls(r)[0].params.arguments.ion_native_token).toBeDefined();
  },
  40000,
);

native(
  "Cursor native HTTP MCP executes the configured project hook",
  async () => {
    const r = await run("project-hook-http-mcp");
    expect(r.hookRequests).toHaveLength(1);
    expect(calls(r)).toHaveLength(1);
    expect(calls(r)[0].params.arguments.ion_native_token).not.toBe(
      "model-forged",
    );
  },
  40000,
);

const controller = readFileSync(
  new URL("./cursor-native-controller.mjs", import.meta.url),
  "utf8",
);
