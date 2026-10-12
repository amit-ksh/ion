import http from "node:http";
import https from "node:https";
import WebSocket from "ws";
import { canceled, EVENT_LIMIT, failure } from "./config.mjs";

/** Native HTTP does not follow redirects and never consumes failed response bodies. */
export function getJson(origin, path, token, signal, timeoutMs) {
  canceled(signal);
  return new Promise((resolve, reject) => {
    const url = new URL(path, origin);
    let settled = false,
      response,
      timer;
    const finish = (error, value) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      signal.removeEventListener("abort", abort);
      response?.destroy();
      request.destroy();
      error ? reject(error) : resolve(value);
    };
    const abort = () =>
      finish(
        failure("canceled", "Speech check canceled; no audio is replayed."),
      );
    const request = (url.protocol === "https:" ? https : http).get(
      url,
      {
        headers: token
          ? { Authorization: `Bearer ${token}`, Accept: "application/json" }
          : { Accept: "application/json" },
        agent: false,
        maxHeaderSize: 16_384,
        rejectUnauthorized: true,
      },
      (received) => {
        response = received;
        const status = received.statusCode ?? 0;
        if (status >= 300 && status < 400)
          return finish(
            failure(
              "redirect",
              "Speech endpoint redirected; serve the exact API routes directly.",
            ),
          );
        if (status !== 200) return finish(null, { status, data: null });
        const type = received.headers["content-type"];
        if (
          typeof type !== "string" ||
          !/^application\/json(?:;|$)/i.test(type)
        )
          return finish(
            failure(
              "response",
              "Speech endpoint returned an invalid JSON response.",
            ),
          );
        const declared = received.headers["content-length"];
        if (
          declared !== undefined &&
          (!/^[0-9]+$/.test(declared) || Number(declared) > EVENT_LIMIT)
        )
          return finish(
            failure("response_limit", "Speech HTTP response exceeds 64 KiB."),
          );
        const buffers = [];
        let size = 0;
        received.on("data", (chunk) => {
          size += chunk.length;
          if (size > EVENT_LIMIT || buffers.length >= 4096)
            return finish(
              failure(
                "response_limit",
                "Speech HTTP response exceeds its bounded limit.",
              ),
            );
          buffers.push(chunk);
        });
        received.once("error", () =>
          finish(
            failure(
              "network",
              "Speech HTTP response failed; check the endpoint and trusted TLS certificate.",
            ),
          ),
        );
        received.once("aborted", () =>
          finish(
            failure("network", "Speech HTTP response ended before completion."),
          ),
        );
        received.once("end", () => {
          try {
            finish(null, {
              status,
              data: JSON.parse(
                new TextDecoder("utf-8", { fatal: true }).decode(
                  Buffer.concat(buffers, size),
                ),
              ),
            });
          } catch {
            finish(
              failure(
                "response",
                "Speech endpoint returned invalid JSON or UTF8.",
              ),
            );
          }
        });
      },
    );
    request.once("error", () =>
      finish(
        failure(
          "network",
          "Speech HTTP connection failed; check the endpoint and trusted TLS certificate.",
        ),
      ),
    );
    signal.addEventListener("abort", abort, { once: true });
    timer = setTimeout(
      () =>
        finish(failure("deadline", "Speech HTTP check exceeded its deadline.")),
      timeoutMs,
    );
    if (signal.aborted) abort();
  });
}

