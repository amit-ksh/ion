import { fileURLToPath } from "node:url";
import { homedir } from "node:os";
import { scalar } from "../acceptance/cli.mjs";
import { DeployError, fail, absolute } from "./files.mjs";
export const HELP = {
  inspect:
    "Inspect a verified local native install and model without loading it or opening a listener.\nRequired: --binary <absolute executable>, --index <absolute installed model-index.json>, --model <absolute GGUF>.\nOptional: --backend cpu|cuda (default cpu). No network or model download.\nExamples: node services/speech/deploy.mjs inspect --binary /opt/ion-speech/bin/nemo-speech --index /opt/ion-speech/share/nemo-speech/model-index.json --model /var/lib/ion-speech/models/<indexed-file>.gguf\nnode services/speech/deploy.mjs inspect --binary <absolute-binary> --index <absolute-index> --model <absolute-gguf> --backend cuda",
  bundle:
    "Create a new source-only deployment package containing a strict file allowlist and SHA-256 manifests.\nRequired: --output <absolute new directory>, whose parent exists. Never replaces an existing directory.\nExamples: node services/speech/deploy.mjs bundle --output /private/ion-speech-package\nnode services/speech/deploy.mjs verify --directory /private/ion-speech-package",
  verify:
    "Verify package entries, bounds and complete SHA-256 values; this proves transfer integrity, not release authenticity or deployment.\nRequired: --directory <absolute package directory>.\nExamples: node services/speech/deploy.mjs verify --directory /private/ion-speech-package\nnode /private/ion-speech-package/services/speech/deploy.mjs verify --directory /private/ion-speech-package",
};
export function parseArgs(argv) {
  const command = argv[0];
  if (!command) return { command: "home" };
  if (command === "--help" && argv.length === 1) return { command: "help" };
  if (!Object.hasOwn(HELP, command))
    throw fail("usage", "Use inspect, bundle or verify, or --help alone.", 2);
  const allowed = {
    inspect: ["--binary", "--index", "--model", "--backend"],
    bundle: ["--output"],
    verify: ["--directory"],
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
        `Unrecognized or duplicate option. Valid ${command} flags: ${allowed.join(", ")}, --help.`,
        2,
      );
    const value = argv[++i];
    if (!value || value.startsWith("--"))
      throw fail("usage", `${key} requires a value.`, 2);
    values.set(key, value);
  }
  if (values.has("--help")) return { command: "help", topic: command };
  if (command === "inspect") {
    const backend = values.get("--backend") ?? "cpu";
    if (!["cpu", "cuda"].includes(backend))
      throw fail("usage", "--backend must be cpu or cuda.", 2);
    return {
      command,
      binary: absolute(values.get("--binary"), "--binary"),
      index: absolute(values.get("--index"), "--index"),
      model: absolute(values.get("--model"), "--model"),
      backend,
    };
  }
  const key = command === "bundle" ? "output" : "directory";
  return { command, [key]: absolute(values.get(`--${key}`), `--${key}`) };
}
export async function main(
  argv,
  { signal, write = (text) => process.stdout.write(`${text}\n`) } = {},
) {
  let config;
  try {
    config = parseArgs(argv);
    if (config.command === "home") {
      write(
        `bin: ${scalar(fileURLToPath(new URL("../deploy.mjs", import.meta.url)).replace(homedir(), "~"))}\ndescription: Prepare and inspect a personal speech deployment without provisioning resources\nstate: no deployment target inferred\nhelp: ${scalar("Run node services/speech/deploy.mjs inspect --binary <absolute-binary> --index <absolute-index> --model <absolute-gguf>, or bundle --output <new-absolute-directory>.")}`,
      );
      return 0;
    }
    if (config.command === "help") {
      write(
        `description: Prepare and inspect an explicit personal speech deployment\nhelp: ${scalar(config.topic ? HELP[config.topic] : Object.values(HELP).join("\n\n"))}`,
      );
      return 0;
    }
    if (
      Number(process.versions.node.split(".")[0]) < 22 ||
      typeof globalThis.Bun !== "undefined"
    )
      throw fail("runtime", "Execute this command with Node 22 or newer.");
    let result, scope;
    if (config.command === "inspect") {
      const { inspect } = await import("./inspect.mjs");
      result = await inspect(config, { signal });
      scope = "local_install_integrity";
    } else {
      const { bundle, verify } = await import("./package.mjs");
      result =
        config.command === "bundle"
          ? await bundle(config.output, { signal })
          : await verify(config.directory, { signal });
      scope = "package_integrity";
    }
    write(
      `status: passed\nscope: ${scope}\n${Object.entries(result)
        .map(([key, value]) => `${key}: ${scalar(value)}`)
        .join(
          "\n",
        )}\nhelp: ${scalar(config.command === "inspect" ? "Binary/index release authenticity remains an owner gate. Follow docs/ai-ide-plan/50-speech-deployment-device-review.md on the actual Linux target." : "Retain and compare manifest_sha256 through a trusted channel before target installation. Follow the packaged deployment and device checklist.")}`,
    );
    return 0;
  } catch (error) {
    const safe =
      error instanceof DeployError
        ? error
        : fail(
            "preparation",
            "Preparation failed; check the selected local files and documented host requirements privately.",
          );
    write(
      `status: failed\ncode: ${safe.code}\nerror: ${scalar(safe.message)}\nhelp: ${scalar(config && HELP[config.command] ? HELP[config.command] : "Run node services/speech/deploy.mjs --help; no private dependency output is printed.")}`,
    );
    return safe.exitCode;
  }
}
