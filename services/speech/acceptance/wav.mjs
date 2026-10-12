import { open } from "node:fs/promises";
import { AcceptanceError, canceled, failure, FILE_LIMIT } from "./config.mjs";

export async function privateFile(path, limit, signal, label) {
  canceled(signal);
  let file;
  try {
    file = await open(path, "r");
  } catch {
    throw failure(
      "fixture",
      `Cannot open the ${label} file; check its local path and access.`,
    );
  }
  try {
    const metadata = await file.stat();
    if (!metadata.isFile() || metadata.size < 1 || metadata.size > limit)
      throw failure(
        "fixture",
        `${label} must be a regular nonempty file within its size limit.`,
      );
    const bytes = Buffer.alloc(metadata.size);
    let offset = 0;
    while (offset < bytes.length) {
      canceled(signal);
      const { bytesRead } = await file.read(
        bytes,
        offset,
        bytes.length - offset,
        offset,
      );
      if (!bytesRead)
        throw failure(
          "fixture",
          `${label} changed while being read; select a stable fixture.`,
        );
      offset += bytesRead;
    }
    const extra = Buffer.alloc(1);
    if ((await file.read(extra, 0, 1, offset)).bytesRead)
      throw failure(
        "fixture",
        `${label} changed while being read; select a stable fixture.`,
      );
    canceled(signal);
    return bytes;
  } finally {
    await file.close();
  }
}
export function parseWav(bytes, maxSeconds) {
  const fail = () => {
    throw failure(
      "fixture",
      "WAV must be uncompressed mono PCM16 RIFF/WAVE at 8000..96000 Hz, 2 seconds to the selected audio limit.",
    );
  };
  if (
    bytes.length < 44 ||
    bytes.length > FILE_LIMIT ||
    bytes.toString("latin1", 0, 4) !== "RIFF" ||
    bytes.toString("latin1", 8, 12) !== "WAVE" ||
    bytes.readUInt32LE(4) !== bytes.length - 8
  )
    fail();
  let format,
    pcm,
    offset = 12,
    chunks = 0;
  while (offset < bytes.length) {
    if (++chunks > 64 || offset + 8 > bytes.length) fail();
    const id = bytes.toString("latin1", offset, offset + 4);
    const length = bytes.readUInt32LE(offset + 4);
    offset += 8;
    if (offset + length + (length % 2) > bytes.length) fail();
    if (id === "fmt ") {
      if (format || length < 16 || length > 64) fail();
      format = {
        code: bytes.readUInt16LE(offset),
        channels: bytes.readUInt16LE(offset + 2),
        sampleRate: bytes.readUInt32LE(offset + 4),
        byteRate: bytes.readUInt32LE(offset + 8),
        blockAlign: bytes.readUInt16LE(offset + 12),
        bits: bytes.readUInt16LE(offset + 14),
      };
    } else if (id === "data") {
      if (pcm) fail();
      pcm = bytes.subarray(offset, offset + length);
    }
    offset += length + (length % 2);
  }
  if (
    !format ||
    !pcm ||
    !pcm.length ||
    pcm.length % 2 ||
    format.code !== 1 ||
    format.channels !== 1 ||
    format.bits !== 16 ||
    format.blockAlign !== 2 ||
    format.sampleRate < 8000 ||
    format.sampleRate > 96000 ||
    format.byteRate !== 2 * format.sampleRate
  )
    fail();
  const durationSeconds = pcm.length / (2 * format.sampleRate);
  if (durationSeconds < 2 || durationSeconds > maxSeconds) fail();
  return { pcm, sampleRate: format.sampleRate, durationSeconds };
}
export function words(text) {
  return text
    .normalize("NFKC")
    .toLowerCase()
    .split(/[^\p{L}\p{N}\p{M}]+/u)
    .filter(Boolean);
}
export async function loadFixture(config, signal) {
  const wav = parseWav(
    await privateFile(config.wav, FILE_LIMIT, signal, "WAV"),
    config.maxAudioSeconds,
  );
  let text;
  try {
    text = new TextDecoder("utf-8", { fatal: true }).decode(
      await privateFile(config.expected, 4096, signal, "expected text"),
    );
  } catch (error) {
    if (error instanceof AcceptanceError) throw error;
    throw failure("fixture", "Expected text must be valid UTF8.");
  }
  const expected = words(text);
  if (!expected.length || expected.length > 512)
    throw failure(
      "fixture",
      "Expected text must contain 1..512 words within 4096 UTF8 bytes.",
    );
  return { ...wav, expected };
}
export function wordErrorRate(expected, actual) {
  const hypothesis = words(actual);
  if (hypothesis.length > 2048)
    throw failure(
      "transcript_limit",
      "Transcript exceeds the bounded recognition check.",
    );
  let previous = Array.from({ length: hypothesis.length + 1 }, (_, i) => i);
  for (let i = 1; i <= expected.length; i++) {
    const current = [i];
    for (let j = 1; j <= hypothesis.length; j++)
      current[j] = Math.min(
        previous[j] + 1,
        current[j - 1] + 1,
        previous[j - 1] + (expected[i - 1] === hypothesis[j - 1] ? 0 : 1),
      );
    previous = current;
  }
  return previous[hypothesis.length] / expected.length;
}
