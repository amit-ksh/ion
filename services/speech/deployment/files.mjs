import { createHash } from "node:crypto";
import { open, lstat } from "node:fs/promises";
import { isAbsolute } from "node:path";

export class DeployError extends Error {
  constructor(code, message, exitCode = 1) {
    super(message);
    this.code = code;
    this.exitCode = exitCode;
  }
}
export function fail(code, message, exitCode = 1) {
  return new DeployError(code, message, exitCode);
}
export function canceled(signal) {
  if (signal?.aborted)
    throw fail("canceled", "Preparation canceled; no service was started.");
}
export function absolute(value, label) {
  if (
    typeof value !== "string" ||
    !isAbsolute(value) ||
    value.length > 4096 ||
    /[\x00-\x1f]/.test(value)
  )
    throw fail(
      "usage",
      `${label} requires an absolute local path without control characters.`,
      2,
    );
  return value;
}
function identity(a, b) {
  // Windows path lstat reports dev=0 while handle fstat returns a volume ID.
  // Keep its file ID check; Unix also requires the device ID to agree.
  return (
    a.ino === b.ino &&
    (a.dev === b.dev ||
      (process.platform === "win32" && (a.dev === 0 || b.dev === 0)))
  );
}
export async function stableFile(
  path,
  limit,
  { signal, collect = false } = {},
) {
  canceled(signal);
  let file;
  try {
    const link = await lstat(path);
    if (
      !link.isFile() ||
      link.isSymbolicLink() ||
      link.size < 1 ||
      link.size > limit
    )
      throw fail(
        "file",
        "Select a stable nonempty regular file within the documented size limit, without a symlink.",
      );
    file = await open(path, "r");
    const before = await file.stat();
    if (
      !identity(before, link) ||
      before.size !== link.size ||
      before.mtimeMs !== link.mtimeMs
    )
      throw fail(
        "file",
        "A selected file changed during preparation; retry with a stable artifact.",
      );
    const hash = createHash("sha256"),
      chunks = [],
      chunk = Buffer.alloc(Math.min(1024 * 1024, before.size));
    let length = 0;
    for (;;) {
      canceled(signal);
      const { bytesRead } = await file.read(
        chunk,
        0,
        Math.min(chunk.length, limit - length + 1),
        length,
      );
      if (!bytesRead) break;
      length += bytesRead;
      if (length > before.size || length > limit)
        throw fail(
          "file",
          "A selected file grew during preparation; use a stable artifact.",
        );
      hash.update(chunk.subarray(0, bytesRead));
      if (collect) chunks.push(Buffer.from(chunk.subarray(0, bytesRead)));
    }
    const after = await file.stat();
    const current = await lstat(path);
    if (
      length !== before.size ||
      after.size !== before.size ||
      after.mtimeMs !== before.mtimeMs ||
      after.ctimeMs !== before.ctimeMs ||
      !identity(current, before) ||
      current.size !== before.size ||
      current.mtimeMs !== before.mtimeMs
    )
      throw fail(
        "file",
        "A selected file changed during preparation; use a stable artifact.",
      );
    canceled(signal);
    return {
      size: length,
      sha256: hash.digest("hex"),
      ...(collect ? { bytes: Buffer.concat(chunks, length) } : {}),
    };
  } catch (error) {
    if (error instanceof DeployError) throw error;
    throw fail(
      "file",
      "Cannot read a selected file; check its private local path and access.",
    );
  } finally {
    await file?.close();
  }
}
export function json(bytes, label) {
  try {
    return JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes));
  } catch {
    throw fail("data", `${label} must be bounded valid UTF8 JSON.`);
  }
}
export function record(value) {
  return value && !Array.isArray(value) && typeof value === "object";
}
export function fields(value, names) {
  return (
    record(value) &&
    Object.keys(value).sort().join("|") === [...names].sort().join("|")
  );
}
