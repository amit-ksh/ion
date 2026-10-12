import { mkdir, writeFile, readdir, lstat } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";
import { fail, canceled, stableFile, json, fields } from "./files.mjs";
export const BUNDLE_FILES = [
  "services/speech/deploy.mjs",
  "services/speech/device-review.mjs",
  "services/speech/deployment/cli.mjs",
  "services/speech/deployment/files.mjs",
  "services/speech/deployment/inspect.mjs",
  "services/speech/deployment/package.mjs",
  "services/speech/deployment/review.mjs",
  "services/speech/deployment/review-cli.mjs",
  "services/speech/deployment/deployment.test.mjs",
  "services/speech/acceptance/acceptance.test.mjs",
  "services/speech/ion-speech-launch.sh",
  "services/speech/ion-speech.service",
  "services/speech/Caddyfile",
  "services/speech/speech.env.example",
  "services/speech/README.md",
  "services/speech/package.json",
  "services/speech/bun.lock",
  "services/speech/accept.mjs",
  "services/speech/acceptance/cli.mjs",
  "services/speech/acceptance/config.mjs",
  "services/speech/acceptance/checks.mjs",
  "services/speech/acceptance/transport.mjs",
  "services/speech/acceptance/wav.mjs",
  "docs/ai-ide-plan/41-speech-service-acceptance.md",
  "docs/ai-ide-plan/50-speech-deployment-device-review.md",
].sort();
const ROOT = fileURLToPath(new URL("../../../", import.meta.url));
const ASSET_LIMIT = 256 * 1024;
function manifestText(files) {
  return (
    JSON.stringify(
      { version: 1, scope: "speech_deployment_preparation", files },
      null,
      2,
    ) + "\n"
  );
}
function sumsText(files) {
  return files.map((file) => `${file.sha256}  ${file.path}`).join("\n") + "\n";
}
export async function bundle(output, { signal, source = ROOT } = {}) {
  // Capture the complete allowlist before creating output. No model, credential,
  // environment, private review or third-party dependency enters the package.
  const assets = [];
  let total = 0;
  for (const path of BUNDLE_FILES) {
    const file = await stableFile(
      join(source, ...path.split("/")),
      ASSET_LIMIT,
      { signal, collect: true },
    );
    total += file.size;
    if (total > 2 * 1024 * 1024)
      throw fail(
        "package",
        "Deployment source assets exceed their package limit.",
      );
    assets.push({ path, ...file });
  }
  canceled(signal);
  try {
    await mkdir(output, { mode: 0o700 });
  } catch {
    throw fail(
      "output",
      "Choose a new package directory in an existing writable parent; existing directories are never replaced.",
    );
  }
  try {
    for (const asset of assets) {
      canceled(signal);
      const destination = join(output, ...asset.path.split("/"));
      await mkdir(dirname(destination), { recursive: true, mode: 0o700 });
      await writeFile(destination, asset.bytes, {
        flag: "wx",
        mode: asset.path.endsWith(".sh") ? 0o700 : 0o600,
      });
    }
    const files = assets.map(({ path, size, sha256 }) => ({
      path,
      size,
      sha256,
    }));
    const manifest = manifestText(files);
    await writeFile(join(output, "manifest.json"), manifest, {
      flag: "wx",
      mode: 0o600,
    });
    await writeFile(join(output, "SHA256SUMS"), sumsText(files), {
      flag: "wx",
      mode: 0o600,
    });
    canceled(signal);
    return {
      files: files.length,
      bytes: total,
      manifest_sha256: createHash("sha256").update(manifest).digest("hex"),
      deployed: false,
    };
  } catch (error) {
    if (error?.code === "canceled") throw error;
    throw fail(
      "output",
      "Package creation was interrupted; remove only the newly selected output directory after reviewing it, then retry with a new directory.",
    );
  }
}
async function entries(directory, prefix = "") {
  const result = [];
  for (const item of await readdir(directory, { withFileTypes: true })) {
    const path = prefix + item.name;
    if (item.isSymbolicLink() || (!item.isFile() && !item.isDirectory()))
      throw fail(
        "package",
        "The package contains a symlink or nonregular entry.",
      );
    if (item.isDirectory()) {
      if (!BUNDLE_FILES.some((file) => file.startsWith(path + "/")))
        throw fail("package", "The package contains an unexpected directory.");
      result.push(...(await entries(join(directory, item.name), path + "/")));
    } else result.push(path);
    if (result.length > BUNDLE_FILES.length + 2)
      throw fail("package", "The package contains unexpected files.");
  }
  return result.sort();
}
export async function verify(directory, { signal } = {}) {
  canceled(signal);
  const root = await lstat(directory);
  if (!root.isDirectory() || root.isSymbolicLink())
    throw fail(
      "package",
      "Select a regular package directory without a symlink.",
    );
  const found = await entries(directory);
  if (
    found.join("|") !==
    [...BUNDLE_FILES, "manifest.json", "SHA256SUMS"].sort().join("|")
  )
    throw fail(
      "package",
      "The package must contain exactly the documented source allowlist and its two manifests.",
    );
  const manifest = await stableFile(
    join(directory, "manifest.json"),
    64 * 1024,
    { signal, collect: true },
  );
  const value = json(manifest.bytes, "Package manifest");
  if (
    !fields(value, ["version", "scope", "files"]) ||
    value.version !== 1 ||
    value.scope !== "speech_deployment_preparation" ||
    !Array.isArray(value.files) ||
    value.files.length !== BUNDLE_FILES.length
  )
    throw fail(
      "package",
      "Use the unchanged version-1 speech preparation manifest.",
    );
  let total = 0;
  for (let i = 0; i < BUNDLE_FILES.length; i++) {
    const expected = value.files[i];
    if (
      !fields(expected, ["path", "size", "sha256"]) ||
      expected.path !== BUNDLE_FILES[i] ||
      !Number.isSafeInteger(expected.size) ||
      expected.size < 1 ||
      expected.size > ASSET_LIMIT ||
      !/^[a-f0-9]{64}$/.test(expected.sha256)
    )
      throw fail(
        "package",
        "The manifest contains invalid, unbounded or unexpected asset metadata.",
      );
    const actual = await stableFile(
      join(directory, ...expected.path.split("/")),
      ASSET_LIMIT,
      { signal },
    );
    if (actual.size !== expected.size || actual.sha256 !== expected.sha256)
      throw fail(
        "package",
        "A deployment package asset does not match its manifest; transfer the reviewed package again.",
      );
    total += actual.size;
    if (total > 2 * 1024 * 1024)
      throw fail("package", "The deployment package exceeds its byte limit.");
  }
  const sums = await stableFile(join(directory, "SHA256SUMS"), 64 * 1024, {
    signal,
    collect: true,
  });
  if (!sums.bytes.equals(Buffer.from(sumsText(value.files))))
    throw fail(
      "package",
      "SHA256SUMS does not match the strict deployment manifest.",
    );
  canceled(signal);
  return {
    files: value.files.length,
    bytes: total,
    manifest_sha256: manifest.sha256,
    deployed: false,
  };
}
