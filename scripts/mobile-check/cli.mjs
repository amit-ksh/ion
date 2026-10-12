import { stat, open, opendir } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, resolve, join } from "node:path";
import { homedir } from "node:os";
import { promisify } from "node:util";
import { execFile } from "node:child_process";
const run = promisify(execFile),
  base = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const VERSION = "ion-mobile-check/1";
const HELP =
  "node scripts/mobile-check.mjs [check] [--project <mobile-dir>] [--dependencies <installed-repo-root>] [--sdk <android-sdk-dir>] [--devices]\nNative build preflight. Defaults: apps/mobile and its workspace dependencies. No installation/build/account call. Default only reads files. --devices runs bounded ADB inventory against the local server (may start it and contact authorized devices), without printing serial numbers. Redirected ADB environment settings are excluded.\nExamples: node scripts/mobile-check.mjs; node scripts/mobile-check.mjs check --sdk <sdk-dir> --devices; node scripts/mobile-check.mjs check --project <mobile-dir> --dependencies <installed-repo-root>";
// Fixed scalar/table schema. TOON string escaping follows JSON for reserved syntax.
const scalar = (value) =>
  typeof value !== "string"
    ? String(value)
    : !value ||
        value.trim() !== value ||
        /^(true|false|null|[+-]?[0-9]+(?:\.[0-9]+)?(?:e[+-]?[0-9]+)?)$/i.test(
          value,
        ) ||
        /[\\":,\[\]{}\x00-\x1f]/.test(value) ||
        /^[-#]/.test(value)
      ? JSON.stringify(value)
      : value;
export function parseArgs(args) {
  const config = {
    command: "check",
    project: resolve(base, "apps/mobile"),
    dependencies: null,
    sdk: null,
    devices: false,
  };
  if (args.length === 1 && ["-v", "-V", "--version"].includes(args[0]))
    return { command: "version" };
  let i = 0;
  if (args[0] === "check") i++;
  for (; i < args.length; i++) {
    const flag = args[i];
    if (flag === "--help") {
      config.command = "help";
      continue;
    }
    if (flag === "--devices") {
      if (config.devices) throw new Error("Duplicate --devices");
      config.devices = true;
      continue;
    }
    const field = {
      "--project": "project",
      "--dependencies": "dependencies",
      "--sdk": "sdk",
    }[flag];
    if (!field)
      throw new Error(
        "Unknown argument. Valid flags: --project, --dependencies, --sdk, --devices, --help",
      );
    if (
      (config[field] !== null && field !== "project") ||
      (field === "project" && config.projectExplicit)
    )
      throw new Error("Duplicate path flag");
    const value = args[++i];
    if (
      !value ||
      value.startsWith("--") ||
      value.length > 4096 ||
      /[\x00-\x1f]/.test(value)
    )
      throw new Error("Each path flag requires one bounded directory");
    config[field] = resolve(value);
    if (field === "project") config.projectExplicit = true;
  }
  return config;
}
const toolFile = async (path) => {
  try {
    const info = await stat(path);
    return info.isFile() && info.size > 0;
  } catch {
    return false;
  }
};
async function entries(path) {
  const values = [];
  const directory = await opendir(path, { bufferSize: 32 });
  for await (const entry of directory) {
    if (values.length >= 256)
      throw new Error("SDK inventory exceeds its bound");
    values.push(entry.name);
  }
  return values;
}
function adbEnvironment(env) {
  const allowed = new Set([
    "path",
    "pathext",
    "systemroot",
    "windir",
    "temp",
    "tmp",
    "home",
    "userprofile",
    "localappdata",
    "appdata",
  ]);
  return Object.fromEntries(
    Object.entries(env).filter(
      ([name, value]) =>
        allowed.has(name.toLowerCase()) && typeof value === "string",
    ),
  );
}
async function json(path) {
  const file = await open(path, "r");
  try {
    const stat = await file.stat();
    if (!stat.isFile() || stat.size > 131072)
      throw new Error("Configuration exceeds its read bound");
    const buffer = Buffer.alloc(131073);
    let length = 0;
    while (length < buffer.length) {
      const { bytesRead } = await file.read(
        buffer,
        length,
        buffer.length - length,
        null,
      );
      if (!bytesRead) break;
      length += bytesRead;
    }
    if (length > 131072)
      throw new Error("Configuration exceeds its read bound");
    return JSON.parse(buffer.subarray(0, length).toString("utf8"));
  } finally {
    await file.close();
  }
}
export function deviceCounts(text) {
  if (typeof text !== "string" || text.length > 16384)
    throw new Error("Device inventory exceeds its bound");
  const result = { ready: 0, unauthorized: 0, offline: 0, other: 0 };
  for (const line of text.split(/\r?\n/)) {
    if (!line.trim() || line.startsWith("List of devices attached")) continue;
    const match = /^\S+\s+(device|unauthorized|offline)(?:\s.*)?$/.exec(line);
    if (!match) result.other++;
    else result[match[1] === "device" ? "ready" : match[1]]++;
    if (result.ready + result.unauthorized + result.offline + result.other > 64)
      throw new Error("Device inventory exceeds its bound");
  }
  return result;
}
export async function inspect(
  config,
  {
    env = process.env,
    platform = process.platform,
    invoke = run,
    home = homedir(),
  } = {},
) {
  const checks = [],
    add = (id, status, detail) => checks.push({ id, status, detail });
  const app = await json(join(config.project, "app.json")),
    pkg = await json(join(config.project, "package.json"));
  if (!app.expo || pkg.name !== "@waku/mobile")
    throw new Error("Choose the Ion native mobile project");
  const root = resolve(config.project, "../..");
  const dependencyRoot = config.dependencies ?? root;
  let installed = null;
  try {
    installed = await json(
      join(dependencyRoot, "node_modules/expo/package.json"),
    );
  } catch {}
  const major = /^[~^]?(\d+)\./.exec(pkg.dependencies?.expo ?? "")?.[1];
  const installedVersion =
    typeof installed?.version === "string" &&
    /^\d+\.\d+\.\d+(?:[-+][a-zA-Z0-9.-]{1,80})?$/.test(installed.version)
      ? installed.version
      : null;
  add(
    "expo_dependencies",
    installedVersion?.startsWith(major + ".") ? "present" : "required",
    installedVersion
      ? `Expo ${installedVersion}`
      : "Install the declared workspace dependencies before building",
  );
  const audio = (app.expo.plugins ?? []).find(
    (v) => Array.isArray(v) && v[0] === "expo-audio",
  );
  add(
    "microphone_config",
    typeof audio?.[1]?.microphonePermission === "string" &&
      audio[1].microphonePermission.length > 0
      ? "present"
      : "required",
    "Native microphone permission must be generated into the app",
  );
  add(
    "custom_native_build",
    "native_only",
    "Paste input, terminal and speech require this project's native app; Expo Go is insufficient for acceptance",
  );
  let eas = null;
  try {
    eas = await json(join(config.project, "eas.json"));
  } catch {}
  add(
    "preview_profile",
    eas?.build?.preview?.distribution === "internal" ? "present" : "required",
    "Internal preview profile; signing and account access are not verified",
  );
  const sdk =
    config.sdk ??
    env.ANDROID_HOME ??
    env.ANDROID_SDK_ROOT ??
    (platform === "win32"
      ? join(home, "AppData/Local/Android/Sdk")
      : platform === "darwin"
        ? join(home, "Library/Android/sdk")
        : join(home, "Android/Sdk"));
  const adb = join(
    sdk,
    "platform-tools",
    platform === "win32" ? "adb.exe" : "adb",
  );
  const haveAdb = await toolFile(adb);
  let platformAvailable = false,
    buildToolsAvailable = false,
    inventoryBound = false;
  try {
    const platforms = await entries(join(sdk, "platforms"));
    for (const name of platforms.filter((n) => /^android-\d+$/.test(n))) {
      if (await toolFile(join(sdk, "platforms", name, "android.jar"))) {
        platformAvailable = true;
        break;
      }
    }
    const tools = await entries(join(sdk, "build-tools"));
    for (const name of tools.filter((n) => /^\d+\.\d+\.\d+$/.test(n))) {
      const dir = join(sdk, "build-tools", name),
        win = platform === "win32";
      const files = [
        win ? "aapt2.exe" : "aapt2",
        win ? "apksigner.bat" : "apksigner",
        win ? "d8.bat" : "d8",
      ];
      if (
        (
          await Promise.all(files.map((file) => toolFile(join(dir, file))))
        ).every(Boolean)
      ) {
        buildToolsAvailable = true;
        break;
      }
    }
  } catch (error) {
    inventoryBound = error.message === "SDK inventory exceeds its bound";
  }
  add(
    "android_sdk",
    haveAdb && platformAvailable && buildToolsAvailable && !inventoryBound
      ? "present"
      : "required",
    inventoryBound
      ? "SDK folder exceeds 256 entries; select a bounded installed SDK"
      : "Need nonempty ADB, android.jar, aapt2, apksigner and d8 files; tool runtime and build version compatibility remain unverified",
  );
  add(
    "java_home",
    env.JAVA_HOME &&
      (await toolFile(
        join(env.JAVA_HOME, "bin", platform === "win32" ? "java.exe" : "java"),
      ))
      ? "present"
      : "required",
    "Nonempty java executable required; use the JDK for the generated Android build, version compatibility is not inferred",
  );
  if (config.devices) {
    if (!haveAdb)
      add(
        "android_devices",
        "unavailable",
        "ADB unavailable; install platform-tools and enable USB debugging on Moto G32",
      );
    else {
      try {
        const { stdout } = await invoke(
          adb,
          ["-H", "127.0.0.1", "-P", "5037", "devices", "-l"],
          {
            timeout: 5000,
            maxBuffer: 16384,
            windowsHide: true,
            env: adbEnvironment(env),
          },
        );
        const counts = deviceCounts(stdout);
        add(
          "android_devices",
          counts.ready ? "connected" : "required",
          `${counts.ready} authorized, ${counts.unauthorized} unauthorized, ${counts.offline} offline, ${counts.other} unknown; model/app/OS checks remain`,
        );
      } catch {
        add(
          "android_devices",
          "unavailable",
          "Device inventory failed or timed out; authorize USB debugging and rerun --devices",
        );
      }
    }
  } else
    add(
      "android_devices",
      "not_checked",
      "Run check --devices after connecting and authorizing Moto G32",
    );
  add(
    "ios_local_build",
    platform === "darwin" ? "not_checked" : "unavailable",
    platform === "darwin"
      ? "Verify Xcode, signing and the connected iPhone 15"
      : "Local iOS compilation requires macOS/Xcode; use an authorized EAS build or a Mac",
  );
  add(
    "physical_acceptance",
    "pending",
    "No native app install, UI, microphone, trusted TLS or cloud service acceptance is established by this preflight",
  );
  return {
    checks,
    ready: checks.filter(
      (c) => c.status === "present" || c.status === "connected",
    ).length,
  };
}
export async function main(
  args,
  { write = (v) => process.stdout.write(v + "\n"), ...deps } = {},
) {
  let config;
  try {
    config = parseArgs(args);
  } catch {
    write(
      `status: usage_error\nerror: ${scalar("Invalid arguments; use only the flags below")}\nhelp: ${scalar(HELP)}`,
    );
    return 2;
  }
  if (config.command === "version") {
    write(VERSION);
    return 0;
  }
  if (config.command === "help") {
    write(
      `description: Native phone build and optional local ADB inventory preflight\nhelp: ${scalar(HELP)}`,
    );
    return 0;
  }
  try {
    const result = await inspect(config, deps);
    write(
      `bin: ${scalar(fileURLToPath(new URL("../mobile-check.mjs", import.meta.url)).replace(homedir(), "~"))}\ndescription: Native phone build and optional local ADB inventory preflight\nstatus: inspected\nacceptance: pending\nchecks[${result.checks.length}]{id,status,detail}:\n${result.checks.map((c) => `  ${scalar(c.id)},${scalar(c.status)},${scalar(c.detail)}`).join("\n")}\nhelp: ${scalar("Use docs/ai-ide-plan/52-physical-device-review.md for Moto G32 and iPhone 15 build/install steps. This command performs no installation or acceptance pass.")}`,
    );
    return 0;
  } catch {
    write(
      `status: failed\nerror: ${scalar("Cannot inspect the selected native project; configuration must be bounded valid JSON")}\nhelp: ${scalar(HELP)}`,
    );
    return 1;
  }
}
