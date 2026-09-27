import { describe, expect, test } from 'bun:test';
import { GatewayAuthority } from './authority';

const effect = {
  toolkit: 'github', tool: 'GITHUB_CREATE_ISSUE', accountId: 'test-account',
  target: 'owner/repository', body: 'Proposed issue body', scopes: ['issues:write'],
};
const allowedTools = [{
  toolkit: effect.toolkit, tool: effect.tool, accountId: effect.accountId,
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
    expect(authority.inspect(device.id, device.token, proposal.id).effect.body).toBe('Proposed issue body');
    expect(() => authority.decide(proposal.id, 'different fingerprint', true)).toThrow();
    authority.decide(proposal.id, proposal.fingerprint, true);
    expect(authority.beginExecution(proposal.id).body).toBe('Proposed issue body');
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
      ...effect, tool: 'GMAIL_SEND_EMAIL',
    })).toThrow('outside the gateway grant');
    expect(() => authority.propose(device.id, device.token, {
      ...effect, scopes: ['admin'],
    })).toThrow('outside the gateway grant');
    expect(() => authority.propose(device.id, device.token, {
      ...effect, scopes: [],
    })).toThrow('outside the gateway grant');
  });
});
