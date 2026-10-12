import { randomBytes } from "node:crypto";
import { canceled, failure } from "./config.mjs";
import { loadFixture, wordErrorRate } from "./wav.mjs";
import { delay, getJson, SpeechChannel } from "./transport.mjs";

const partialType = "conversation.item.input_audio_transcription.delta";
const finalType = "conversation.item.input_audio_transcription.completed";
function record(value) {
  return value && !Array.isArray(value) && typeof value === "object";
}
function protocol(message) {
  return failure("protocol", message);
}
function checkEventError(event) {
  if (event.type === "error")
    throw failure(
      "server_rejected",
      "Speech service rejected the session; inspect sanitized local service diagnostics.",
    );
}
async function negotiate(channel, config, model, rate) {
  const created = await channel.next(config.handshakeMs);
  checkEventError(created);
  if (
    created.type !== "session.created" ||
    !record(created.session) ||
    created.session.input_audio_format !== "pcm16" ||
    created.session.model !== model ||
    !Number.isInteger(created.session.sample_rate) ||
    created.session.sample_rate < 8000 ||
    created.session.sample_rate > 96000
  )
    throw protocol(
      "Speech session did not advertise the selected model and valid PCM16 input.",
    );
  await channel.send(
    JSON.stringify({
      type: "session.update",
      session: { sample_rate: rate, language: config.language },
    }),
  );
  const updated = await channel.next(config.handshakeMs);
  checkEventError(updated);
  if (
    updated.type !== "session.updated" ||
    !record(updated.session) ||
    updated.session.sample_rate !== rate ||
    updated.session.language !== config.language
  )
    throw protocol(
      "Speech service did not acknowledge the requested sample rate and language.",
    );
}
async function checkSocketAuth(config, token, signal) {
  const channel = new SpeechChannel(
    config.url,
    token,
    signal,
    config.handshakeMs,
  );
  try {
    try {
      await channel.next(config.handshakeMs);
    } catch (error) {
      if (error.code === "auth_denied") return;
      throw error;
    }
    throw failure(
      "auth_exposed",
      "Speech realtime route admitted a missing or incorrect token.",
    );
  } finally {
    await channel.shutdown();
  }
}

async function stream(channel, config, fixture, signal, index) {
  let audioStarted = false,
    commitSent = false,
    sentBytes = 0,
    partial = "",
    transcript = "";
  let partials = 0,
    finals = 0,
    commitAt = 0,
    finalizeTimer,
    finalizationExpired = false;
  const startedAt = performance.now();
  const receive = (async () => {
    for (;;) {
      const event = await channel.next(config.timeoutMs);
      checkEventError(event);
      if (!audioStarted)
        throw protocol(
          "Speech service emitted transcription before audio started.",
        );
      if (event.type === partialType) {
        if (
          typeof event.delta !== "string" ||
          partial.length + transcript.length + event.delta.length > 16_384
        )
          throw protocol(
            "Speech preview exceeded its limit or has invalid content.",
          );
        partial += event.delta;
        if (event.delta.trim()) partials++;
      } else if (event.type === finalType) {
        if (
          typeof event.transcript !== "string" ||
          transcript.length + event.transcript.length + 1 > 16_384
        )
          throw protocol(
            "Speech final text exceeded its limit or has invalid content.",
          );
        const text = event.transcript.trim();
        if (text) {
          transcript += `${transcript ? " " : ""}${text}`;
          finals++;
        }
        partial = "";
      } else if (event.type === "input_audio_buffer.committed") {
        if (
          !commitSent ||
          sentBytes !== fixture.pcm.length ||
          partial ||
          !finals ||
          !partials
        )
          throw protocol(
            "Speech acknowledgment arrived before complete final text and a partial preview.",
          );
        const wer = wordErrorRate(fixture.expected, transcript);
        if (wer > config.maxWer)
          throw failure(
            "recognition_mismatch",
            "Speech recognition exceeded --max-wer; compare the private fixture and expected text locally.",
          );
        return {
          client: index + 1,
          status: "passed",
          audio_bytes: sentBytes,
          partials,
          finals,
          commit_ms: Math.max(0, Math.round(performance.now() - commitAt)),
          elapsed_ms: Math.max(0, Math.round(performance.now() - startedAt)),
          wer: Number(wer.toFixed(4)),
        };
      } else
        throw protocol(
          "Speech service emitted an unexpected event during recognition.",
        );
    }
  })();
  // A rejected receive promise must be observed immediately while the sender is pacing audio.
  let receiveError;
  receive.catch((error) => {
    receiveError = error;
    channel.fail(error);
  });
  try {
    const chunkBytes = Math.max(
      2,
      Math.floor((fixture.sampleRate * config.chunkMs) / 1000) * 2,
    );
    for (let offset = 0; offset < fixture.pcm.length; offset += chunkBytes) {
      canceled(signal);
      if (receiveError) throw receiveError;
      await delay(
        startedAt +
          (offset / (2 * fixture.sampleRate)) * 1000 -
          performance.now(),
        signal,
      );
      audioStarted = true;
      const chunk = fixture.pcm.subarray(
        offset,
        Math.min(fixture.pcm.length, offset + chunkBytes),
      );
      await channel.send(chunk);
      sentBytes += chunk.length;
    }
    await delay(
      startedAt + fixture.durationSeconds * 1000 - performance.now(),
      signal,
    );
    if (receiveError) throw receiveError;
    commitSent = true;
    commitAt = performance.now();
    finalizeTimer = setTimeout(() => {
      finalizationExpired = true;
      channel.fail(
        failure(
          "finalization_deadline",
          "Speech finalization exceeded --finalize-seconds; no partial transcript is accepted.",
        ),
      );
    }, config.finalizeMs);
    await channel.send(JSON.stringify({ type: "input_audio_buffer.commit" }));
    return await receive;
  } catch (error) {
    channel.fail(error);
    await receive.catch(() => {});
    if (finalizationExpired)
      throw failure(
        "finalization_deadline",
        "Speech finalization exceeded --finalize-seconds; no partial transcript is accepted.",
      );
    throw error;
  } finally {
    clearTimeout(finalizeTimer);
  }
}
async function recognize(config, token, model, fixture, signal) {
  const channels = Array.from(
    { length: config.clients },
    () => new SpeechChannel(config.url, token, signal, config.handshakeMs),
  );
  try {
    // All connections acknowledge the real fixture format before any client sends PCM.
    await Promise.all(
      channels.map((channel) =>
        negotiate(channel, config, model, fixture.sampleRate),
      ),
    );
    const results = channels.map((channel, index) =>
      stream(channel, config, fixture, signal, index),
    );
    try {
      return await Promise.all(results);
    } catch (error) {
      for (const channel of channels) channel.fail(error);
      await Promise.allSettled(results);
      throw error;
    }
  } finally {
    await Promise.all(channels.map((channel) => channel.shutdown()));
  }
}
async function cancelPrefix(config, token, model, fixture, signal) {
  const channel = new SpeechChannel(
    config.url,
    token,
    signal,
    config.handshakeMs,
  );
  try {
    await negotiate(channel, config, model, fixture.sampleRate);
    await channel.send(
      fixture.pcm.subarray(0, Math.floor(fixture.sampleRate / 10) * 2),
    );
    await channel.send(JSON.stringify({ type: "response.cancel" }));
    for (let i = 0; i < 32; i++) {
      const event = await channel.next(config.handshakeMs);
      checkEventError(event);
      if (event.type === "input_audio_buffer.cleared") return;
      if (event.type !== partialType && event.type !== finalType)
        throw protocol("Speech Cancel returned an unexpected event.");
    }
    throw failure(
      "cancel_limit",
      "Speech Cancel did not acknowledge clearing within its event bound.",
    );
  } finally {
    await channel.shutdown();
  }
}

