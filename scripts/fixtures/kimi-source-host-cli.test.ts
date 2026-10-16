import { expect, test } from "bun:test";
import {
  mkdtempSync,
  mkdirSync,
  readFileSync,
  copyFileSync,
  writeFileSync,
  rmSync,
  existsSync,
  realpathSync,
} from "node:fs";
import { join, dirname, resolve, sep } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import { connection } from "../kimi-host/teams.mjs";
const node = process.env.ION_TEST_KIMI_NODE;
const source = process.env.ION_TEST_KIMI_SOURCE;
const native = test.skipIf(!node || !source);
const prepare = fileURLToPath(
  new URL("../kimi-host/prepare-source-host.mjs", import.meta.url),
);
const host = fileURLToPath(new URL("../kimi-host/host.mjs", import.meta.url));
async function run(file: string, args: string[], env?: Record<string, string>) {
  const child = Bun.spawn([node!, file, ...args], {
    env: env ?? process.env,
    stdout: "pipe",
    stderr: "pipe",
    stdin: "ignore",
  });
  const timer = setTimeout(() => child.kill(), 20000);
  try {
    const [stdout, stderr, code] = await Promise.all([
      new Response(child.stdout).text(),
      new Response(child.stderr).text(),
      child.exited,
    ]);
    return { stdout, stderr, code };
  } finally {
    clearTimeout(timer);
  }
}
function fixture() {
  const base = realpathSync(process.env.ION_NATIVE_FIXTURE_ROOT ?? tmpdir());
  const root = mkdtempSync(join(base, "kcli-"));
  return {
    root,
    close() {
      const exact = realpathSync(root);
      if (dirname(exact) !== base || !exact.startsWith(base + sep))
        throw Error("Cleanup escaped fixture parent");
      rmSync(exact, { recursive: true, force: true });
    },
  };
}
native(
  "Kimi source host version probes are bare and require no account/configuration",
  async () => {
    for (const file of [prepare, host])
      for (const flag of ["--version", "-v", "-V"]) {
        const r = await run(file, [flag], {
          SystemRoot: process.env.SystemRoot ?? "",
          PATH: dirname(node!),
        });
        expect(r).toEqual({ stdout: "0.1.0\n", stderr: "", code: 0 });
      }
  },
);
native(
  "Kimi source host setup rejects unknown, repeated and missing flags before output",
  async () => {
    const f = fixture();
    try {
      for (const args of [
        ["--wat", "yes"],
        ["--source", "x", "--source", "y"],
        ["--source"],
        ["--output", join(f.root, "host"), "--wat", "yes", "--help"],
      ]) {
        const r = await run(prepare, args);
        expect(r.code).toBe(2);
        expect(r.stdout).toStartWith("error:");
        expect(existsSync(join(f.root, "host"))).toBe(false);
      }
    } finally {
      f.close();
    }
  },
);
native(
  "Kimi source host refuses a changed pinned contract without rewriting source",
  async () => {
    const f = fixture();
    try {
      const copied = join(f.root, "source");
      mkdirSync(copied);
      copyFileSync(
        join(source!, "tsconfig.json"),
        join(copied, "tsconfig.json"),
      );
      writeFileSync(join(copied, "pnpm-lock.yaml"), "changed");
      const r = await run(prepare, [
        "--source",
        copied,
        "--node",
        node!,
        "--native-command",
        node!,
        "--output",
        join(f.root, "host"),
      ]);
      expect(r.code).toBe(2);
      expect(r.stdout).toContain("identity changed");
      expect(readFileSync(join(copied, "pnpm-lock.yaml"), "utf8")).toBe(
        "changed",
      );
      expect(existsSync(join(f.root, "host"))).toBe(false);
    } finally {
      f.close();
    }
  },
);
native(
  "Kimi local source host forwards native login and preserves configuration",
  async () => {
    const f = fixture();
    try {
      const cli = join(f.root, "login.mjs");
      writeFileSync(
        cli,
        `import {writeFileSync} from 'node:fs';if(process.argv[2]==='--version')console.log('2.2.0');else writeFileSync(process.env.KIMI_CODE_HOME+'/login.json',JSON.stringify({args:process.argv.slice(2),home:process.env.KIMI_CODE_HOME}));`,
      );
      const output = join(f.root, "host");
      const r = await run(prepare, [
        "--source",
        resolve(source!),
        "--node",
        node!,
        "--native-command",
        node!,
        "--native-entry",
        cli,
        "--output",
        output,
      ]);
      expect(r.code, r.stdout + r.stderr).toBe(0);
      const config = readFileSync(join(output, "host.json"), "utf8");
      const repeated = await run(prepare, [
        "--source",
        resolve(source!),
        "--node",
        node!,
        "--native-command",
        node!,
        "--native-entry",
        cli,
        "--output",
        output,
      ]);
      expect(repeated.code, repeated.stdout + repeated.stderr).toBe(0);
      expect(repeated.stdout).toContain("already prepared");

      const home = join(f.root, "home");
      mkdirSync(home);
      writeFileSync(join(home, "config.toml"), "owner-config-marker");
      for (const args of [
        ["acp", "--login", "--region", "global"],
        ["login"],
      ]) {
        const login = await run(join(output, "host.mjs"), args, {
          ...process.env,
          KIMI_CODE_HOME: home,
        });
        expect(login.code, login.stderr).toBe(0);
        const forwarded = JSON.parse(
          readFileSync(join(home, "login.json"), "utf8"),
        );
        expect(forwarded.home).toBe(home);
        expect(forwarded.args).toEqual([
          "acp",
          "--login",
          ...(args[0] === "acp" ? ["--region", "global"] : []),
        ]);
      }
      const bad = await run(join(output, "host.mjs"), [
        "acp",
        "--login",
        "--bogus",
      ]);
      expect(bad.code).toBe(1);
      expect(bad.stdout).toBe("");
      const probe = await run(join(output, "host.mjs"), ["--ion-host-version"]);
      expect(probe.code, probe.stderr).toBe(0);
      expect(probe.stdout).toContain(
        "source/472192c34c1efad89a369870eff0da0b708ce9bb protocol/1",
      );
      expect(readFileSync(join(output, "host.json"), "utf8")).toBe(config);
      expect(readFileSync(join(home, "config.toml"), "utf8")).toBe(
        "owner-config-marker",
      );
    } finally {
      f.close();
    }
  },
);
test("Kimi callback transport refuses nonlocal URLs, embedded credentials and invalid secrets", () => {
  for (const endpoint of [
    "https://127.0.0.1:1234",
    "http://example.com:1234",
    "http://user@127.0.0.1:1234",
    "http://127.0.0.1:1234/call",
    "http://127.0.0.1:1234/?x=1",
    "http://127.0.0.1:1234/#fragment",
  ])
    expect(() => connection(endpoint, "a".repeat(64))).toThrow();
  expect(() => connection("http://127.0.0.1:1234", "weak")).toThrow();
});

const ptyNative = test.skipIf(!node || !source || process.platform !== "win32");
ptyNative(
  "Kimi pinned Windows terminal assets load and execute under the host Node ABI",
  async () => {
    const f = fixture();
    try {
      const work = join(f.root, "work");
      mkdirSync(work);
      const controller = fileURLToPath(
        new URL("./kimi-pty-controller.cjs", import.meta.url),
      );
      const result = await run(controller, [], {
        SystemRoot: process.env.SystemRoot!,
        WINDIR: process.env.WINDIR!,
        PATH: dirname(node!),
        HOME: work,
        USERPROFILE: work,
        TEMP: work,
        TMP: work,
        ION_TEST_KIMI_SOURCE: resolve(source!),
        ION_KIMI_PTY_WORK: work,
      });
      expect(result.code, result.stderr + result.stdout).toBe(0);
      expect(JSON.parse(result.stdout.trim())).toMatchObject({
        platform: "win32",
        arch: "x64",
        pty: "1.1.0",
        exitCode: 0,
        marker: true,
      });
    } finally {
      f.close();
    }
  },
);
