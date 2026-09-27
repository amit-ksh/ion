/**
 * The local authority rules for a future user-owned gateway. This module has
 * no network listener or Composio client: the owner-facing control surface
 * must call createInvite/approve/revoke, while paired devices get only the
 * token-facing methods. Secrets are returned once and only their digests are
 * retained here.
 */
import { createHash, randomBytes, timingSafeEqual } from 'node:crypto';

export type DeviceGrant = 'inspect' | 'propose';
export type EffectStatus = 'pending' | 'denied' | 'approved' | 'uncertain' | 'succeeded';

export interface ExternalEffect {
  toolkit: string;
  tool: string;
  accountId: string;
  target: string;
  body: string;
  scopes: string[];
}

export interface AllowedTool {
  toolkit: string;
  tool: string;
  accountId: string;
  scopes: string[];
}

interface Invite {
  digest: Buffer;
  expiresAt: number;
  deviceLifetimeMs: number;
  grants: DeviceGrant[];
}

interface Device {
  id: string;
  name: string;
  digest: Buffer;
  expiresAt: number;
  grants: DeviceGrant[];
  revoked: boolean;
}

interface Proposal {
  id: string;
  deviceId: string;
  effect: ExternalEffect;
  fingerprint: string;
  status: EffectStatus;
  resultReference?: string;
}

export interface ProposalView {
  id: string;
  deviceId: string;
  effect: ExternalEffect;
  fingerprint: string;
  status: EffectStatus;
  resultReference?: string;
}

function digest(value: string): Buffer {
  return createHash('sha256').update(value, 'utf8').digest();
}

function equalDigest(a: Buffer, b: Buffer): boolean {
  return a.length === b.length && timingSafeEqual(a, b);
}

function token(): string {
  return randomBytes(32).toString('base64url');
}

function publicProposal(proposal: Proposal): ProposalView {
  return {
    id: proposal.id,
    deviceId: proposal.deviceId,
    effect: { ...proposal.effect, scopes: [...proposal.effect.scopes] },
    fingerprint: proposal.fingerprint,
    status: proposal.status,
    resultReference: proposal.resultReference,
  };
}

/** In-memory policy core. Persisted state and remote serving come later. */
export class GatewayAuthority {
  private readonly allowedTools: AllowedTool[];
  private readonly invites = new Map<string, Invite>();
  private readonly devices = new Map<string, Device>();
  private readonly proposals = new Map<string, Proposal>();

  constructor(
    allowedTools: AllowedTool[],
    private readonly now: () => number = Date.now,
  ) {
    this.allowedTools = allowedTools.map((entry) => ({ ...entry, scopes: [...entry.scopes] }));
  }

  /** Trusted owner operation. The invite is single-use and valid for <= 5m. */
  createInvite(
    grants: DeviceGrant[],
    lifetimeMs = 5 * 60_000,
    deviceLifetimeMs = 24 * 60 * 60_000,
  ): string {
    if (lifetimeMs <= 0 || lifetimeMs > 5 * 60_000) {
      throw new Error('Pairing invite lifetime must be at most five minutes');
    }
    if (!grants.length || grants.some((grant) => grant !== 'inspect' && grant !== 'propose')) {
      throw new Error('Choose explicit device grants');
    }
    if (deviceLifetimeMs <= 0 || deviceLifetimeMs > 24 * 60 * 60_000) {
      throw new Error('Device grant lifetime must be at most one day');
    }
    for (const [key, invite] of this.invites) {
      if (this.now() >= invite.expiresAt) this.invites.delete(key);
    }
    const secret = token();
    this.invites.set(digest(secret).toString('hex'), {
      digest: digest(secret),
      expiresAt: this.now() + lifetimeMs,
      deviceLifetimeMs,
      grants: [...new Set(grants)],
    });
    return secret;
  }

  pair(inviteSecret: string, name: string): {
    id: string;
    token: string;
    expiresAt: number;
    grants: DeviceGrant[];
  } {
    if (!name.trim() || name.length > 80) throw new Error('Enter a device name');
    const key = digest(inviteSecret).toString('hex');
    const invite = this.invites.get(key);
    if (!invite || !equalDigest(invite.digest, digest(inviteSecret)) || this.now() >= invite.expiresAt) {
      throw new Error('Pairing invite is invalid or expired');
    }
    this.invites.delete(key);
    const id = crypto.randomUUID();
    const deviceToken = token();
    const device: Device = {
      id,
      name: name.trim(),
      digest: digest(deviceToken),
      expiresAt: this.now() + invite.deviceLifetimeMs,
      grants: [...invite.grants],
      revoked: false,
    };
    this.devices.set(id, device);
    return { id, token: deviceToken, expiresAt: device.expiresAt, grants: [...device.grants] };
  }

