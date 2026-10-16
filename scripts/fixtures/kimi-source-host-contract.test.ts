// Real released Kimi source/HTTP contract, separate from installed-CLI acceptance.
import { expect, test } from "bun:test";
import { mkdtempSync, mkdirSync, realpathSync, rmSync } from "node:fs";
import { dirname, join, resolve, sep } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath, pathToFileURL } from "node:url";

type Json = Record<string, any>;
const source = process.env.ION_TEST_KIMI_SOURCE;
const node = process.env.ION_TEST_KIMI_NODE;
const native = test.skipIf(!source || !node);
const controller = fileURLToPath(
  new URL("./kimi-source-host-controller.mjs", import.meta.url),
);
async function run(mode: string): Promise<Json> {
  const base = realpathSync(process.env.ION_NATIVE_FIXTURE_ROOT ?? tmpdir());
  const root = mkdtempSync(join(base, "ksh-"));
  for (const folder of ["appdata", "local", "tmp"])
    mkdirSync(join(root, folder));
  let child: Bun.Subprocess<"ignore", "pipe", "pipe"> | undefined;
  try {
    child = Bun.spawn(
      [
        resolve(node!),
        "--import",
        pathToFileURL(join(source!, "node_modules/tsx/dist/loader.mjs")).href,
        "--import",
        pathToFileURL(join(source!, "build/register-raw-text-loader.mjs")).href,
        controller,
        mode,
      ],
      {
        cwd: source!,
        env: {
          SystemRoot: process.env.SystemRoot,
          WINDIR: process.env.WINDIR,
          PATH: dirname(resolve(node!)),
          USERPROFILE: join(root, "home"),
          HOME: join(root, "home"),
          APPDATA: join(root, "appdata"),
          LOCALAPPDATA: join(root, "local"),
          TEMP: join(root, "tmp"),
          TMP: join(root, "tmp"),
          KIMI_CODE_HOME: join(root, "home"),
          ION_TEST_KIMI_SOURCE: resolve(source!),
          ION_KIMI_CONTRACT_TEMP: root,
        },
        stdin: "ignore",
        stdout: "pipe",
        stderr: "pipe",
      },
    );
    const timer = setTimeout(() => child?.kill(), 150_000);
    let stdout: string, stderr: string, code: number;
    try {
      [stdout, stderr, code] = await Promise.all([
        new Response(child.stdout).text(),
        new Response(child.stderr).text(),
        child.exited,
      ]);
    } finally {
      clearTimeout(timer);
    }
    expect(code!, stderr!).toBe(0);
    const lines = stdout!.trim().split(/\r?\n/);
    const result = JSON.parse(lines.at(-1)!) as Json;
    expect(result.mode).toBe(mode);
    expect(result.node).toBe("v24.19.0");
    expect(result.failures).toEqual(mode === "collision" ? ["collision"] : []);
    expect(result.selectedModel).toBe("ion-fixture");
    expect(result.blocked).toEqual([]);
    return result;
  } finally {
    if (child && child.exitCode === null) {
      child.kill();
      await child.exited;
    }
    const exact = realpathSync(root);
    if (dirname(exact) !== base || !exact.startsWith(base + sep))
      throw Error("Fixture cleanup escaped its explicit parent");
    rmSync(exact, { recursive: true, force: true });
  }
}
function toolMessages(r: Json): Json[] {
  return r.requests.flatMap((x: Json) =>
    x.messages.filter((m: Json) => m.role === "tool"),
  );
}
native(
  "Kimi source host retains native profile/tools/auth and returns exact witnessed native results",
  async () => {
    const r = await run("allow");
    expect(r.callbacks).toHaveLength(1);
    expect(r.observations).toHaveLength(1);
    const { aborted, ...call } = r.callbacks[0];
    expect(aborted).toBe(false);
    expect(r.observations[0]).toEqual(call);
    expect(call).toMatchObject({
      agentId: "main",
      name: "list_teams",
      args: { query: "Engineering" },
    });
    expect(call.sessionId).toStartWith("session_");
    expect(r.permissions).toHaveLength(1);
    expect(r.results).toHaveLength(1);
    expect(r.results[0].isError).toBe(false);
    expect(
      toolMessages(r).some(
        (m) =>
          m.tool_call_id === call.callId &&
          JSON.stringify(m.content).includes("Engineering"),
      ),
    ).toBe(true);
    expect(r.requests).toHaveLength(2);
    expect(JSON.stringify(r.requests[0].messages)).toContain(
      "ion-workspace-instruction-marker",
    );
    const advertised = r.requests[0].tools.map((t: Json) => t.function.name);
    for (const name of [
      "list_teams",
      "invoke_team",
      "get_team_runs",
      "await_team",
      "read_team_document",
    ])
      expect(advertised).toContain(name);
  },
  180_000,
);
native(
  "Kimi source host separates identical native calls and later turns",
  async () => {
    const r = await run("identical");
    // Released Kimi deduplicates equal name/arguments within a native step.
    // Both native call IDs remain observable, but only the original executes.
    expect(r.observations).toHaveLength(4);
    expect(
      new Set(r.observations.map((c: Json) => `${c.turnId}:${c.callId}`)).size,
    ).toBe(4);
    expect(r.callbacks).toHaveLength(2);
    expect(r.results).toHaveLength(4);
    expect(r.results.every((c: Json) => !c.isError)).toBe(true);
    expect(
      new Set(r.callbacks.map((c: Json) => `${c.turnId}:${c.callId}`)).size,
    ).toBe(2);
    expect(new Set(r.callbacks.map((c: Json) => c.turnId)).size).toBe(2);
    for (const { aborted, ...call } of r.callbacks) {
      expect(aborted).toBe(false);
      expect(r.observations).toContainEqual(call);
    }
  },
  180_000,
);
native(
  "Kimi native denial prevents the executable callback",
  async () => {
    const r = await run("deny");
    expect(r.permissions).toHaveLength(1);
    expect(r.callbacks).toEqual([]);
    expect(r.results.some((c: Json) => c.isError)).toBe(true);
  },
  180_000,
);
for (const mode of ["missing-witness", "changed-witness"])
  native(
    `Kimi source host refuses ${mode} independently of native approval`,
    async () => {
      const r = await run(mode);
      expect(r.permissions).toHaveLength(1);
      expect(r.observations).toHaveLength(1);
      expect(r.callbacks).toEqual([]);
      expect(r.results.some((c: Json) => c.isError)).toBe(true);
      expect(JSON.stringify(toolMessages(r))).toContain(
        "refused missing or changed witness",
      );
    },
    180_000,
  );
