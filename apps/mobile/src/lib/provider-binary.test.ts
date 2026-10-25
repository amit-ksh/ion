import { expect, test } from 'bun:test';
import { providerStartBinary } from './provider-binary';

test('nebius starts without a binary path while CLI harnesses still need one', () => {
  expect(providerStartBinary('nebius', { installed: true, path: null })).toBe('');
  expect(() => providerStartBinary('nebius', { installed: false, path: null })).toThrow('not installed');
  expect(providerStartBinary('codex', { installed: true, path: '/bin/codex' })).toBe('/bin/codex');
  expect(() => providerStartBinary('codex', { installed: true, path: null })).toThrow('codex is not installed');
});
