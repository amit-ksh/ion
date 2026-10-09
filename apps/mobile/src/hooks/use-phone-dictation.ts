import type { WakuClient } from '@waku/client';
import {
  requestRecordingPermissionsAsync,
  setAudioModeAsync,
  useAudioStream,
} from 'expo-audio';
import * as Crypto from 'expo-crypto';
import { File, Paths } from 'expo-file-system';
import { useEffect, useRef, useState } from 'react';
import { AppState, Platform } from 'react-native';

import { readSpeechEndpoint } from '@/lib/speech-storage';
import { type SpeechEndpoint } from '@/lib/speech-endpoint';
import {
  CloudSpeechStream,
  nativeSpeechSocket,
} from '@/lib/speech-stream-native';
import { transcribePhoneWav } from '@/lib/daemon-api';
import { phoneDictationWav } from '@/lib/phone-dictation-wav';

type Phase = 'idle' | 'preparing' | 'recording' | 'transcribing';
type Capture = {
  client: WakuClient;
  contextKey: string;
  speech: SpeechEndpoint | null;
  live: CloudSpeechStream | null;
  controller: AbortController;
};
let microphoneOwner: symbol | null = null;
let resettingAudioMode: Promise<void> = Promise.resolve();

/** Configured cloud streams live; otherwise a bounded WAV goes to the selected desktop. */
export function usePhoneDictation(
  client: WakuClient | null,
  contextKey: string,
  onTranscript: (text: string) => void,
  onPreview?: (text: string | null) => void,
) {
  const [phase, setPhase] = useState<Phase>('idle');
  const phaseRef = useRef<Phase>('idle');
  const changePhase = (next: Phase) => {
    phaseRef.current = next;
    if (mounted.current) setPhase(next);
  };
  const [error, setError] = useState<string | null>(null);
  const [cloud, setCloud] = useState(false);
  const owner = useRef(Symbol('phone dictation'));
  const ownsMicrophone = () => microphoneOwner === owner.current;
  const releaseMicrophone = () => {
    if (!ownsMicrophone()) return;
    microphoneOwner = null;
    // Serialize resets before a subsequent capture changes the shared audio mode.
    resettingAudioMode = resettingAudioMode
      .then(() => setAudioModeAsync({ allowsRecording: false }))
      .catch(() => {});
  };
  const abort = useRef<AbortController | null>(null);
  const durationTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const chunks = useRef<Uint8Array[]>([]);
  const byteCount = useRef(0);
  const sampleRate = useRef(0);
  const active = useRef(false);
  const starting = useRef<number | null>(null);
  const generation = useRef(0);
  const mounted = useRef(true);
  const onTranscriptRef = useRef(onTranscript);
  onTranscriptRef.current = onTranscript;
  const onPreviewRef = useRef(onPreview);
  onPreviewRef.current = onPreview;
  const currentContext = useRef(contextKey);
  currentContext.current = contextKey;
  const currentClient = useRef(client);
  currentClient.current = client;
  const target = useRef<Capture | null>(null);
  const stopRef = useRef<() => Promise<void>>(async () => {});
  const clearTimer = () => {
    if (durationTimer.current) clearTimeout(durationTimer.current);
    durationTimer.current = null;
  };
  const current = (capture: Capture) =>
    mounted.current &&
    target.current === capture &&
    currentClient.current === capture.client &&
    currentContext.current === capture.contextKey;

  const { stream } = useAudioStream({
    encoding: 'int16',
    channels: 1,
    sampleRate: 16_000,
    onBuffer: (buffer) => {
      const destination = target.current;
      if (!active.current || !destination) return;
      if (!current(destination)) {
        discard();
        return;
      }
      if (
        buffer.channels !== 1 ||
        !Number.isInteger(buffer.sampleRate) ||
        buffer.sampleRate < 16_000 ||
        buffer.sampleRate > 48_000 ||
        buffer.data.byteLength % 2 ||
        (sampleRate.current && sampleRate.current !== buffer.sampleRate)
      ) {
        failCapture(
          destination,
          'Microphone format changed. Try recording again.',
        );
        return;
      }
      sampleRate.current = buffer.sampleRate;
      byteCount.current += buffer.data.byteLength;
      if (
        byteCount.current > buffer.sampleRate * 2 * 60 ||
        byteCount.current + 44 > 8 * 1024 * 1024
      ) {
        failCapture(
          destination,
          'Recording reached the 60-second limit. Start a shorter dictation.',
        );
        return;
      }
      try {
        if (destination.live)
          destination.live.append(buffer.data, buffer.sampleRate);
        else chunks.current.push(new Uint8Array(buffer.data).slice());
      } catch (cause) {
        failCapture(
          destination,
          cause instanceof Error ? cause.message : 'Speech streaming failed',
        );
      }
    },
  });

  function discard() {
    generation.current++;
    // Clear the target before abort emits a failure, so intentional cancellation stays quiet.
    target.current = null;
    abort.current?.abort();
    abort.current = null;
    const wasActive = active.current;
    active.current = false;
    if (wasActive) stream.stop();
    clearTimer();
    chunks.current = [];
    releaseMicrophone();
    changePhase('idle');
    if (mounted.current) setError(null);
    onPreviewRef.current?.(null);
  }
  function failCapture(capture: Capture, message: string) {
    if (target.current !== capture) return;
    discard();
    if (mounted.current) setError(message);
  }

  useEffect(() => {
    mounted.current = true;
    const status = stream.addListener(
      'audioStreamStatus',
      ({ isStreaming }) => {
        if (
          !isStreaming &&
          active.current &&
          phaseRef.current === 'recording' &&
          target.current
        ) {
          failCapture(
            target.current,
            'Microphone stopped. Audio was discarded.',
          );
        }
      },
    );
    const app = AppState.addEventListener('change', (state) => {
      if (state !== 'active') discard();
    });
    return () => {
      mounted.current = false;
      discard();
      status.remove();
      app.remove();
    };
  }, [stream]);
  useEffect(() => {
    if (phaseRef.current !== 'idle') discard();
  }, [client, contextKey]);

  async function start() {
    if (
      Platform.OS === 'web' ||
      !client ||
      phaseRef.current !== 'idle' ||
      starting.current !== null
    )
      return;
    if (microphoneOwner !== null)
      throw new Error('Another dictation is already using the microphone');
    microphoneOwner = owner.current;
    const operation = ++generation.current;
    starting.current = operation;
    const controller = new AbortController();
    abort.current = controller;
    changePhase('preparing');
    setError(null);
    onPreviewRef.current?.(null);
    const valid = () =>
      mounted.current &&
      operation === generation.current &&
      currentClient.current === client &&
      currentContext.current === contextKey &&
      !controller.signal.aborted;
    try {
      const permission = await requestRecordingPermissionsAsync();
      if (!permission.granted)
        throw new Error('Microphone permission is required for dictation');
      if (!valid()) return;
      const speech = await readSpeechEndpoint();
      if (!valid()) return;
      const destination: Capture = {
        client,
        contextKey,
        speech,
        live: null,
        controller,
      };
      target.current = destination;
      setCloud(!!speech);
      if (speech) {
        destination.live = await CloudSpeechStream.open(
          speech,
          nativeSpeechSocket,
          controller.signal,
          {
            onPreview: (text) => {
              if (current(destination)) onPreviewRef.current?.(text);
            },
            onFailure: (cause) => failCapture(destination, cause.message),
          },
        );
      }
      if (!valid()) {
        destination.live?.cancel();
        return;
      }
      await resettingAudioMode;
      if (!valid()) return;
      await setAudioModeAsync({
        allowsRecording: true,
        playsInSilentMode: true,
      });
      if (!valid()) {
        releaseMicrophone();
        return;
      }
      chunks.current = [];
      byteCount.current = 0;
      sampleRate.current = 0;
      active.current = true;
      await stream.start();
      if (!valid() || !active.current) {
        stream.stop();
        return;
      }
      changePhase('recording');
      durationTimer.current = setTimeout(() => {
        if (current(destination) && active.current)
          void stopRef.current().catch(() => {});
      }, 60_000);
    } catch (cause) {
      if (operation === generation.current) {
        discard();
        if (mounted.current)
          setError(
            cause instanceof Error
              ? cause.message
              : 'Could not start dictation',
          );
        throw cause;
      }
    } finally {
      if (starting.current === operation) starting.current = null;
      if (
        operation === generation.current &&
        (phaseRef.current as Phase) === 'preparing'
      )
        discard();
    }
  }

  async function stop() {
    if (!active.current || phaseRef.current !== 'recording') return;
    active.current = false;
    stream.stop();
    clearTimer();
    releaseMicrophone();
    const destination = target.current;
    if (!destination || !current(destination)) {
      discard();
      return;
    }
    changePhase('transcribing');
    let file: File | null = null;
    const timeout = setTimeout(() => destination.controller.abort(), 120_000);
    try {
      let transcript: string;
      if (destination.live) transcript = await destination.live.finish();
      else {
        const wav = phoneDictationWav(chunks.current, sampleRate.current);
        chunks.current = [];
        file = new File(
          Paths.cache,
          `ion-dictation-${Crypto.randomUUID()}.wav`,
        );
        file.create();
        file.write(wav);
        const base64 = await file.base64();
        if (!current(destination) || destination.controller.signal.aborted)
          return;
        transcript = await transcribePhoneWav(destination.client, base64);
      }
      if (!current(destination) || destination.controller.signal.aborted)
        return;
      onTranscriptRef.current(transcript);
      setError(null);
    } catch (cause) {
      if (current(destination)) {
        setError(
          cause instanceof Error
            ? cause.message
            : 'Speech transcription failed',
        );
        throw cause;
      }
    } finally {
      clearTimeout(timeout);
      try {
        file?.delete();
      } catch {
        /* Disposable cache file. */
      }
      if (target.current === destination) {
        chunks.current = [];
        target.current = null;
        abort.current = null;
        destination.live?.cancel();
        changePhase('idle');
        onPreviewRef.current?.(null);
      }
    }
  }
  stopRef.current = stop;
  return {
    phase,
    error,
    cloud,
    start,
    stop,
    discard,
    available: Platform.OS !== 'web' && !!stream,
  };
}