/** ws enforces frame/fragment limits before application parsing; no redirect, compression or replay. */
export class SpeechChannel {
  constructor(origin, token, signal, handshakeMs) {
    canceled(signal);
    this.signal = signal;
    this.queue = [];
    this.queueBytes = 0;
    this.eventCount = 0;
    this.eventBytes = 0;
    this.failure = null;
    this.waiter = null;
    this.closed = false;
    const url = new URL("/v1/audio/transcriptions/realtime", origin);
    url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
    this.socket = new WebSocket(url, {
      headers: token ? { Authorization: `Bearer ${token}` } : {},
      followRedirects: false,
      handshakeTimeout: handshakeMs,
      maxPayload: EVENT_LIMIT,
      maxFragments: 128,
      maxBufferedChunks: 256,
      perMessageDeflate: false,
      rejectUnauthorized: true,
      agent: false,
      closeTimeout: 250,
    });
    this.abort = () =>
      this.fail(
        failure("canceled", "Speech check canceled; no audio is replayed."),
      );
    signal.addEventListener("abort", this.abort, { once: true });
    this.socket.once("unexpected-response", (request, response) => {
      const status = response.statusCode;
      request.destroy();
      response.destroy();
      this.fail(
        status === 401 || status === 403
          ? failure("auth_denied", "Speech socket refused authentication.")
          : status >= 300 && status < 400
            ? failure(
                "redirect",
                "Speech socket redirected; serve the exact realtime route directly.",
              )
            : failure(
                "socket_handshake",
                "Speech socket handshake failed; verify the explicit realtime route.",
              ),
      );
    });
    this.socket.on("error", () =>
      this.fail(
        failure(
          "socket_transport",
          "Speech socket failed its transport, TLS or receive limits.",
        ),
      ),
    );
    this.socket.on("close", (code) => {
      if (!this.closed)
        this.fail(
          code === 1008
            ? failure("auth_denied", "Speech socket refused authentication.")
            : failure(
                "socket_closed",
                "Speech socket closed before the check completed.",
              ),
        );
    });
    this.socket.on("message", (data, binary) => {
      if (this.closed || this.failure) return;
      if (binary || data.length > EVENT_LIMIT)
        return this.fail(
          failure(
            "protocol",
            "Speech socket returned a non-JSON or oversized event.",
          ),
        );
      let event;
      try {
        event = JSON.parse(
          new TextDecoder("utf-8", { fatal: true }).decode(data),
        );
      } catch {
        return this.fail(
          failure("protocol", "Speech socket returned invalid JSON or UTF8."),
        );
      }
      if (
        !event ||
        Array.isArray(event) ||
        typeof event !== "object" ||
        typeof event.type !== "string"
      )
        return this.fail(
          failure("protocol", "Speech socket returned an invalid event."),
        );
      this.eventBytes += data.length;
      if (++this.eventCount > 4096 || this.eventBytes > 2 * 1024 * 1024)
        return this.fail(
          failure(
            "event_limit",
            "Speech socket exceeded its total event limit.",
          ),
        );
      if (this.waiter) {
        const waiter = this.waiter;
        this.waiter = null;
        clearTimeout(waiter.timer);
        waiter.resolve(event);
      } else {
        if (
          this.queue.length >= 32 ||
          this.queueBytes + data.length > 256 * 1024
        )
          return this.fail(
            failure(
              "event_limit",
              "Speech socket exceeded its pending event limit.",
            ),
          );
        this.queue.push({ event, bytes: data.length });
        this.queueBytes += data.length;
      }
    });
    if (signal.aborted) this.abort();
  }
  fail(error) {
    if (this.failure || this.closed) return;
    this.failure = error;
    this.queue = [];
    this.queueBytes = 0;
    if (this.waiter) {
      const waiter = this.waiter;
      this.waiter = null;
      clearTimeout(waiter.timer);
      waiter.reject(error);
    }
    this.socket.terminate();
  }
  next(timeoutMs) {
    if (this.failure) return Promise.reject(this.failure);
    if (this.closed)
      return Promise.reject(
        failure("socket_closed", "Speech socket is closed."),
      );
    if (this.queue.length) {
      const { event, bytes } = this.queue.shift();
      this.queueBytes -= bytes;
      return Promise.resolve(event);
    }
    if (this.waiter)
      return Promise.reject(
        failure("protocol", "Speech check attempted overlapping event reads."),
      );
    return new Promise((resolve, reject) => {
      const waiter = {
        resolve,
        reject,
        timer: setTimeout(() => {
          if (this.waiter === waiter) this.waiter = null;
          reject(
            failure(
              "deadline",
              "Speech socket did not complete the expected event before its deadline.",
            ),
          );
        }, timeoutMs),
      };
      this.waiter = waiter;
    });
  }
  async send(data) {
    canceled(this.signal);
    if (this.failure) throw this.failure;
    if (
      this.socket.readyState !== WebSocket.OPEN ||
      this.socket.bufferedAmount > 256 * 1024
    )
      throw failure(
        "socket_send",
        "Speech socket cannot accept bounded audio.",
      );
    await new Promise((resolve, reject) => {
      this.socket.send(data, (error) =>
        error
          ? reject(
              failure(
                "socket_send",
                "Speech socket failed while sending audio or control.",
              ),
            )
          : resolve(),
      );
    });
  }
  async shutdown() {
    if (this.closed) return;
    this.closed = true;
    this.signal.removeEventListener("abort", this.abort);
    this.queue = [];
    this.queueBytes = 0;
    if (this.waiter) {
      clearTimeout(this.waiter.timer);
      this.waiter.reject(failure("socket_closed", "Speech socket is closed."));
      this.waiter = null;
    }
    if (this.socket.readyState === WebSocket.CLOSED) return;
    await new Promise((resolve) => {
      const timer = setTimeout(() => {
        this.socket.terminate();
        resolve();
      }, 250);
      this.socket.once("close", () => {
        clearTimeout(timer);
        resolve();
      });
      if (this.socket.readyState === WebSocket.OPEN) this.socket.close(1000);
      else this.socket.terminate();
    });
  }
}
export function delay(ms, signal) {
  canceled(signal);
  if (ms <= 0) return Promise.resolve();
  return new Promise((resolve, reject) => {
    const abort = () => {
      clearTimeout(timer);
      signal.removeEventListener("abort", abort);
      reject(
        failure("canceled", "Speech check canceled; no audio is replayed."),
      );
    };
    const timer = setTimeout(() => {
      signal.removeEventListener("abort", abort);
      resolve();
    }, ms);
    signal.addEventListener("abort", abort, { once: true });
    if (signal.aborted) abort();
  });
}
