export interface ConnectedAccountSummary {
  id: string;
  toolkit: string;
  status: string;
}

export interface ConnectedAccountPage {
  items: ConnectedAccountSummary[];
  nextCursor: string | null;
}

/** Read-only account discovery. Never return Composio's credential-bearing state. */
export class ComposioCatalog {
  constructor(
    private readonly apiKey: string,
    private readonly request: typeof fetch = fetch,
  ) {
    if (!apiKey.trim()) throw new Error('Composio project key is required');
  }

  async listConnectedAccounts(cursor?: string): Promise<ConnectedAccountPage> {
    if (cursor !== undefined && (cursor.length > 512 || !cursor.trim())) {
      throw new Error('Invalid account cursor');
    }
    const url = new URL('https://backend.composio.dev/api/v3.1/connected_accounts');
    url.searchParams.set('limit', '100');
    if (cursor) url.searchParams.set('cursor', cursor);
    const response = await this.request(url, {
      method: 'GET',
      headers: { 'x-api-key': this.apiKey, accept: 'application/json' },
      signal: AbortSignal.timeout(10_000),
    });
    if (!response.ok) throw new Error(`Composio account lookup failed (${response.status})`);
    const payload: unknown = await response.json();
    if (!payload || typeof payload !== 'object' || !Array.isArray((payload as Record<string, unknown>).items)) {
      throw new Error('Invalid Composio account response');
    }
    const data = payload as { items: unknown[]; next_cursor?: unknown };
    const items = data.items.map((raw): ConnectedAccountSummary => {
      if (!raw || typeof raw !== 'object') throw new Error('Invalid Composio account response');
      const account = raw as Record<string, unknown>;
      const toolkit = account.toolkit;
      if (typeof account.id !== 'string' || typeof account.status !== 'string' ||
          !toolkit || typeof toolkit !== 'object' ||
          typeof (toolkit as Record<string, unknown>).slug !== 'string') {
        throw new Error('Invalid Composio account response');
      }
      return {
        id: account.id,
        toolkit: (toolkit as { slug: string }).slug,
        status: account.status,
      };
    });
    const nextCursor = data.next_cursor;
    if (nextCursor != null && typeof nextCursor !== 'string') {
      throw new Error('Invalid Composio account response');
    }
    return { items, nextCursor: nextCursor ?? null };
  }
}
