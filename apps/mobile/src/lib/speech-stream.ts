import { speechToken, speechUrl, type SpeechEndpoint } from './speech-endpoint';

export interface SpeechSocket {
  readonly readyState: number;
  readonly bufferedAmount: number;
  onopen: (() => void) | null;
  onmessage: ((event: { data: unknown }) => void) | null;
  onerror: (() => void) | null;
  onclose: (() => void) | null;
  send(data: string | ArrayBuffer): void;
  close(): void;
}
export type SpeechSocketFactory = (
  url: string,
  headers: Record<string, string>,
) => SpeechSocket;

type Callbacks = {
  onPreview: (text: string) => void;
  onFailure: (error: Error) => void;
};
const MAX_TEXT = 16_384;
const MAX_EVENT = 65_536;

/** One explicit capture. No reconnect, audio replay, or alternate destination. */
export class CloudSpeechStream {
  private readonly socket: SpeechSocket;
  private readonly signal: AbortSignal;
  private readonly callbacks: Callbacks;
  private closed = false;
  private created = false;
  private configured = false;
  private sampleRate = 0;
  private audioBytes = 0;
  private pending: ArrayBuffer[] = [];
  private pendingBytes = 0;
  private committed = '';
  private partial = '';
  private finishing = false;
  private commitSent = false;
  private previewTimer: ReturnType<typeof setTimeout> | null = null;
  private startupTimer: ReturnType<typeof setTimeout> | null = null;
  private finishTimer: ReturnType<typeof setTimeout> | null = null;
  private resolveConnected!: () => void;
  private rejectConnected!: (error: Error) => void;
  private resolveFinished: ((text: string) => void) | null = null;
  private rejectFinished: ((error: Error) => void) | null = null;
  private readonly connected: Promise<void>;

  static async open(
    endpoint: SpeechEndpoint,
    factory: SpeechSocketFactory,
    signal: AbortSignal,
    callbacks: Callbacks,
  ): Promise<CloudSpeechStream> {
    const stream = new CloudSpeechStream(endpoint, factory, signal, callbacks);
    await stream.connected;
    return stream;
  }
  private constructor(
    endpoint: SpeechEndpoint,
    factory: SpeechSocketFactory,
    signal: AbortSignal,
    callbacks: Callbacks,
  ) {
    const url = `${speechUrl(endpoint.baseUrl).replace(/^https:/, 'wss:')}/v1/audio/transcriptions/realtime`;
    const token = speechToken(endpoint.token);
    this.signal = signal;
    this.callbacks = callbacks;
    this.connected = new Promise((resolve, reject) => {
      this.resolveConnected = resolve;
      this.rejectConnected = reject;
    });
    this.socket = factory(url, { Authorization: `Bearer ${token}` });
    this.socket.onopen = () => {}; // Session creation, not HTTP upgrade alone, establishes readiness.
    this.socket.onmessage = (event) => {
      if (this.closed) return;
      try {
        this.receive(event.data);
      } catch {
        this.fail(new Error('Invalid or incomplete speech streaming response'));
      }
    };
    this.socket.onerror = () =>
      this.fail(
        new Error(
          'Speech streaming connection failed. Check the service, TLS and token.',
        ),
      );
    this.socket.onclose = () =>
      this.fail(new Error('Speech streaming disconnected before completion'));
    this.startupTimer = setTimeout(
      () => this.fail(new Error('Speech service did not become ready')),
      15_000,
    );
    signal.addEventListener('abort', this.aborted);
    if (signal.aborted) this.aborted();
  }
  private readonly aborted = () => this.fail(new Error('Dictation canceled'));

  append(pcm: ArrayBuffer, sampleRate: number): void {
    if (this.closed || this.finishing)
      throw new Error('Speech recording is no longer active');
    if (
      !Number.isInteger(sampleRate) ||
      sampleRate < 16_000 ||
      sampleRate > 48_000 ||
      (this.sampleRate && this.sampleRate !== sampleRate) ||
      pcm.byteLength % 2 ||
      pcm.byteLength > sampleRate * 2
    ) {
      this.fail(
        new Error('Microphone format changed or audio chunk is too large'),
      );
      throw new Error('Invalid microphone format');
    }
    if (!pcm.byteLength) return;
    if (this.audioBytes + pcm.byteLength > sampleRate * 2 * 60) {
      this.fail(new Error('Dictation reached the 60-second limit'));
      throw new Error('Dictation reached the 60-second limit');
    }
    if (!this.sampleRate) {
      this.sampleRate = sampleRate;
      this.send(
        JSON.stringify({
          type: 'session.update',
          session: { sample_rate: sampleRate, language: 'auto' },
        }),
      );
      this.startupTimer = setTimeout(
        () =>
          this.fail(
            new Error('Speech service did not accept the microphone format'),
          ),
        15_000,
      );
    }
    this.audioBytes += pcm.byteLength;
    if (!this.configured) {
      if (this.pendingBytes + pcm.byteLength > sampleRate * 2 * 2) {
        this.fail(
          new Error('Speech service is too slow to start; audio was discarded'),
        );
        throw new Error('Speech service is too slow to start');
      }
      this.pending.push(pcm.slice(0));
      this.pendingBytes += pcm.byteLength;
    } else {
      this.send(pcm);
    }
  }

