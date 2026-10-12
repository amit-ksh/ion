#!/usr/bin/env node
// Version probes load no project/dependency graph.
const args = process.argv.slice(2);
if (args.length === 1 && ["-v", "-V", "--version"].includes(args[0])) {
  process.stdout.write("ion-mobile-check/1\n");
} else {
  const { main } = await import("./mobile-check/cli.mjs");
  process.exitCode = await main(args);
}
