import assert from "node:assert/strict";
import { test } from "node:test";
import http from "node:http";
import https from "node:https";
import { mkdtemp, writeFile, rm, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import { basename, dirname, join, resolve } from "node:path";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { WebSocketServer } from "ws";
import {
  AcceptanceError,
  endpoint,
  parseArgs,
  tokenFromEnvironment,
} from "./config.mjs";
import { parseWav, wordErrorRate } from "./wav.mjs";
import { getJson, SpeechChannel } from "./transport.mjs";
import { runChecks } from "./checks.mjs";
import { main, scalar, table } from "./cli.mjs";

const TOKEN = "private-local-fixture-token-32-characters";
const MODEL = "nemotron-3.5-asr-streaming-0.6b";
function wave(seconds = 2, rate = 16000) {
  const pcm = Buffer.alloc(seconds * rate * 2);
  const bytes = Buffer.alloc(44 + pcm.length);
  bytes.write("RIFF");
  bytes.writeUInt32LE(bytes.length - 8, 4);
  bytes.write("WAVEfmt ", 8);
  bytes.writeUInt32LE(16, 16);
  bytes.writeUInt16LE(1, 20);
  bytes.writeUInt16LE(1, 22);
  bytes.writeUInt32LE(rate, 24);
  bytes.writeUInt32LE(rate * 2, 28);
  bytes.writeUInt16LE(2, 32);
  bytes.writeUInt16LE(16, 34);
  bytes.write("data", 36);
  bytes.writeUInt32LE(pcm.length, 40);
  return bytes;
}
async function fixtures(fn) {
  const directory = await mkdtemp(join(tmpdir(), "ion-speech-test-"));
  try {
    const wav = join(directory, "private.wav"),
      expected = join(directory, "private.txt");
    await writeFile(wav, wave());
    await writeFile(expected, "Hello ion user");
    return await fn({ wav, expected });
  } finally {
    if (
      dirname(resolve(directory)) !== resolve(tmpdir()) ||
      !basename(directory).startsWith("ion-speech-test-")
    )
      throw Error("Unsafe test cleanup path");
    await rm(directory, { recursive: true, force: true });
  }
}
async function service(mode, fn) {
  const modelId =
    mode === "canonical_model"
      ? "nemotron-3.5"
      : mode === "uppercase_model"
        ? "Nemotron-3.5-asr-streaming-0.6b"
        : MODEL;
  const state = {
    requests: [],
    audio: [],
    commits: 0,
    cancels: 0,
    sockets: 0,
    negotiated: 0,
    maxConcurrent: 0,
    leaks: 0,
  };
  const sockets = new Set();
  const wss = new WebSocketServer({ noServer: true, perMessageDeflate: false });
  const json = (response, data, status = 200) => {
    response.writeHead(status, { "Content-Type": "application/json" });
    response.end(JSON.stringify(data));
  };
  const server = http.createServer((request, response) => {
    state.requests.push({
      path: request.url,
      authorized: request.headers.authorization === `Bearer ${TOKEN}`,
    });
    if (request.url === "/leak") {
      state.leaks++;
      return json(response, { secret: TOKEN });
    }
    if (mode === "http_redirect") {
      response.writeHead(302, { Location: "/leak" });
      return response.end();
    }
    if (request.url === "/ready")
      return json(response, { ready: mode !== "not_ready" });
    if (request.url === "/v1/models") {
      if (
        mode !== "public_http" &&
        request.headers.authorization !== `Bearer ${TOKEN}`
      )
        return json(response, { error: { message: TOKEN } }, 401);
      if (mode === "large_http")
        return json(response, { data: "x".repeat(65537) });
      return json(response, {
        data: [
          {
            id: modelId,
            capability:
              mode === "wrong_capability" ? "speech" : "transcription",
          },
        ],
      });
    }
    json(response, { error: { message: TOKEN } }, 404);
  });
  server.on("upgrade", (request, socket, head) => {
    if (mode === "socket_redirect") {
      socket.end(
        "HTTP/1.1 302 Found\r\nLocation: /leak\r\nContent-Length: 0\r\n\r\n",
      );
      return;
    }
    const authorized = request.headers.authorization === `Bearer ${TOKEN}`;
    if (!authorized && mode !== "public_socket" && mode !== "policy_auth") {
      socket.end("HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\n\r\n");
      return;
    }
    wss.handleUpgrade(request, socket, head, (ws) => {
      sockets.add(ws);
      state.sockets++;
      state.maxConcurrent = Math.max(state.maxConcurrent, state.sockets);
      ws.on("error", () => {});
      ws.once("close", () => {
        sockets.delete(ws);
        state.sockets--;
      });
      if (!authorized && mode === "policy_auth") {
        ws.close(1008);
        return;
      }
      const send = (value) => ws.send(JSON.stringify(value));
      if (mode === "fragment_limit") {
        for (let i = 0; i < 130; i++) ws.send("x", { fin: i === 129 });
        return;
      }
      if (mode === "event_flood") {
        for (let i = 0; i < 40; i++)
          send({ type: "private-noop", text: TOKEN });
        return;
      }
      if (mode === "large_socket") {
        ws.send("x".repeat(65537));
        return;
      }
      if (mode === "binary_socket") {
        ws.send(Buffer.from("secret audio"));
        return;
      }
      send({
        type: "session.created",
        session: {
          input_audio_format: mode === "bad_format" ? "float32" : "pcm16",
          sample_rate: 16000,
          model: mode === "bad_model" ? "other" : modelId,
        },
      });
      let bytes = 0,
        first = true;
      ws.on("message", (data, binary) => {
        if (binary) {
          if (first) {
            state.audio.push({ negotiated: state.negotiated });
            first = false;
            if (mode === "early_commit")
              send({ type: "input_audio_buffer.committed" });
            else if (mode !== "no_partial")
              send({
                type: "conversation.item.input_audio_transcription.delta",
                delta: "Hello ion",
              });
          }
          bytes += data.length;
          return;
        }
        const event = JSON.parse(data);
        if (event.type === "session.update") {
          state.negotiated++;
          if (mode === "negotiation_stall") return;
          send({
            type: "session.updated",
            session:
              mode === "bad_ack"
                ? { ...event.session, sample_rate: 8000 }
                : event.session,
          });
        } else if (event.type === "input_audio_buffer.commit") {
          state.commits++;
          if (mode === "final_stall") return;
          if (mode !== "no_final")
            send({
              type: "conversation.item.input_audio_transcription.completed",
              transcript:
                mode === "mismatch"
                  ? "unrelated private words"
                  : "Hello ion user",
            });
          if (mode === "late_partial")
            send({
              type: "conversation.item.input_audio_transcription.delta",
              delta: "unfinished",
            });
          send({ type: "input_audio_buffer.committed" });
        } else if (event.type === "response.cancel") {
          state.cancels++;
          if (mode !== "cancel_stall")
            send({ type: "input_audio_buffer.cleared" });
        }
      });
    });
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const url = `http://127.0.0.1:${server.address().port}`;
  try {
    return await fn(url, state);
  } finally {
    for (const ws of sockets) ws.terminate();
    await new Promise((resolve) => wss.close(resolve));
    server.closeAllConnections();
    await new Promise((resolve) => server.close(resolve));
  }
}
function config(url, extra = {}) {
  return {
    command: "check",
    url,
    tokenEnv: "ION_SPEECH_TOKEN",
    language: "auto",
    handshakeMs: 400,
    timeoutMs: 5000,
    ...extra,
  };
}
async function accepts(mode, fn, overrides = {}) {
  return fixtures((files) =>
    service(mode, (url, state) =>
      fn(
        config(url, {
          command: "accept",
          ...files,
          maxAudioSeconds: 60,
          clients: 2,
          maxWer: 0.25,
          chunkMs: 100,
          finalizeMs: 400,
          ...overrides,
        }),
        state,
      ),
    ),
  );
}
async function rejectsCheck(mode, code, overrides = {}) {
  await service(mode, async (url) =>
    assert.rejects(
      runChecks(config(url, overrides), TOKEN),
      (error) => error instanceof AcceptanceError && error.code === code,
    ),
  );
}

test("CLI rejects unknown, duplicate and command-inappropriate flags before dependencies", async () => {
  for (const args of [
    ["check", "--private-token", TOKEN],
    ["check", "--clients", "2"],
    ["accept", "--url", "https://example.com", "--url", "https://example.com"],
  ]) {
    let output = "";
    assert.equal(
      await main(args, {
        env: {},
        write: (value) => {
          output += value;
        },
      }),
      2,
    );
    assert.ok(!output.includes(TOKEN));
    assert.match(output, /Valid|Duplicate/);
  }
});
test("CLI checks complete bounds and flag-only help", () => {
  assert.equal(parseArgs(["check", "--help"]).command, "help");
  assert.throws(
    () =>
      parseArgs([
        "check",
        "--url",
        "https://example.com",
        "--timeout-seconds",
        "601",
      ]),
    { code: "usage" },
  );
  assert.throws(() => parseArgs(["accept", "--url", "https://example.com"]), {
    code: "usage",
  });
  assert.throws(
    () =>
      parseArgs([
        "check",
        "--url",
        "https://example.com",
        "--handshake-seconds",
        "1.5",
      ]),
    { code: "usage" },
  );
});
test("endpoint requires an explicit clean origin and loopback HTTP opt-in", () => {
  assert.equal(
    endpoint("https://speech.example.com/"),
    "https://speech.example.com",
  );
  assert.equal(
    endpoint("http://127.0.0.1:8080", true),
    "http://127.0.0.1:8080",
  );
  for (const value of [
    "https://user:password@example.com",
    "https://example.com?token=secret",
    "https://example.com/path",
    "https://example.com#x",
    "http://127.0.0.1:8080",
    "http://remote.example.com",
    "http://localhost:8080",
  ])
    assert.throws(
      () =>
        endpoint(
          value,
          value.includes("remote") || value.includes("localhost"),
        ),
      { code: "usage" },
    );
});
test("token placeholder, header injection and argument disclosure are refused", () => {
  for (const token of [
    undefined,
    "short",
    "replace-with-a-random-service-token",
    `private${"a".repeat(32)}\r\nInjected: yes`,
    "a".repeat(513),
  ])
    assert.throws(
      () =>
        tokenFromEnvironment(
          { tokenEnv: "ION_SPEECH_TOKEN" },
          { ION_SPEECH_TOKEN: token },
        ),
      { code: "usage" },
    );
  assert.equal(
    tokenFromEnvironment(
      { tokenEnv: "ION_SPEECH_TOKEN" },
      { ION_SPEECH_TOKEN: TOKEN },
    ),
    TOKEN,
  );
});
test("WAV parser honors actual rates, duration and PCM data", () => {
  for (const rate of [8000, 16000, 48000, 96000]) {
    const wav = parseWav(wave(2, rate), 60);
    assert.equal(wav.sampleRate, rate);
    assert.equal(wav.pcm.length, rate * 4);
  }
});
test("WAV parser refuses stereo, float, inconsistent header and out-of-bound durations", () => {
  const stereo = wave();
  stereo.writeUInt16LE(2, 22);
  const float = wave();
  float.writeUInt16LE(3, 20);
  const badRate = wave();
  badRate.writeUInt32LE(1, 28);
  const truncated = wave().subarray(0, 44);
  for (const bytes of [stereo, float, badRate, truncated, wave(1), wave(61)])
    assert.throws(() => parseWav(bytes, 60), { code: "fixture" });
});
test("WAV parser rejects duplicate chunks and malformed RIFF declarations", () => {
  const original = wave();
  const duplicate = Buffer.concat([original, original.subarray(36)]);
  duplicate.writeUInt32LE(duplicate.length - 8, 4);
  assert.throws(() => parseWav(duplicate, 60), { code: "fixture" });
  const bad = wave();
  bad.writeUInt32LE(0, 4);
  assert.throws(() => parseWav(bad, 60), { code: "fixture" });
});
test("word error rate is bounded and normalizes case, punctuation and Unicode text", () => {
  assert.equal(wordErrorRate(["hello", "ion", "user"], "Hello, ion user!"), 0);
  assert.equal(wordErrorRate(["hello", "ion", "user"], "hello user"), 1 / 3);
  assert.equal(wordErrorRate(["नमस्ते", "दुनिया"], "नमस्ते दुनिया"), 0);
});
test("TOON output quotes metadata and reports explicit empty collections", () => {
  assert.equal(scalar("true"), '"true"');
  assert.equal(scalar("https://example.com"), '"https://example.com"');
  assert.equal(table("clients", ["client", "status"], []), "clients: []");
});
test("fast version path is bare and agrees with package identity without dependencies", async () => {
  const pkg = JSON.parse(
    await readFile(new URL("../package.json", import.meta.url), "utf8"),
  );
  for (const flag of ["--version", "-v", "-V"]) {
    const output = await new Promise((resolve, reject) => {
      const child = spawn(
        process.execPath,
        [fileURLToPath(new URL("../accept.mjs", import.meta.url)), flag],
        { stdio: ["ignore", "pipe", "pipe"] },
      );
      let stdout = "",
        stderr = "";
      child.stdout.on("data", (bytes) => {
        stdout += bytes;
      });
      child.stderr.on("data", (bytes) => {
        stderr += bytes;
      });
      child.once("error", reject);
      child.once("exit", (code) => resolve({ code, stdout, stderr }));
    });
    assert.deepEqual(output, {
      code: 0,
      stdout: `${pkg.version}\n`,
      stderr: "",
    });
  }
});
test("readiness verifies real HTTP and socket auth without audio or commit", async () => {
  await service("ok", async (url, state) => {
    const result = await runChecks(config(url), TOKEN);
    assert.equal(result.scope, "readiness_only");
    assert.equal(result.checks.length, 5);
    assert.equal(state.audio.length, 0);
    assert.equal(state.commits, 0);
    assert.equal(state.cancels, 0);
    assert.equal(
      state.requests.filter((r) => r.path === "/v1/models").length,
      3,
    );
  });
});
test("socket policy close and explicit HTTP auth refusal are supported", async () => {
  await service("policy_auth", async (url) =>
    assert.equal((await runChecks(config(url), TOKEN)).status, "passed"),
  );
});
test("public HTTP is rejected before audio", () =>
  rejectsCheck("public_http", "auth_exposed"));
test("public socket is rejected before audio", () =>
  rejectsCheck("public_socket", "auth_exposed"));
test("readiness false and non-transcription inventory are refused", async () => {
  await rejectsCheck("not_ready", "not_ready");
  await rejectsCheck("wrong_capability", "model");
});
test("HTTP redirected routes do not forward credentials", async () => {
  await service("http_redirect", async (url, state) => {
    await assert.rejects(
      getJson(url, "/v1/models", TOKEN, new AbortController().signal, 400),
      { code: "redirect" },
    );
    assert.equal(state.leaks, 0);
    assert.equal(state.requests.length, 1);
  });
});
test("socket redirects do not forward credentials or follow another route", async () => {
  await service("socket_redirect", async (url, state) => {
    await assert.rejects(runChecks(config(url), TOKEN), { code: "redirect" });
    assert.equal(state.leaks, 0);
  });
});
test("bounded HTTP and transport-level socket receive sizes are enforced", async () => {
  await rejectsCheck("large_http", "response_limit");
  await rejectsCheck("large_socket", "socket_transport");
});
test("non-JSON binary events and wrong model or PCM format are refused", async () => {
  await rejectsCheck("binary_socket", "protocol");
  await rejectsCheck("bad_format", "protocol");
  await rejectsCheck("bad_model", "protocol");
});
test("a wrong format acknowledgment and stalled negotiation are refused", async () => {
  await rejectsCheck("bad_ack", "protocol");
  await rejectsCheck("negotiation_stall", "deadline", { handshakeMs: 40 });
});
test("private fixture validation completes before any service request", async () => {
  await fixtures((files) =>
    service("ok", async (url, state) => {
      await writeFile(files.wav, Buffer.from("private invalid audio"));
      await assert.rejects(
        runChecks(
          config(url, { command: "accept", ...files, maxAudioSeconds: 60 }),
          TOKEN,
        ),
        { code: "fixture" },
      );
      assert.equal(state.requests.length, 0);
    }),
  );
});
test("concurrent acceptance verifies PCM, partials, finals, acknowledgment and Cancel", async () => {
  await accepts("ok", async (conf, state) => {
    const result = await runChecks(conf, TOKEN);
    assert.equal(result.scope, "fixture_acceptance");
    assert.equal(result.clients.length, 2);
    assert.equal(state.commits, 2);
    assert.equal(state.cancels, 1);
    assert.ok(state.maxConcurrent >= 2);
    assert.ok(state.audio.slice(0, 2).every((value) => value.negotiated === 3));
    assert.ok(
      result.clients.every(
        (value) =>
          value.audio_bytes === 64000 &&
          value.partials === 1 &&
          value.finals === 1 &&
          value.wer === 0,
      ),
    );
    assert.ok(!JSON.stringify(result).includes("Hello"));
  });
});
test("recognition rejects an acknowledgment before all audio and final text", async () => {
  await accepts("early_commit", async (conf) =>
    assert.rejects(runChecks(conf, TOKEN), { code: "protocol" }),
  );
});
test("recognition refuses unmatched expected words", async () => {
  await accepts(
    "mismatch",
    async (conf) =>
      assert.rejects(runChecks(conf, TOKEN), { code: "recognition_mismatch" }),
    { clients: 1 },
  );
});
test("missing partial, missing final and remaining partial are never accepted", async () => {
  for (const mode of ["no_partial", "no_final", "late_partial"])
    await accepts(
      mode,
      async (conf) =>
        assert.rejects(runChecks(conf, TOKEN), { code: "protocol" }),
      { clients: 1 },
    );
});
test("stalled commit fails at finalization deadline without replay", async () => {
  await accepts(
    "final_stall",
    async (conf, state) => {
      await assert.rejects(runChecks(conf, TOKEN), {
        code: "finalization_deadline",
      });
      assert.equal(state.commits, 1);
    },
    { clients: 1, finalizeMs: 40 },
  );
});
test("Cancel requires the server's clear acknowledgment", async () => {
  await accepts(
    "cancel_stall",
    async (conf, state) => {
      await assert.rejects(runChecks(conf, TOKEN), { code: "deadline" });
      assert.equal(state.cancels, 1);
    },
    { clients: 1, handshakeMs: 40 },
  );
});
test("external cancellation closes all active sockets and never commits or replays", async () => {
  await accepts("ok", async (conf, state) => {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), 250);
    try {
      await assert.rejects(
        runChecks(conf, TOKEN, { signal: controller.signal }),
        { code: "canceled" },
      );
    } finally {
      clearTimeout(timer);
    }
    assert.equal(state.commits, 0);
    assert.equal(state.cancels, 0);
    assert.equal(state.audio.length, 2);
  });
});
test("whole-check deadline aborts streaming and keeps the result failed", async () => {
  await accepts(
    "ok",
    async (conf, state) => {
      await assert.rejects(runChecks(conf, TOKEN), { code: "deadline" });
      assert.equal(state.commits, 0);
    },
    { timeoutMs: 250 },
  );
});
test("CLI failure output does not include token, provider body or private paths", async () => {
  await service("not_ready", async (url) => {
    let output = "";
    assert.equal(
      await main(["check", "--url", url, "--allow-loopback-http"], {
        env: { ION_SPEECH_TOKEN: TOKEN },
        write: (value) => {
          output += value;
        },
      }),
      1,
    );
    assert.match(output, /not_ready/);
    assert.ok(!output.includes(TOKEN));
    assert.ok(!output.includes(url));
  });
});

async function childResult(command, args, env = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      env: { ...process.env, ...env },
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "",
      stderr = "";
    child.stdout.on("data", (bytes) => {
      stdout += bytes;
    });
    child.stderr.on("data", (bytes) => {
      stderr += bytes;
    });
    child.once("error", reject);
    child.once("exit", (code) => resolve({ code, stdout, stderr }));
  });
}
test("version probes stay near the same-process Node startup floor", async () => {
  const measure = async (args) => {
    const start = performance.now();
    const result = await childResult(process.execPath, args);
    assert.equal(result.code, 0);
    return performance.now() - start;
  };
  const floors = [],
    versions = [];
  for (let i = 0; i < 5; i++) {
    floors.push(await measure(["-e", ""]));
    versions.push(
      await measure([
        fileURLToPath(new URL("../accept.mjs", import.meta.url)),
        "--version",
      ]),
    );
  }
  const median = (values) => values.sort((a, b) => a - b)[2];
  assert.ok(
    median(versions) <= median(floors) * 1.8 + 20,
    "version probe loaded work beyond the Node startup floor",
  );
});
test("service launch preflight rejects placeholder tokens without exposing them", async (t) => {
  const shell =
    process.platform === "win32"
      ? "C:/Program Files/Git/usr/bin/sh.exe"
      : "/bin/sh";
  const launch = fileURLToPath(
    new URL("../ion-speech-launch.sh", import.meta.url),
  );
  try {
    assert.equal((await childResult(shell, ["-n", launch])).code, 0);
  } catch (error) {
    if (error.code === "ENOENT") {
      t.skip("POSIX shell unavailable");
      return;
    }
    throw error;
  }
  for (const value of [
    "replace-with-a-random-service-token",
    "short",
    `private${"a".repeat(32)}\r\nInjected: yes`,
    "a".repeat(513),
  ]) {
    const result = await childResult(shell, [launch, "--check"], {
      NEMO_SPEECH_HTTP_API_KEY: value,
    });
    assert.equal(result.code, 1);
    assert.ok(!result.stdout.includes(value));
    assert.equal(result.stderr, "");
  }
  const ready = await childResult(shell, [launch, "--check"], {
    NEMO_SPEECH_HTTP_API_KEY: TOKEN,
    ION_SPEECH_BINARY: process.platform === "win32" ? "/bin/sh" : "/bin/sh",
    ION_SPEECH_MODEL_PATH: fileURLToPath(
      new URL("../README.md", import.meta.url),
    ),
  });
  assert.equal(ready.code, 1);
  assert.match(ready.stdout, /GGUF/);
  assert.ok(!ready.stdout.includes(TOKEN));
  assert.equal(
    (
      await childResult(shell, [launch, "--unknown"], {
        NEMO_SPEECH_HTTP_API_KEY: TOKEN,
      })
    ).code,
    1,
  );
});

