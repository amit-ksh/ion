import type { SpeechEndpoint } from './speech-service';
let endpoint: SpeechEndpoint | null = null;
export async function readSpeechEndpoint() { return endpoint; }
export async function saveSpeechEndpoint(value: SpeechEndpoint) { endpoint = value; }
export async function removeSpeechEndpoint() { endpoint = null; }

export const resolveSpeechEndpoint = readSpeechEndpoint;