  finish(): Promise<string> {
    if (this.closed || this.finishing || !this.audioBytes)
      return Promise.reject(new Error('No new microphone audio to finalize'));
    this.finishing = true;
    const result = new Promise<string>((resolve, reject) => {
      this.resolveFinished = resolve;
      this.rejectFinished = reject;
    });
    this.finishTimer = setTimeout(
      () =>
        this.fail(
          new Error(
            'Speech service did not confirm a final transcript. Try a shorter recording.',
          ),
        ),
      20_000,
    );
    if (this.configured) {
      try {
        this.commit();
      } catch {
        /* fail() rejects result. */
      }
    }
    return result;
  }
  cancel(): void {
    if (!this.closed) this.aborted();
  }
  private send(data: string | ArrayBuffer): void {
    if (
      this.socket.readyState !== 1 ||
      this.socket.bufferedAmount > 512 * 1024
    ) {
      this.fail(
        new Error('Speech connection cannot keep up; audio was discarded'),
      );
      throw new Error('Speech connection cannot keep up');
    }
    try {
      this.socket.send(data);
    } catch {
      this.fail(new Error('Speech streaming connection failed'));
      throw new Error('Speech streaming connection failed');
    }
  }
  private commit(): void {
    this.send(JSON.stringify({ type: 'input_audio_buffer.commit' }));
    this.commitSent = true;
  }
  private receive(data: unknown): void {
    if (typeof data !== 'string' || data.length > MAX_EVENT)
      throw new Error('Invalid speech event');
    const event = JSON.parse(data) as {
      type?: string;
      delta?: unknown;
      transcript?: unknown;
    };
    switch (event.type) {
      case 'session.created':
        if (this.created) throw new Error('Duplicate session creation');
        this.created = true;
        if (this.startupTimer) clearTimeout(this.startupTimer);
        this.startupTimer = null;
        this.resolveConnected();
        return;
      case 'session.updated':
        if (!this.created || !this.sampleRate || this.configured)
          throw new Error('Unexpected format acknowledgment');
        this.configured = true;
        if (this.startupTimer) clearTimeout(this.startupTimer);
        this.startupTimer = null;
        for (const pcm of this.pending) this.send(pcm);
        this.pending = [];
        this.pendingBytes = 0;
        if (this.finishing) this.commit();
        return;
      case 'conversation.item.input_audio_transcription.delta':
        if (
          !this.configured ||
          typeof event.delta !== 'string' ||
          this.committed.length + this.partial.length + event.delta.length + 1 >
            MAX_TEXT
        )
          throw new Error('Invalid preview');
        this.partial += event.delta;
        break;
      case 'conversation.item.input_audio_transcription.completed': {
        if (!this.configured || typeof event.transcript !== 'string')
          throw new Error('Invalid segment');
        const text = event.transcript.trim();
        if (this.committed.length + text.length + 1 > MAX_TEXT)
          throw new Error('Transcript too long');
        if (text) this.committed += `${this.committed ? ' ' : ''}${text}`;
        this.partial = '';
        break;
      }
      case 'input_audio_buffer.committed':
        if (!this.commitSent || this.partial || !this.committed)
          throw new Error('Incomplete final transcript');
        this.resolveFinished?.(this.committed);
        this.teardown();
        return;
      case 'error':
        this.fail(new Error('Speech service rejected the streaming session'));
        return;
      default:
        return;
    }
    // Coalesce preview updates at at most 8 Hz; the current draft is never modified here.
    if (!this.previewTimer)
      this.previewTimer = setTimeout(() => {
        this.previewTimer = null;
        if (!this.closed)
          this.callbacks.onPreview(`${this.committed} ${this.partial}`.trim());
      }, 125);
  }
  private fail(error: Error): void {
    if (this.closed) return;
    this.rejectConnected(error);
    this.rejectFinished?.(error);
    const notify = this.created;
    this.teardown();
    if (notify) this.callbacks.onFailure(error);
  }
  private teardown(): void {
    this.closed = true;
    for (const timer of [
      this.previewTimer,
      this.startupTimer,
      this.finishTimer,
    ])
      if (timer) clearTimeout(timer);
    this.previewTimer = this.startupTimer = this.finishTimer = null;
    this.pending = [];
    this.pendingBytes = 0;
    this.committed = '';
    this.partial = '';
    this.signal.removeEventListener('abort', this.aborted);
    const wasConnecting = this.socket.readyState === 0;
    this.socket.onmessage = this.socket.onerror = this.socket.onclose = null;
    this.socket.onopen = wasConnecting
      ? () => {
          this.socket.onopen = null;
          this.socket.close();
        }
      : null;
    this.socket.close();
  }
}
