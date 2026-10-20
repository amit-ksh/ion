import type { Server, ServerWebSocket, WebSocketHandler } from "bun";
import { CloudFailure, type Account } from "./auth";
import { ServiceStore, readJson, secret, hash } from "./service-store";

export interface SpeechOptions {
  origin: string;
  token: string;
  globalStreams?: number;
  dailySeconds?: number;
}
type Lease = {
  id: string;
  workspace: string;
  created: number;
  expires: number;
  maximum: number;
  seconds: number;
  state: "reserved" | "active" | "done";
};
export type SpeechSocketData = {
  lease: Lease;
  upstream: WebSocket;
  bytes: number;
  rate: number;
  updated: boolean;
  committing: boolean;
  timer?: ReturnType<typeof setTimeout>;
  pending: string[];
  socket?: ServerWebSocket<SpeechSocketData>;
};
const allowed = new Set([
  "session.created",
  "session.updated",
  "conversation.item.input_audio_transcription.delta",
  "conversation.item.input_audio_transcription.completed",
  "input_audio_buffer.committed",
  "input_audio_buffer.cleared",
]);
export class ManagedSpeech {
  readonly enabled: boolean;
  private closing = false;
  readonly sockets = new Set<ServerWebSocket<SpeechSocketData>>();
  constructor(
    private store: ServiceStore,
    private publicOrigin: string,
    private options?: SpeechOptions,
  ) {
    this.enabled = !!options;
    if (options) {
      if (
        !Number.isInteger(options.globalStreams ?? 4) ||
        (options.globalStreams ?? 4) < 1 ||
        (options.globalStreams ?? 4) > 32 ||
        !Number.isInteger(options.dailySeconds ?? 3600) ||
        (options.dailySeconds ?? 3600) < 120 ||
        (options.dailySeconds ?? 3600) > 86400
      )
        throw new Error("Invalid managed speech limits");
      const url = new URL(options.origin);
      if (
        url.origin !== options.origin ||
        (url.protocol !== "https:" &&
          !(
            url.protocol === "http:" &&
            ["127.0.0.1", "localhost"].includes(url.hostname)
          )) ||
        !options.token ||
        /[\r\n]/.test(options.token)
      )
        throw new Error("Invalid private speech worker configuration");
    }
  }
  private require() {
    if (!this.options)
      throw new CloudFailure(503, "Managed speech is not configured");
    return this.options;
  }
  reserve(account: Account, maximum: number) {
    const options = this.require();
    if (!Number.isInteger(maximum) || maximum < 1 || maximum > 120)
      throw new CloudFailure(400, "Speech sessions allow 1–120 seconds");
    const lease = this.store.transaction(() => {
      const now = Date.now(),
        day = Math.floor(now / 86_400_000);
      const records = this.store.all<Lease>(account.workspaceId, "speech");
      if (records.filter((v) => v.created > now - 3_600_000).length >= 120)
        throw new CloudFailure(429, "Speech request limit reached");
      if (records.some((v) => v.expires > now && v.state !== "done"))
        throw new CloudFailure(409, "A speech session is already open");
      const used = records
        .filter((v) => Math.floor(v.created / 86_400_000) === day)
        .reduce(
          (sum, v) =>
            sum +
            (v.state === "done"
              ? v.seconds
              : v.state === "reserved" && v.expires <= now
                ? 0
                : v.maximum),
          0,
        );
      if (used + maximum > (options.dailySeconds ?? 3600))
        throw new CloudFailure(429, "Today's speech allowance is used");
      const active = this.store.db
        .query<{ n: number }, [number]>(
          "SELECT count(*) AS n FROM service_objects WHERE kind='speech' AND json_extract(value,'$.expires')>? AND json_extract(value,'$.state')!='done'",
        )
        .get(now)!.n;
      if (active >= (options.globalStreams ?? 4))
        throw new CloudFailure(429, "Speech is busy; try again later");
      const item: Lease = {
        id: secret(),
        workspace: account.workspaceId,
        created: now,
        expires: now + 30_000,
        maximum,
        seconds: 0,
        state: "reserved",
      };
      this.store.put(account.workspaceId, "speech", item.id, item);
      return item;
    });
    const ticket = this.store.capability(
      account.workspaceId,
      "speech",
      { id: lease.id },
      Date.now() + 30_000,
    );
    return {
      baseUrl: this.publicOrigin,
      token: ticket,
      expiresAt: Date.now() + 30_000,
      maximumSeconds: maximum,
      model: "nvidia/nemotron-3.5-asr-streaming-0.6b",
    };
  }
  private ticket(request: Request, consume: boolean): Lease {
    this.require();
    const ticket =
      /^Bearer ([A-Za-z0-9_-]{43})$/.exec(
        request.headers.get("authorization") ?? "",
      )?.[1] ?? "";
    const auth = this.store.inspect(ticket, "speech", consume);
    const lease = this.store.get<Lease>(
      auth.workspace,
      "speech",
      auth.value.id,
    );
    if (!lease || lease.state !== "reserved" || lease.expires <= Date.now())
      throw new CloudFailure(401, "Speech session expired");
    if (consume) {
      lease.state = "active";
      lease.expires = Date.now() + lease.maximum * 1000 + 25_000;
      this.store.put(lease.workspace, "speech", lease.id, lease);
    }
    return lease;
  }
  private settle(lease: Lease, seconds: number) {
    if (this.closing) return;
    const current = this.store.get<Lease>(lease.workspace, "speech", lease.id);
    if (!current || current.state === "done") return;
    current.state = "done";
    current.seconds = Math.min(
      current.maximum,
      Math.max(0, Math.ceil(seconds)),
    );
    this.store.put(lease.workspace, "speech", lease.id, current);
  }
  async handle(request: Request, account?: Account): Promise<Response> {
    const path = new URL(request.url).pathname;
    if (
      path === "/v1/speech/sessions" &&
      request.method === "POST" &&
      account
    ) {
      const input = await readJson(request, 2000);
      return Response.json(this.reserve(account, input.maximumSeconds ?? 120), {
        status: 201,
        headers: { "Cache-Control": "no-store" },
      });
    }
    if (path === "/v1/models" && request.method === "GET") {
      this.ticket(request, false);
      const options = this.require();
      const result = await fetch(options.origin + "/v1/models", {
        headers: { Authorization: "Bearer " + options.token },
        redirect: "error",
        signal: AbortSignal.timeout(10_000),
      });
      if (!result.ok)
        throw new CloudFailure(503, "Speech model is unavailable");
      const body = await result.text();
      if (body.length > 64000)
        throw new CloudFailure(503, "Invalid speech inventory");
      const value = JSON.parse(body);
      if (
        !value.data?.some(
          (v: any) => typeof v.id === "string" && /nemotron-3[._]5/.test(v.id),
        )
      )
        throw new CloudFailure(503, "Nemotron 3.5 is not ready");
      return Response.json(
        { data: [{ id: "nvidia/nemotron-3.5-asr-streaming-0.6b" }] },
        { headers: { "Cache-Control": "no-store" } },
      );
    }
    if (path === "/v1/audio/transcriptions" && request.method === "POST") {
      const lease = this.ticket(request, true);
      let seconds = lease.maximum;
      try {
        const body = (await request.formData()) as unknown as {
            get(name: string): any;
          },
          file = body.get("file");
        if (!(file instanceof File) || file.size > 23_100_000)
          throw new CloudFailure(400, "Choose a bounded PCM16 WAV");
        const bytes = new Uint8Array(await file.arrayBuffer());
        const view = new DataView(bytes.buffer);
        if (
          bytes.length < 44 ||
          Buffer.from(bytes.subarray(0, 4)).toString() !== "RIFF" ||
          Buffer.from(bytes.subarray(8, 12)).toString() !== "WAVE"
        )
          throw new CloudFailure(400, "Invalid WAV");
        let rate = 0,
          alignment = 0,
          data = 0;
        for (let offset = 12; offset + 8 <= bytes.length;) {
          const name = Buffer.from(
              bytes.subarray(offset, offset + 4),
            ).toString(),
            length = view.getUint32(offset + 4, true),
            end = offset + 8 + length;
          if (end > bytes.length)
            throw new CloudFailure(400, "Invalid WAV bounds");
          if (name === "fmt ") {
            if (
              length < 16 ||
              view.getUint16(offset + 8, true) !== 1 ||
              view.getUint16(offset + 10, true) !== 1 ||
              view.getUint16(offset + 22, true) !== 16
            )
              throw new CloudFailure(400, "Use mono PCM16 WAV");
            rate = view.getUint32(offset + 12, true);
            alignment = view.getUint16(offset + 20, true);
          }
          if (name === "data") data += length;
          offset = end + (length % 2);
        }
        seconds = data / (rate * 2);
        if (
          rate < 8000 ||
          rate > 96000 ||
          alignment !== 2 ||
          !Number.isFinite(seconds) ||
          seconds <= 0 ||
          seconds > lease.maximum
        )
          throw new CloudFailure(400, "WAV exceeds session limits");
        const options = this.require(),
          form = new FormData();
        form.set("file", new Blob([bytes]), "dictation.wav");
        form.set("response_format", "json");
        form.set(
          "language",
          ["auto", "en-US", "hi-IN"].includes(String(body.get("language")))
            ? String(body.get("language"))
            : "auto",
        );
        const response = await fetch(
          options.origin + "/v1/audio/transcriptions",
          {
            method: "POST",
            headers: { Authorization: "Bearer " + options.token },
            body: form,
            redirect: "error",
            signal: AbortSignal.timeout(120_000),
          },
        );
        if (!response.ok)
          throw new CloudFailure(503, "Speech recognition failed");
        const text = await response.text();
        if (text.length > 64000)
          throw new CloudFailure(503, "Speech response exceeded its limit");
        const value = JSON.parse(text);
        if (typeof value.text !== "string")
          throw new CloudFailure(503, "Invalid speech response");
        return Response.json(
          { text: value.text },
          { headers: { "Cache-Control": "no-store" } },
        );
      } finally {
        this.settle(lease, seconds);
      }
    }
    throw new CloudFailure(404, "Speech route not found");
  }
  upgrade(request: Request, server: Server<SpeechSocketData>): boolean {
    const lease = this.ticket(request, true),
      options = this.require();
    const url =
      options.origin.replace(/^http/, "ws") +
      "/v1/audio/transcriptions/realtime";
    const NativeSocket = WebSocket as unknown as {
      new (
        url: string,
        options: { headers: Record<string, string> },
      ): WebSocket;
    };
    const upstream = new NativeSocket(url, {
      headers: { Authorization: "Bearer " + options.token },
    });
    const data: SpeechSocketData = {
      lease,
      upstream,
      bytes: 0,
      rate: 16000,
      updated: false,
      committing: false,
      pending: [],
    };
    const fail = () => {
      data.socket?.close(1011, "Speech session ended");
      upstream.close();
      this.settle(lease, data.bytes / (2 * data.rate));
    };
    upstream.onmessage = (event) => {
      if (typeof event.data !== "string" || event.data.length > 64000) {
        fail();
        return;
      }
      try {
        const value = JSON.parse(event.data);
        if (value.type === "error") {
          fail();
          return;
        }
        if (!allowed.has(value.type)) return;
        if (data.socket) {
          if (data.socket.send(event.data) < 0) fail();
        } else {
          data.pending.push(event.data);
          if (data.pending.reduce((n, v) => n + v.length, 0) > 64000) fail();
        }
      } catch {
        fail();
      }
    };
    upstream.onerror = fail;
    upstream.onclose = () => {
      data.socket?.close();
      this.settle(lease, data.bytes / (2 * data.rate));
    };
    if (!server.upgrade(request, { data })) {
      upstream.close();
      this.settle(lease, 0);
      return false;
    }
    return true;
  }
  readonly websocket: WebSocketHandler<SpeechSocketData> = {
    maxPayloadLength: 256_000,
    backpressureLimit: 256_000,
    closeOnBackpressureLimit: true,
    idleTimeout: 20,
    open: (socket) => {
      const data = socket.data;
      data.socket = socket;
      this.sockets.add(socket);
      for (const event of data.pending) socket.send(event);
      data.pending = [];
      data.timer = setTimeout(
        () => socket.close(1008, "Speech time limit reached"),
        data.lease.maximum * 1000 + 25_000,
      );
    },
    message: (socket, message) => {
      const data = socket.data;
      try {
        if (
          data.upstream.readyState !== WebSocket.OPEN ||
          data.upstream.bufferedAmount > 256_000
        )
          throw new Error();
        if (typeof message === "string") {
          if (message.length > 4000) throw new Error();
          const value = JSON.parse(message);
          if (value.type === "session.update" && !data.updated && !data.bytes) {
            const rate = value.session?.sample_rate;
            if (!Number.isInteger(rate) || rate < 8000 || rate > 96000)
              throw new Error();
            data.rate = rate;
            data.updated = true;
            data.upstream.send(
              JSON.stringify({
                type: "session.update",
                session: {
                  sample_rate: rate,
                  language: ["auto", "en-US", "hi-IN"].includes(
                    value.session?.language,
                  )
                    ? value.session.language
                    : "auto",
                },
              }),
            );
          } else if (
            value.type === "input_audio_buffer.commit" &&
            !data.committing
          ) {
            data.committing = true;
            data.upstream.send(message);
          } else if (
            ["input_audio_buffer.clear", "response.cancel"].includes(value.type)
          ) {
            data.upstream.send(message);
            socket.close();
          } else throw new Error();
        } else {
          if (
            !data.updated ||
            data.committing ||
            message.length % 2 ||
            message.length > data.rate * 2
          )
            throw new Error();
          data.bytes += message.length;
          if (data.bytes > data.rate * 2 * data.lease.maximum)
            throw new Error();
          data.upstream.send(message);
        }
      } catch {
        socket.close(1008, "Speech session limit or format rejected");
      }
    },
    close: (socket) => {
      const data = socket.data;
      clearTimeout(data.timer);
      data.upstream.close();
      this.sockets.delete(socket);
      this.settle(data.lease, data.bytes / (2 * data.rate));
    },
  };
  close() {
    for (const socket of this.sockets) {
      this.settle(
        socket.data.lease,
        socket.data.bytes / (2 * socket.data.rate),
      );
      socket.close(1001, "Service stopping");
    }
    this.closing = true;
  }
}
