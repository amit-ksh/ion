import { createHash } from "node:crypto";
import { existsSync, lstatSync, readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";

// Windows can report access-time/attribute changes as fs.watch "change" events.
// Restart only for changed bytes or directory entries, including deletion.
export function contentChangeFilter(
  base: string,
): (filename: string | Buffer | null) => boolean {
  const fingerprints = new Map<string, string>();
  const root = resolve(base);
  function scan(path: string, target: Map<string, string>): void {
    try {
      if (!existsSync(path)) return;
      const stat = lstatSync(path);
      if (stat.isSymbolicLink()) return;
      if (stat.isDirectory()) {
        for (const name of readdirSync(path)) scan(join(path, name), target);
      } else if (stat.isFile()) {
        target.set(
          path,
          createHash("sha256").update(readFileSync(path)).digest("hex"),
        );
      }
    } catch (error) {
      // Atomic editor saves can remove a path between the stat and read.
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
    }
  }
  scan(root, fingerprints);
  return (filename) => {
    const path = filename === null ? root : resolve(root, filename.toString());
    const current = new Map<string, string>();
    scan(path, current);
    let changed = false;
    for (const [key, value] of fingerprints) {
      if (
        key !== path &&
        !key.startsWith(path + "/") &&
        !key.startsWith(path + "\\")
      )
        continue;
      if (current.get(key) !== value) changed = true;
      if (!current.has(key)) fingerprints.delete(key);
    }
    for (const [key, value] of current) {
      if (fingerprints.get(key) !== value) changed = true;
      fingerprints.set(key, value);
    }
    return changed;
  };
}
