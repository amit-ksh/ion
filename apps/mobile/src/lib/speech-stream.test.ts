import { describe, expect, test } from 'bun:test';
import {
  CloudSpeechStream,
  type SpeechSocket,
  type SpeechSocketFactory,
} from './speech-stream';

class Socket implements SpeechSocket {
  readyState = 1;
  bufferedAmount = 0;
  onopen: (() => void) | null = null;
  onmessage: ((event: { data: unknown }) => void) | null = null;
  onerror: (() => void) | null = null;
  onclose: (() => void) | null = null;
  sent: (string | ArrayBuffer)[] = [];
  closes = 0;
  send(data: string | ArrayBuffer) {
    this.sent.push(data instanceof ArrayBuffer ? data.slice(0) : data);
  }
  close() {
    this.closes++;
    this.readyState = 3;
  }
  event(event: unknown) {
    this.onmessage?.({ data: JSON.stringify(event) });
  }
  types() {
    return this.sent.map((data) =>
      typeof data === 'string' ? JSON.parse(data).type : 'pcm',
    );
  }
}
const endpoint = {
  baseUrl: 'https://speech.example',
  token: 'test-private-token',
};
async function capture() {
  const socket = new Socket();
  const controller = new AbortController();
  const previews: string[] = [];
  const failures: Error[] = [];
  const addresses: unknown[] = [];
  const factory: SpeechSocketFactory = (url, headers) => {
    addresses.push([url, headers]);
    return socket;
  };
  const opening = CloudSpeechStream.open(endpoint, factory, controller.signal, {
    onPreview: (text) => previews.push(text),
    onFailure: (error) => failures.push(error),
  });
  socket.event({ type: 'session.created' });
  const stream = await opening;
  return { stream, socket, controller, previews, failures, addresses };
}
const pcm = () => new Int16Array([1, -1]).buffer;
const configure = (socket: Socket) => socket.event({ type: 'session.updated' });
const final = (socket: Socket, transcript: string) =>
  socket.event({
    type: 'conversation.item.input_audio_transcription.completed',
    transcript,
  });
const delta = (socket: Socket, text: string) =>
  socket.event({
    type: 'conversation.item.input_audio_transcription.delta',
    delta: text,
  });
const ack = (socket: Socket) =>
  socket.event({ type: 'input_audio_buffer.committed' });

