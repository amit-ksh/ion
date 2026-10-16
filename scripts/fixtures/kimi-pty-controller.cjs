// Native Windows asset/ABI probe only; full Kimi Bash acceptance is separate.
const assert = require("node:assert/strict");
const { join, isAbsolute } = require("node:path");
const { readFileSync } = require("node:fs");
const source = process.env.ION_TEST_KIMI_SOURCE;
const work = process.env.ION_KIMI_PTY_WORK;
assert.equal(process.platform, "win32");
assert(source && work && isAbsolute(source) && isAbsolute(work));
const root = join(source, "packages/agent-core-v2/node_modules/node-pty");
const metadata = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
assert.equal(metadata.version, "1.1.0");
const pty = require(root);
const systemRoot = process.env.SystemRoot;
assert(systemRoot && isAbsolute(systemRoot));
const command = join(systemRoot, "System32/cmd.exe");
// /d disables cmd AutoRun; a private cwd/home and literal echo are the only work.
const child = pty.spawn(
  command,
  ["/d", "/q", "/c", "echo ion-terminal-asset-proof"],
  {
    name: "xterm-color",
    cols: 80,
    rows: 24,
    cwd: work,
    env: {
      SystemRoot: systemRoot,
      WINDIR: systemRoot,
      COMSPEC: command,
      PATH: join(systemRoot, "System32"),
      HOME: work,
      USERPROFILE: work,
      TEMP: work,
      TMP: work,
    },
  },
);
let text = "";
const timer = setTimeout(() => {
  child.kill();
  process.stderr.write("Native PTY probe timed out\n");
  process.exitCode = 1;
}, 10000);
child.onData((data) => {
  text += data;
  if (text.length > 16384) {
    child.kill();
    throw Error("PTY output exceeded its bound");
  }
});
child.onExit((event) => {
  clearTimeout(timer);
  setTimeout(() => {
    assert.equal(event.exitCode, 0);
    assert(text.includes("ion-terminal-asset-proof"));
    process.stdout.write(
      JSON.stringify({
        node: process.version,
        platform: process.platform,
        arch: process.arch,
        pty: metadata.version,
        exitCode: event.exitCode,
        marker: true,
      }) + "\n",
      () => process.exit(0),
    );
  }, 50);
});
