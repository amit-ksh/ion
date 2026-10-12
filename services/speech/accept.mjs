#!/usr/bin/env node
// Answer identification before loading the command graph or network dependencies.
const args = process.argv.slice(2);
if (args.length === 1 && ["--version", "-v", "-V"].includes(args[0])) {
  console.log("0.1.0");
} else {
  const controller = new AbortController();
  const cancel = () => controller.abort();
  process.once("SIGINT", cancel);
  process.once("SIGTERM", cancel);
  try {
    const { main } = await import("./acceptance/cli.mjs");
    process.exitCode = await main(args, { signal: controller.signal });
  } catch {
    console.log(
      "status: failed\ncode: setup\nerror: Speech checker could not start.\nhelp: Install dependencies with bun install --cwd services/speech --ignore-scripts and use Node 22 or newer.",
    );
    process.exitCode = 1;
  } finally {
    process.removeListener("SIGINT", cancel);
    process.removeListener("SIGTERM", cancel);
  }
}
