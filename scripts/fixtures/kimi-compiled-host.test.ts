import { expect, test } from "bun:test";
import {
  mkdtempSync,
  mkdirSync,
  writeFileSync,
  readFileSync,
  rmSync,
  realpathSync,
  copyFileSync,
} from "node:fs";
import { join, dirname, sep, resolve } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import {
  identity,
  inventory,
  verifyPackage,
} from "../kimi-host/compiled-package.mjs";
const packageRoot = process.env.ION_TEST_KIMI_PACKAGE;
const node = process.env.ION_TEST_KIMI_NODE;
const native = test.skipIf(!packageRoot || !node);
function fixture() {
  const base = realpathSync(process.env.ION_NATIVE_FIXTURE_ROOT ?? tmpdir());
  const root = mkdtempSync(join(base, "kcompiled-"));
  return {
    root,
    close() {
      const exact = realpathSync(root);
      if (dirname(exact) !== base || !exact.startsWith(base + sep))
        throw Error("Fixture cleanup escaped parent");
      rmSync(exact, { recursive: true, force: true });
    },
  };
}
async function manifest(root: string) {
  const files = await inventory(root);
  writeFileSync(
    join(root, "package-manifest.json"),
    JSON.stringify({
      format: 1,
      identity,
      target: { platform: process.platform, arch: process.arch },
      files,
    }),
  );
}
async function run(args: string[], env?: Record<string, string | undefined>) {
  const started = performance.now();
  const child = Bun.spawn(
    [
      join(packageRoot!, process.platform === "win32" ? "node.exe" : "node"),
      join(packageRoot!, "host.mjs"),
      ...args,
    ],
    {
      env: env ?? process.env,
      stdin: "ignore",
      stdout: "pipe",
      stderr: "pipe",
    },
  );
  const timer = setTimeout(() => child.kill(), 60000);
  try {
    const [stdout, stderr, code] = await Promise.all([
      new Response(child.stdout).text(),
      new Response(child.stderr).text(),
      child.exited,
    ]);
    return { stdout, stderr, code, ms: performance.now() - started };
  } finally {
    clearTimeout(timer);
  }
}
test("compiled Kimi package verifier rejects changed, extra and missing modules", async () => {
  const f = fixture();
  try {
    writeFileSync(join(f.root, "native.js"), "export const native=true;\n");
    await manifest(f.root);
    expect((await verifyPackage(f.root)).identity).toBe(identity);
    writeFileSync(join(f.root, "native.js"), "changed");
    await expect(verifyPackage(f.root)).rejects.toThrow("changed");
    writeFileSync(join(f.root, "native.js"), "export const native=true;\n");
    writeFileSync(join(f.root, "extra.js"), "injected");
    await expect(verifyPackage(f.root)).rejects.toThrow("Unexpected");
    rmSync(join(f.root, "extra.js"));
    rmSync(join(f.root, "native.js"));
    await expect(verifyPackage(f.root)).rejects.toThrow();
  } finally {
    f.close();
  }
});
test("compiled Kimi manifest refuses duplicate, foreign-platform and escaping entries", async () => {
  const f = fixture();
  try {
    writeFileSync(join(f.root, "native.js"), "native");
    await manifest(f.root);
    const original = JSON.parse(
      readFileSync(join(f.root, "package-manifest.json"), "utf8"),
    );
    for (const modified of [
      { ...original, files: [...original.files, ...original.files] },
      { ...original, target: { platform: "foreign", arch: process.arch } },
      {
        ...original,
        files: [{ path: "../escape.js", bytes: 0, sha256: "a".repeat(64) }],
      },
    ]) {
      writeFileSync(
        join(f.root, "package-manifest.json"),
        JSON.stringify(modified),
      );
      await expect(verifyPackage(f.root)).rejects.toThrow();
    }
  } finally {
    f.close();
  }
});
native(
  "compiled Kimi package verifies from its deployed directory with bundled runtime",
  async () => {
    const r = await run(["--verify"]);
    expect(r.code, r.stdout + r.stderr).toBe(0);
    expect(r.stdout).toContain("status: verified");
    const m = JSON.parse(
      readFileSync(join(packageRoot!, "package-manifest.json"), "utf8"),
    );
    expect(m.runtime.compiler.modules).toBeGreaterThan(1000);
    expect(m.files.some((f: any) => f.path.endsWith("conpty.node"))).toBe(true);
    expect(m.files.some((f: any) => f.path.endsWith("conpty.dll"))).toBe(true);
    expect(m.files.some((f: any) => f.path === "NODE-LICENSE.txt")).toBe(true);
    expect(m.files.some((f: any) => f.path === "engine/notices/LICENSE")).toBe(
      true,
    );
    console.log(
      `Compiled package verification: ${Math.round(r.ms)} ms, ${m.files.length} files`,
    );
  },
);
native(
  "compiled Kimi config and login stay outside the immutable package",
  async () => {
    const f = fixture();
    try {
      const native = join(f.root, "login.mjs");
      writeFileSync(
        native,
        `import {writeFileSync} from 'node:fs';if(process.argv[2]==='--version')console.log('2.2.0');else writeFileSync(process.env.KIMI_CODE_HOME+'/login.json',JSON.stringify(process.argv.slice(2)));`,
      );
      const config = join(f.root, "host.json");
      const args = [
        "configure",
        "--native-command",
        node!,
        "--native-entry",
        native,
        "--output",
        config,
      ];
      const initial = await run(args);
      expect(initial.code, initial.stdout + initial.stderr).toBe(0);
      const before = readFileSync(config, "utf8");
      const again = await run(args);
      expect(again.code, again.stdout + again.stderr).toBe(0);
      expect(readFileSync(config, "utf8")).toBe(before);
      const home = join(f.root, "home");
      mkdirSync(home);
      const env = {
        ...process.env,
        ION_KIMI_HOST_CONFIG: config,
        KIMI_CODE_HOME: home,
      };
      for (const command of [
        ["acp", "--login", "--region", "global"],
        ["login"],
      ]) {
        const r = await run(command, env);
        expect(r.code, r.stdout + r.stderr).toBe(0);
        expect(
          JSON.parse(readFileSync(join(home, "login.json"), "utf8")),
        ).toEqual([
          "acp",
          "--login",
          ...(command.length > 1 ? ["--region", "global"] : []),
        ]);
      }
      const version = await run(["--ion-host-version"], env);
      expect(version.code, version.stdout + version.stderr).toBe(0);
      expect(version.stdout.trim()).toBe(identity);
    } finally {
      f.close();
    }
  },
);
native(
  "compiled Kimi CLI rejects unknown configuration arguments without overwriting files",
  async () => {
    const f = fixture();
    try {
      const out = join(f.root, "owner.json");
      writeFileSync(out, "owner");
      for (const args of [
        ["configure", "--wat", "x"],
        ["configure", "--native-command", node!, "--output", out],
        ["acp", "--login", "--wat"],
      ]) {
        const r = await run(args);
        expect(r.code).not.toBe(0);
        expect(readFileSync(out, "utf8")).toBe("owner");
      }
    } finally {
      f.close();
    }
  },
);

