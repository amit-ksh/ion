import { fetch } from 'expo/fetch';
import { File } from 'expo-file-system';
export const SPEECH_MODEL = 'nvidia/nemotron-3.5-asr-streaming-0.6b';
import { speechUrl, type SpeechEndpoint } from './speech-endpoint';
export { speechUrl, type SpeechEndpoint } from './speech-endpoint';
export async function testSpeechService(endpoint: SpeechEndpoint): Promise<void> {
  const response = await fetch(`${speechUrl(endpoint.baseUrl)}/v1/models`, {
    headers: { Authorization: `Bearer ${endpoint.token}` }, redirect: 'error', signal: AbortSignal.timeout(15_000),
  });
  if (!response.ok) throw new Error(`Speech service connection failed (HTTP ${response.status})`);
  const data = await response.json() as { data?: { id?: string }[] };
  if (!data.data?.some(model => /nemotron-3[._]5/.test(model.id ?? ''))) {
    throw new Error('Load the Nemotron 3.5 ASR model on the speech service');
  }
}
export async function transcribeCloudWav(endpoint: SpeechEndpoint, uri: string, signal: AbortSignal): Promise<string> {
  const form = new FormData();
  // Expo File implements bytes/arrayBuffer; RN's Blob type also requires an unrelated image property.
  form.append('file', new File(uri) as unknown as Blob, 'dictation.wav');
  form.append('model', SPEECH_MODEL);
  form.append('response_format', 'json');
  form.append('language', 'auto');
  const response = await fetch(`${speechUrl(endpoint.baseUrl)}/v1/audio/transcriptions`, {
    method: 'POST', headers: { Authorization: `Bearer ${endpoint.token}` }, body: form, signal, redirect: 'error',
  });
  if (!response.ok) throw new Error(`Speech transcription failed (HTTP ${response.status})`);
  const result = await response.json() as { text?: unknown };
  if (typeof result.text !== 'string' || !result.text.trim() || result.text.length > 65_536) throw new Error('No valid speech transcript returned');
  return result.text.trim();
}
