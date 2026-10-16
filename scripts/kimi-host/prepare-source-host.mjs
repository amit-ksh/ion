#!/usr/bin/env node
// Prepare an explicit local source-host launcher. This is not a binary bundle.
import {
  mkdirSync,
  writeFileSync,
  copyFileSync,
  existsSync,
  readFileSync,
} from "node:fs";
import { resolve, join, extname } from "node:path";
import { spawnSync } from "node:child_process";
import { identity, version, verifySource } from "./identity.mjs";
const args = process.argv.slice(2);
const help =
  "node prepare-source-host.mjs --source <pinned checkout with frozen dependencies> --node <Node 24 executable> --native-command <Kimi 2.2.0 executable> [--native-entry <native CLI JS entry>] --output <new directory>. Examples: --source D:/kimi-source --node C:/node/node.exe --native-command C:/kimi/kimi.exe --output D:/ion-kimi-host; --source /opt/kimi-source --node /usr/bin/node --native-command /usr/bin/node --native-entry /opt/kimi/cli.js --output /opt/ion-kimi-host";
if (args.length === 1 && ["--version", "-v", "-V"].includes(args[0])) {
  console.log(version);
  process.exit(0);
}
if (args.length === 0 || (args.length === 1 && args[0] === "--help")) {
  console.log(
    "description: Prepare a separately selected local Ion Kimi source host.\nhelp: " +
      JSON.stringify(help),
  );
  process.exit(0);
}
try {
  const allowed = [
    "--source",
    "--node",
    "--native-command",
    "--native-entry",
    "--output",
  ];
  const values = {};
  for (let i = 0; i < args.length; i += 2) {
    const key = args[i];
    if (
      !allowed.includes(key) ||
      values[key] ||
      !args[i + 1] ||
      args[i + 1].startsWith("--")
    )
      throw Error(
        "Unknown, repeated or missing argument: " + key + ". " + help,
      );
    values[key] = resolve(args[i + 1]);
  }
  for (const key of allowed.filter((k) => k !== "--native-entry"))
    if (!values[key]) throw Error("Missing " + key + ". " + help);
  const source = values["--source"],
    node = values["--node"],
    nativeCommand = values["--native-command"],
    output = values["--output"];
  if (
    process.platform === "win32" &&
    [node, nativeCommand].some((p) => extname(p).toLowerCase() !== ".exe")
  )
    throw Error(
      "Use actual Windows executables. For a JS CLI, use --native-command node.exe --native-entry <CLI entry>.",
    );
  verifySource(source);
  const nativeArgs = values["--native-entry"] ? [values["--native-entry"]] : [];
  for (const p of [node, nativeCommand, ...nativeArgs])
    if (!existsSync(p))
      throw Error("Configured executable or entry does not exist.");
  const probe = (command, args) => {
    const r = spawnSync(command, args, {
      encoding: "utf8",
      timeout: 15000,
      maxBuffer: 65536,
      windowsHide: true,
    });
    if (r.error || r.status !== 0)
      throw Error("Configured runtime version probe failed.");
    return r.stdout.trim();
  };
  if (!/^v24\.(?:1[5-9]|[2-9]\d)\.\d+$/.test(probe(node, ["--version"])))
    throw Error("Use Node 24.15 or later in the Node 24 release line.");
  if (
    !/(?:^|\s)2\.2\.0(?:$|\s)/.test(
      probe(nativeCommand, [...nativeArgs, "--version"]),
    )
  )
    throw Error("Native login delegation requires Kimi 2.2.0.");
  const launcher = join(
    output,
    process.platform === "win32" ? "ion-kimi-host.cmd" : "ion-kimi-host",
  );
  const launcherText =
    process.platform === "win32"
      ? `@echo off\r\nsetlocal DisableDelayedExpansion\r\n"${node.replaceAll("%", "%%")}" "${join(output, "host.mjs").replaceAll("%", "%%")}" %*\r\n`
      : `#!/bin/sh\nexec '${node.replaceAll("'", "'\\''")}' '${join(output, "host.mjs").replaceAll("'", "'\\''")}' "$@"\n`;
  const manifest =
    JSON.stringify(
      { identity, source, nativeCommand, nativeArgs, launcher },
      null,
      2,
    ) + "\n";
  if (existsSync(output)) {
    const same =
      ["host.mjs", "identity.mjs", "teams.mjs"].every(
        (file) =>
          existsSync(join(output, file)) &&
          readFileSync(join(output, file)).equals(
            readFileSync(new URL(file, import.meta.url)),
          ),
      ) &&
      existsSync(join(output, "host.json")) &&
      readFileSync(join(output, "host.json"), "utf8") === manifest &&
      existsSync(launcher) &&
      readFileSync(launcher, "utf8") === launcherText;
    if (!same)
      throw Error(
        "Output differs from the requested host. Choose a new directory to preserve its files.",
      );
    console.log(
      "status: already prepared\nlauncher: " + JSON.stringify(launcher),
    );
    process.exit(0);
  }
  mkdirSync(output, { recursive: true });
  for (const file of ["host.mjs", "identity.mjs", "teams.mjs"])
    copyFileSync(new URL(file, import.meta.url), join(output, file));
  writeFileSync(
    join(output, "host.json"),
    JSON.stringify(
      { identity, source, nativeCommand, nativeArgs, launcher },
      null,
      2,
    ) + "\n",
  );
  writeFileSync(launcher, launcherText, { mode: 0o755 });
  console.log(
    "status: prepared\nlauncher: " +
      JSON.stringify(launcher) +
      "\nhelp: Set ION_KIMI_SOURCE_HOST to this launcher before starting Ion. Keep the native Kimi binary selection, pinned source checkout and frozen dependencies available.",
  );
} catch (error) {
  console.log(
    "error: " +
      JSON.stringify(error.message) +
      "\nhelp: " +
      JSON.stringify(help),
  );
  process.exitCode = 2;
}