  /** Trusted owner operation. Revocation also denies future proposals. */
  revokeDevice(id: string): void {
    const device = this.devices.get(id);
    if (!device) throw new Error('Device not found');
    device.revoked = true;
    for (const proposal of this.proposals.values()) {
      if (proposal.deviceId === id && proposal.status === 'pending') proposal.status = 'denied';
    }
  }

  authenticate(id: string, secret: string, grant: DeviceGrant): void {
    const device = this.devices.get(id);
    if (!device || device.revoked || this.now() >= device.expiresAt ||
        !equalDigest(device.digest, digest(secret)) || !device.grants.includes(grant)) {
      throw new Error('Device is not authorized');
    }
  }

  propose(id: string, secret: string, effect: ExternalEffect): ProposalView {
    this.authenticate(id, secret, 'propose');
    if (![effect.toolkit, effect.tool, effect.accountId, effect.target, effect.body]
      .every((value) => typeof value === 'string' && value.trim()) ||
      !Array.isArray(effect.scopes) || effect.scopes.some((scope) => typeof scope !== 'string')) {
      throw new Error('External effect needs an exact toolkit, tool, account, target, body, and scopes');
    }
    if (effect.body.length > 65_536 || effect.target.length > 2_048 || effect.scopes.length > 32) {
      throw new Error('External effect exceeds the review limit');
    }
    const allowed = this.allowedTools.find((entry) =>
      entry.toolkit === effect.toolkit && entry.tool === effect.tool &&
      entry.accountId === effect.accountId &&
      effect.scopes.length === entry.scopes.length &&
      new Set(effect.scopes).size === entry.scopes.length &&
      effect.scopes.every((scope) => entry.scopes.includes(scope)));
    if (!allowed) throw new Error('Tool, account, or scope is outside the gateway grant');
    const snapshot: ExternalEffect = { ...effect, scopes: [...effect.scopes] };
    const fingerprint = digest(JSON.stringify(snapshot)).toString('hex');
    const proposal: Proposal = {
      id: crypto.randomUUID(), deviceId: id, effect: snapshot, fingerprint, status: 'pending',
    };
    this.proposals.set(proposal.id, proposal);
    return publicProposal(proposal);
  }

  /** Trusted owner operation. The reviewed fingerprint binds the exact body. */
  decide(proposalId: string, fingerprint: string, approve: boolean): ProposalView {
    const proposal = this.proposals.get(proposalId);
    if (!proposal || proposal.status !== 'pending' || proposal.fingerprint !== fingerprint) {
      throw new Error('Proposal changed or is no longer pending');
    }
    const device = this.devices.get(proposal.deviceId);
    if (!device || device.revoked || this.now() >= device.expiresAt) {
      throw new Error('Proposing device is no longer authorized');
    }
    proposal.status = approve ? 'approved' : 'denied';
    return publicProposal(proposal);
  }

  /** Claim before the external call; an interrupted call is never replayed. */
  beginExecution(proposalId: string): ExternalEffect {
    const proposal = this.proposals.get(proposalId);
    if (!proposal || proposal.status !== 'approved') {
      throw new Error('Effect has no unused approval');
    }
    const device = this.devices.get(proposal.deviceId);
    if (!device || device.revoked || this.now() >= device.expiresAt) {
      throw new Error('Proposing device is no longer authorized');
    }
    proposal.status = 'uncertain';
    return { ...proposal.effect, scopes: [...proposal.effect.scopes] };
  }

  /** Trusted executor operation, after a confirmed external result. */
  recordSuccess(proposalId: string, resultReference: string): ProposalView {
    const proposal = this.proposals.get(proposalId);
    if (!proposal || proposal.status !== 'uncertain' || !resultReference.trim()) {
      throw new Error('No uncertain effect or confirmed result reference');
    }
    proposal.resultReference = resultReference;
    proposal.status = 'succeeded';
    return publicProposal(proposal);
  }

  inspect(id: string, secret: string, proposalId: string): ProposalView {
    this.authenticate(id, secret, 'inspect');
    const proposal = this.proposals.get(proposalId);
    if (!proposal || proposal.deviceId !== id) throw new Error('Proposal not found');
    return publicProposal(proposal);
  }
}
