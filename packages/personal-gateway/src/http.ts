import { createHash, timingSafeEqual } from 'node:crypto';
import type { DurableGatewayAuthority } from './durable-authority';
import type { DeviceGrant, ExternalEffect } from './authority';
import type { ComposioCatalog } from './composio';
import type { ComposioExecutor } from './composio-execution';

const maxBodyBytes = 70_000;

function json(value: unknown, status = 200): Response {
  return Response.json(value, {
    status,
    headers: { 'Cache-Control': 'no-store', 'Referrer-Policy': 'no-referrer' },
  });
}

function bearer(request: Request): string {
  const header = request.headers.get('authorization') ?? '';
  const match = /^Bearer ([A-Za-z0-9_-]{20,256})$/.exec(header);
  if (!match) throw new HttpFailure(401, 'Authorization required');
  return match[1]!;
}

function sameSecret(a: string, b: string): boolean {
  const hash = (value: string) => createHash('sha256').update(value).digest();
  return timingSafeEqual(hash(a), hash(b));
}

class HttpFailure extends Error {
  constructor(readonly status: number, message: string) {
    super(message);
  }
}

async function body(request: Request): Promise<Record<string, unknown>> {
  if (!request.headers.get('content-type')?.toLowerCase().startsWith('application/json')) {
    throw new HttpFailure(415, 'JSON body required');
  }
  const reader = request.body?.getReader();
  if (!reader) throw new HttpFailure(400, 'JSON body required');
  const chunks: Uint8Array[] = [];
  let size = 0;
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      size += value.byteLength;
      if (size > maxBodyBytes) throw new HttpFailure(413, 'Request body too large');
      chunks.push(value);
    }
  } finally {
    reader.releaseLock();
  }
  try {
    const parsed: unknown = JSON.parse(Buffer.concat(chunks).toString('utf8'));
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) throw new Error();
    return parsed as Record<string, unknown>;
  } catch {
    throw new HttpFailure(400, 'Invalid JSON object');
  }
}

function string(value: unknown): string {
  if (typeof value !== 'string') throw new HttpFailure(400, 'Invalid request body');
  return value;
}

function device(request: Request): { id: string; token: string } {
  const id = request.headers.get('x-waku-device-id') ?? '';
  if (!/^[0-9a-f-]{36}$/.test(id)) throw new HttpFailure(401, 'Device authorization required');
  return { id, token: bearer(request) };
}

function authorizedDevice(
  request: Request,
  authority: DurableGatewayAuthority,
  grant: DeviceGrant,
): { id: string; token: string } {
  const identity = device(request);
  try {
    authority.authenticate(identity.id, identity.token, grant);
  } catch {
    throw new HttpFailure(401, 'Device authorization required');
  }
  return identity;
}

/** Owner routes require a separate 32+ character secret; device tokens cannot use them. */
export function createGatewayHandler(
  authority: DurableGatewayAuthority,
  ownerSecret: string,
  composio?: ComposioCatalog,
  executor?: ComposioExecutor,
) {
  if (ownerSecret.length < 32 || !/^[A-Za-z0-9_-]+$/.test(ownerSecret)) {
    throw new Error('Owner token must be at least 32 URL-safe characters');
  }

  return async (request: Request): Promise<Response> => {
    try {
      const url = new URL(request.url);
      const path = url.pathname;
      const method = request.method;

      if (path === '/v1/health' && method === 'GET') return json({ status: 'ok' });

      if (path.startsWith('/v1/owner/')) {
        if (!sameSecret(bearer(request), ownerSecret)) {
          throw new HttpFailure(401, 'Authorization required');
        }
        if (path === '/v1/owner/devices' && method === 'GET') {
          return json({ devices: authority.listDevices() });
        }
        if (path === '/v1/owner/proposals' && method === 'GET') {
          return json({ proposals: authority.listProposals() });
        }
        if (path === '/v1/owner/connected-accounts' && method === 'GET') {
          if (!composio) return json({ error: 'Composio is not configured' }, 503);
          try {
            return json(await composio.listConnectedAccounts(url.searchParams.get('cursor') ?? undefined));
          } catch {
            return json({ error: 'Connected-account lookup failed' }, 502);
          }
        }
        if (path === '/v1/owner/invites' && method === 'POST') {
          const input = await body(request);
          const grants = input.grants;
          if (!Array.isArray(grants)) throw new HttpFailure(400, 'Invalid grants');
          const invite = authority.createInvite(grants as DeviceGrant[]);
          return json({ invite, expiresInSeconds: 300 }, 201);
        }
        const revoke = /^\/v1\/owner\/devices\/([0-9a-f-]{36})\/revoke$/.exec(path);
        if (revoke && method === 'POST') {
          authority.revokeDevice(revoke[1]!);
          return json({ revoked: true });
        }
        const decide = /^\/v1\/owner\/proposals\/([0-9a-f-]{36})\/decision$/.exec(path);
        if (decide && method === 'POST') {
          const input = await body(request);
          if (typeof input.approve !== 'boolean') throw new HttpFailure(400, 'Invalid decision');
          return json(authority.decide(decide[1]!, string(input.fingerprint), input.approve));
        }
        const execute = /^\/v1\/owner\/proposals\/([0-9a-f-]{36})\/execute$/.exec(path);
        if (execute && method === 'POST') {
          if (!executor) return json({ error: 'Composio is not configured' }, 503);
          const outcome = await executor.executeApproved(authority, execute[1]!);
          return json(outcome, outcome.status === 'succeeded' ? 200 : 409);
        }
      }

      if (path === '/v1/pair' && method === 'POST') {
        const input = await body(request);
        try {
          return json(authority.pair(string(input.invite), string(input.name)), 201);
        } catch {
          throw new HttpFailure(403, 'Pairing failed');
        }
      }

      if (path === '/v1/device/proposals' && method === 'POST') {
        const identity = authorizedDevice(request, authority, 'propose');
        const input = await body(request);
        const rawEffect = input.effect;
        if (!rawEffect || typeof rawEffect !== 'object' || Array.isArray(rawEffect)) {
          throw new HttpFailure(400, 'Invalid effect');
        }
        const effect = rawEffect as ExternalEffect;
        return json(authority.propose(identity.id, identity.token, effect), 201);
      }
      const inspect = /^\/v1\/device\/proposals\/([0-9a-f-]{36})$/.exec(path);
      if (inspect && method === 'GET') {
        const identity = authorizedDevice(request, authority, 'inspect');
        return json(authority.inspect(identity.id, identity.token, inspect[1]!));
      }

      return json({ error: 'Not found' }, 404);
    } catch (error) {
      if (error instanceof HttpFailure) return json({ error: error.message }, error.status);
      // Do not reflect internal state, credentials, or SQL errors to clients.
      return json({ error: 'Request rejected' }, 400);
    }
  };
}
