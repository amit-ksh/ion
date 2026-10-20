/** Native runtimes need AbortController but not AbortSignal.timeout.
 * The deadline includes response-body decoding and is cleared on every exit. */
async function timed<T>(
  send: typeof fetch,
  input: string | URL,
  init: RequestInit,
  read: (response: Response) => Promise<T>,
  milliseconds: number,
): Promise<T> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), milliseconds);
  try {
    return await read(
      await send(input, {
        ...init,
        redirect: "error",
        signal: controller.signal,
      }),
    );
  } finally {
    clearTimeout(timer);
  }
}
export function requestJson<T = unknown>(
  send: typeof fetch,
  input: string | URL,
  init: RequestInit = {},
  milliseconds = 15_000,
): Promise<T> {
  return timed(
    send,
    input,
    init,
    async (response) => {
      if (!response.ok) {
        await response.body?.cancel();
        throw new Error("Cloud request failed");
      }
      return (await response.json()) as T;
    },
    milliseconds,
  );
}
export function requestAccepted(
  send: typeof fetch,
  input: string | URL,
  init: RequestInit,
  milliseconds = 15_000,
): Promise<boolean> {
  return timed(
    send,
    input,
    init,
    async (response) => {
      await response.body?.cancel();
      return response.ok;
    },
    milliseconds,
  );
}