describe('shared mobile speech stream', () => {
  test('keeps credentials out of the URL and configures actual PCM before sending', async () => {
    const { stream, socket, addresses } = await capture();
    expect(addresses).toEqual([
      [
        'wss://speech.example/v1/audio/transcriptions/realtime',
        { Authorization: 'Bearer test-private-token' },
      ],
    ]);
    const first = pcm();
    stream.append(first, 48_000);
    expect(socket.types()).toEqual(['session.update']);
    expect(JSON.parse(socket.sent[0] as string).session.sample_rate).toBe(
      48_000,
    );
    new Int16Array(first)[0] = 50; // Native buffer ownership may change after the callback.
    configure(socket);
    expect(Array.from(new Int16Array(socket.sent[1] as ArrayBuffer))).toEqual([
      1, -1,
    ]);
    stream.cancel();
  });
  test('drains pending PCM before commit and waits for the complete acknowledgment', async () => {
    const { stream, socket, failures } = await capture();
    stream.append(pcm(), 16_000);
    const result = stream.finish();
    configure(socket);
    expect(socket.types()).toEqual([
      'session.update',
      'pcm',
      'input_audio_buffer.commit',
    ]);
    let resolved = false;
    void result.then(() => {
      resolved = true;
    });
    final(socket, 'First.');
    await Promise.resolve();
    expect(resolved).toBe(false);
    final(socket, 'Second.');
    ack(socket);
    expect(await result).toBe('First. Second.');
    expect(socket.closes).toBe(1);
    expect(failures).toEqual([]);
  });
  test('accepts an empty final after a prior endpoint without duplicating it', async () => {
    const { stream, socket } = await capture();
    stream.append(pcm(), 16_000);
    configure(socket);
    final(socket, 'Already finished.');
    const result = stream.finish();
    final(socket, '');
    ack(socket);
    expect(await result).toBe('Already finished.');
  });
  test('cancellation releases buffered audio and never finalizes or reconnects', async () => {
    const { stream, socket, controller, failures } = await capture();
    stream.append(pcm(), 16_000);
    controller.abort();
    expect(socket.types()).toEqual(['session.update']);
    expect(socket.closes).toBe(1);
    configure(socket);
    final(socket, 'Late result');
    ack(socket);
    expect(failures.map((error) => error.message)).toEqual([
      'Dictation canceled',
    ]);
    await expect(stream.finish()).rejects.toThrow('No new microphone audio');
  });
  test('rejects network failure while finishing, and hides provider error contents', async () => {
    const { stream, socket, failures } = await capture();
    stream.append(pcm(), 16_000);
    configure(socket);
    const result = stream.finish();
    const outcome = result.then(
      () => null,
      (error) => error as Error,
    );
    socket.event({ type: 'error', error: { message: 'test-private-token' } });
    expect((await outcome)?.message).toContain(
      'rejected the streaming session',
    );
    expect(failures[0].message).not.toContain('test-private-token');
    expect(socket.closes).toBe(1);
  });
  test('does not publish incomplete partials when a commit acknowledgment arrives', async () => {
    const { stream, socket } = await capture();
    stream.append(pcm(), 16_000);
    configure(socket);
    delta(socket, 'unfinished');
    const result = stream.finish();
    const outcome = result.then(
      () => null,
      (error) => error as Error,
    );
    ack(socket);
    expect((await outcome)?.message).toContain('incomplete');
  });
  test('bounds startup audio to two seconds and rejects format changes', async () => {
    const first = await capture();
    first.stream.append(new ArrayBuffer(32_000), 16_000);
    first.stream.append(new ArrayBuffer(32_000), 16_000);
    expect(() => first.stream.append(pcm(), 16_000)).toThrow('too slow');
    expect(first.socket.types()).toEqual(['session.update']);
    expect(first.socket.closes).toBe(1);
    const second = await capture();
    second.stream.append(pcm(), 16_000);
    configure(second.socket);
    expect(() => second.stream.append(pcm(), 48_000)).toThrow(
      'Invalid microphone',
    );
    expect(second.socket.closes).toBe(1);
  });
  test('bounds slow connection buffering, preview text, and malformed server events', async () => {
    const first = await capture();
    first.stream.append(pcm(), 16_000);
    configure(first.socket);
    first.socket.bufferedAmount = 600_000;
    expect(() => first.stream.append(pcm(), 16_000)).toThrow('keep up');
    expect(first.socket.closes).toBe(1);
    const second = await capture();
    second.stream.append(pcm(), 16_000);
    configure(second.socket);
    delta(second.socket, 'x'.repeat(16_384));
    expect(second.socket.closes).toBe(1);
    const third = await capture();
    third.socket.onmessage?.({ data: 'x'.repeat(65_537) });
    expect(third.socket.closes).toBe(1);
  });
  test('coalesces partial previews without publishing final draft text', async () => {
    const { stream, socket, previews } = await capture();
    stream.append(pcm(), 16_000);
    configure(socket);
    delta(socket, 'Hello');
    delta(socket, ' there');
    expect(previews).toEqual([]);
    await new Promise((resolve) => setTimeout(resolve, 145));
    expect(previews).toEqual(['Hello there']);
    delta(socket, ' ignored');
    stream.cancel();
    await new Promise((resolve) => setTimeout(resolve, 145));
    expect(previews).toEqual(['Hello there']);
  });
  test('cancellation before native connection still closes a late socket upgrade', async () => {
    const socket = new Socket();
    socket.readyState = 0;
    const controller = new AbortController();
    const opening = CloudSpeechStream.open(
      endpoint,
      () => socket,
      controller.signal,
      { onPreview() {}, onFailure() {} },
    );
    const outcome = opening.then(
      () => null,
      (error) => error as Error,
    );
    controller.abort();
    expect((await outcome)?.message).toBe('Dictation canceled');
    expect(socket.closes).toBe(1);
    // Android registers a native WebSocket only onOpen; close while connecting can
    // be a native no-op. A late upgrade must be explicitly closed as well.
    socket.readyState = 1;
    socket.onopen?.();
    expect(socket.closes).toBe(2);
    expect(socket.sent).toEqual([]);
  });
  test('rejects unsafe endpoint and token values before opening a socket', async () => {
    let calls = 0;
    const factory: SpeechSocketFactory = () => {
      calls++;
      return new Socket();
    };
    for (const baseUrl of [
      'http://speech.example',
      'https://secret@speech.example',
      'https://speech.example/?key=x',
      'https://speech.example/path',
    ]) {
      await expect(
        CloudSpeechStream.open(
          { ...endpoint, baseUrl },
          factory,
          new AbortController().signal,
          { onPreview() {}, onFailure() {} },
        ),
      ).rejects.toThrow('HTTPS');
    }
    await expect(
      CloudSpeechStream.open(
        { ...endpoint, token: 'bad\r\nsecret' },
        factory,
        new AbortController().signal,
        { onPreview() {}, onFailure() {} },
      ),
    ).rejects.toThrow('token');
    expect(calls).toBe(0);
  });
});
