import { afterEach, expect, test } from 'bun:test';
import { gatewayBaseUrl, pairGateway, gatewayReviewQueue, decideGatewayProposal } from './gateway-client';

const originalFetch = globalThis.fetch;
afterEach(() => { globalThis.fetch = originalFetch; });

test('gateway URL accepts trusted origins without path or embedded credentials', () => {
  expect(gatewayBaseUrl('https://gateway.example:8787/')).toBe('https://gateway.example:8787');
  expect(gatewayBaseUrl('http://127.0.0.1:8787/')).toBe('http://127.0.0.1:8787');
  expect(() => gatewayBaseUrl('http://gateway.example/')).toThrow('HTTPS');
  expect(() => gatewayBaseUrl('https://token@gateway.example/')).toThrow('without credentials');
  expect(() => gatewayBaseUrl('https://gateway.example/owner')).toThrow('without credentials');
});

test('phone pairs and sends its device token only to gateway review routes', async () => {
  const calls: Array<{ url: string; init: RequestInit }> = [];
  globalThis.fetch = (async (input, init = {}) => {
    calls.push({ url: String(input), init });
    if (String(input).endsWith('/v1/pair')) {
      return Response.json({
        id: '11111111-1111-4111-8111-111111111111', token: 'paired-device-token',
        expiresAt: Date.now() + 60_000, grants: ['review'],
      });
    }
    if (String(input).endsWith('/v1/device/review-queue')) return Response.json({ proposals: [] });
    return Response.json({ id: '22222222-2222-4222-8222-222222222222', status: 'denied' });
  }) as typeof fetch;
  const pairing = await pairGateway('https://gateway.example/', 'one-use-invite');
  expect((await gatewayReviewQueue(pairing))).toEqual([]);
  await decideGatewayProposal(pairing, {
    id: '22222222-2222-4222-8222-222222222222', deviceId: 'another-device',
    fingerprint: 'a'.repeat(64), status: 'pending',
    effect: { toolkit: 'github', tool: 'GITHUB_CREATE_ISSUE', version: 'v1',
      accountId: 'account', target: 'repo', body: '{}', scopes: [] },
  }, false);
  expect(calls.map((call) => call.url)).toEqual([
    'https://gateway.example/v1/pair',
    'https://gateway.example/v1/device/review-queue',
    'https://gateway.example/v1/device/proposals/22222222-2222-4222-8222-222222222222/decision',
  ]);
  expect(JSON.stringify(calls[0]?.init)).not.toContain('paired-device-token');
  expect(JSON.stringify(calls[1]?.init.headers)).toContain('paired-device-token');
  expect(calls.every((call) => call.init.redirect === 'error')).toBe(true);
});
