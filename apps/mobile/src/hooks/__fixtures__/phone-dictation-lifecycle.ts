// Runs in a separate Bun process: native module doubles must not leak into other tests.
import assert from "node:assert/strict";
import { mock } from "bun:test";

type Effect = { deps: unknown[]; cleanup?: () => void };
const slots: any[] = [];
let cursor = 0,
  queued = false,
  disposed = false;
let effects: Array<() => void> = [];
let client: any = { connected: true },
  context = "task-one";
let result: any;
const transcripts: string[] = [];
const previews: unknown[] = [];
let permission = true,
  stopFails = false,
  streamStops = 0,
  transfers = 0;
let modeRecording = false,
  startHook: (() => Promise<void>) | undefined;
let transcribeHook: (() => Promise<string>) | undefined;
let onBuffer: (buffer: any) => void;
const statuses = new Set<(value: any) => void>();
const appStates = new Set<(value: string) => void>();
const signOuts = new Set<() => void>();
const schedule = () => {
  if (queued || disposed) return;
  queued = true;
  queueMicrotask(() => {
    queued = false;
    if (!disposed) render();
  });
};
mock.module("react", () => ({
  useRef(value: unknown) {
    const index = cursor++;
    return (slots[index] ??= { current: value });
  },
  useState(value: unknown) {
    const index = cursor++;
    if (!(index in slots)) slots[index] = value;
    return [
      slots[index],
      (next: unknown) => {
        slots[index] = next;
        schedule();
      },
    ];
  },
  useEffect(run: () => (() => void) | void, deps: unknown[]) {
    const index = cursor++,
      old = slots[index] as Effect | undefined;
    if (
      old &&
      old.deps.length === deps.length &&
      deps.every((value, i) => Object.is(value, old.deps[i]))
    )
      return;
    effects.push(() => {
      old?.cleanup?.();
      slots[index] = { deps, cleanup: run() };
    });
  },
}));
const stream = {
  isStreaming: false,
  async start() {
    if (startHook) await startHook();
    this.isStreaming = true;
    statuses.forEach((next) => next({ isStreaming: true }));
  },
  stop() {
    streamStops++;
    if (stopFails) throw new Error("Native microphone stop failed");
    this.isStreaming = false;
    statuses.forEach((next) => next({ isStreaming: false }));
  },
  addListener(_name: string, next: (value: any) => void) {
    statuses.add(next);
    return {
      remove() {
        statuses.delete(next);
      },
    };
  },
};
mock.module("expo-audio", () => ({
  requestRecordingPermissionsAsync: async () => ({ granted: permission }),
  setAudioModeAsync: async (mode: { allowsRecording: boolean }) => {
    modeRecording = mode.allowsRecording;
  },
  useAudioStream(options: { onBuffer: typeof onBuffer }) {
    onBuffer = options.onBuffer;
    return { stream, isStreaming: stream.isStreaming };
  },
}));
mock.module("react-native", () => ({
  Platform: { OS: "android" },
  AppState: {
    addEventListener(_name: string, next: (value: string) => void) {
      appStates.add(next);
      return {
        remove() {
          appStates.delete(next);
        },
      };
    },
  },
}));
mock.module("expo-crypto", () => ({ randomUUID: () => "fixture" }));
mock.module("expo-file-system", () => ({
  Paths: { cache: "fixture-cache" },
  File: class {
    create() {}
    write(_bytes: Uint8Array) {}
    async base64() {
      return "fixture-wav";
    }
    delete() {}
  },
}));
mock.module("../../lib/cloud-account", () => ({
  cloudAccount: {
    onSignOut(next: () => void) {
      signOuts.add(next);
      return () => {
        signOuts.delete(next);
      };
    },
  },
}));
mock.module("../../lib/daemon-api", () => ({
  transcribePhoneWav: async () => {
    transfers++;
    return transcribeHook ? transcribeHook() : "Fixture transcript";
  },
}));
const { usePhoneDictation } = await import("../use-phone-dictation");
function render() {
  cursor = 0;
  result = usePhoneDictation(
    client,
    context,
    (text) => transcripts.push(text),
    (text) => previews.push(text),
  );
  const pending = effects;
  effects = [];
  pending.forEach((run) => run());
}
async function settle() {
  for (let i = 0; i < 12; i++) await Promise.resolve();
}
function pcm() {
  onBuffer({
    channels: 1,
    sampleRate: 16000,
    data: new Uint8Array([1, 0, 2, 0]).buffer,
  });
}
let checks = 0;
try {
  render();
  await result.start();
  await settle();
  assert.equal(
    result.phase,
    "recording",
    "Successful start must remain recording after rerender",
  );
  pcm();
  await result.stop();
  await settle();
  assert.equal(result.phase, "idle");
  assert.deepEqual(transcripts, ["Fixture transcript"]);
  assert.equal(transfers, 1);
  checks++;

  await result.start();
  await settle();
  pcm();
  result.discard();
  await settle();
  assert.equal(result.phase, "idle");
  assert.equal(transfers, 1);
  assert.equal(modeRecording, false);
  checks++;

  permission = false;
  await assert.rejects(result.start(), /permission/);
  await settle();
  assert.equal(result.phase, "idle");
  assert.match(result.error, /permission/);
  checks++;
  permission = true;

  await result.start();
  await settle();
  appStates.forEach((next) => next("background"));
  await settle();
  assert.equal(result.phase, "idle");
  assert.equal(modeRecording, false);
  assert.equal(transfers, 1);
  checks++;

  await result.start();
  await settle();
  context = "task-two";
  render();
  await settle();
  assert.equal(result.phase, "idle");
  assert.equal(transfers, 1);
  checks++;

  let releaseStart!: () => void;
  startHook = () =>
    new Promise<void>((resolve) => {
      releaseStart = resolve;
    });
  const starting = result.start();
  await settle();
  assert.equal(result.phase, "preparing");
  signOuts.forEach((next) => next());
  await settle();
  releaseStart();
  await starting;
  await settle();
  assert.equal(result.phase, "idle");
  assert.equal(stream.isStreaming, false);
  assert.equal(transfers, 1);
  checks++;
  startHook = undefined;

  startHook = async () => {
    throw new Error("Native microphone startup failed");
  };
  await assert.rejects(result.start(), /startup failed/);
  await settle();
  assert.equal(result.phase, "idle");
  assert.match(result.error, /startup failed/);
  assert.equal(modeRecording, false);
  assert.equal(transfers, 1);
  checks++;
  startHook = undefined;

  let completeTranscript!: (text: string) => void;
  transcribeHook = () =>
    new Promise<string>((resolve) => {
      completeTranscript = resolve;
    });
  await result.start();
  await settle();
  pcm();
  const finalizing = result.stop();
  await settle();
  assert.equal(result.phase, "transcribing");
  context = "task-three";
  render();
  await settle();
  completeTranscript("Must not enter task three");
  await finalizing;
  await settle();
  assert.equal(result.phase, "idle");
  assert.deepEqual(transcripts, ["Fixture transcript"]);
  transcribeHook = undefined;
  checks++;

  await result.start();
  await settle();
  pcm();
  stopFails = true;
  await assert.rejects(result.stop(), /Microphone could not stop/);
  await settle();
  assert.equal(
    result.phase,
    "idle",
    "Native stop failure must not strand the hook in recording",
  );
  assert.equal(
    modeRecording,
    false,
    "Native stop failure must release shared recording mode",
  );
  assert.equal(
    transfers,
    2,
    "Native stop failure must never upload unfinalized audio",
  );
  assert.match(result.error, /Close and reopen/);
  await assert.rejects(result.start(), /still active/);
  checks++;

  stopFails = false;
  stream.stop();
  await result.start();
  await settle();
  stopFails = true;
  assert.doesNotThrow(() => result.discard());
  await settle();
  assert.equal(result.phase, "idle");
  assert.equal(modeRecording, false);
  assert.match(result.error, /Microphone could not stop/);
  assert.equal(transfers, 2);
  checks++;
  console.log(
    JSON.stringify({ checks, streamStops, transfers, microphoneUsed: false }),
  );
} finally {
  stopFails = false;
  result?.discard();
  await settle();
  disposed = true;
  for (const slot of slots) slot?.cleanup?.();
  await settle();
}
