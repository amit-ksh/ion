#!/usr/bin/env node
// Separate source host: select its generated launcher explicitly in Ion.
import { readFileSync, existsSync } from "node:fs";
import { join, isAbsolute } from "node:path";
import { pathToFileURL } from "node:url";
import { spawn } from "node:child_process";
import { identity, version, verifySource } from "./identity.mjs";
const args = process.argv.slice(2);
if (args.length === 1 && ["--version", "-v", "-V"].includes(args[0])) {
  process.stdout.write(version + "\n");
  process.exit(0);
}
const help =
  "Ion Kimi source host. Commands: acp; acp --login [--region global|mainland-cn]; login [--region global|mainland-cn]; --version; --ion-host-version. Set ION_KIMI_SOURCE_HOST to this generated launcher before starting Ion; keep the native Kimi binary selection.";
if (args.length === 0 || (args.length === 1 && args[0] === "--help")) {
  process.stdout.write("description: " + help + "\n");
  process.exit(0);
}
try {
  const config = JSON.parse(
    readFileSync(new URL("./host.json", import.meta.url), "utf8"),
  );
  if (
    config.identity !== identity ||
    !isAbsolute(config.nativeCommand) ||
    !existsSync(config.nativeCommand) ||
    !Array.isArray(config.nativeArgs) ||
    config.nativeArgs.some((a) => typeof a !== "string") ||
    !isAbsolute(config.launcher)
  )
    throw Error("Invalid host configuration; prepare this source host again.");
  verifySource(config.source);
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
    // Forward to the explicitly configured native CLI. No substitute auth flow,
    // token store, model, API route or synthetic login is installed here.
    const child = spawn(
      config.nativeCommand,
      [...config.nativeArgs, "acp", "--login", ...rest],
      { stdio: "inherit", env: process.env, windowsHide: true },
    );
    child.on("error", () => {
      process.stderr.write(
        "Ion Kimi host: native login could not start. Recheck the configured native executable.\n",
      );
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
      throw Error(
        "Ion Kimi host requires Node 24.15 or later in the Node 24 release line.",
      );
    const endpoint = process.env.ION_KIMI_TEAM_ENDPOINT;
    const token = process.env.ION_KIMI_TEAM_TOKEN;
    // Do not expose callback credentials to native shell tools or child agents.
    delete process.env.ION_KIMI_TEAM_ENDPOINT;
    delete process.env.ION_KIMI_TEAM_TOKEN;
    if (!endpoint || !token)
      throw Error(
        "Start this host through an Ion Kimi task to enable its Teams connection.",
      );
    const { register } = await import(
      pathToFileURL(
        join(config.source, "node_modules/tsx/dist/esm/api/index.mjs"),
      ).href
    );
    register({ tsconfig: join(config.source, "tsconfig.json") });
    await import(
      pathToFileURL(join(config.source, "build/register-raw-text-loader.mjs"))
        .href
    );
    const { installTeams, connection } = await import("./teams.mjs");
    await installTeams(config.source, connection(endpoint, token));
    const { runAcpServer } = await import(
      pathToFileURL(join(config.source, "packages/acp-server/src/start.ts"))
        .href
    );
    await runAcpServer({
      agentInfo: { name: "Ion Kimi source host", version },
      terminalAuthLegacyCommand: config.launcher,
      ...(process.env.KIMI_CODE_HOME
        ? { terminalAuthEnv: { KIMI_CODE_HOME: process.env.KIMI_CODE_HOME } }
        : {}),
    });
    // Match the released CLI after its awaited core/storage shutdown. Native
    // terminal workers may retain Node handles after the ACP connection closes.
    process.exit(0);
  }
} catch (error) {
  // ACP stdout stays exclusively JSON-RPC, including failures before startup.
  process.stderr.write(
    "Ion Kimi host: " +
      (error instanceof Error ? error.message : "startup failed") +
      "\n",
  );
  process.exitCode = 1;
}