test("global help/version never silently ignores unknown arguments", async () => {
  let output = "";
  assert.equal(
    await main(["--help", "--private-token", TOKEN], {
      write: (text) => {
        output += text;
      },
    }),
    2,
  );
  assert.ok(!output.includes(TOKEN));
  output = "";
  assert.equal(
    await main(["check", "--version"], {
      write: (text) => {
        output += text;
      },
    }),
    0,
  );
  assert.equal(output, "0.1.0");
});
test("an exact inventory selector cannot validate a different client model", async () => {
  await rejectsCheck("ok", "model", { model: "different-model" });
});
test("socket fragmented-message and pending-event bounds reject unbounded peers", async () => {
  await rejectsCheck("fragment_limit", "socket_transport");
  await service("event_flood", async (url) => {
    const channel = new SpeechChannel(
      url,
      TOKEN,
      new AbortController().signal,
      400,
    );
    try {
      await new Promise((resolve) => setTimeout(resolve, 50));
      await assert.rejects(channel.next(400), { code: "event_limit" });
    } finally {
      await channel.shutdown();
    }
  });
});
test("HTTP and socket TLS verification reject an untrusted self-signed service before auth", async (t) => {
  const openssl =
    process.platform === "win32"
      ? "C:/Program Files/Git/usr/bin/openssl.exe"
      : "openssl";
  await fixtures(async (files) => {
    const key = join(dirname(files.wav), "fixture.key"),
      cert = join(dirname(files.wav), "fixture.crt");
    try {
      const generated = await childResult(openssl, [
        "req",
        "-x509",
        "-newkey",
        "rsa:2048",
        "-nodes",
        "-days",
        "1",
        "-subj",
        "/CN=localhost",
        "-keyout",
        key,
        "-out",
        cert,
      ]);
      if (generated.code !== 0) {
        t.skip("Local certificate generator unavailable");
        return;
      }
    } catch (error) {
      if (error.code === "ENOENT") {
        t.skip("Local certificate generator unavailable");
        return;
      }
      throw error;
    }
    let requests = 0;
    const server = https.createServer(
      { key: await readFile(key), cert: await readFile(cert) },
      (_, response) => {
        requests++;
        response.end("private body");
      },
    );
    await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
    const url = `https://127.0.0.1:${server.address().port}`;
    try {
      await assert.rejects(
        getJson(url, "/v1/models", TOKEN, new AbortController().signal, 400),
        { code: "network" },
      );
      const channel = new SpeechChannel(
        url,
        TOKEN,
        new AbortController().signal,
        400,
      );
      try {
        await assert.rejects(channel.next(400), { code: "socket_transport" });
      } finally {
        await channel.shutdown();
      }
      assert.equal(requests, 0);
    } finally {
      server.closeAllConnections();
      await new Promise((resolve) => server.close(resolve));
    }
  });
});

