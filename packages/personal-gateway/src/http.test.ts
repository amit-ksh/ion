import { afterEach, describe, expect, test } from 'bun:test';
import { mkdtempSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { startGateway } from './server';

const ownerToken = 'owner_' + 'a'.repeat(48);
const effect = {
  toolkit: 'github', tool: 'GITHUB_CREATE_ISSUE', accountId: 'test-account',
  target: 'owner/repository', body: 'Exact issue body', scopes: ['issues:write'],
};
const allowedTools = [{
  toolkit: effect.toolkit, tool: effect.tool, accountId: effect.accountId,
  scopes: [...effect.scopes],
}];
const directories: string[] = [];
function databasePath(): string {
  const directory = mkdtempSync(join(tmpdir(), 'waku-gateway-http-'));
  directories.push(directory);
  return join(directory, 'authority.sqlite');
}
afterEach(() => {
  for (const directory of directories.splice(0)) rmSync(directory, { recursive: true, force: true });
});

function request(path: string, method: string, token?: string, data?: unknown, deviceId?: string): Request {
  const headers = new Headers();
  if (token) headers.set('Authorization', `Bearer ${token}`);
  if (deviceId) headers.set('X-Waku-Device-Id', deviceId);
  if (data !== undefined) headers.set('Content-Type', 'application/json');
  return new Request(`http://localhost${path}`, {
    method, headers, body: data === undefined ? undefined : JSON.stringify(data),
  });
}

describe('personal gateway HTTP boundary', () => {
  test('owner can invite, review, deny, and revoke without giving a device owner access', async () => {
    const running = startGateway({ databasePath: databasePath(), ownerToken, allowedTools });
    expect(running.server.url.hostname).toBe('127.0.0.1');
    const call = (path: string, method: string, token?: string, data?: unknown, deviceId?: string) =>
      running.server.fetch(request(path, method, token, data, deviceId));
    try {
      const health = await fetch(new URL('/v1/health', running.server.url));
      expect(health.status).toBe(200);
      const inviteResponse = await call('/v1/owner/invites', 'POST', ownerToken, {
        grants: ['inspect', 'propose'],
      });
      expect(inviteResponse.status).toBe(201);
      expect(inviteResponse.headers.get('cache-control')).toBe('no-store');
      const { invite } = await inviteResponse.json() as { invite: string };

      const pairedResponse = await call('/v1/pair', 'POST', undefined, { invite, name: 'Test desktop' });
      expect(pairedResponse.status).toBe(201);
      const device = await pairedResponse.json() as { id: string; token: string };
      expect((await call('/v1/pair', 'POST', undefined, { invite, name: 'Replay' })).status).toBe(403);
      expect((await call('/v1/owner/devices', 'GET', device.token)).status).toBe(401);

      const proposed = await call('/v1/device/proposals', 'POST', device.token, { effect }, device.id);
      expect(proposed.status).toBe(201);
      const proposal = await proposed.json() as { id: string; fingerprint: string };
      const listed = await (await call('/v1/owner/proposals', 'GET', ownerToken)).json() as {
        proposals: Array<{ id: string; effect: typeof effect }>;
      };
      expect(listed.proposals[0]?.id).toBe(proposal.id);
      expect(listed.proposals[0]?.effect).toEqual(effect);

      expect((await call(`/v1/owner/proposals/${proposal.id}/decision`, 'POST', device.token, {
        fingerprint: proposal.fingerprint, approve: true,
      })).status).toBe(401);
      const denial = await call(`/v1/owner/proposals/${proposal.id}/decision`, 'POST', ownerToken, {
        fingerprint: proposal.fingerprint, approve: false,
      });
      expect((await denial.json() as { status: string }).status).toBe('denied');
      const inspected = await call(`/v1/device/proposals/${proposal.id}`, 'GET', device.token, undefined, device.id);
      expect((await inspected.json() as { status: string }).status).toBe('denied');

      expect((await call(`/v1/owner/devices/${device.id}/revoke`, 'POST', ownerToken)).status).toBe(200);
      expect((await call(`/v1/device/proposals/${proposal.id}`, 'GET', device.token, undefined, device.id)).status).toBe(401);
    } finally {
      await running.close();
    }
  });

  test('non-loopback binding requires TLS before opening a database', () => {
    expect(() => startGateway({
      databasePath: databasePath(), ownerToken, allowedTools, hostname: '0.0.0.0',
    })).toThrow('requires TLS');
  });
});
