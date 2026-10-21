export interface ConnectedApp { id: string; toolkit: string; status: string }
export interface CatalogApp { toolkit: string; name: string; description: string; connectable: boolean; account?: ConnectedApp }
export interface AppsCatalog { managed: true; items: CatalogApp[]; nextCursor: string | null }
const object = (value: unknown): value is Record<string, unknown> => Boolean(value) && typeof value === 'object' && !Array.isArray(value);
const identifier = (value: unknown): value is string => typeof value === 'string' && /^[A-Za-z0-9_-]{1,160}$/.test(value);
const slug = (value: unknown): value is string => typeof value === 'string' && /^[a-z][a-z0-9_]{0,99}$/.test(value);
export function appConnectionPending(app: CatalogApp): boolean {
  return Boolean(app.account && !['ACTIVE', 'REVOKED', 'FAILED', 'EXPIRED'].includes(app.account.status));
}
export function parseAppsCatalog(value: unknown): AppsCatalog {
  if (!object(value) || value.managed !== true || !Array.isArray(value.items) || value.items.length > 50
    || !(value.nextCursor === null || typeof value.nextCursor === 'string' && value.nextCursor.length <= 1024)) throw new Error('Invalid app catalog. Refresh before connecting.');
  const items = value.items.map((row): CatalogApp => {
    if (!object(row) || !slug(row.toolkit) || typeof row.name !== 'string' || !row.name.trim() || row.name.length > 256
      || typeof row.description !== 'string' || row.description.length > 512 || typeof row.connectable !== 'boolean') throw new Error('Invalid app catalog entry.');
    let account: ConnectedApp | undefined;
    if (row.account !== undefined) {
      if (!object(row.account) || !identifier(row.account.id) || row.account.toolkit !== row.toolkit
        || typeof row.account.status !== 'string' || row.account.status.length > 64) throw new Error('Invalid app connection.');
      account = { id: row.account.id, toolkit: row.toolkit, status: row.account.status };
    }
    return { toolkit: row.toolkit, name: row.name, description: row.description, connectable: row.connectable, ...(account ? { account } : {}) };
  });
  if (new Set(items.map(item => item.toolkit)).size !== items.length) throw new Error('Duplicate apps in catalog.');
  return { managed: true, items, nextCursor: value.nextCursor as string | null };
}
export function appConnectionUrl(value: unknown): string {
  if (typeof value !== 'string' || value.length > 4096) throw new Error('Invalid app sign-in link.');
  const url = new URL(value);
  if (url.protocol !== 'https:' || url.username || url.password
    || !['composio.dev', 'composio.app'].some(host => url.hostname === host || url.hostname.endsWith(`.${host}`))) throw new Error('Invalid app sign-in link.');
  return value;
}