test("WAV identifiers do not treat high-bit bytes as ASCII aliases", () => {
  const bytes = wave();
  bytes[0] |= 0x80;
  assert.throws(() => parseWav(bytes, 60), { code: "fixture" });
});
test("invalid expected UTF8 is refused before network traffic", async () => {
  await fixtures((files) =>
    service("ok", async (url, state) => {
      await writeFile(files.expected, Buffer.from([0xff]));
      await assert.rejects(
        runChecks(
          config(url, { command: "accept", ...files, maxAudioSeconds: 60 }),
          TOKEN,
        ),
        { code: "fixture" },
      );
      assert.equal(state.requests.length, 0);
    }),
  );
});

test("canonical served-model name satisfies the actual desktop and phone inventory predicate", async () => {
  await service("canonical_model", async (url) => {
    assert.equal((await runChecks(config(url), TOKEN)).status, "passed");
    const inventory = await getJson(
      url,
      "/v1/models",
      TOKEN,
      new AbortController().signal,
      400,
    );
    const id = inventory.data.data[0].id;
    assert.ok(id.includes("nemotron-3.5") || id.includes("nemotron-3_5"));
    assert.match(id, /nemotron-3[._]5/);
  });
  await rejectsCheck("uppercase_model", "model");
});
test("launcher pins the supported canonical API name and verifies model SHA before serving", async (t) => {
  const shell =
    process.platform === "win32"
      ? "C:/Program Files/Git/usr/bin/sh.exe"
      : "/bin/sh";
  try {
    await childResult(shell, ["-c", "command -v sha256sum >/dev/null"]);
  } catch (error) {
    if (error.code === "ENOENT") {
      t.skip("POSIX shell unavailable");
      return;
    }
    throw error;
  }
  const launch = fileURLToPath(
    new URL("../ion-speech-launch.sh", import.meta.url),
  );
  const posixPath = (value) =>
    process.platform === "win32"
      ? `/${value[0].toLowerCase()}/${value.slice(3).replaceAll("\\", "/")}`
      : value;
  await fixtures(async (files) => {
    const model = join(dirname(files.wav), "selected-model.gguf"),
      binary = join(dirname(files.wav), "native-stub.sh");
    const bytes = Buffer.from(
      "GGUF private synthetic artifact for preflight only",
    );
    await writeFile(model, bytes);
    await writeFile(binary, '#!/bin/sh\nprintf "%s\\n" "$@"\n');
    const permissions = await childResult(shell, [
      "-c",
      '/usr/bin/chmod +x "$1"',
      "speech-test",
      posixPath(binary),
    ]);
    assert.equal(permissions.code, 0);
    const expected = createHash("sha256").update(bytes).digest("hex");
    const env = {
      NEMO_SPEECH_HTTP_API_KEY: TOKEN,
      ...(process.platform === "win32" ? { PATH: "/usr/bin:/bin" } : {}),
      ION_SPEECH_BINARY: posixPath(binary),
      ION_SPEECH_MODEL_PATH: posixPath(model),
      ION_SPEECH_MODEL_SHA256: expected.toUpperCase(),
    };
    const prepared = await childResult(shell, [launch, "--check"], env);
    assert.equal(prepared.code, 0);
    assert.match(prepared.stdout, /model_sha256,passed/);
    assert.ok(!prepared.stdout.includes(expected));
    assert.ok(!prepared.stdout.includes(TOKEN));
    assert.ok(!prepared.stdout.includes(posixPath(model)));
    const served = await childResult(shell, [launch], env);
    assert.equal(served.code, 0);
    const args = served.stdout.trim().split("\n");
    assert.equal(args[args.indexOf("--asr-model") + 1], posixPath(model));
    assert.equal(args[args.indexOf("--asr.model.name") + 1], "nemotron-3.5");
    assert.equal(args[args.indexOf("--host") + 1], "127.0.0.1");
    assert.ok(!args.includes(TOKEN));
    for (const hash of ["", "a".repeat(64), "z".repeat(64)]) {
      const refused = await childResult(shell, [launch], {
        ...env,
        ION_SPEECH_MODEL_SHA256: hash,
      });
      assert.equal(refused.code, 1);
      assert.ok(!refused.stdout.includes("--asr-model"));
    }
    await writeFile(model, Buffer.from("changed artifact"));
    assert.equal((await childResult(shell, [launch], env)).code, 1);
  });
});
