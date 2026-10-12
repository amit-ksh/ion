import { homedir } from "node:os";
import { fileURLToPath } from "node:url";
import { scalar, table } from "../acceptance/cli.mjs";
import { fail, DeployError, absolute, stableFile } from "./files.mjs";
import { DEVICES, CASES, origin, initialize, auditFile } from "./review.mjs";
export const HELP = {
  plan: "Show the pending physical-device cases for iPhone 15 and Moto G32. No microphone or device access.\nFlags: --help.\nExamples: node services/speech/device-review.mjs plan\nnode services/speech/device-review.mjs init --revision <40-char-commit> --output <private-absolute-json>",
  init: "Create a new private owner-observation template. All cases start pending.\nRequired: --revision <40-char-commit>, --output <new-absolute-json>. Optional: --origin <https-root-origin>.\nExamples: node services/speech/device-review.mjs init --revision <40-char-commit> --output /private/speech-devices.json\nnode services/speech/device-review.mjs init --revision <40-char-commit> --output /private/speech-devices.json --origin https://speech.example.com",
  audit:
    "Validate private physical-device observations and summarize their evidence metadata. This does not run or certify device tests.\nRequired: --file <absolute-review-json>. No tokens, transcripts, paths or raw observations are printed.\nExamples: node services/speech/device-review.mjs audit --file /private/speech-devices.json\nnode services/speech/device-review.mjs evidence --file /private/sanitized-observation.json",
  evidence:
    "Hash a stable private evidence file without printing or copying its contents (limit 8 MiB). The hash does not authenticate its claims.\nRequired: --file <absolute-evidence-file>.\nExamples: node services/speech/device-review.mjs evidence --file /private/sanitized-observation.json\nnode services/speech/device-review.mjs evidence --file /private/sanitized-screen.png",
};
export function parseArgs(argv) {
  const command = argv[0];
  if (!command) return { command: "home" };
  if (command === "--help" && argv.length === 1) return { command: "help" };
  if (!Object.hasOwn(HELP, command))
    throw fail(
      "usage",
      "Use plan, init, audit or evidence, or --help alone.",
      2,
    );
  const allowed = {
    plan: [],
    init: ["--revision", "--output", "--origin"],
    audit: ["--file"],
    evidence: ["--file"],
  }[command];
  const values = new Map();
  for (let i = 1; i < argv.length; i++) {
    const key = argv[i];
    if (key === "--help") {
      if (values.has(key)) throw fail("usage", "Specify --help once.", 2);
      values.set(key, true);
      continue;
    }
    if (!allowed.includes(key) || values.has(key))
      throw fail(
        "usage",
        `Unrecognized or duplicate option. Valid ${command} flags: ${[...allowed, "--help"].join(", ")}.`,
        2,
      );
    const value = argv[++i];
    if (!value || value.startsWith("--"))
      throw fail("usage", `${key} requires a value.`, 2);
    values.set(key, value);
  }
  if (values.has("--help")) return { command: "help", topic: command };
  if (command === "plan") return { command };
  if (command === "init") {
    if (!/^[a-f0-9]{40}$/.test(values.get("--revision") ?? ""))
      throw fail(
        "usage",
        "--revision requires the exact full 40-character source commit SHA.",
        2,
      );
    return {
      command,
      revision: values.get("--revision"),
      output: absolute(values.get("--output"), "--output"),
      origin: origin(values.get("--origin")),
    };
  }
  return { command, file: absolute(values.get("--file"), "--file") };
}
export async function main(
  argv,
  { signal, write = (text) => process.stdout.write(`${text}\n`) } = {},
) {
  let config;
  try {
    config = parseArgs(argv);
    if (config.command === "home" || config.command === "plan") {
      write(
        `bin: ${scalar(fileURLToPath(new URL("../device-review.mjs", import.meta.url)).replace(homedir(), "~"))}\ndescription: Track private owner observations from real iPhone 15 and Moto G32 speech tests\nscope: physical_device_plan\nreview_performed: false\n${table(
          "devices",
          ["device", "model", "status"],
          DEVICES.map((device) => ({
            device: device.id,
            model: device.model,
            status: "pending",
          })),
        )}\n${table(
          "cases",
          ["check", "status"],
          CASES.map((test) => ({ check: test.id, status: "pending" })),
        )}\nhelp: ${scalar("Use init --revision <40-char-commit> --output <private-absolute-json>; then follow docs/ai-ide-plan/50-speech-deployment-device-review.md on both actual devices.")}`,
      );
      return 0;
    }
    if (config.command === "help") {
      write(
        `description: Track private physical-device observations\nhelp: ${scalar(config.topic ? HELP[config.topic] : Object.values(HELP).join("\n\n"))}`,
      );
      return 0;
    }
    if (
      Number(process.versions.node.split(".")[0]) < 22 ||
      typeof globalThis.Bun !== "undefined"
    )
      throw fail("runtime", "Execute this command with Node 22 or newer.");
    if (config.command === "init") {
      const result = await initialize(config, { signal });
      write(
        `status: created\nscope: physical_device_plan\n${Object.entries(result)
          .map(([key, value]) => `${key}: ${scalar(value)}`)
          .join(
            "\n",
          )}\nhelp: ${scalar("Record only actual device observations in the private file; use audit --file <private-absolute-json> to validate it.")}`,
      );
      return 0;
    }
    if (config.command === "evidence") {
      const result = await stableFile(config.file, 8 * 1024 * 1024, { signal });
      write(
        `status: hashed\nscope: private_evidence_integrity\nbytes: ${result.size}\nsha256: ${result.sha256}`,
      );
      return 0;
    }
    const result = await auditFile(config.file, { signal });
    write(
      `status: ${result.failed || result.blocked || result.pending ? "incomplete_observations" : "observations_recorded"}\nscope: owner_device_observations\nproof: owner_reported_not_automatically_verified\n${Object.entries(
        result,
      )
        .filter(([key]) => key !== "rows")
        .map(([key, value]) => `${key}: ${scalar(value)}`)
        .join(
          "\n",
        )}\n${table("checks", ["device", "check", "status"], result.rows)}`,
    );
    return result.failed || result.blocked || result.pending ? 1 : 0;
  } catch (error) {
    const safe =
      error instanceof DeployError
        ? error
        : fail(
            "review",
            "Cannot validate the private review; check its local path and documented bounded schema.",
          );
    write(
      `status: failed\ncode: ${safe.code}\nerror: ${scalar(safe.message)}\nhelp: ${scalar(config && HELP[config.command] ? HELP[config.command] : "Run node services/speech/device-review.mjs --help; no private observations are printed.")}`,
    );
    return safe.exitCode;
  }
}
