import { openDatabase, type DatabaseSource, type SqlDatabase, type Binding as SQLQueryBindings } from "./database";
import {
  GatewayAuthority,
  type AllowedTool,
  type AuthoritySnapshot,
  type DeviceGrant,
  type DeviceView,
  type ExternalEffect,
  type ProposalView,
} from './authority';

/**
 * Synchronous gateway-process store. Each operation loads the latest committed
 * state; mutations take an IMMEDIATE transaction so separate processes cannot
 * both consume an invite or claim the same approved external effect.
 */
export class DurableGatewayAuthority {
  private readonly db: SqlDatabase;

  constructor(
    databasePath: DatabaseSource,
    private readonly allowedTools: AllowedTool[],
    private readonly now: () => number = Date.now,
  ) {
    this.db = openDatabase(databasePath);
    this.db.run('PRAGMA busy_timeout = 5000');
    // A single small snapshot favors simple rollback journaling on Windows;
    // IMMEDIATE transactions still serialize competing gateway processes.
    this.db.run('PRAGMA journal_mode = DELETE');
    this.db.run('PRAGMA synchronous = FULL');
    this.db.run('CREATE TABLE IF NOT EXISTS gateway_state (id INTEGER PRIMARY KEY CHECK (id = 1), snapshot TEXT NOT NULL)');
  }

  close(): void {
    this.db.close(true);
  }

  private load(): GatewayAuthority {
    const statement = this.db.prepare<{ snapshot: string }, []>(
      'SELECT snapshot FROM gateway_state WHERE id = 1',
    );
    let row: { snapshot: string } | null;
    try {
      row = statement.get();
    } finally {
      statement.finalize();
    }
    if (!row) return new GatewayAuthority(this.allowedTools, this.now);
    return GatewayAuthority.restore(this.allowedTools, JSON.parse(row.snapshot) as AuthoritySnapshot, this.now);
  }

  private read<T>(operation: (authority: GatewayAuthority) => T): T {
    return operation(this.load());
  }

  private write<T>(operation: (authority: GatewayAuthority) => T): T {
    return this.db.atomic(() => {
      const authority = this.load();
      const result = operation(authority);
      this.db.run(
        'INSERT INTO gateway_state (id, snapshot) VALUES (1, ?1) ON CONFLICT (id) DO UPDATE SET snapshot = excluded.snapshot',
        [JSON.stringify(authority.snapshot())],
      );
      return result;
    });
  }

  createInvite(grants: DeviceGrant[], lifetimeMs?: number, deviceLifetimeMs?: number): string {
    return this.write((authority) => authority.createInvite(grants, lifetimeMs, deviceLifetimeMs));
  }

  pair(inviteSecret: string, name: string): ReturnType<GatewayAuthority['pair']> {
    return this.write((authority) => authority.pair(inviteSecret, name));
  }

  revokeDevice(id: string): void {
    this.write((authority) => authority.revokeDevice(id));
  }

  authenticate(id: string, secret: string, grant: DeviceGrant): void {
    this.read((authority) => authority.authenticate(id, secret, grant));
  }

  /** Trusted owner reads; never expose these through a paired-device endpoint. */
  listDevices(): DeviceView[] {
    return this.read((authority) => authority.listDevices());
  }

  listProposals(): ProposalView[] {
    return this.read((authority) => authority.listProposals());
  }

  /** Account identifiers explicitly named by this gateway's tool grants. */
  allowedAccountIds(): Set<string> {
    return new Set(this.allowedTools.map((entry) => entry.accountId));
  }

  listAllowedTools(): AllowedTool[] {
    return this.allowedTools.map((entry) => ({ ...entry, scopes: [...entry.scopes] }));
  }

  propose(id: string, secret: string, effect: ExternalEffect): ProposalView {
    return this.write((authority) => authority.propose(id, secret, effect));
  }

  decide(proposalId: string, fingerprint: string, approve: boolean): ProposalView {
    return this.write((authority) => authority.decide(proposalId, fingerprint, approve));
  }

  reviewQueue(id: string, secret: string): ProposalView[] {
    return this.read((authority) => authority.reviewQueue(id, secret));
  }

  reviewStatus(id: string, secret: string, proposalId: string): ProposalView {
    return this.read((authority) => authority.reviewStatus(id, secret, proposalId));
  }

  review(id: string, secret: string, proposalId: string, fingerprint: string, approve: boolean): ProposalView {
    return this.write((authority) => authority.review(id, secret, proposalId, fingerprint, approve));
  }

  /** The uncertain claim is committed before this method returns the effect. */
  beginExecution(proposalId: string): ExternalEffect {
    return this.write((authority) => authority.beginExecution(proposalId));
  }

  recordSuccess(proposalId: string, resultReference: string): ProposalView {
    return this.write((authority) => authority.recordSuccess(proposalId, resultReference));
  }

  inspect(id: string, secret: string, proposalId: string): ProposalView {
    return this.read((authority) => authority.inspect(id, secret, proposalId));
  }
}
