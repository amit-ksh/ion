import { readFile, realpath, lstat, readdir } from "node:fs/promises";
import { createHash } from "node:crypto";
import { resolve, relative, isAbsolute, sep, join } from "node:path";
export const version = "0.2.0";
export const identity =
  "ion-kimi-host/0.2.0 kimi/2.2.0 source/472192c34c1efad89a369870eff0da0b708ce9bb protocol/1 compiled/1";
export const sha = (bytes) => createHash("sha256").update(bytes).digest("hex");
export async function inventory(root) {
  const rows = [];
  async function walk(folder) {
    for (const e of await readdir(folder, { withFileTypes: true })) {
      const path = join(folder, e.name);
      if (e.isSymbolicLink())
        throw Error("Compiled host contains an unsupported file link.");
      if (e.isDirectory()) await walk(path);
      else if (e.isFile()) {
        const rel = relative(root, path).split(sep).join("/");
        if (["host.json", "package-manifest.json"].includes(rel)) continue;
        const data = await readFile(path);
        rows.push({ path: rel, bytes: data.length, sha256: sha(data) });
      }
    }
  }
  await walk(root);
  return rows.sort((a, b) => a.path.localeCompare(b.path));
}
export async function verifyPackage(root, { bootstrapOnly = false } = {}) {
  root = await realpath(root);
  const bytes = await readFile(join(root, "package-manifest.json"));
  if (bytes.length > 16 * 1024 * 1024)
    throw Error("Compiled host manifest exceeds its limit.");
  const manifest = JSON.parse(bytes.toString("utf8"));
  if (
    manifest.identity !== identity ||
    manifest.format !== 1 ||
    manifest.target.platform !== process.platform ||
    manifest.target.arch !== process.arch ||
    !Array.isArray(manifest.files) ||
    manifest.files.length > 100000
  )
    throw Error("Compiled host identity or platform mismatch.");

  const seen = new Set();
  const entries = manifest.files.map((file) => {
    if (
      typeof file.path !== "string" ||
      file.path.includes("\\") ||
      !Number.isSafeInteger(file.bytes) ||
      file.bytes < 0 ||
      file.bytes > 512 * 1024 * 1024
    )
      throw Error("Invalid compiled host file identity.");
    const path = resolve(root, file.path),
      rel = relative(root, path);
    if (
      seen.has(file.path) ||
      isAbsolute(rel) ||
      rel === ".." ||
      rel.startsWith(".." + sep) ||
      !/^[a-f0-9]{64}$/.test(file.sha256)
    )
      throw Error("Invalid compiled host file identity.");
    seen.add(file.path);
    return { ...file, absolute: path };
  });
  const critical = new Set([
    "host.mjs",
    "compiled-package.mjs",
    "teams.mjs",
    process.platform === "win32" ? "node.exe" : "node",
  ]);
  if (bootstrapOnly && [...critical].some((path) => !seen.has(path)))
    throw Error("Compiled host bootstrap is incomplete.");
  const selected = bootstrapOnly
    ? entries.filter((file) => critical.has(file.path))
    : entries;
  let next = 0;

  async function worker() {
    while (next < selected.length) {
      const file = selected[next++],
        path = file.absolute;
      if (
        (await lstat(path)).isSymbolicLink() ||
        (await realpath(path)) !== path
      )
        throw Error("Compiled host file link changed.");
      const data = await readFile(path);
      if (data.length !== file.bytes || sha(data) !== file.sha256)
        throw Error("Compiled host file changed: " + file.path);
    }
  }
  await Promise.all(Array.from({ length: 16 }, worker));

  if (bootstrapOnly) return manifest;
  // Added modules can shadow a dependency even when all original hashes match.
  let count = 0;
  async function walk(folder) {
    for (const item of await readdir(folder, { withFileTypes: true })) {
      const path = join(folder, item.name);
      if (item.isSymbolicLink())
        throw Error("Compiled host file link changed.");
      if (item.isDirectory()) await walk(path);
      else if (item.isFile()) {
        const rel = relative(root, path).split(sep).join("/");
        if (["host.json", "package-manifest.json"].includes(rel)) continue;
        count++;
        if (!seen.has(rel))
          throw Error("Unexpected compiled host file: " + rel);
      }
    }
  }
  await walk(root);
  if (count !== manifest.files.length)
    throw Error("Compiled host file inventory changed.");

  return manifest;
}
