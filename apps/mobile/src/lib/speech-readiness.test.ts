import { expect, test } from "bun:test";
import { verifySpeechService } from "./speech-readiness";
import type { SpeechSocket } from "./speech-stream";
const endpoint = {
  baseUrl: "https://speech.example",
  token: "private-test-only",
};
class Socket implements SpeechSocket {
  readyState = 1;
  bufferedAmount = 0;
  onopen: (() => void) | null = null;
  onmessage: ((event: { data: unknown }) => void) | null = null;
  onerror: (() => void) | null = null;
  onclose: (() => void) | null = null;
  sent: unknown[] = [];
  closes = 0;
  send(data: string | ArrayBuffer) {
    this.sent.push(data);
  }
  close() {
    this.closes++;
    this.readyState = 3;
  }
  event(type: string) {
    this.onmessage?.({ data: JSON.stringify({ type }) });
  }
}
const inventory = () =>
  new Response(
    JSON.stringify({
      data: [{ id: "nvidia/nemotron-3.5-asr-streaming-0.6b" }],
    }),
  );
const tick = () => new Promise<void>((resolve) => setTimeout(resolve, 0));
test("inventory alone cannot pass; format acknowledgment passes without audio or commit", async () => {
  const socket = new Socket(),
    addresses: unknown[] = [];
  let settled = false;
  const result = verifySpeechService(endpoint, {
    fetch: async (url, options) => {
      addresses.push([url, options]);
      return inventory();
    },
    socket: (url, headers) => {
      addresses.push([url, headers]);
      return socket;
    },
  });
  void result.then(() => {
    settled = true;
  });
  await tick();
  expect(settled).toBe(false);
  socket.event("session.created");
  await tick();
  expect(settled).toBe(false);
  expect(socket.sent.map((data) => JSON.parse(data as string))).toEqual([
    {
      type: "session.update",
      session: { sample_rate: 16000, language: "auto" },
    },
  ]);
  socket.event("session.updated");
  await result;
  expect(socket.closes).toBe(1);
  expect(socket.sent).toHaveLength(1);
  expect(addresses[1]).toEqual([
    "wss://speech.example/v1/audio/transcriptions/realtime",
    { Authorization: "Bearer private-test-only" },
  ]);
});
test("a failed streaming route cannot be reported as connected", async () => {
  const socket = new Socket();
  const result = verifySpeechService(endpoint, {
    fetch: async () => inventory(),
    socket: () => socket,
  });
  const rejected = result.catch((error) => error as Error);
  await tick();
  socket.onerror?.();
  expect(((await rejected) as Error).message).toContain(
    "streaming connection failed",
  );
  expect(socket.closes).toBe(1);
});
test("cancel while awaiting format acknowledgment closes once without saving or sending audio", async () => {
  const socket = new Socket(),
    controller = new AbortController();
  const result = verifySpeechService(
    endpoint,
    { fetch: async () => inventory(), socket: () => socket },
    controller.signal,
  );
  const rejected = result.catch((error) => error as Error);
  await tick();
  socket.event("session.created");
  await tick();
  controller.abort();
  expect(((await rejected) as Error).message).toContain("canceled");
  expect(socket.closes).toBe(1);
  expect(socket.sent).toHaveLength(1);
  expect(socket.onmessage).toBeNull();
});
test("a pre-canceled check makes no requests", async () => {
  const controller = new AbortController();
  controller.abort();
  let requests = 0;
  await expect(
    verifySpeechService(
      endpoint,
      {
        fetch: async () => {
          requests++;
          return inventory();
        },
        socket: () => {
          requests++;
          return new Socket();
        },
      },
      controller.signal,
    ),
  ).rejects.toThrow();
  expect(requests).toBe(0);
});
test("bounded inventory rejects oversized, malformed, missing and wrong models before streaming", async () => {
  for (const response of [
    new Response("x".repeat(65_537)),
    new Response("{"),
    new Response("{}"),
    new Response(JSON.stringify({ data: [{ id: "other-model" }] })),
    new Response("denied", { status: 401 }),
  ]) {
    let sockets = 0;
    await expect(
      verifySpeechService(endpoint, {
        fetch: async () => response,
        socket: () => {
          sockets++;
          return new Socket();
        },
      }),
    ).rejects.toThrow();
    expect(sockets).toBe(0);
  }
});
test("early format acknowledgment and provider diagnostics are rejected without exposing content", async () => {
  const socket = new Socket();
  const result = verifySpeechService(endpoint, {
    fetch: async () => inventory(),
    socket: () => socket,
  });
  const rejected = result.catch((error) => error as Error);
  await tick();
  socket.event("session.updated");
  expect(((await rejected) as Error).message).toContain(
    "Invalid or incomplete",
  );
  expect(socket.closes).toBe(1);
});
