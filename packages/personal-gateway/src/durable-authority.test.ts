import { afterEach, describe, expect, test } from 'bun:test';
import { Database } from 'bun:sqlite';
import { mkdtempSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { DurableGatewayAuthority } from './durable-authority';

const effect = {
  toolkit: 'github', tool: 'GITHUB_CREATE_ISSUE', accountId: 'test-account',
  target: 'owner/repository', body: 'Exact issue body', scopes: ['issues:write'],
};
const allowedTools = [{
  toolkit: effect.toolkit, tool: effect.tool, accountId: effect.accountId,
  scopes: [...effect.scopes],
}];

const testDirectories: string[] = [];
function databasePath(): string {
  const directory = mkdtempSync(join(tmpdir(), 'waku-gateway-'));
  testDirectories.push(directory);
  return join(directory, 'authority.sqlite');
}

afterEach(() => {
  for (const directory of testDirectories.splice(0)) rmSync(directory, { recursive: true, force: true });
});

describe('durable personal gateway authority', () => {
  test('pairing is single-use across separate connections and stores no raw token', () => {
    const path = databasePath();
    const first = new DurableGatewayAuthority(path, allowedTools);
    const second = new DurableGatewayAuthority(path, allowedTools);
    try {
      const invite = first.createInvite(['inspect', 'propose']);
      const device = second.pair(invite, 'Test desktop');
      expect(() => first.pair(invite, 'Second desktop')).toThrow();
      first.authenticate(device.id, device.token, 'inspect');
      expect(first.listDevices()).toEqual([{
        id: device.id, name: 'Test desktop', expiresAt: device.expiresAt,
        grants: ['inspect', 'propose'], revoked: false,
      }]);

      const db = new Database(path, { readonly: true });
      try {
        const statement = db.prepare<{ snapshot: string }, []>(
          'SELECT snapshot FROM gateway_state WHERE id = 1',
        );
        const row = statement.get();
        statement.finalize();
        expect(row?.snapshot).not.toContain(invite);
        expect(row?.snapshot).not.toContain(device.token);
      } finally {
        db.close(true);
      }
      first.revokeDevice(device.id);
      expect(() => second.authenticate(device.id, device.token, 'inspect')).toThrow();
      expect(second.listDevices()[0]?.revoked).toBe(true);
    } finally {
      first.close();
      second.close();
    }
  });

  test('approved effect is claimed durably before any external call', () => {
    const path = databasePath();
    const first = new DurableGatewayAuthority(path, allowedTools);
    const device = first.pair(first.createInvite(['inspect', 'propose']), 'Test desktop');
    const proposal = first.propose(device.id, device.token, effect);
    expect(first.listProposals()).toEqual([proposal]);
    first.decide(proposal.id, proposal.fingerprint, true);
    expect(first.beginExecution(proposal.id)).toEqual(effect);
    const competing = new DurableGatewayAuthority(path, allowedTools);
    expect(() => competing.beginExecution(proposal.id)).toThrow('no unused approval');
    competing.close();
    first.close();

    const restarted = new DurableGatewayAuthority(path, allowedTools);
    try {
      expect(restarted.inspect(device.id, device.token, proposal.id).status).toBe('uncertain');
      expect(() => restarted.beginExecution(proposal.id)).toThrow('no unused approval');
      restarted.recordSuccess(proposal.id, 'issue-1');
    } finally {
      restarted.close();
    }
  });

  test('a removed tool grant blocks an approval that was already stored', () => {
    const path = databasePath();
    const first = new DurableGatewayAuthority(path, allowedTools);
    const device = first.pair(first.createInvite(['inspect', 'propose']), 'Test desktop');
    const proposal = first.propose(device.id, device.token, effect);
    first.decide(proposal.id, proposal.fingerprint, true);
    first.close();

    const restricted = new DurableGatewayAuthority(path, []);
    try {
      expect(() => restricted.beginExecution(proposal.id)).toThrow('outside the gateway grant');
      expect(restricted.inspect(device.id, device.token, proposal.id).status).toBe('approved');
    } finally {
      restricted.close();
    }
  });
});
