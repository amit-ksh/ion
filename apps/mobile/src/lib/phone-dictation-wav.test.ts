import { expect, test } from 'bun:test';
import { phoneDictationWav } from './phone-dictation-wav';

test('writes a canonical mono PCM WAV with an exact byte count', () => {
  const wav = phoneDictationWav([new Uint8Array([1, 2]), new Uint8Array([3, 4])], 16_000);
  expect(wav.length).toBe(48);
  expect(new TextDecoder().decode(wav.subarray(0, 4))).toBe('RIFF');
  expect(new DataView(wav.buffer).getUint32(40, true)).toBe(4);
  expect([...wav.subarray(44)]).toEqual([1, 2, 3, 4]);
});

test('rejects empty, odd, and overlong PCM before transfer', () => {
  expect(() => phoneDictationWav([], 16_000)).toThrow();
  expect(() => phoneDictationWav([new Uint8Array(3)], 16_000)).toThrow();
  expect(() => phoneDictationWav([new Uint8Array(16_000 * 2 * 61)], 16_000)).toThrow();
});
