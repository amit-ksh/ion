import { describe, expect, test } from 'bun:test';
import { GatewayAuthority } from './authority';

const effect = {
  toolkit: 'github', tool: 'GITHUB_CREATE_ISSUE', version: '20251027_00', accountId: 'test-account',
  target: 'owner/repository', body: '{"title":"Proposed issue body"}', scopes: ['issues:write'],
};
const allowedTools = [{
  toolkit: effect.toolkit, tool: effect.tool, version: effect.version, accountId: effect.accountId,
  scopes: [...effect.scopes],
}];

describe('personal gateway authority', () => {
  test('single-use pairing, expiry, and revocation', () => {
    let now = 1_000;
    const authority = new GatewayAuthority(allowedTools, () => now);
    const invite = authority.createInvite(['inspect', 'propose'], 100);
    const device = authority.pair(invite, 'Test phone');
    expect(() => authority.pair(invite, 'Another phone')).toThrow();
    authority.authenticate(device.id, device.token, 'inspect');
    expect(() => authority.authenticate(device.id, 'wrong', 'inspect')).toThrow();
    authority.revokeDevice(device.id);
    expect(() => authority.authenticate(device.id, device.token, 'inspect')).toThrow();
    const expiredInvite = authority.createInvite(['inspect'], 100);
    now += 100;
    expect(() => authority.pair(expiredInvite, 'Late phone')).toThrow();
    const shortLived = authority.pair(authority.createInvite(['inspect'], 100, 100), 'Short-lived');
    now += 100;
    expect(() => authority.authenticate(shortLived.id, shortLived.token, 'inspect')).toThrow();
  });

  test('an exact approved write can be claimed only once', () => {
    const authority = new GatewayAuthority(allowedTools);
    const device = authority.pair(authority.createInvite(['inspect', 'propose']), 'Phone');
    const input = { ...effect, scopes: [...effect.scopes] };
    const proposal = authority.propose(device.id, device.token, input);
    input.body = 'Changed after proposal';
    expect(authority.inspect(device.id, device.token, proposal.id).effect.body).toBe('{"title":"Proposed issue body"}');
    expect(() => authority.decide(proposal.id, 'different fingerprint', true)).toThrow();
    authority.decide(proposal.id, proposal.fingerprint, true);
    expect(authority.beginExecution(proposal.id).body).toBe('{"title":"Proposed issue body"}');
    expect(() => authority.beginExecution(proposal.id)).toThrow();
    expect(authority.inspect(device.id, device.token, proposal.id).status).toBe('uncertain');
    authority.recordSuccess(proposal.id, 'https://github.com/owner/repository/issues/1');
    expect(authority.inspect(device.id, device.token, proposal.id).status).toBe('succeeded');
  });

  test('denial and revoked devices cannot execute', () => {
    const authority = new GatewayAuthority(allowedTools);
    const device = authority.pair(authority.createInvite(['inspect', 'propose']), 'Phone');
    const denied = authority.propose(device.id, device.token, effect);
    authority.decide(denied.id, denied.fingerprint, false);
    expect(() => authority.beginExecution(denied.id)).toThrow();
    const pending = authority.propose(device.id, device.token, effect);
    authority.revokeDevice(device.id);
    expect(() => authority.decide(pending.id, pending.fingerprint, true)).toThrow();
    expect(() => authority.beginExecution(pending.id)).toThrow();
  });

  test('rejects tools and scopes outside the explicit grant', () => {
    const authority = new GatewayAuthority(allowedTools);
    const device = authority.pair(authority.createInvite(['propose']), 'Phone');
    expect(() => authority.propose(device.id, device.token, {
      ...effect, tool: 'GITHUB_DELETE_REPOSITORY',
    })).toThrow('outside the gateway grant');
    expect(() => authority.propose(device.id, device.token, {
      ...effect, scopes: ['admin'],
    })).toThrow('outside the gateway grant');
    expect(() => authority.propose(device.id, device.token, {
      ...effect, scopes: [],
    })).toThrow('outside the gateway grant');
    expect(() => authority.propose(device.id, device.token, {
      ...effect, version: 'latest',
    })).toThrow('pinned version');
    expect(() => authority.propose(device.id, device.token, {
      ...effect, version: '20251028_00',
    })).toThrow('outside the gateway grant');
  });

  test('legacy proposals without a pinned tool version fail closed on restore', () => {
    const authority = new GatewayAuthority(allowedTools);
    const device = authority.pair(authority.createInvite(['inspect', 'propose']), 'Phone');
    const proposal = authority.propose(device.id, device.token, effect);
    authority.decide(proposal.id, proposal.fingerprint, true);
    const snapshot = authority.snapshot();
    const legacy = {
      ...snapshot,
      version: 1 as const,
      proposals: snapshot.proposals.map(({ effect: current, ...stored }) => {
        const { version: _unboundVersion, ...oldEffect } = current;
        return { ...stored, effect: oldEffect };
      }),
    };
    const restored = GatewayAuthority.restore(allowedTools, legacy);
    restored.authenticate(device.id, device.token, 'inspect');
    expect(restored.inspect(device.id, device.token, proposal.id).status).toBe('denied');
    expect(() => restored.beginExecution(proposal.id)).toThrow('no unused approval');
    expect(restored.snapshot().version).toBe(2);
    expect(GatewayAuthority.restore(allowedTools, restored.snapshot())
      .inspect(device.id, device.token, proposal.id).status).toBe('denied');
  });

  test('a changed persisted write body is rejected before approval', () => {
    const authority = new GatewayAuthority(allowedTools);
    const device = authority.pair(authority.createInvite(['propose']), 'Phone');
    authority.propose(device.id, device.token, effect);
    const changed = authority.snapshot();
    changed.proposals[0]!.effect.body = '{"title":"Different issue"}';
    expect(() => GatewayAuthority.restore(allowedTools, changed)).toThrow('Corrupt gateway proposal snapshot');
  });
});
