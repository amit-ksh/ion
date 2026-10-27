import { expect, test } from 'bun:test';
import { parseDaemonPairingCode } from './daemon-pairing';

const input = {
  type: 'ion-pairing',
  version: 1,
  name: 'My desktop',
  address: 'ws://192.168.1.5:4096',
  token: 'test-secret',
};
test('scans the versioned desktop QR format and percent-encoded pairing links', () => {
  const expected = {
    name: input.name,
    address: input.address,
    token: input.token,
  };
  expect(parseDaemonPairingCode(JSON.stringify(input))).toEqual(expected);
  const params = new URLSearchParams(expected);
  expect(parseDaemonPairingCode(`ion://pair?${params}`)).toEqual(expected);
  expect(parseDaemonPairingCode(`waku://pair?${params}`)).toEqual(expected);
  expect(
    parseDaemonPairingCode(
      JSON.stringify({ ...input, address: 'wss://desktop.example' }),
    ).address,
  ).toBe('wss://desktop.example');
});

test('rejects unrelated, malformed and insecure QR codes without exposing secrets', () => {
  for (const data of [
    'https://example.com',
    'ion://other?token=test-secret',
    '{}',
    'null',
    'x'.repeat(8193),
    JSON.stringify({ ...input, version: 2 }),
    JSON.stringify({ ...input, token: '' }),
    JSON.stringify({ ...input, token: 'test-secret\nunsafe' }),
    ...[
      'ws://public.example',
      'file:///local',
      'ws://user:pass@192.168.1.5',
      'wss://desktop.example/path',
      'wss://desktop.example?token=secret',
    ].map((address) => JSON.stringify({ ...input, address })),
    'ion://pair?address=wss://desktop.example&token=test-secret&token=other',
  ]) {
    expect(() => parseDaemonPairingCode(data)).toThrow(
      'This is not a valid Ion desktop pairing code.',
    );
  }
});
