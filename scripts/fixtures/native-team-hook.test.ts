import { expect, test } from "bun:test";
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { randomUUID } from "node:crypto";

const binary = process.env.ION_TEST_ION_DAEMON;
const key = "ION_SOURCE_TEAM_HOOK_TOKEN";
const endpoint = "ION_SOURCE_TEAM_HOOK_ENDPOINT";
const token =
  randomUUID().replaceAll("-", "") + randomUUID().replaceAll("-", "");
const limit = 44 * 1024;
async function run(
  input: string,
  override: NodeJS.ProcessEnv = {},
  args: string[] = [],
) {
  const env: NodeJS.ProcessEnv = {};
  for (const name of ["SYSTEMROOT", "WINDIR", "PATH", "PATHEXT", "TEMP", "TMP"])
    if (process.env[name]) env[name] = process.env[name];
  // No daemon token, provider account, app home or database configuration.
  Object.assign(
    env,
    { [endpoint]: "http://127.0.0.1:1/hook", [key]: token },
    override,
  );
  const child = spawn(binary!, ["--source-team-hook", ...args], {
    env,
    windowsHide: true,
    stdio: ["pipe", "pipe", "pipe"],
  });
  let stdout = "",
    stderr = "";
  child.stdout.on("data", (chunk) => {
    stdout += chunk;
    if (stdout.length > limit + 1024) child.kill();
  });
  child.stderr.on("data", (chunk) => {
    stderr += chunk;
  });
  child.stdin.on("error", () => {});
  child.stdin.end(input);
  const code = await new Promise<number | null>((resolve, reject) => {
    const timeout = setTimeout(() => {
      child.kill();
      reject(new Error("Native helper exceeded its fixture deadline"));
    }, 10000);
    child.once("error", (error) => {
      clearTimeout(timeout);
      reject(error);
    });
    child.once("close", (code) => {
      clearTimeout(timeout);
      resolve(code);
    });
  });
  expect(stderr).toBe("");
  expect(stdout).not.toContain(token);
  return { code, value: JSON.parse(stdout) };
}
async function server(
  reply: (request: any, response: any, data: string) => void,
) {
  let received = 0;
  const instance = createServer(async (request, response) => {
    received++;
    let data = "";
    for await (const chunk of request) data += chunk;
    expect(request.url).toBe("/hook");
    expect(request.headers.authorization).toBe(`Bearer ${token}`);
    reply(request, response, data);
  });
  await new Promise<void>((resolve) =>
    instance.listen(0, "127.0.0.1", resolve),
  );
  const address = instance.address();
  if (!address || typeof address === "string")
    throw new Error("Missing fixture endpoint");
  return {
    url: `http://127.0.0.1:${address.port}/hook`,
    received: () => received,
    close: async () => {
      instance.closeAllConnections();
      await new Promise<void>((resolve) => instance.close(() => resolve()));
    },
  };
}
test.skipIf(!binary)(
  "native hook helper runs before daemon authentication and rejects unknown flags",
  async () => {
    const help = await run("", {}, ["--help"]);
    expect(help.code).toBe(0);
    expect(help.value.environment).toEqual([endpoint, key]);
    const unknown = await run("", {}, ["--bad-option"]);
    expect(unknown.code).toBe(2);
    expect(unknown.value.decision).toBe("deny");
  },
);
test.skipIf(!binary)(
  "native hook helper sends exactly one authenticated JSON event and returns the owner response",
  async () => {
    const event = {
      sessionId: "native-session",
      toolUseId: "native-call",
      toolInput: { query: "工程 🚀" },
    };
    const response = {
      hookSpecificOutput: {
        updatedInput: { query: "工程 🚀", __ion_contract_binding: "bound" },
      },
    };
    const host = await server((_request, reply, data) => {
      expect(JSON.parse(data)).toEqual(event);
      reply.writeHead(200, { "content-type": "application/json" });
      reply.end(JSON.stringify(response));
    });
    try {
      const result = await run(JSON.stringify(event), { [endpoint]: host.url });
      expect(result.code).toBe(0);
      expect(result.value).toEqual(response);
      expect(host.received()).toBe(1);
    } finally {
      await host.close();
    }
  },
);
test.skipIf(!binary)(
  "native hook helper refuses malformed oversized nonobject input before contacting owner",
  async () => {
    const host = await server((_request, reply) => {
      reply.end("{}");
    });
    try {
      for (const input of [
        "not JSON",
        "[]",
        "null",
        "{} trailing",
        JSON.stringify({ x: "x".repeat(limit) }),
      ]) {
        const result = await run(input, { [endpoint]: host.url });
        expect(result.code).toBe(2);
        expect(result.value.decision).toBe("deny");
      }
      expect(host.received()).toBe(0);
    } finally {
      await host.close();
    }
  },
);
test.skipIf(!binary)(
  "native hook helper refuses endpoint aliases remote routes and invalid credentials",
  async () => {
    const host = await server((_request, reply) => {
      reply.end("{}");
    });
    try {
      for (const value of [
        host.url.replace("127.0.0.1", "localhost"),
        host.url.replace("127.0.0.1", "127.1"),
        host.url + "?x",
        host.url + "#x",
        host.url.replace("/hook", "/other"),
        host.url.replace("http:", "https:"),
        host.url.replace("127.0.0.1", "user@127.0.0.1"),
      ]) {
        expect((await run("{}", { [endpoint]: value })).value.decision).toBe(
          "deny",
        );
      }
      for (const value of [
        "",
        "short",
        token + "\r\nextra",
        token + " ",
        "x".repeat(257),
      ]) {
        expect(
          (await run("{}", { [endpoint]: host.url, [key]: value })).value
            .decision,
        ).toBe("deny");
      }
      expect(host.received()).toBe(0);
    } finally {
      await host.close();
    }
  },
);
test.skipIf(!binary)(
  "native hook helper refuses redirects failed malformed and excessive owner responses",
  async () => {
    for (const mode of [
      "redirect",
      "failure",
      "array",
      "bad-json",
      "oversized",
    ]) {
      const host = await server((_request, reply) => {
        if (mode === "redirect") {
          reply.writeHead(302, { location: "http://127.0.0.1:1/forbidden" });
          reply.end("{}");
        } else if (mode === "failure") {
          reply.writeHead(500);
          reply.end("private error");
        } else {
          reply.writeHead(200, { "content-type": "application/json" });
          reply.end(
            mode === "array"
              ? "[]"
              : mode === "bad-json"
                ? "private invalid response"
                : JSON.stringify({ x: "x".repeat(limit) }),
          );
        }
      });
      try {
        const result = await run("{}", { [endpoint]: host.url });
        expect(result.code).toBe(2);
        expect(result.value.decision).toBe("deny");
        expect(JSON.stringify(result.value)).not.toContain("private");
        expect(host.received()).toBe(1);
      } finally {
        await host.close();
      }
    }
  },
);

test.skipIf(!binary)(
  "native hook helper expires an owner that accepts the event but never responds",
  async () => {
    const host = await server(() => {});
    try {
      const started = Date.now();
      const result = await run("{}", { [endpoint]: host.url });
      expect(result.code).toBe(2);
      expect(result.value.decision).toBe("deny");
      expect(host.received()).toBe(1);
      expect(Date.now() - started).toBeLessThan(10000);
    } finally {
      await host.close();
    }
  },
  15000,
);
