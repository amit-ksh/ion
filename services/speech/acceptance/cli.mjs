import { fileURLToPath } from "node:url";
import { homedir } from "node:os";
import {
  AcceptanceError,
  HELP,
  parseArgs,
  tokenFromEnvironment,
  VERSION,
} from "./config.mjs";

// This CLI emits only its own bounded scalar/table schema, with TOON quoting at the output boundary.
export function scalar(value) {
  if (typeof value !== "string") return String(value);
  if (
    !value ||
    value.trim() !== value ||
    /^(true|false|null|[+-]?[0-9]+(?:\.[0-9]+)?(?:e[+-]?[0-9]+)?)$/i.test(
      value,
    ) ||
    /[\\":,\[\]{}\x00-\x1f]/.test(value) ||
    /^[-#]/.test(value)
  )
    return JSON.stringify(value);
  return value;
}
export function table(name, fields, values) {
  if (!values.length) return `${name}: []`;
  return `${name}[${values.length}]{${fields.join(",")}}:\n${values.map((value) => `  ${fields.map((field) => scalar(value[field])).join(",")}`).join("\n")}`;
}
function errorHelp(error, config) {
  if (error.exitCode === 2)
    return config?.command && HELP[config.command]
      ? HELP[config.command]
      : "Run node services/speech/accept.mjs --help; supply only the documented flags and a private token environment.";
  const specific = {
    not_ready:
      "Verify the service's selected local GGUF, binary and resource limits; then run check --url <https-origin>.",
    inventory:
      "Verify the private token environment and model readiness; then run check --url <https-origin>.",
    model:
      "Load the Nemotron 3.5 transcription model; use check --url <https-origin> --model <exact-inventory-id> when needed.",
    auth_exposed:
      "Require service API authentication before using it remotely; then run check --url <https-origin>.",
    auth_denied:
      "Verify the private service token and allowed API route; then run check --url <https-origin>.",
    redirect:
      "Serve the exact /v1 API and transcription socket directly; run check --url <direct-https-origin>.",
    finalization_deadline:
      "Resolve the native server's finalization stall or resource issue; then rerun accept with the same private fixture.",
    recognition_mismatch:
      "Check the private WAV's spoken words and expected text, model language and selected WER tolerance; then rerun accept.",
    fixture:
      "Select a stable private mono PCM16 WAV and UTF8 expected text; run accept --url <https-origin> --wav <private.wav> --expected-text-file <private.txt>.",
    canceled:
      "Start a new explicit check or accept when ready; the canceled audio was not replayed.",
    network:
      "Verify the direct endpoint and trusted TLS certificate; then run check --url <https-origin>.",
    socket_transport:
      "Verify the trusted TLS certificate and bounded explicit realtime route; then run check --url <https-origin>.",
    runtime:
      "Use node services/speech/accept.mjs with Node 22 or newer; reserve Bun for the dependency installation step.",
  };
  return (
    specific[error.code] ??
    "Install dependencies with bun install --cwd services/speech --ignore-scripts; run node services/speech/accept.mjs --help for the selected check."
  );
}
export async function main(
  argv,
  {
    env = process.env,
    signal,
    write = (text) => process.stdout.write(`${text}\n`),
  } = {},
) {
  let config;
  const checks = [];
  try {
    config = parseArgs(argv);
    if (config.command === "version") {
      write(VERSION);
      return 0;
    }
    if (config.command === "help") {
      write(
        `description: Validate an explicit personal speech deployment without exposing audio or secrets\nhelp: ${scalar(config.topic ? HELP[config.topic] : `${HELP.check}\n\n${HELP.accept}`)}`,
      );
      return 0;
    }
    if (config.command === "home") {
      const binary = fileURLToPath(
        new URL("../accept.mjs", import.meta.url),
      ).replace(homedir(), "~");
      write(
        `bin: ${scalar(binary)}\ndescription: Validate an explicit personal speech deployment without exposing audio or secrets\nconfiguration:\n  token_environment_set: ${typeof env.ION_SPEECH_TOKEN === "string" && env.ION_SPEECH_TOKEN.length > 0}\n  deployment_checked: false\nhelp: ${scalar("Run node services/speech/accept.mjs check --url <https-origin>, or accept --url <https-origin> --wav <private.wav> --expected-text-file <private.txt>.")}`,
      );
      return 0;
    }
    const token = tokenFromEnvironment(config, env);
    if (
      Number(process.versions.node.split(".")[0]) < 22 ||
      typeof globalThis.Bun !== "undefined"
    )
      throw new AcceptanceError(
        "runtime",
        "Run this command with Node 22 or newer; Bun's socket shim does not establish the required receive-limit contract.",
      );
    const { runChecks } = await import("./checks.mjs");
    const result = await runChecks(config, token, {
      signal,
      onCheck: (value) => checks.push(value),
    });
    write(
      `status: ${result.status}\nscope: ${result.scope}\n${table("checks", ["check", "status"], result.checks)}\n${table("clients", ["client", "status", "audio_bytes", "partials", "finals", "commit_ms", "elapsed_ms", "wer"], result.clients)}`,
    );
    if (result.scope === "readiness_only")
      write(
        `help: ${scalar("Run accept with an explicit private WAV and expected-text-file to check recognition and concurrent clients.")}`,
      );
    return 0;
  } catch (error) {
    const safe =
      error instanceof AcceptanceError
        ? error
        : new AcceptanceError(
            "check_failed",
            "Speech check failed; confirm dependencies, stable fixtures and endpoint access. No private diagnostics are printed.",
          );
    write(
      `status: failed\ncode: ${safe.code}\nerror: ${scalar(safe.message)}\n${table("checks", ["check", "status"], checks)}\nhelp: ${scalar(errorHelp(safe, config))}`,
    );
    return safe.exitCode;
  }
}
