import type { WakuClient } from '@waku/client';
import { requestRecordingPermissionsAsync, setAudioModeAsync, useAudioStream } from 'expo-audio';
import * as Crypto from 'expo-crypto';
import { File, Paths } from 'expo-file-system';
import { useEffect, useRef, useState } from 'react';
import { Platform } from 'react-native';

import { transcribePhoneWav } from '@/lib/daemon-api';
import { phoneDictationWav } from '@/lib/phone-dictation-wav';

type Phase = 'idle' | 'preparing' | 'recording' | 'transcribing';
let microphoneOwner: symbol | null = null;

/** Explicit short capture; bytes go only to the selected authenticated daemon. */
export function usePhoneDictation(
  client: WakuClient | null,
  contextKey: string,
  onTranscript: (text: string) => void,
) {
  const [phase, setPhase] = useState<Phase>('idle');
  const [error, setError] = useState<string | null>(null);
  const owner = useRef(Symbol('phone dictation'));
  const ownsMicrophone = () => microphoneOwner === owner.current;
  const releaseMicrophone = () => {
    if (!ownsMicrophone()) return;
    microphoneOwner = null;
    void setAudioModeAsync({ allowsRecording: false }).catch(() => {});
  };
  const chunks = useRef<Uint8Array[]>([]);
  const byteCount = useRef(0);
  const sampleRate = useRef(0);
  const active = useRef(false);
  const starting = useRef(false);
  const mounted = useRef(true);
  const tooLong = useRef(false);
  const onTranscriptRef = useRef(onTranscript);
  onTranscriptRef.current = onTranscript;
  const currentContext = useRef(contextKey);
  currentContext.current = contextKey;
  const currentClient = useRef(client);
  currentClient.current = client;
  const target = useRef<{ client: WakuClient; contextKey: string } | null>(null);
  const { stream } = useAudioStream({
    encoding: 'int16', channels: 1, sampleRate: 16_000,
    onBuffer: (buffer) => {
      if (!active.current) return;
      if (buffer.channels !== 1 || !Number.isInteger(buffer.sampleRate) ||
          buffer.sampleRate < 16_000 || buffer.sampleRate > 48_000 ||
          (sampleRate.current && sampleRate.current !== buffer.sampleRate)) {
        tooLong.current = true;
        active.current = false;
        stream.stop();
        chunks.current = [];
        target.current = null;
        releaseMicrophone();
        setPhase('idle');
        setError('Microphone format changed. Try recording again.');
        return;
      }
      const next = new Uint8Array(buffer.data).slice();
      sampleRate.current = buffer.sampleRate;
      byteCount.current += next.byteLength;
      if (byteCount.current > buffer.sampleRate * 2 * 60 || byteCount.current + 44 > 8 * 1024 * 1024) {
        tooLong.current = true;
        active.current = false;
        stream.stop();
        chunks.current = [];
        target.current = null;
        releaseMicrophone();
        setPhase('idle');
        setError('Recording reached the 60-second limit. Start a shorter dictation.');
        return;
      }
      chunks.current.push(next);
    },
  });

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      if (active.current) stream?.stop();
      active.current = false;
      chunks.current = [];
      target.current = null;
      releaseMicrophone();
    };
  }, [stream]);

  async function start() {
    if (Platform.OS === 'web' || !stream || !client || phase !== 'idle' || starting.current) return;
    if (microphoneOwner !== null) throw new Error('Another dictation is already using the microphone');
    microphoneOwner = owner.current;
    starting.current = true;
    setPhase('preparing');
    setError(null);
    try {
      const permission = await requestRecordingPermissionsAsync();
      if (!permission.granted) throw new Error('Microphone permission is required for dictation');
      if (!mounted.current || currentClient.current !== client || currentContext.current !== contextKey) {
        throw new Error('Task or daemon changed before recording. Try again.');
      }
      await setAudioModeAsync({ allowsRecording: true, playsInSilentMode: true });
      chunks.current = [];
      byteCount.current = 0;
      sampleRate.current = 0;
      tooLong.current = false;
      target.current = { client, contextKey };
      active.current = true;
      await stream.start();
      if (!mounted.current || currentClient.current !== client || currentContext.current !== contextKey) {
        stream.stop();
        throw new Error('Task or daemon changed during recording. Audio was discarded.');
      }
      setPhase('recording');
    } catch (cause) {
      active.current = false;
      target.current = null;
      chunks.current = [];
      if (mounted.current) setPhase('idle');
      releaseMicrophone();
      throw cause;
    } finally { starting.current = false; }
  }

  async function stop() {
    if (!stream || !active.current || phase !== 'recording') return;
    active.current = false;
    stream.stop();
    releaseMicrophone();
    if (tooLong.current) return;
    const destination = target.current;
    if (!client || destination?.client !== client || destination.contextKey !== currentContext.current) {
      chunks.current = [];
      target.current = null;
      setPhase('idle');
      throw new Error('Task or daemon changed. Audio was discarded.');
    }
    setPhase('transcribing');
    const file = new File(Paths.cache, `ion-dictation-${Crypto.randomUUID()}.wav`);
    try {
      const wav = phoneDictationWav(chunks.current, sampleRate.current);
      chunks.current = [];
      file.create();
      file.write(wav);
      const base64 = await file.base64();
      if (!mounted.current || currentClient.current !== destination.client ||
          currentContext.current !== destination.contextKey) {
        throw new Error('Task or daemon changed. Audio was discarded.');
      }
      const transcript = await transcribePhoneWav(destination.client, base64);
      if (!mounted.current || target.current !== destination ||
          currentClient.current !== destination.client ||
          currentContext.current !== destination.contextKey) {
        throw new Error('Task or daemon changed. Transcription was discarded.');
      }
      onTranscriptRef.current(transcript);
      setError(null);
    } catch (cause) {
      if (mounted.current) setError(cause instanceof Error ? cause.message : String(cause));
      throw cause;
    } finally {
      try { file.delete(); } catch { /* The cache directory is disposable. */ }
      target.current = null;
      if (mounted.current) setPhase('idle');
    }
  }

  function discard() {
    if (active.current) stream?.stop();
    releaseMicrophone();
    active.current = false;
    chunks.current = [];
    target.current = null;
    setPhase('idle');
    setError(null);
  }

  return { phase, error, start, stop, discard, available: Platform.OS !== 'web' && !!stream };
}