native(
  "Kimi native Stop aborts the executable callback",
  async () => {
    const r = await run("stop-execution");
    expect(r.callbacks).toHaveLength(1);
    expect(r.callbacks[0].aborted).toBe(true);
  },
  180_000,
);
native(
  "Kimi source host refuses collisions before additive registration",
  async () => {
    const r = await run("collision");
    expect(r.registrations).toEqual([]);
    expect(r.requests).toEqual([]);
    expect(r.callbacks).toEqual([]);
  },
  180_000,
);
native(
  "Kimi native profile refresh keeps defaults and reactivates additive Ion tools",
  async () => {
    const r = await run("profile-refresh");
    expect(r.callbacks).toHaveLength(2);
    expect(r.registrations).toHaveLength(1);
    expect(r.requests).toHaveLength(4);
  },
  180_000,
);

native(
  "Kimi source host keeps the native Read tool executable beside Ion tools",
  async () => {
    const r = await run("native-read");
    expect(r.callbacks).toHaveLength(1);
    expect(r.registrations[0].before).toContain("Read");
    expect(r.results).toHaveLength(2);
    expect(r.results.every((c: Json) => !c.isError)).toBe(true);
    expect(JSON.stringify(toolMessages(r))).toContain(
      "ion-native-builtin-read-marker",
    );
  },
  180_000,
);
native(
  "Kimi native Stop at approval starts no executable callback",
  async () => {
    const r = await run("stop-approval");
    expect(r.permissions).toHaveLength(1);
    expect(r.callbacks).toEqual([]);
    expect(r.promptResults[0].stopReason).toBe("cancelled");
  },
  180_000,
);
native(
  "Kimi native close/resume replaces scoped authority and reattaches Ion tools",
  async () => {
    const r = await run("resume");
    expect(r.registrations).toHaveLength(2);
    expect(r.registrations[0].sessionId).toBe(r.registrations[1].sessionId);
    expect(
      r.revocations.some(
        (x: Json) => x.sessionId === r.registrations[0].sessionId,
      ),
    ).toBe(true);
    expect(r.callbacks).toHaveLength(2);
    expect(r.callbacks[0].sessionId).toBe(r.callbacks[1].sessionId);
    expect(r.callbacks[0].callId).not.toBe(r.callbacks[1].callId);
    expect(r.requests).toHaveLength(4);
  },
  180_000,
);
