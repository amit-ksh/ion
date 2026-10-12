import { execFile } from "node:child_process";
import { basename } from "node:path";
import { promisify } from "node:util";
import { fail, canceled, stableFile, json, record } from "./files.mjs";
const run = promisify(execFile);
export const MODEL_REPO = "nvidia/nemotron-3.5-asr-streaming-0.6b";
export function indexedArtifact(index) {
  if (
    !record(index) ||
    index.schema_version !== 1 ||
    !Array.isArray(index.models) ||
    index.models.length > 128
  )
    throw fail("index", "Use the verified installed schema-1 model index.");
  const models = index.models.filter(
    (model) => record(model) && model.repo === MODEL_REPO,
  );
  if (models.length !== 1)
    throw fail(
      "index",
      "The installed index must identify exactly one Nemotron 3.5 repository.",
    );
  const model = models[0];
  const artifacts = Array.isArray(model.artifacts)
    ? model.artifacts.filter(
        (file) => record(file) && file.role === "asr" && file.type === "file",
      )
    : [];
  if (!/^[a-f0-9]{40}$/.test(model.revision) || artifacts.length !== 1)
    throw fail(
      "index",
      "The selected repository needs a pinned revision and one ASR file artifact.",
    );
  const artifact = artifacts[0];
  if (
    !/^[a-zA-Z0-9._-]+\.gguf$/.test(artifact.filename) ||
    !/^[a-f0-9]{64}$/.test(artifact.sha256) ||
    !Number.isSafeInteger(artifact.size) ||
    artifact.size < 4 ||
    artifact.size > 8 * 1024 ** 3
  )
    throw fail(
      "index",
      "The ASR artifact needs a safe GGUF filename, bounded size and pinned SHA-256.",
    );
  return {
    revision: model.revision,
    filename: artifact.filename,
    size: artifact.size,
    sha256: artifact.sha256,
  };
}
function nativeEnvironment(env) {
  const names = [
    "PATH",
    "SystemRoot",
    "WINDIR",
    "TEMP",
    "TMP",
    "HOME",
    "USERPROFILE",
    "LOCALAPPDATA",
    "APPDATA",
    "LD_LIBRARY_PATH",
  ];
  return Object.fromEntries(
    names
      .filter((name) => typeof env[name] === "string")
      .map((name) => [name, env[name]]),
  );
}
export async function nativeJson(binary, args, signal, env = process.env) {
  canceled(signal);
  try {
    const { stdout } = await run(binary, ["--quiet", "--json", ...args], {
      timeout: 15_000,
      maxBuffer: 1024 * 1024,
      windowsHide: true,
      signal,
      env: nativeEnvironment(env),
    });
    canceled(signal);
    return json(Buffer.from(stdout), "Native diagnostic");
  } catch (error) {
    canceled(signal);
    if (error?.code === "data") throw error;
    throw fail(
      "native",
      "The selected binary did not finish its bounded diagnostic successfully; check the verified install privately.",
    );
  }
}
export async function inspect(config, { signal, native = nativeJson } = {}) {
  const index = await stableFile(config.index, 1024 * 1024, {
    signal,
    collect: true,
  });
  const expected = indexedArtifact(json(index.bytes, "Model index"));
  if (basename(config.model) !== expected.filename)
    throw fail(
      "model",
      "Select the exact ASR filename from the installed verified index.",
    );
  const model = await stableFile(config.model, expected.size, { signal });
  if (model.size !== expected.size || model.sha256 !== expected.sha256)
    throw fail(
      "model",
      "Local model bytes do not match the installed index; do not serve this artifact.",
    );
  const binary = await stableFile(config.binary, 512 * 1024 * 1024, { signal });
  const doctor = await native(config.binary, ["doctor"], signal);
  if (
    !record(doctor) ||
    !record(doctor.features) ||
    doctor.features.asr !== true ||
    doctor.features.http !== true ||
    doctor.features.realtime_websocket !== true ||
    !/^[0-9]+\.[0-9]+\.[0-9]+(?:[-+][a-zA-Z0-9.-]+)?$/.test(doctor.version)
  )
    throw fail(
      "native",
      "Use a native ASR build with HTTP and realtime WebSocket support.",
    );
  if (
    config.backend === "cuda" &&
    (doctor.features.backend_cuda !== true ||
      doctor.accelerator_available !== true ||
      doctor.driver_runtime_compatible !== true)
  )
    throw fail(
      "backend",
      "The selected CUDA build or compatible accelerator is unavailable; choose an appropriate verified target build.",
    );
  const info = await native(
    config.binary,
    ["model", "info", config.model],
    signal,
  );
  if (
    !record(info) ||
    info.role !== "asr" ||
    info.architecture !== "asr" ||
    info.runtime_compatible !== true ||
    info.size !== model.size ||
    !Array.isArray(info.errors) ||
    info.errors.length
  )
    throw fail(
      "native",
      "The selected native binary does not accept this verified ASR artifact.",
    );
  canceled(signal);
  return {
    version: doctor.version,
    backend: config.backend,
    backend_readiness: "metadata_only",
    deployment_host: process.platform === "linux",
    model_revision: expected.revision,
    model_bytes: model.size,
    model_sha256: model.sha256,
    binary_sha256: binary.sha256,
    index_sha256: index.sha256,
    binary_trust: "owner_gate",
    index_trust: "owner_gate",
  };
}
