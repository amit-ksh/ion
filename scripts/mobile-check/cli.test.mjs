import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile, rm, copyFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { inspect, main, deviceCounts } from "./cli.mjs";
const entry = fileURLToPath(new URL("../mobile-check.mjs", import.meta.url));
async function fixture(fn) {
  const root = await mkdtemp(join(tmpdir(), "ion-mobile-check-"));
  try {
    const project = join(root, "apps/mobile"),
      sdk = join(root, "sdk"),
      jdk = join(root, "jdk");
    for (const path of [
      project,
      join(root, "node_modules/expo"),
      join(sdk, "platform-tools"),
      join(sdk, "platforms/android-36"),
      join(sdk, "build-tools/36.0.0"),
      join(jdk, "bin"),
    ])
      await mkdir(path, { recursive: true });
    await writeFile(
      join(project, "app.json"),
      JSON.stringify({
        expo: {
          plugins: [
            ["expo-audio", { microphonePermission: "Allow microphone" }],
          ],
        },
      }),
    );
    await writeFile(
      join(project, "package.json"),
      JSON.stringify({
        name: "@waku/mobile",
        dependencies: { expo: "~57.0.18" },
      }),
    );
    await writeFile(
      join(project, "eas.json"),
      JSON.stringify({ build: { preview: { distribution: "internal" } } }),
    );
    await writeFile(
      join(root, "node_modules/expo/package.json"),
      JSON.stringify({ version: "57.0.18" }),
    );
    for (const file of [
      join(sdk, "platform-tools/adb.exe"),
      join(sdk, "platforms/android-36/android.jar"),
      join(sdk, "build-tools/36.0.0/aapt2.exe"),
      join(sdk, "build-tools/36.0.0/apksigner.bat"),
      join(sdk, "build-tools/36.0.0/d8.bat"),
      join(jdk, "bin/java.exe"),
    ])
      await writeFile(file, "tool fixture");
    await fn({ root, project, sdk, jdk });
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}
const at = (result, id) => result.checks.find((row) => row.id === id);
test("inspection identifies prerequisites without launching tools or claiming acceptance", () =>
  fixture(async (c) => {
    const result = await inspect(
      { project: c.project, dependencies: c.root, sdk: c.sdk, devices: false },
      {
        platform: "win32",
        env: { JAVA_HOME: c.jdk },
        invoke: () => assert.fail("Unexpected device command"),
      },
    );
    for (const id of [
      "expo_dependencies",
      "android_sdk",
      "java_home",
      "microphone_config",
      "preview_profile",
    ])
      assert.equal(at(result, id).status, "present");
    assert.equal(at(result, "custom_native_build").status, "native_only");
    assert.equal(at(result, "android_devices").status, "not_checked");
    assert.equal(at(result, "ios_local_build").status, "unavailable");
    assert.equal(at(result, "physical_acceptance").status, "pending");
  }));
test("explicit inventory uses a bounded command and returns no device identifiers", () =>
  fixture(async (c) => {
    const output = [];
    const code = await main(
      [
        "check",
        "--project",
        c.project,
        "--dependencies",
        c.root,
        "--sdk",
        c.sdk,
        "--devices",
      ],
      {
        platform: "win32",
        env: {
          JAVA_HOME: c.jdk,
          ADB_SERVER_SOCKET: "tcp:remote:7777",
          PRIVATE_TOKEN: "secret",
        },
        write: (v) => output.push(v),
        invoke: async (bin, args, opts) => {
          assert.equal(bin, join(c.sdk, "platform-tools/adb.exe"));
          assert.deepEqual(args, [
            "-H",
            "127.0.0.1",
            "-P",
            "5037",
            "devices",
            "-l",
          ]);
          assert.equal(opts.env.ADB_SERVER_SOCKET, undefined);
          assert.equal(opts.env.PRIVATE_TOKEN, undefined);
          assert.equal(opts.timeout, 5000);
          assert.equal(opts.maxBuffer, 16384);
          assert.equal(opts.windowsHide, true);
          return {
            stdout:
              "List of devices attached\nPRIVATE_A device model:PRIVATE_PHONE\nPRIVATE_B unauthorized\nPRIVATE_C offline\n",
          };
        },
      },
    );
    assert.equal(code, 0);
    assert.match(output[0], /1 authorized, 1 unauthorized, 1 offline/);
    assert.doesNotMatch(output[0], /PRIVATE_/);
  }));
test("failed device inspection is sanitized and cannot establish acceptance", () =>
  fixture(async (c) => {
    const result = await inspect(
      { project: c.project, sdk: c.sdk, devices: true },
      {
        platform: "win32",
        env: {},
        invoke: async () => {
          throw new Error("PRIVATE_TOKEN secret trace");
        },
      },
    );
    assert.equal(at(result, "android_devices").status, "unavailable");
    assert.doesNotMatch(JSON.stringify(result), /PRIVATE_TOKEN/);
    assert.equal(at(result, "physical_acceptance").status, "pending");
  }));
test("missing SDK and incompatible Expo are reported as setup requirements", () =>
  fixture(async (c) => {
    await writeFile(
      join(c.root, "node_modules/expo/package.json"),
      JSON.stringify({ version: "56.0.1" }),
    );
    const result = await inspect(
      { project: c.project, sdk: join(c.root, "absent"), devices: true },
      {
        platform: "win32",
        env: {},
        invoke: () => assert.fail("Missing ADB must not run"),
      },
    );
    assert.equal(at(result, "expo_dependencies").status, "required");
    assert.equal(at(result, "android_sdk").status, "required");
    assert.equal(at(result, "android_devices").status, "unavailable");
  }));
test("oversized and malformed project configuration fail without leaking source", () =>
  fixture(async (c) => {
    for (const value of [
      "PRIVATE_CONFIG".repeat(20000),
      "PRIVATE_INVALID_JSON",
    ]) {
      await writeFile(join(c.project, "app.json"), value);
      const output = [];
      assert.equal(
        await main(["--project", c.project], { write: (v) => output.push(v) }),
        1,
      );
      assert.doesNotMatch(output.join(""), /PRIVATE_/);
      assert.match(output[0], /bounded valid JSON/);
    }
  }));
test("unknown, missing, repeated and control-character flags fail before dependencies", async () => {
  for (const args of [
    ["--bogus", "PRIVATE_SECRET"],
    ["--project"],
    ["--devices", "--devices"],
    ["--sdk", "a", "--sdk", "b"],
    ["--project", "a", "--project", "b"],
    ["--project", "x\n"],
    ["--help", "--bogus"],
    ["--devices", "--devices", "--help"],
    ["--project", "--help"],
  ]) {
    const output = [];
    assert.equal(
      await main(args, {
        write: (v) => output.push(v),
        invoke: () => assert.fail("Invalid argv must not launch"),
      }),
      2,
    );
    assert.match(output[0], /--project/);
    assert.doesNotMatch(output[0], /PRIVATE_SECRET/);
  }
});
test("device inventories enforce record and byte bounds", () => {
  assert.throws(() => deviceCounts("x device\n".repeat(65)), /bound/);
  assert.throws(() => deviceCounts("x".repeat(16385)), /bound/);
  assert.deepEqual(deviceCounts("List of devices attached\n"), {
    ready: 0,
    unauthorized: 0,
    offline: 0,
    other: 0,
  });
});
test("all version aliases use the standalone entry before invalid project/dependency reads", () => {
  for (const alias of ["-v", "-V", "--version"])
    assert.equal(
      execFileSync(process.execPath, [entry, alias], {
        cwd: tmpdir(),
        encoding: "utf8",
      }).trim(),
      "ion-mobile-check/1",
    );
});

test("version remains available with no command graph and stays near Node startup", () =>
  fixture(async (c) => {
    const leaf = join(c.root, "mobile-check.mjs");
    await copyFile(entry, leaf);
    const elapsed = (args) => {
      const start = performance.now();
      execFileSync(process.execPath, args, { encoding: "utf8" });
      return performance.now() - start;
    };
    const floors = [],
      versions = [];
    for (let i = 0; i < 3; i++) {
      floors.push(elapsed(["-e", "console.log(1)"]));
      versions.push(elapsed([leaf, "--version"]));
    }
    const median = (values) => values.sort((a, b) => a - b)[1];
    assert.ok(
      median(versions) <= median(floors) * 2 + 125,
      "Version probe should pay only Node startup, without loading the command graph",
    );
  }));

test("empty tool files and empty JDK folders cannot satisfy prerequisites", () =>
  fixture(async (c) => {
    await writeFile(join(c.sdk, "platform-tools/adb.exe"), "");
    await writeFile(join(c.jdk, "bin/java.exe"), "");
    const result = await inspect(
      { project: c.project, sdk: c.sdk, devices: true },
      {
        platform: "win32",
        env: { JAVA_HOME: c.jdk },
        invoke: () => assert.fail("Empty ADB must not execute"),
      },
    );
    assert.equal(at(result, "android_sdk").status, "required");
    assert.equal(at(result, "java_home").status, "required");
    assert.equal(at(result, "android_devices").status, "unavailable");
  }));
test("SDK scans stop at bounded folder inventories", () =>
  fixture(async (c) => {
    for (let i = 0; i < 256; i++)
      await writeFile(join(c.sdk, "platforms", `extra-${i}`), "");
    const result = await inspect(
      { project: c.project, sdk: c.sdk, devices: false },
      { platform: "win32", env: {} },
    );
    assert.equal(at(result, "android_sdk").status, "required");
    assert.match(at(result, "android_sdk").detail, /256 entries/);
  }));
test("help never performs device inventory", async () => {
  const output = [];
  assert.equal(
    await main(["check", "--devices", "--help"], {
      write: (v) => output.push(v),
      invoke: () => assert.fail("Help must not run ADB"),
    }),
    0,
  );
  assert.match(output[0], /may start it/);
});
