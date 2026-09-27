import { afterEach, describe, expect, test } from 'bun:test';
import { mkdtempSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { startGateway } from './server';
import { ComposioCatalog } from './composio';
import { ComposioExecutor } from './composio-execution';

const ownerToken = 'owner_' + 'a'.repeat(48);
const effect = {
  toolkit: 'github', tool: 'GITHUB_CREATE_ISSUE', version: '20251027_00', accountId: 'test-account',
  target: 'owner/repository', body: '{"title":"Exact issue body"}', scopes: ['issues:write'],
};
const allowedTools = [{
  toolkit: effect.toolkit, tool: effect.tool, version: effect.version, accountId: effect.accountId,
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
      const consolePage = await call('/owner', 'GET');
      expect(consolePage.status).toBe(200);
      expect(consolePage.headers.get('content-security-policy')).toContain("script-src 'self'");
      expect(await consolePage.text()).toContain('Pending approvals');
      expect((await call('/owner/app.js', 'GET')).status).toBe(200);
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
      expect((await call('/v1/owner/allowlist', 'GET', device.token)).status).toBe(401);
      expect(await (await call('/v1/owner/allowlist', 'GET', ownerToken)).json()).toEqual({ tools: allowedTools });
      const session = await call('/v1/device/session', 'GET', device.token, undefined, device.id);
      expect((await session.json() as { device: { id: string } }).device.id).toBe(device.id);
      expect((await call('/v1/device/session', 'GET', ownerToken, undefined, device.id)).status).toBe(401);

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

  test('a review-only phone can decide an exact proposal but cannot execute it', async () => {
    const running = startGateway({ databasePath: databasePath(), ownerToken, allowedTools });
    const call = (path: string, method: string, token?: string, data?: unknown, deviceId?: string) =>
      running.server.fetch(request(path, method, token, data, deviceId));
    try {
      const pair = async (grants: string[], name: string) => {
        const { invite } = await (await call('/v1/owner/invites', 'POST', ownerToken, { grants })).json() as { invite: string };
        return (await (await call('/v1/pair', 'POST', undefined, { invite, name })).json()) as { id: string; token: string };
      };
      const desktop = await pair(['inspect', 'propose'], 'Desktop');
      const phone = await pair(['review'], 'Phone');
      const proposal = await (await call('/v1/device/proposals', 'POST', desktop.token, { effect }, desktop.id)).json() as { id: string; fingerprint: string };
      expect((await call('/v1/device/review-queue', 'GET', desktop.token, undefined, desktop.id)).status).toBe(401);
      expect((await call('/v1/device/proposals', 'POST', phone.token, { effect }, phone.id)).status).toBe(401);
      expect((await call('/v1/device/session', 'GET', phone.token, undefined, phone.id)).status).toBe(200);
      const queue = await (await call('/v1/device/review-queue', 'GET', phone.token, undefined, phone.id)).json() as { proposals: Array<{ id: string; effect: typeof effect }> };
      expect(queue.proposals).toEqual([expect.objectContaining({ id: proposal.id, effect })]);
      expect((await call(`/v1/device/proposals/${proposal.id}/decision`, 'POST', phone.token, {
        fingerprint: '0'.repeat(64), approve: true,
      }, phone.id)).status).toBe(400);
      const decided = await call(`/v1/device/proposals/${proposal.id}/decision`, 'POST', phone.token, {
        fingerprint: proposal.fingerprint, approve: true,
      }, phone.id);
      expect((await decided.json() as { status: string; reviewedBy: string }).status).toBe('approved');
      expect((await call(`/v1/device/proposals/${proposal.id}/decision`, 'POST', phone.token, {
        fingerprint: proposal.fingerprint, approve: true,
      }, phone.id)).status).toBe(200);
      expect((await call(`/v1/owner/proposals/${proposal.id}/execute`, 'POST', phone.token, undefined, phone.id)).status).toBe(401);
      expect((await (await call('/v1/device/review-queue', 'GET', phone.token, undefined, phone.id)).json() as { proposals: unknown[] }).proposals).toEqual([]);
      await call(`/v1/owner/devices/${phone.id}/revoke`, 'POST', ownerToken);
      expect((await call('/v1/device/review-queue', 'GET', phone.token, undefined, phone.id)).status).toBe(401);
    } finally {
      await running.close();
    }
  });

  test('desktop publish and phone task grants stay separate at the HTTP boundary', async () => {
    const running = startGateway({ databasePath: databasePath(), ownerToken, allowedTools });
    const call = (path: string, method: string, token?: string, data?: unknown, deviceId?: string) =>
      running.server.fetch(request(path, method, token, data, deviceId));
    try {
      const pair = async (grants: string[]) => {
        const { invite } = await (await call('/v1/owner/invites', 'POST', ownerToken, { grants })).json() as { invite: string };
        return (await (await call('/v1/pair', 'POST', undefined, { invite, name: grants.join('-') })).json()) as { id: string; token: string };
      };
      const desktop = await pair(['publish']);
      const phone = await pair(['taskRead', 'taskControl']);
      expect((await call('/v1/device/session', 'GET', desktop.token, undefined, desktop.id)).status).toBe(200);
      expect((await call('/v1/device/session', 'GET', phone.token, undefined, phone.id)).status).toBe(200);
      const task = {
        id: '11111111-1111-4111-8111-111111111111', projectId: '22222222-2222-4222-8222-222222222222',
        projectName: 'Test project', title: 'Live task', provider: 'nebius', status: 'running',
        runtimeId: '33333333-3333-4333-8333-333333333333', updatedAt: 10,
      };
      expect((await call(`/v1/device/tasks/${task.id}`, 'PUT', phone.token, { task }, phone.id)).status).toBe(401);
      expect((await call(`/v1/device/tasks/${task.id}`, 'PUT', desktop.token, { task }, desktop.id)).status).toBe(200);
      expect((await call('/v1/device/heartbeat', 'POST', phone.token, {}, phone.id)).status).toBe(401);
      expect((await call('/v1/device/heartbeat', 'POST', desktop.token, {}, desktop.id)).status).toBe(200);
      expect((await call('/v1/device/tasks', 'GET', desktop.token, undefined, desktop.id)).status).toBe(401);
      expect(await (await call('/v1/device/tasks', 'GET', phone.token, undefined, phone.id)).json()).toEqual({ tasks: [{ ...task, publisherSeenAt: expect.any(Number) }], publishedAt: expect.any(Number) });
      const event = await call('/v1/device/events', 'POST', desktop.token, {
        eventId: 'runtime:1', taskId: task.id, kind: 'artifact', text: 'Review result', at: 11,
      }, desktop.id);
      expect(event.status).toBe(201);
      const feed = await (await call('/v1/device/events?after=0', 'GET', phone.token, undefined, phone.id)).json() as { events: Array<{ kind: string }> };
      expect(feed.events.map((entry) => entry.kind)).toEqual(['status', 'artifact']);
      const commandResponse = await call('/v1/device/commands', 'POST', phone.token, {
        idempotencyKey: '44444444-4444-4444-8444-444444444444', taskId: task.id,
        runtimeId: task.runtimeId, kind: 'stop', payload: {},
      }, phone.id);
      expect(commandResponse.status).toBe(201);
      const command = await commandResponse.json() as { id: string; status: string };
      expect(command.status).toBe('queued');
      expect((await call('/v1/device/commands', 'GET', phone.token, undefined, phone.id)).status).toBe(401);
      expect((await call(`/v1/device/commands/${command.id}/claim`, 'POST', phone.token, {}, phone.id)).status).toBe(401);
      expect((await call(`/v1/device/commands/${command.id}/claim`, 'POST', desktop.token, {}, desktop.id)).status).toBe(200);
      expect((await (await call(`/v1/device/commands/${command.id}`, 'GET', phone.token, undefined, phone.id)).json() as { status: string }).status).toBe('uncertain');
      expect((await call(`/v1/device/commands/${command.id}/claim`, 'POST', desktop.token, {}, desktop.id)).status).toBe(400);
      expect((await call('/v1/device/tasks/reconcile', 'POST', phone.token, { activeIds: [] }, phone.id)).status).toBe(401);
      expect((await call('/v1/device/tasks/reconcile', 'POST', desktop.token, { activeIds: [] }, desktop.id)).status).toBe(200);
      expect((await (await call('/v1/device/tasks', 'GET', phone.token, undefined, phone.id)).json() as { tasks: unknown[] }).tasks).toEqual([]);
    } finally {
      await running.close();
    }
  });

  test('connected-account lookup strips credential state and remains owner-only', async () => {
    let requestedUrl = '';
    let keyHeader = '';
    const mockFetch = (async (input: Parameters<typeof fetch>[0], init?: RequestInit) => {
      requestedUrl = String(input);
      keyHeader = new Headers(init?.headers).get('x-api-key') ?? '';
      return Response.json({
        items: [{
          id: 'test-account', toolkit: { slug: 'github' }, status: 'ACTIVE',
          state: { val: { oauth_token: 'never-expose-this' } },
        }, { id: 'ungranted-account', toolkit: { slug: 'slack' }, status: 'ACTIVE' }],
        next_cursor: 'next-page',
      });
    }) as typeof fetch;
    const running = startGateway({
      databasePath: databasePath(), ownerToken, allowedTools,
      composio: new ComposioCatalog('test-project-key', mockFetch),
    });
    try {
      const unauthorized = await running.server.fetch(request('/v1/owner/connected-accounts', 'GET'));
      expect(unauthorized.status).toBe(401);
      const response = await running.server.fetch(request('/v1/owner/connected-accounts', 'GET', ownerToken));
      const result = await response.json();
      expect(result).toEqual({
        items: [
          { id: 'test-account', toolkit: 'github', status: 'ACTIVE' },
          { id: 'ungranted-account', toolkit: 'slack', status: 'ACTIVE' },
        ],
        nextCursor: 'next-page',
      });
      expect(JSON.stringify(result)).not.toContain('never-expose-this');
      expect(requestedUrl).toBe('https://backend.composio.dev/api/v3.1/connected_accounts?limit=100');
      expect(keyHeader).toBe('test-project-key');
      const { invite } = await (await running.server.fetch(request('/v1/owner/invites', 'POST', ownerToken, {
        grants: ['inspect'],
      }))).json() as { invite: string };
      const device = await (await running.server.fetch(request('/v1/pair', 'POST', undefined, {
        invite, name: 'Account viewer',
      }))).json() as { id: string; token: string };
      const deviceResponse = await running.server.fetch(request('/v1/device/connected-accounts', 'GET', device.token, undefined, device.id));
      expect(await deviceResponse.json()).toEqual({
        items: [{ id: 'test-account', toolkit: 'github', status: 'ACTIVE' }],
        nextCursor: 'next-page',
      });
      expect((await running.server.fetch(request('/v1/device/connected-accounts', 'GET', ownerToken, undefined, device.id))).status).toBe(401);
    } finally {
      await running.close();
    }
  });

  test('only the owner can execute an approved proposal once', async () => {
    let calls = 0;
    const executor = new ComposioExecutor('test-project-key', (async () => {
      calls++;
      return Response.json({ successful: true, log_id: 'log_http' });
    }));
    const running = startGateway({ databasePath: databasePath(), ownerToken, allowedTools, executor });
    const call = (path: string, method: string, token?: string, data?: unknown, deviceId?: string) =>
      running.server.fetch(request(path, method, token, data, deviceId));
    try {
      const { invite } = await (await call('/v1/owner/invites', 'POST', ownerToken, {
        grants: ['inspect', 'propose'],
      })).json() as { invite: string };
      const device = await (await call('/v1/pair', 'POST', undefined, {
        invite, name: 'Test desktop',
      })).json() as { id: string; token: string };
      const proposal = await (await call('/v1/device/proposals', 'POST', device.token, {
        effect,
      }, device.id)).json() as { id: string; fingerprint: string };
      const path = `/v1/owner/proposals/${proposal.id}/execute`;
      expect((await call(path, 'POST', device.token)).status).toBe(401);
      expect(calls).toBe(0);
      expect((await call(`/v1/owner/proposals/${proposal.id}/decision`, 'POST', ownerToken, {
        fingerprint: proposal.fingerprint, approve: true,
      })).status).toBe(200);
      const executed = await call(path, 'POST', ownerToken);
      expect(executed.status).toBe(200);
      expect(await executed.json()).toEqual({ status: 'succeeded', resultReference: 'log_http' });
      expect((await call(path, 'POST', ownerToken)).status).toBe(400);
      expect(calls).toBe(1);
    } finally {
      await running.close();
    }
  });
});
