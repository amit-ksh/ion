import { afterEach, describe, expect, test } from 'bun:test';
import { mkdtempSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { ComposioExecutor } from './composio-execution';
import { DurableGatewayAuthority } from './durable-authority';

const effect = {
  toolkit: 'github', tool: 'GITHUB_CREATE_ISSUE', version: '20251027_00',
  accountId: 'ca_test', target: 'owner/repository',
  body: '{"owner":"owner","repo":"repository","title":"Reviewed title"}',
  scopes: ['issues:write'],
};
const allowedTools = [{
  toolkit: effect.toolkit, tool: effect.tool, version: effect.version,
  accountId: effect.accountId, scopes: [...effect.scopes],
}];
const directories: string[] = [];
function databasePath(): string {
  const directory = mkdtempSync(join(tmpdir(), 'waku-gateway-execute-'));
  directories.push(directory);
  return join(directory, 'authority.sqlite');
}
afterEach(() => {
  for (const directory of directories.splice(0)) rmSync(directory, { recursive: true, force: true });
});

function approved(authority: DurableGatewayAuthority) {
  const device = authority.pair(authority.createInvite(['inspect', 'propose']), 'Test desktop');
  const proposal = authority.propose(device.id, device.token, effect);
  authority.decide(proposal.id, proposal.fingerprint, true);
  return { device, proposal };
}

describe('approval-gated Composio execution', () => {
  test('commits the single-use claim before one pinned, account-specific call', async () => {
    const authority = new DurableGatewayAuthority(databasePath(), allowedTools);
    const { device, proposal } = approved(authority);
    let calls = 0;
    let seenStatus = '';
    let requestedUrl = '';
    let sentBody: unknown;
    let keyHeader = '';
    const mockFetch = (async (input: Parameters<typeof fetch>[0], init?: RequestInit) => {
      calls++;
      seenStatus = authority.inspect(device.id, device.token, proposal.id).status;
      requestedUrl = String(input);
      keyHeader = new Headers(init?.headers).get('x-api-key') ?? '';
      sentBody = JSON.parse(String(init?.body));
      return Response.json({ successful: true, log_id: 'log_confirmed', data: { secret: 'omit-me' } });
    });
    try {
      const executor = new ComposioExecutor('test-project-key', mockFetch);
      expect(await executor.executeApproved(authority, proposal.id)).toEqual({
        status: 'succeeded', resultReference: 'log_confirmed',
      });
      expect(seenStatus).toBe('uncertain');
      expect(calls).toBe(1);
      expect(requestedUrl).toBe('https://backend.composio.dev/api/v3.1/tools/execute/GITHUB_CREATE_ISSUE');
      expect(keyHeader).toBe('test-project-key');
      expect(sentBody).toEqual({
        connected_account_id: 'ca_test', version: '20251027_00',
        arguments: { owner: 'owner', repo: 'repository', title: 'Reviewed title' },
      });
      expect(authority.inspect(device.id, device.token, proposal.id).status).toBe('succeeded');
      expect(() => authority.beginExecution(proposal.id)).toThrow('no unused approval');
    } finally {
      authority.close();
    }
  });

  test('lost response remains uncertain after restart and cannot be replayed', async () => {
    const path = databasePath();
    const authority = new DurableGatewayAuthority(path, allowedTools);
    const { device, proposal } = approved(authority);
    let calls = 0;
    const executor = new ComposioExecutor('test-project-key', (async () => {
      calls++;
      throw new Error('connection lost after send');
    }));
    expect(await executor.executeApproved(authority, proposal.id)).toEqual({ status: 'uncertain' });
    authority.close();

    const restarted = new DurableGatewayAuthority(path, allowedTools);
    try {
      expect(restarted.inspect(device.id, device.token, proposal.id).status).toBe('uncertain');
      expect(() => restarted.beginExecution(proposal.id)).toThrow('no unused approval');
      expect(calls).toBe(1);
    } finally {
      restarted.close();
    }
  });

  test('denied proposal never reaches Composio', async () => {
    const authority = new DurableGatewayAuthority(databasePath(), allowedTools);
    const device = authority.pair(authority.createInvite(['propose']), 'Test desktop');
    const proposal = authority.propose(device.id, device.token, effect);
    authority.decide(proposal.id, proposal.fingerprint, false);
    let calls = 0;
    const executor = new ComposioExecutor('test-project-key', (async () => {
      calls++;
      return Response.json({ successful: true, log_id: 'wrong' });
    }));
    try {
      await expect(executor.executeApproved(authority, proposal.id)).rejects.toThrow('no unused approval');
      expect(calls).toBe(0);
    } finally {
      authority.close();
    }
  });

  test('a queued external result stays uncertain for owner reconciliation', async () => {
    const authority = new DurableGatewayAuthority(databasePath(), allowedTools);
    const { device, proposal } = approved(authority);
    const executor = new ComposioExecutor('test-project-key', async () =>
      Response.json({ successful: true, log_id: 'log_queued', data: { status: 'queued' } }));
    try {
      expect(await executor.executeApproved(authority, proposal.id)).toEqual({ status: 'uncertain' });
      expect(authority.inspect(device.id, device.token, proposal.id).status).toBe('uncertain');
    } finally {
      authority.close();
    }
  });
});
