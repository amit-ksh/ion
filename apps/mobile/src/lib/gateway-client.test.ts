import { afterEach, expect, test } from 'bun:test';
import { gatewayBaseUrl, pairGateway, gatewayReviewQueue, decideGatewayProposal, gatewayAutomationOwners, gatewayEnqueueAutomation, gatewayAutomationCommandByKey } from './gateway-client';

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

test('workflow routes send only the phone grant and reject invalid or duplicate owner projections', async () => {
  const pairing = { baseUrl: 'https://gateway.example', id: '11111111-1111-4111-8111-111111111111', token: 'workflow-device-token', expiresAt: Date.now() + 100_000, grants: ['automationRead','automationControl'] };
  const calls: { url: string; init: RequestInit }[] = [];
  const owner = { version: 1 as const, revision: 3, publisherId: '22222222-2222-4222-8222-222222222222', name: 'Desktop', publishedAt: Date.now(), truncated: false, projects: [], definitions: [], runs: [] };
  let owners: unknown[] = [owner];
  globalThis.fetch = (async (input, init = {}) => {
    calls.push({ url: String(input), init });
    return Response.json(String(input).endsWith('/automations') ? { owners } : String(input).includes('?key=') ? { command: null } : { id: '33333333-3333-4333-8333-333333333333' });
  }) as typeof fetch;
  expect(await gatewayAutomationOwners(pairing)).toEqual([owner]);
  const input = { publisherId: owner.publisherId, idempotencyKey: '44444444-4444-4444-8444-444444444444', fingerprint: 'a'.repeat(64), action: { kind: 'start' as const, runId: '55555555-5555-4555-8555-555555555555', definitionId: '66666666-6666-4666-8666-666666666666', expectedRevision: 3 } };
  await gatewayEnqueueAutomation(pairing, input); expect(await gatewayAutomationCommandByKey(pairing, input.idempotencyKey)).toBeNull();
  expect(calls[1]?.init.body).toBe(JSON.stringify(input)); expect(calls.every((call) => call.init.redirect === 'error')).toBe(true);
  expect(JSON.stringify(calls[1]?.init.headers)).toContain(pairing.token);
  expect(calls[2]?.url).toContain(`/automations/commands?key=${input.idempotencyKey}`);
  owners = [owner, owner]; await expect(gatewayAutomationOwners(pairing)).rejects.toThrow('Invalid workflow feed');
  owners = [{ ...owner, definitions: [{ id: input.action.definitionId }] }]; await expect(gatewayAutomationOwners(pairing)).rejects.toThrow('Invalid workflow feed');
  owners = [{ ...owner, runs: [{ id: input.action.runId }] }]; await expect(gatewayAutomationOwners(pairing)).rejects.toThrow('Invalid workflow feed');
});