export async function runChecks(
  config,
  token,
  { signal, onCheck = () => {} } = {},
) {
  const controller = new AbortController();
  const abort = () => controller.abort();
  signal?.addEventListener("abort", abort, { once: true });
  if (signal?.aborted) abort();
  let expired = false;
  const timer = setTimeout(() => {
    expired = true;
    abort();
  }, config.timeoutMs);
  const checks = [];
  const pass = (name) => {
    const value = { check: name, status: "passed" };
    checks.push(value);
    onCheck(value);
  };
  try {
    canceled(controller.signal);
    const fixture =
      config.command === "accept"
        ? await loadFixture(config, controller.signal)
        : null;
    const ready = await getJson(
      config.url,
      "/ready",
      null,
      controller.signal,
      config.handshakeMs,
    );
    if (
      ready.status !== 200 ||
      !record(ready.data) ||
      ready.data.ready !== true
    )
      throw failure(
        "not_ready",
        "Speech service is not ready; inspect its model setup and resource limits.",
      );
    pass("readiness");
    const inventory = await getJson(
      config.url,
      "/v1/models",
      token,
      controller.signal,
      config.handshakeMs,
    );
    if (
      inventory.status !== 200 ||
      !record(inventory.data) ||
      !Array.isArray(inventory.data.data) ||
      inventory.data.data.length > 64
    )
      throw failure(
        "inventory",
        "Authenticated model inventory is unavailable or invalid.",
      );
    const models = inventory.data.data.filter(
      (item) =>
        record(item) &&
        typeof item.id === "string" &&
        item.id.length <= 256 &&
        item.capability === "transcription" &&
        /nemotron-3[._]5/i.test(item.id) &&
        (!config.model || item.id === config.model),
    );
    if (models.length !== 1)
      throw failure(
        "model",
        "Load exactly one Nemotron 3.5 transcription model, or select its exact inventory ID with --model.",
      );
    const model = models[0].id;
    pass("model_inventory");
    const wrongToken = randomBytes(32).toString("hex");
    for (const supplied of [null, wrongToken]) {
      const refusal = await getJson(
        config.url,
        "/v1/models",
        supplied,
        controller.signal,
        config.handshakeMs,
      );
      if (refusal.status !== 401 && refusal.status !== 403)
        throw failure(
          "auth_exposed",
          "Speech API did not explicitly reject missing and incorrect tokens.",
        );
    }
    pass("http_auth_refusal");
    const channel = new SpeechChannel(
      config.url,
      token,
      controller.signal,
      config.handshakeMs,
    );
    try {
      await negotiate(channel, config, model, fixture?.sampleRate ?? 16_000);
    } finally {
      await channel.shutdown();
    }
    pass("pcm16_negotiation");
    for (const supplied of [null, wrongToken])
      await checkSocketAuth(config, supplied, controller.signal);
    pass("socket_auth_refusal");
    let clients = [];
    if (fixture) {
      clients = await recognize(
        config,
        token,
        model,
        fixture,
        controller.signal,
      );
      pass(config.clients > 1 ? "concurrent_recognition" : "recognition");
      await cancelPrefix(config, token, model, fixture, controller.signal);
      pass("cancel_acknowledgment");
    }
    canceled(controller.signal);
    return {
      status: "passed",
      scope: fixture ? "fixture_acceptance" : "readiness_only",
      checks,
      clients,
    };
  } catch (error) {
    if (expired)
      throw failure(
        "deadline",
        "Speech check exceeded --timeout-seconds; no result is accepted.",
      );
    throw error;
  } finally {
    clearTimeout(timer);
    signal?.removeEventListener("abort", abort);
    controller.abort();
  }
}
