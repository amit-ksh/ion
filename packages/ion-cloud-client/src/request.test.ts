import { expect, test } from "bun:test";
import { requestJson } from "./request";
test("request deadline uses AbortController and does not require AbortSignal.timeout", async () => {
  const previous = AbortSignal.timeout;
  try {
    Object.assign(AbortSignal, { timeout: undefined });
    const send = (async (_url: unknown, init?: RequestInit) =>
      new Promise<Response>((_resolve, reject) => {
        init?.signal?.addEventListener(
          "abort",
          () => reject(new Error("deadline")),
          { once: true },
        );
      })) as unknown as typeof fetch;
    await expect(
      requestJson(send, "https://example.com", {}, 2),
    ).rejects.toThrow("deadline");
  } finally {
    Object.assign(AbortSignal, { timeout: previous });
  }
});
test("JSON reads reject non-success responses without reflecting private provider text", async () => {
  const send = (async () =>
    new Response("private credential detail", {
      status: 401,
    })) as unknown as typeof fetch;
  await expect(requestJson(send, "https://example.com")).rejects.toThrow(
    "Cloud request failed",
  );
});
