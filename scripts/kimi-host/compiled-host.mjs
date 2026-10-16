#!/usr/bin/env node
// Relocatable, compiled Kimi engine. Owner login configuration stays separate.
import { readFileSync, existsSync, writeFileSync } from "node:fs";
import { join, dirname, isAbsolute, resolve } from "node:path";
import { pathToFileURL, fileURLToPath } from "node:url";
import { spawn, execFileSync } from "node:child_process";
import { version, identity, verifyPackage } from "./compiled-package.mjs";
const args = process.argv.slice(2),
  root = dirname(fileURLToPath(import.meta.url));
const help =
  "Ion Kimi compiled host. Commands: acp; acp --login [--region global|mainland-cn]; login [--region global|mainland-cn]; configure --native-command <executable> [--native-entry <CLI entry>] --output <new JSON file>; --verify; --ion-host-version; --version. Set ION_KIMI_HOST_CONFIG to the configured JSON file and ION_KIMI_SOURCE_HOST to this package's launcher before starting Ion.";
if (args.length === 1 && ["--version", "-v", "-V"].includes(args[0])) {
  process.stdout.write(version + "\n");
  process.exit(0);
}
if (args.length === 0 || (args.length === 1 && args[0] === "--help")) {
  process.stdout.write("description: " + help + "\n");
  process.exit(0);
}
const acp = args[0] === "acp" || args[0] === "login";
function configuration(path) {
  const data = JSON.parse(readFileSync(path, "utf8"));
  if (
    data.format !== 1 ||
    typeof data.nativeCommand !== "string" ||
    !isAbsolute(data.nativeCommand) ||
    !existsSync(data.nativeCommand) ||
    !Array.isArray(data.nativeArgs) ||
    data.nativeArgs.some((v) => typeof v !== "string")
  )
    throw Error(
      "Configure the selected native Kimi login executable using the host configure command.",
    );
  return data;
}
try {
  await verifyPackage(root, { bootstrapOnly: args[0] !== "--verify" });
  if (args.length === 1 && args[0] === "--verify") {
    process.stdout.write(
      "status: verified\nidentity: " + JSON.stringify(identity) + "\n",
    );
    process.exit(0);
  }
  if (args[0] === "configure") {
    const values = {};
    for (let i = 1; i < args.length; i += 2) {
      if (
        !["--native-command", "--native-entry", "--output"].includes(args[i]) ||
        values[args[i]] ||
        !args[i + 1] ||
        args[i + 1].startsWith("--")
      )
        throw Error("Unknown, repeated or missing configure argument. " + help);
      values[args[i]] = resolve(args[i + 1]);
    }
    if (!values["--native-command"] || !values["--output"])
      throw Error("Configure requires --native-command and --output. " + help);
    const data = {
      format: 1,
      nativeCommand: values["--native-command"],
      nativeArgs: values["--native-entry"] ? [values["--native-entry"]] : [],
    };
    const actual = execFileSync(
      data.nativeCommand,
      [...data.nativeArgs, "--version"],
      { encoding: "utf8", timeout: 15000, maxBuffer: 65536, windowsHide: true },
    );
    if (!/(?:^|\s)2\.2\.0(?:$|\s)/.test(actual.trim()))
      throw Error("Native login delegation requires Kimi 2.2.0.");
    const text = JSON.stringify(data, null, 2) + "\n",
      path = values["--output"];
    if (existsSync(path) && readFileSync(path, "utf8") !== text)
      throw Error(
        "Configuration differs; use a new output path to preserve the existing file.",
      );
    writeFileSync(path, text);
    process.stdout.write(
      "status: configured\npath: " + JSON.stringify(path) + "\n",
    );
    process.exit(0);
  }
  const configPath =
    process.env.ION_KIMI_HOST_CONFIG || join(root, "host.json");
  if (!isAbsolute(configPath))
    throw Error("Use an absolute ION_KIMI_HOST_CONFIG path.");
  const config = configuration(configPath);
  if (args.length === 1 && args[0] === "--ion-host-version") {
    process.stdout.write(identity + "\n");
    process.exit(0);
  }
  const login =
    args[0] === "login" || (args[0] === "acp" && args[1] === "--login");
  if (login) {
    const rest = args.slice(args[0] === "login" ? 1 : 2);
    if (!(
      rest.length === 0 ||
      (rest.length === 2 &&
        rest[0] === "--region" &&
        ["global", "mainland-cn"].includes(rest[1]))
    ))
      throw Error("Unsupported login arguments. " + help);
    const child = spawn(
      config.nativeCommand,
      [...config.nativeArgs, "acp", "--login", ...rest],
      { env: process.env, stdio: "inherit", windowsHide: true },
    );
    child.on("error", () => {
      process.stderr.write("Ion Kimi host: native login could not start.\n");
      process.exitCode = 1;
    });
    child.on("exit", (code) => {
      process.exitCode = code ?? 1;
    });
  } else {
    if (args.length !== 1 || args[0] !== "acp")
      throw Error("Unsupported host arguments. " + help);
    const [major, minor] = process.versions.node.split(".").map(Number);
    if (major !== 24 || minor < 15)
      throw Error("Use the bundled Node 24 runtime.");
    // Full runtime verification precedes engine loading, auth or any tool work.
    // The short version/login probes verify only the executable bootstrap.
    await verifyPackage(root);
    const endpoint = process.env.ION_KIMI_TEAM_ENDPOINT,
      token = process.env.ION_KIMI_TEAM_TOKEN;
    delete process.env.ION_KIMI_TEAM_ENDPOINT;
    delete process.env.ION_KIMI_TEAM_TOKEN;
    if (!endpoint || !token)
      throw Error("Start this host through an Ion Kimi task.");
    // Stdout remains ACP even if a dependency logs during initial module loading.
    for (const method of ["log", "info", "warn", "debug"])
      console[method] = (...values) =>
        process.stderr.write(values.map(String).join(" ") + "\n");
    const engine = join(root, "engine/node_modules/@moonshot-ai");
    const { installNativeTeams, connection } = await import("./teams.mjs");
    await installNativeTeams(
      connection(endpoint, token),
      (rel) =>
        import(
          pathToFileURL(join(engine, "agent-core-v2/src", rel + ".js")).href
        ),
    );
    const { configureTextBuildWorkerRuntime } = await import(
      pathToFileURL(join(engine, "minidb/src/worker-runtime.js")).href
    );
    configureTextBuildWorkerRuntime(
      join(engine, "minidb/src/worker/text-build-worker.js"),
    );
    const { runAcpServer } = await import(
      pathToFileURL(join(engine, "acp-server/src/start.js")).href
    );
    await runAcpServer({
      agentInfo: { name: "Ion Kimi compiled host", version },
      terminalAuthLegacyCommand: join(
        root,
        process.platform === "win32" ? "ion-kimi-host.cmd" : "ion-kimi-host",
      ),
      ...(process.env.KIMI_CODE_HOME
        ? { terminalAuthEnv: { KIMI_CODE_HOME: process.env.KIMI_CODE_HOME } }
        : {}),
    });
    process.exit(0);
  }
} catch (error) {
  const text =
    "Ion Kimi host: " +
    (error instanceof Error ? error.message : "startup failed");
  if (acp) process.stderr.write(text + "\n");
  else
    process.stdout.write(
      "error: " +
        JSON.stringify(text) +
        "\nhelp: " +
        JSON.stringify(help) +
        "\n",
    );
  process.exitCode = acp ? 1 : 2;
}
