#!/usr/bin/env node
import { mkdir, writeFile, copyFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { join, resolve } from "node:path";
import { execFileSync } from "node:child_process";
import { compileRuntime } from "./compile-runtime.mjs";
import { version, identity, inventory } from "./compiled-package.mjs";
const args = process.argv.slice(2);
const help =
  "node build-compiled-host.mjs --source <pinned source checkout with frozen dependencies> --node <Node 24.19.0 executable> --output <new package directory>. This builds a platform-specific host package; use its configure command to select native Kimi login. Example: --source D:/kimi-source --node C:/node/node.exe --output D:/ion-kimi-package";
if (args.length === 1 && ["--version", "-v", "-V"].includes(args[0])) {
  console.log(version);
  process.exit(0);
}
if (args.length === 0 || (args.length === 1 && args[0] === "--help")) {
  console.log(
    "description: Compile and package the pinned native Kimi host.\nhelp: " +
      JSON.stringify(help),
  );
  process.exit(0);
}
try {
  const values = {};
  for (let i = 0; i < args.length; i += 2) {
    if (
      !["--source", "--node", "--output"].includes(args[i]) ||
      values[args[i]] ||
      !args[i + 1] ||
      args[i + 1].startsWith("--")
    )
      throw Error("Unknown, repeated or missing argument. " + help);
    values[args[i]] = resolve(args[i + 1]);
  }
  for (const key of ["--source", "--node", "--output"])
    if (!values[key]) throw Error("Missing " + key + ". " + help);
  const source = values["--source"],
    node = values["--node"],
    output = values["--output"];
  if (existsSync(output))
    throw Error(
      "Output already exists; choose a new directory to preserve its files.",
    );
  const target = JSON.parse(
    execFileSync(
      node,
      [
        "-p",
        "JSON.stringify({version:process.version,platform:process.platform,arch:process.arch})",
      ],
      { encoding: "utf8", timeout: 15000, maxBuffer: 65536, windowsHide: true },
    ),
  );
  if (
    target.version !== "v24.19.0" ||
    target.platform !== process.platform ||
    target.arch !== process.arch
  )
    throw Error(
      "Build with a matching Node 24.19.0 runtime for this host platform.",
    );
  await mkdir(output, { recursive: true });
  console.error(
    "Compiling pinned Kimi engine and copying frozen runtime dependencies...",
  );
  const runtime = await compileRuntime(source, join(output, "engine"));
  for (const [from, to] of [
    ["compiled-host.mjs", "host.mjs"],
    ["compiled-package.mjs", "compiled-package.mjs"],
    ["teams.mjs", "teams.mjs"],
  ])
    await copyFile(new URL(from, import.meta.url), join(output, to));
  await copyFile(
    node,
    join(output, process.platform === "win32" ? "node.exe" : "node"),
  );
  await copyFile(
    new URL("./licenses/node-24.19.0.txt", import.meta.url),
    join(output, "NODE-LICENSE.txt"),
  );
  const launcher =
    process.platform === "win32" ? "ion-kimi-host.cmd" : "ion-kimi-host";
  const text =
    process.platform === "win32"
      ? '@echo off\r\nsetlocal DisableDelayedExpansion\r\n"%~dp0node.exe" "%~dp0host.mjs" %*\r\n'
      : '#!/bin/sh\nion_host_root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)\nexec "$ion_host_root/node" "$ion_host_root/host.mjs" "$@"\n';
  await writeFile(join(output, launcher), text, { mode: 0o755 });
  await writeFile(
    join(output, "host.json"),
    JSON.stringify(
      { format: 1, nativeCommand: null, nativeArgs: [] },
      null,
      2,
    ) + "\n",
  );
  await writeFile(
    join(output, "runtime-inventory.json"),
    JSON.stringify(runtime, null, 2) + "\n",
  );
  const files = await inventory(output);
  await writeFile(
    join(output, "package-manifest.json"),
    JSON.stringify({ format: 1, identity, target, runtime, files }, null, 2) +
      "\n",
  );
  console.log(
    "status: built\npackage: " +
      JSON.stringify(output) +
      "\nfiles: " +
      files.length +
      "\nbytes: " +
      files.reduce((n, f) => n + f.bytes, 0) +
      "\nhelp: Run the packaged host configure command to select native Kimi login, then select its launcher through ION_KIMI_SOURCE_HOST.",
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
