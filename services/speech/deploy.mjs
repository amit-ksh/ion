#!/usr/bin/env node
const args = process.argv.slice(2);
if (args.length === 1 && ["--version", "-v", "-V"].includes(args[0])) {
  console.log("0.1.0");
} else {
  const controller = new AbortController();
  const cancel = () => controller.abort();
  process.once("SIGINT", cancel);
  process.once("SIGTERM", cancel);
  try {
    const { main } = await import("./deployment/cli.mjs");
    process.exitCode = await main(args, { signal: controller.signal });
  } catch {
    console.log(
      "status: failed\nerror: Deployment preparation could not start.\nhelp: Use Node 22 or newer and node services/speech/deploy.mjs --help.",
    );
    process.exitCode = 1;
  } finally {
    process.removeListener("SIGINT", cancel);
    process.removeListener("SIGTERM", cancel);
  }
}