native(
  "compiled Kimi engine resolves its native module graph with every outside import refused",
  async () => {
    const f = fixture();
    try {
      const controller = join(f.root, "load.mjs");
      writeFileSync(
        controller,
        `import {registerHooks} from 'node:module';import {fileURLToPath,pathToFileURL} from 'node:url';import {join,relative,sep,isAbsolute} from 'node:path';const root=process.env.ION_TEST_KIMI_PACKAGE;registerHooks({resolve(spec,ctx,next){const r=next(spec,ctx);if(r.url.startsWith('file:')){const p=fileURLToPath(r.url);const rel=relative(root,p);if(rel==='..'||rel.startsWith('..'+sep)||isAbsolute(rel))throw Error('Module outside package refused');}return r;}});const start=performance.now();const core=await import(pathToFileURL(join(root,'engine/node_modules/@moonshot-ai/agent-core-v2/src/index.js')));const acp=await import(pathToFileURL(join(root,'engine/node_modules/@moonshot-ai/acp-server/src/start.js')));if(typeof core.bootstrap!=='function'||typeof acp.runAcpServer!=='function')throw Error('Native engine exports missing');process.stdout.write(JSON.stringify({loaded:true,ms:performance.now()-start})+'\\n',()=>process.exit(0));`,
      );
      const child = Bun.spawn(
        [
          join(
            packageRoot!,
            process.platform === "win32" ? "node.exe" : "node",
          ),
          controller,
        ],
        {
          env: {
            SystemRoot: process.env.SystemRoot,
            WINDIR: process.env.WINDIR,
            HOME: f.root,
            USERPROFILE: f.root,
            TEMP: f.root,
            TMP: f.root,
            ION_TEST_KIMI_PACKAGE: resolve(packageRoot!),
          },
          stdin: "ignore",
          stdout: "pipe",
          stderr: "pipe",
        },
      );
      const timer = setTimeout(() => child.kill(), 30000);
      try {
        const [stdout, stderr, code] = await Promise.all([
          new Response(child.stdout).text(),
          new Response(child.stderr).text(),
          child.exited,
        ]);
        expect(code, stderr + stdout).toBe(0);
        expect(JSON.parse(stdout).loaded).toBe(true);
        console.log(
          `Compiled native module loading: ${Math.round(JSON.parse(stdout).ms)} ms`,
        );
      } finally {
        clearTimeout(timer);
      }
    } finally {
      f.close();
    }
  },
);

test("compiled Kimi probes verify bootstrap while ACP verification still rejects changed runtime", async () => {
  const f = fixture();
  try {
    for (const file of [
      "host.mjs",
      "compiled-package.mjs",
      "teams.mjs",
      process.platform === "win32" ? "node.exe" : "node",
      "runtime.js",
    ])
      writeFileSync(join(f.root, file), file);
    await manifest(f.root);
    writeFileSync(join(f.root, "runtime.js"), "changed runtime");
    expect(
      (await verifyPackage(f.root, { bootstrapOnly: true })).identity,
    ).toBe(identity);
    await expect(verifyPackage(f.root)).rejects.toThrow("changed");
    writeFileSync(join(f.root, "host.mjs"), "changed host");
    await expect(
      verifyPackage(f.root, { bootstrapOnly: true }),
    ).rejects.toThrow("changed");
  } finally {
    f.close();
  }
});
native(
  "compiled Kimi ACP refuses changed engine bytes before loading tools or authentication",
  async () => {
    const f = fixture();
    const path = join(
      packageRoot!,
      "engine/node_modules/@moonshot-ai/acp-server/src/start.js",
    );
    const original = readFileSync(path);
    try {
      const config = join(f.root, "config.json");
      writeFileSync(
        config,
        JSON.stringify({ format: 1, nativeCommand: node!, nativeArgs: [] }),
      );
      writeFileSync(
        path,
        Buffer.concat([original, Buffer.from("\n// fixture alteration\n")]),
      );
      const result = await run(["acp"], {
        ...process.env,
        ION_KIMI_HOST_CONFIG: config,
        HOME: f.root,
        USERPROFILE: f.root,
        KIMI_CODE_HOME: f.root,
      });
      expect(result.code).toBe(1);
      expect(result.stdout).toBe("");
      expect(result.stderr).toContain("Compiled host file changed");
    } finally {
      writeFileSync(path, original);
      f.close();
    }
  },
);
