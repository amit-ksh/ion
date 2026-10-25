// The Linux sandbox worker every desktop install ships beside the app.
// Token Factory sandboxes are x86_64 Linux on every host platform, so each
// packager cross-builds the same static musl binary with cargo-zigbuild
// (zig and cargo-zigbuild on PATH, plus the x86_64-unknown-linux-musl target).
import { $ } from "bun";
import { copyFile, mkdir, readFile } from "node:fs/promises";
import { join, resolve } from "node:path";

export const sandboxWorkerFile = "ion-harness-worker-linux-x86_64";
const root = resolve(import.meta.dir, "..");
const target = "x86_64-unknown-linux-musl";

/** Build the worker with the release profile and copy it into `directory`. */
export async function bundleSandboxWorker(directory: string): Promise<string> {
  const targetDirectory = resolve(
    process.env.CARGO_TARGET_DIR || join(root, "target"),
  );
  await $`cargo zigbuild --locked --release --package waku-core --bin ion-harness-worker --target ${target}`.cwd(
    root,
  );
  const built = join(targetDirectory, target, "release", "ion-harness-worker");
  const header = (await readFile(built)).subarray(0, 5);
  // A static x86_64 ELF: magic, 64-bit class.
  if (!header.equals(Buffer.from([0x7f, 0x45, 0x4c, 0x46, 0x02])))
    throw new Error(`${built} is not a 64-bit Linux executable`);
  await mkdir(directory, { recursive: true });
  const destination = join(directory, sandboxWorkerFile);
  await copyFile(built, destination);
  return destination;
}

if (import.meta.main) {
  const [directory] = process.argv.slice(2);
  if (!directory)
    throw new Error("Usage: bun scripts/sandbox-worker.ts <output-dir>");
  console.log(await bundleSandboxWorker(resolve(directory)));
}
