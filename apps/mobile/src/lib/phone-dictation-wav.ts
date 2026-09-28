/** Convert Expo's mono int16 PCM buffers to the exact WAV accepted by ion. */
export function phoneDictationWav(chunks: Uint8Array[], sampleRate: number): Uint8Array {
  const length = chunks.reduce((size, chunk) => size + chunk.byteLength, 0);
  if (!Number.isInteger(sampleRate) || sampleRate < 16_000 || sampleRate > 48_000 ||
      length === 0 || length % 2 !== 0 || length > sampleRate * 2 * 60 ||
      length + 44 > 8 * 1024 * 1024) {
    throw new Error('Record up to 60 seconds of mono microphone audio');
  }
  const wav = new Uint8Array(length + 44);
  const view = new DataView(wav.buffer);
  const label = (offset: number, value: string) => {
    for (let index = 0; index < value.length; index++) wav[offset + index] = value.charCodeAt(index);
  };
  label(0, 'RIFF');
  view.setUint32(4, length + 36, true);
  label(8, 'WAVEfmt ');
  view.setUint32(16, 16, true);
  view.setUint16(20, 1, true);
  view.setUint16(22, 1, true);
  view.setUint32(24, sampleRate, true);
  view.setUint32(28, sampleRate * 2, true);
  view.setUint16(32, 2, true);
  view.setUint16(34, 16, true);
  label(36, 'data');
  view.setUint32(40, length, true);
  let offset = 44;
  for (const chunk of chunks) { wav.set(chunk, offset); offset += chunk.byteLength; }
  return wav;
}
