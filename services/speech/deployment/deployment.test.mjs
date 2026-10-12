import { test } from "node:test";
import assert from "node:assert/strict";
import {
  mkdtemp,
  mkdir,
  writeFile,
  readFile,
  rm,
  rename,
  symlink,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createHash } from "node:crypto";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";
import { stableFile } from "./files.mjs";
import { parseArgs, main } from "./cli.mjs";
import {
  indexedArtifact,
  inspect,
  MODEL_REPO,
  nativeJson,
} from "./inspect.mjs";
import { BUNDLE_FILES, bundle, verify } from "./package.mjs";
import { CASES, template, audit, auditFile, initialize } from "./review.mjs";
import { parseArgs as reviewArgs, main as reviewMain } from "./review-cli.mjs";
const child = promisify(execFile);
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
async function temp(run) {
  const directory = await mkdtemp(join(tmpdir(), "ion-speech-deploy-test-"));
  try {
    return await run(directory);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
}
async function fixture(directory) {
  const bytes = Buffer.from("GGUF synthetic artifact: not a native ASR model");
  const model = join(directory, "nemotron-test.gguf"),
    binary = join(directory, "native-stub"),
    index = join(directory, "model-index.json");
  const entry = {
    schema_version: 1,
    models: [
      {
        repo: MODEL_REPO,
        revision: "b".repeat(40),
        artifacts: [
          {
            role: "asr",
            type: "file",
            filename: "nemotron-test.gguf",
            size: bytes.length,
            sha256: hash(bytes),
          },
        ],
      },
    ],
  };
  await writeFile(model, bytes);
  await writeFile(binary, "synthetic executable marker only");
  await writeFile(index, JSON.stringify(entry));
  return { config: { model, binary, index, backend: "cpu" }, entry, bytes };
}
function diagnostic(config, changes = {}) {
  return async (_, args) =>
    args[0] === "doctor"
      ? {
          version: "0.1.0",
          features: {
            asr: true,
            http: true,
            realtime_websocket: true,
            backend_cuda: false,
          },
          accelerator_available: false,
          driver_runtime_compatible: true,
          ...changes,
        }
      : {
          role: "asr",
          architecture: "asr",
          runtime_compatible: true,
          size: config.bytes.length,
          errors: [],
        };
}
const NOW = new Date("2026-10-12T12:00:00.000Z");
function observed(endpoint = "https://speech.example.com") {
  const value = template("c".repeat(40), endpoint, NOW);
  for (const device of value.devices) {
    device.os_version = device.platform === "ios" ? "iOS 18.0" : "Android 13";
    device.build_sha256 = "d".repeat(64);
    for (const result of device.cases) {
      result.status = "passed";
      result.observed_at = NOW.toISOString();
      result.evidence_sha256 = "e".repeat(64);
      for (const key of Object.keys(result.facts)) result.facts[key] = true;
    }
  }
  return value;
}
test("deploy parsing rejects unknown or duplicate flags, including after help, before dependencies", () => {
  for (const args of [
    ["inspect", "--help", "--tokenn", "private"],
    ["bundle", "--output", "/one", "--output", "/two"],
    ["verify", "--directory"],
    ["inspect", "--binary", "relative"],
    ["inspect", "--backend", "automatic"],
    ["bundle", "--output", "/one", "unexpected"],
  ])
    assert.throws(() => parseArgs(args), { code: "usage", exitCode: 2 });
  assert.deepEqual(parseArgs(["inspect", "--help"]), {
    command: "help",
    topic: "inspect",
  });
});
test("private device parsing rejects unknown options, invalid revision, HTTP and credential URLs", () => {
  for (const args of [
    ["plan", "--help", "--private", "secret"],
    ["init", "--revision", "short", "--output", "/one"],
    [
      "init",
      "--revision",
      "a".repeat(40),
      "--output",
      "/one",
      "--origin",
      "http://speech.example.com",
    ],
    [
      "init",
      "--revision",
      "a".repeat(40),
      "--output",
      "/one",
      "--origin",
      "https://secret@host.example.com",
    ],
    [
      "init",
      "--revision",
      "a".repeat(40),
      "--output",
      "/one",
      "--origin",
      "https://host.example.com/?token=secret",
    ],
  ])
    assert.throws(() => reviewArgs(args), { code: "usage", exitCode: 2 });
});
test("stable reads hash complete bytes and refuse directory, empty, oversized and canceled artifacts", () =>
  temp(async (directory) => {
    const path = join(directory, "private-artifact"),
      bytes = Buffer.alloc(2 * 1024 * 1024 + 31, 53);
    await writeFile(path, bytes);
    assert.deepEqual(await stableFile(path, bytes.length), {
      size: bytes.length,
      sha256: hash(bytes),
    });
    await assert.rejects(stableFile(path, bytes.length - 1), { code: "file" });
    await assert.rejects(stableFile(directory, 1024), { code: "file" });
    await writeFile(path, "");
    await assert.rejects(stableFile(path, 1024), { code: "file" });
    await assert.rejects(
      stableFile(path, 1024, { signal: AbortSignal.abort() }),
      { code: "canceled" },
    );
  }));
test("model index refuses ambiguous repository/artifact and unsafe unpinned metadata", () =>
  temp(async (directory) => {
    const { entry } = await fixture(directory);
    const duplicate = structuredClone(entry);
    duplicate.models.push(duplicate.models[0]);
    assert.throws(() => indexedArtifact(duplicate), { code: "index" });
    for (const [key, value] of [
      ["filename", "../model.gguf"],
      ["sha256", "bad"],
      ["size", 9 * 1024 ** 3],
    ]) {
      const invalid = structuredClone(entry);
      invalid.models[0].artifacts[0][key] = value;
      assert.throws(() => indexedArtifact(invalid), { code: "index" });
    }
    const noRevision = structuredClone(entry);
    noRevision.models[0].revision = "main";
    assert.throws(() => indexedArtifact(noRevision), { code: "index" });
  }));
test("native inspection compares actual model bytes before any diagnostic and labels trust separately", () =>
  temp(async (directory) => {
    const files = await fixture(directory);
    const result = await inspect(files.config, { native: diagnostic(files) });
    assert.equal(result.model_sha256, hash(files.bytes));
    assert.equal(result.index_trust, "owner_gate");
    assert.equal(result.binary_trust, "owner_gate");
    assert.equal(result.backend_readiness, "metadata_only");
    await writeFile(files.config.model, Buffer.alloc(files.bytes.length, 1));
    let called = false;
    await assert.rejects(
      inspect(files.config, {
        native: async () => {
          called = true;
        },
      }),
      { code: "model" },
    );
    assert.equal(called, false);
  }));
test("inspection refuses filename substitution, missing realtime and missing CUDA availability", () =>
  temp(async (directory) => {
    const files = await fixture(directory);
    await assert.rejects(
      inspect(
        { ...files.config, model: join(directory, "other.gguf") },
        { native: diagnostic(files) },
      ),
      { code: "model" },
    );
    await assert.rejects(
      inspect(files.config, {
        native: diagnostic(files, {
          features: { asr: true, http: true, realtime_websocket: false },
        }),
      }),
      { code: "native" },
    );
    await assert.rejects(
      inspect(
        { ...files.config, backend: "cuda" },
        { native: diagnostic(files) },
      ),
      { code: "backend" },
    );
  }));
test("native execution failure does not leak raw child errors, args, environment or paths", async () => {
  await assert.rejects(
    nativeJson(
      "/not-an-installed-private-binary",
      ["private-native-body"],
      undefined,
      { ION_SPEECH_TOKEN: "PRIVATE_TOKEN" },
    ),
    (error) => {
      assert.equal(error.code, "native");
      for (const secret of [
        "private-binary",
        "private-native-body",
        "PRIVATE_TOKEN",
        "ENOENT",
      ])
        assert.ok(!error.message.includes(secret));
      return true;
    },
  );
});
test("source-only bundle roundtrips exact allowlist and does not replace existing output", () =>
  temp(async (directory) => {
    const output = join(directory, "package");
    const created = await bundle(output);
    const checked = await verify(output);
    assert.deepEqual(checked, created);
    assert.equal(created.deployed, false);
    assert.equal(created.files, BUNDLE_FILES.length);
    assert.ok(
      !BUNDLE_FILES.some(
        (path) =>
          path.endsWith(".gguf") ||
          path.endsWith(".env") ||
          path.includes("node_modules"),
      ),
    );
    await assert.rejects(bundle(output), { code: "output" });
  }));
test("bundle refuses tampered, extra, missing and manifest traversal entries", () =>
  temp(async (directory) => {
    const output = join(directory, "package");
    await bundle(output);
    const asset = join(output, ...BUNDLE_FILES[0].split("/"));
    const original = await readFile(asset);
    await writeFile(asset, Buffer.from("changed"));
    await assert.rejects(verify(output), { code: "package" });
    await writeFile(asset, original);
    await writeFile(join(output, "private.env"), "PRIVATE_TOKEN");
    await assert.rejects(verify(output), { code: "package" });
    await rm(join(output, "private.env"));
    await rename(asset, asset + ".removed");
    await assert.rejects(verify(output), { code: "package" });
    await rename(asset + ".removed", asset);
    const manifest = JSON.parse(
      await readFile(join(output, "manifest.json"), "utf8"),
    );
    manifest.files[0].path = "../../private";
    await writeFile(join(output, "manifest.json"), JSON.stringify(manifest));
    await assert.rejects(verify(output), { code: "package" });
  }));
test("bundle refuses checksum-list replacement and cancellation before creating output", () =>
  temp(async (directory) => {
    const output = join(directory, "package");
    await bundle(output);
    await writeFile(join(output, "SHA256SUMS"), "synthetic altered sums");
    await assert.rejects(verify(output), { code: "package" });
    const canceledOutput = join(directory, "cancel-package");
    await assert.rejects(
      bundle(canceledOutput, { signal: AbortSignal.abort() }),
      { code: "canceled" },
    );
    await assert.rejects(readFile(join(canceledOutput, "manifest.json")), {
      code: "ENOENT",
    });
  }));
test("source package refuses a symlink or junction within the allowlist", () =>
  temp(async (directory) => {
    const output = join(directory, "package");
    await bundle(output);
    const speech = join(output, "services", "speech");
    const real = join(output, "speech-outside");
    await rename(speech, real);
    await symlink(
      real,
      speech,
      process.platform === "win32" ? "junction" : "dir",
    );
    await assert.rejects(verify(output), { code: "package" });
  }));
test("device initialization creates 46 pending observations and never overwrites", () =>
  temp(async (directory) => {
    const path = join(directory, "private-review.json");
    const result = await initialize({ revision: "c".repeat(40), output: path });
    assert.equal(result.pending, 46);
    assert.equal(result.endpoint_set, false);
    const checked = await auditFile(path);
    assert.equal(checked.pending, 46);
    assert.equal(checked.passed, 0);
    await assert.rejects(
      initialize({ revision: "c".repeat(40), output: path }),
      { code: "output" },
    );
  }));
test("an observed passing case requires OS/build/time/evidence/endpoint and every fact", () => {
  const value = observed();
  assert.equal(audit(value, NOW).passed, 46);
  for (const key of ["os_version", "build_sha256"]) {
    const invalid = structuredClone(value);
    invalid.devices[0][key] = null;
    assert.throws(() => audit(invalid, NOW), { code: "review" });
  }
  for (const key of ["observed_at", "evidence_sha256"]) {
    const invalid = structuredClone(value);
    invalid.devices[0].cases[0][key] = null;
    assert.throws(() => audit(invalid, NOW), { code: "review" });
  }
  const endpoint = structuredClone(value);
  endpoint.endpoint = null;
  assert.throws(() => audit(endpoint, NOW), { code: "review" });
  const fact = structuredClone(value);
  fact.devices[0].cases[0].facts.app_opens = false;
  assert.throws(() => audit(fact, NOW), { code: "review" });
});
test("a reported failure needs at least one observed false fact; pending and blocked never claim proof", () => {
  const value = observed();
  value.devices[0].cases[0].status = "failed";
  assert.throws(() => audit(value, NOW), { code: "review" });
  value.devices[0].cases[0].facts.app_opens = false;
  assert.equal(audit(value, NOW).failed, 1);
  const pending = template("c".repeat(40), undefined, NOW);
  pending.devices[0].cases[0].status = "blocked";
  pending.devices[0].cases[0].reason = "build_unavailable";
  assert.equal(audit(pending, NOW).blocked, 1);
  pending.devices[0].cases[0].evidence_sha256 = "a".repeat(64);
  assert.throws(() => audit(pending, NOW), { code: "review" });
});
test("review refuses changed schema/device/case identities, secret fields and future observations", () => {
  for (const mutate of [
    (value) => (value.token = "PRIVATE_TOKEN"),
    (value) => (value.devices[0].model = "simulator"),
    (value) => value.devices.reverse(),
    (value) => value.devices[0].cases.pop(),
    (value) => (value.devices[0].cases[0].facts.transcript = "private words"),
    (value) =>
      (value.devices[0].cases[0].observed_at = "2030-01-01T00:00:00.000Z"),
    (value) => (value.review_id = "-".repeat(36)),
    (value) =>
      (value.devices[0].cases[0].observed_at = "2026-02-30T00:00:00.000Z"),
  ]) {
    const value = observed();
    mutate(value);
    assert.throws(() => audit(value, NOW), { code: "review" });
  }
});
test("review audit output labels owner claims, omits endpoint/path/private data and rejects fabricated input", () =>
  temp(async (directory) => {
    const file = join(directory, "PRIVATE_REVIEW.json");
    await writeFile(file, JSON.stringify(observed()));
    const lines = [];
    assert.equal(
      await reviewMain(["audit", "--file", file], {
        write: (value) => lines.push(value),
      }),
      0,
    );
    const output = lines.join("\n");
    assert.match(output, /owner_reported_not_automatically_verified/);
    assert.ok(!output.includes("speech.example.com"));
    assert.ok(!output.includes(file));
    const value = observed();
    value.token = "PRIVATE_TOKEN";
    await writeFile(file, JSON.stringify(value));
    const errors = [];
    assert.equal(
      await reviewMain(["audit", "--file", file], {
        write: (value) => errors.push(value),
      }),
      1,
    );
    assert.ok(!errors.join().includes("PRIVATE_TOKEN"));
  }));
test("pending review audit exits nonzero and reports a concrete incomplete count", () =>
  temp(async (directory) => {
    const file = join(directory, "pending.json");
    await writeFile(
      file,
      JSON.stringify(template("c".repeat(40), undefined, NOW)),
    );
    const lines = [];
    assert.equal(
      await reviewMain(["audit", "--file", file], {
        write: (value) => lines.push(value),
      }),
      1,
    );
    assert.match(lines.join(), /pending: 46/);
    assert.match(lines.join(), /incomplete_observations/);
  }));
test("evidence hashing never prints or copies private contents, and help does not hide invalid flags", () =>
  temp(async (directory) => {
    const file = join(directory, "PRIVATE_EVIDENCE.txt"),
      bytes = Buffer.from("PRIVATE_TOKEN private spoken words");
    await writeFile(file, bytes);
    const lines = [];
    assert.equal(
      await reviewMain(["evidence", "--file", file], {
        write: (value) => lines.push(value),
      }),
      0,
    );
    assert.match(lines.join(), new RegExp(hash(bytes)));
    assert.ok(!lines.join().includes("PRIVATE_TOKEN"));
    assert.ok(!lines.join().includes(file));
    const errors = [];
    assert.equal(
      await main(["inspect", "--help", "--token", "PRIVATE_TOKEN"], {
        write: (value) => errors.push(value),
      }),
      2,
    );
    assert.ok(!errors.join().includes("PRIVATE_TOKEN"));
  }));
test("new version entrypoints work before dependency loading", async () => {
  for (const name of ["deploy.mjs", "device-review.mjs"]) {
    const entry = fileURLToPath(new URL(`../${name}`, import.meta.url));
    for (const flag of ["--version", "-v", "-V"])
      assert.equal(
        (
          await child(process.execPath, [entry, flag], {
            timeout: 10_000,
            windowsHide: true,
          })
        ).stdout.trim(),
        "0.1.0",
      );
    const measure = async (args) => {
      const start = performance.now();
      await child(process.execPath, args, {
        timeout: 10_000,
        windowsHide: true,
      });
      return performance.now() - start;
    };
    const floors = [],
      versions = [];
    for (let i = 0; i < 5; i++) {
      floors.push(await measure(["-e", ""]));
      versions.push(await measure([entry, "--version"]));
    }
    const median = (values) => values.sort((a, b) => a - b)[2];
    assert.ok(
      median(versions) <= median(floors) * 1.8 + 20,
      "version probe loaded work beyond the Node startup floor",
    );
  }
});
