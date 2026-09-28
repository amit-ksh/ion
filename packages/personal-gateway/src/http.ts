import { createHash, timingSafeEqual } from 'node:crypto';
import type { DurableGatewayAuthority } from './durable-authority';
import type { DeviceGrant, ExternalEffect } from './authority';
import type { ComposioCatalog } from './composio';
import type { ComposioExecutor } from './composio-execution';
import { ownerCss, ownerHtml, ownerJs } from './owner-ui';
import type { GatewayPush } from './push';
import { DurableTaskRelay, type RelayCommandKind, type RelayComputerTarget, type RelayEvent, type RelayInteractionInput, type RelayTask } from './task-relay';

const maxBodyBytes = 70_000;

function json(value: unknown, status = 200): Response {
  return Response.json(value, {
    status,
    headers: { 'Cache-Control': 'no-store', 'Referrer-Policy': 'no-referrer' },
  });
}

function ownerAsset(value: string, type: string): Response {
  return new Response(value, { headers: {
    'Content-Type': type,
    'Cache-Control': 'no-store',
    'Referrer-Policy': 'no-referrer',
    'X-Content-Type-Options': 'nosniff',
    'X-Frame-Options': 'DENY',
    'Content-Security-Policy': "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; form-action 'none'; frame-ancestors 'none'",
  } });
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
  relay: DurableTaskRelay,
  push: GatewayPush,
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
      if (path === '/owner' && method === 'GET') return ownerAsset(ownerHtml, 'text/html; charset=utf-8');
      if (path === '/owner/style.css' && method === 'GET') return ownerAsset(ownerCss, 'text/css; charset=utf-8');
      if (path === '/owner/app.js' && method === 'GET') return ownerAsset(ownerJs, 'text/javascript; charset=utf-8');

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
        if (path === '/v1/owner/allowlist' && method === 'GET') {
          return json({ tools: authority.listAllowedTools() });
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
          push.unregister(revoke[1]!);
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
      if (path === '/v1/device/tasks' && method === 'GET') {
        authorizedDevice(request, authority, 'taskRead');
        return json(relay.listTasks());
      }
      if (path === '/v1/device/push' && method === 'GET') {
        const identity = authorizedDevice(request, authority, 'taskRead');
        return json({ enabled: push.isRegistered(identity.id) });
      }
      if (path === '/v1/device/push' && method === 'PUT') {
        const identity = authorizedDevice(request, authority, 'taskRead');
        const input = await body(request);
        push.register(identity.id, string(input.token));
        return json({ enabled: true });
      }
      if (path === '/v1/device/push' && method === 'DELETE') {
        const identity = authorizedDevice(request, authority, 'taskRead');
        push.unregister(identity.id);
        return json({ enabled: false });
      }
      if (path === '/v1/device/heartbeat' && method === 'POST') {
        const identity = authorizedDevice(request, authority, 'publish');
        return json(relay.heartbeat(identity.id));
      }
      if (path === '/v1/device/computer-target' && method === 'POST') {
        const identity = authorizedDevice(request, authority, 'publish');
        const input = await body(request);
        return json(relay.setComputerTarget(identity.id, input.target as RelayComputerTarget));
      }
      const publishTask = /^\/v1\/device\/tasks\/([0-9a-f-]{36})$/.exec(path);
      if (publishTask && method === 'PUT') {
        const identity = authorizedDevice(request, authority, 'publish');
        const input = await body(request);
        const task = input.task as RelayTask;
        if (!task || task.id !== publishTask[1]) throw new HttpFailure(400, 'Invalid task');
        return json(relay.publishTask(task, identity.id));
      }
      if (path === '/v1/device/tasks/reconcile' && method === 'POST') {
        const identity = authorizedDevice(request, authority, 'publish');
        const input = await body(request);
        if (!Array.isArray(input.activeIds)) throw new HttpFailure(400, 'Invalid task snapshot');
        return json(relay.reconcileTasks(identity.id, input.activeIds as string[]));
      }
      if (path === '/v1/device/events' && method === 'GET') {
        authorizedDevice(request, authority, 'taskRead');
        const after = Number(url.searchParams.get('after') ?? '0');
        return json(relay.events(after));
      }
      if (path === '/v1/device/events' && method === 'POST') {
        authorizedDevice(request, authority, 'publish');
        const input = await body(request);
        return json(relay.appendEvent(
          string(input.eventId), string(input.taskId), string(input.kind) as RelayEvent['kind'],
          string(input.text), Number(input.at),
        ), 201);
      }
      if (path === '/v1/device/interactions' && method === 'POST') {
        const identity = authorizedDevice(request, authority, 'publish');
        const input = await body(request);
        const interaction = relay.publishInteraction(identity.id, input.interaction as RelayInteractionInput);
        if (interaction.created) push.enqueue(interaction.taskId, `interaction:${interaction.id}`);
        return json(interaction, 201);
      }
      if (path === '/v1/device/interactions/resolve' && method === 'POST') {
        const identity = authorizedDevice(request, authority, 'publish');
        const input = await body(request);
        return json({ interaction: relay.resolveInteractionRequest(identity.id,
          string(input.taskId), string(input.runtimeId), string(input.requestId)) });
      }
      const taskInteractions = /^\/v1\/device\/tasks\/([0-9a-f-]{36})\/interactions$/.exec(path);
      if (taskInteractions && method === 'GET') {
        authorizedDevice(request, authority, 'taskRead');
        return json({ interactions: relay.pendingInteractions(taskInteractions[1]!) });
      }
      const readInteraction = /^\/v1\/device\/interactions\/([0-9a-f-]{36})$/.exec(path);
      if (readInteraction && method === 'GET') {
        const identity = authorizedDevice(request, authority, 'publish');
        const interaction = relay.interactionForPublisher(identity.id, readInteraction[1]!);
        if (!interaction) throw new HttpFailure(404, 'Interaction not found');
        return json(interaction);
      }
      const resolveInteraction = /^\/v1\/device\/interactions\/([0-9a-f-]{36})\/resolve$/.exec(path);
      if (resolveInteraction && method === 'POST') {
        const identity = authorizedDevice(request, authority, 'publish');
        return json(relay.resolveInteraction(identity.id, resolveInteraction[1]!));
      }
      if (path === '/v1/device/commands' && method === 'GET') {
        authorizedDevice(request, authority, 'publish');
        return json({ commands: relay.pendingCommands() });
      }
      if (path === '/v1/device/commands' && method === 'POST') {
        const identity = authorizedDevice(request, authority, 'taskControl');
        const input = await body(request);
        return json(relay.enqueue(
          identity.id, string(input.idempotencyKey), string(input.taskId),
          string(input.runtimeId), string(input.kind) as RelayCommandKind,
          input.payload as Record<string, string>,
        ), 201);
      }
      const readCommand = /^\/v1\/device\/commands\/([0-9a-f-]{36})$/.exec(path);
      if (readCommand && method === 'GET') {
        const identity = authorizedDevice(request, authority, 'taskControl');
        const command = relay.command(readCommand[1]!);
        if (!command || command.deviceId !== identity.id) throw new HttpFailure(404, 'Command not found');
        return json(command);
      }
      const claimCommand = /^\/v1\/device\/commands\/([0-9a-f-]{36})\/claim$/.exec(path);
      if (claimCommand && method === 'POST') {
        authorizedDevice(request, authority, 'publish');
        return json(relay.claim(claimCommand[1]!));
      }
      const settleCommand = /^\/v1\/device\/commands\/([0-9a-f-]{36})\/settle$/.exec(path);
      if (settleCommand && method === 'POST') {
        authorizedDevice(request, authority, 'publish');
        const input = await body(request);
        if (typeof input.applied !== 'boolean') throw new HttpFailure(400, 'Invalid command result');
        return json(relay.settle(settleCommand[1]!, input.applied));
      }
      if (path === '/v1/device/review-queue' && method === 'GET') {
        const identity = authorizedDevice(request, authority, 'review');
        return json({ proposals: authority.reviewQueue(identity.id, identity.token) });
      }
      const reviewStatus = /^\/v1\/device\/reviews\/([0-9a-f-]{36})$/.exec(path);
      if (reviewStatus && method === 'GET') {
        const identity = authorizedDevice(request, authority, 'review');
        return json(authority.reviewStatus(identity.id, identity.token, reviewStatus[1]!));
      }
      const deviceDecision = /^\/v1\/device\/proposals\/([0-9a-f-]{36})\/decision$/.exec(path);
      if (deviceDecision && method === 'POST') {
        const identity = authorizedDevice(request, authority, 'review');
        const input = await body(request);
        if (typeof input.approve !== 'boolean') throw new HttpFailure(400, 'Invalid decision');
        return json(authority.review(
          identity.id, identity.token, deviceDecision[1]!, string(input.fingerprint), input.approve,
        ));
      }
      if (path === '/v1/device/session' && method === 'GET') {
        const identity = device(request);
        // Session inspection is available to any currently paired device,
        // including review-only phones.
        const grants: DeviceGrant[] = ['inspect', 'propose', 'review', 'publish', 'taskRead', 'taskControl'];
        if (!grants.some((grant) => {
          try { authority.authenticate(identity.id, identity.token, grant); return true; }
          catch { return false; }
        })) throw new HttpFailure(401, 'Device authorization required');
        return json({ device: authority.listDevices().find((entry) => entry.id === identity.id) });
      }
      if (path === '/v1/device/connected-accounts' && method === 'GET') {
        authorizedDevice(request, authority, 'inspect');
        if (!composio) return json({ error: 'Composio is not configured' }, 503);
        try {
          const page = await composio.listConnectedAccounts(url.searchParams.get('cursor') ?? undefined);
          const allowed = authority.allowedAccountIds();
          return json({ ...page, items: page.items.filter((account) => allowed.has(account.id)) });
        } catch {
          return json({ error: 'Connected-account lookup failed' }, 502);
        }
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
