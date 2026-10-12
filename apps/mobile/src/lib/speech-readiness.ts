import { speechToken, speechUrl, type SpeechEndpoint } from "./speech-endpoint";
import { CloudSpeechStream, type SpeechSocketFactory } from "./speech-stream";

type Transport = {
  fetch: (
    url: string,
    options: RequestInit,
  ) => Promise<Pick<Response, "ok" | "status" | "body">>;
  socket: SpeechSocketFactory;
};
const INVENTORY_LIMIT = 65_536;

/** Explicit check: bounded HTTP inventory and streaming format, with no microphone/audio. */
export async function verifySpeechService(
  endpoint: SpeechEndpoint,
  transport: Transport,
  signal?: AbortSignal,
): Promise<void> {
  const checked = {
    baseUrl: speechUrl(endpoint.baseUrl),
    token: speechToken(endpoint.token),
  };
  const controller = new AbortController();
  const abort = () => controller.abort();
  signal?.addEventListener("abort", abort);
  if (signal?.aborted) controller.abort();
  const timer = setTimeout(abort, 35_000);
  try {
    controller.signal.throwIfAborted();
    const response = await transport.fetch(`${checked.baseUrl}/v1/models`, {
      headers: { Authorization: `Bearer ${checked.token}` },
      redirect: "error",
      signal: controller.signal,
    });
    if (!response.ok)
      throw new Error(
        `Speech service connection failed (HTTP ${response.status})`,
      );
    if (!response.body) throw new Error("Invalid speech model inventory");
    const reader = response.body.getReader();
    const chunks: Uint8Array[] = [];
    let size = 0;
    try {
      for (;;) {
        controller.signal.throwIfAborted();
        const chunk = await reader.read();
        if (chunk.done) break;
        size += chunk.value.byteLength;
        if (size > INVENTORY_LIMIT)
          throw new Error("Speech model inventory is too large");
        chunks.push(chunk.value);
      }
    } finally {
      await reader.cancel().catch(() => {});
      reader.releaseLock();
    }
    const bytes = new Uint8Array(size);
    let offset = 0;
    for (const chunk of chunks) {
      bytes.set(chunk, offset);
      offset += chunk.length;
    }
    let data: unknown;
    try {
      data = JSON.parse(
        new TextDecoder("utf-8", { fatal: true }).decode(bytes),
      );
    } catch {
      throw new Error("Invalid speech model inventory");
    }
    const models =
      data && typeof data === "object"
        ? (data as { data?: unknown }).data
        : null;
    if (
      !Array.isArray(models) ||
      !models.some(
        (model) =>
          model &&
          typeof model === "object" &&
          typeof model.id === "string" &&
          /nemotron-3[._]5/.test(model.id),
      )
    )
      throw new Error("Load the Nemotron 3.5 ASR model on the speech service");
    controller.signal.throwIfAborted();
    await CloudSpeechStream.test(checked, transport.socket, controller.signal);
    controller.signal.throwIfAborted();
  } finally {
    clearTimeout(timer);
    signal?.removeEventListener("abort", abort);
    controller.abort();
  }
}
