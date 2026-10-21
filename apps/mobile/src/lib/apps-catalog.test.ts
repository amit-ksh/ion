import { expect, test } from 'bun:test';
import { appConnectionPending, appConnectionUrl, parseAppsCatalog } from './apps-catalog';
const row = { toolkit: 'gmail', name: 'Gmail', description: 'Mail', connectable: true };
test('retains connection state without treating an app connection as an action grant', () => {
  expect(parseAppsCatalog({ managed: true, nextCursor: null, items: [{ ...row, account: { id: 'ca_123', toolkit: 'gmail', status: 'ACTIVE' } }] }).items[0]?.account?.status).toBe('ACTIVE');
});
test('rejects mismatched accounts, unbounded pages, and duplicate app identities', () => {
  expect(() => parseAppsCatalog({ managed: true, nextCursor: null, items: [{ ...row, account: { id: 'ca_1', toolkit: 'slack', status: 'ACTIVE' } }] })).toThrow();
  expect(() => parseAppsCatalog({ managed: true, nextCursor: null, items: Array(51).fill(row) })).toThrow();
  expect(() => parseAppsCatalog({ managed: true, nextCursor: null, items: [row, row] })).toThrow();
});
test('app sign-in only opens bounded HTTPS links to Composio hosts', () => {
  expect(appConnectionUrl('https://connect.composio.dev/link/123')).toContain('https://');
  for (const url of ['http://composio.dev/link', 'https://composio.dev.evil.test/', 'https://user@composio.dev/', 'file:///tmp/private', 'intent://composio.dev']) expect(() => appConnectionUrl(url)).toThrow();
});
test('unfinished app sign-in cannot be presented as a fresh connection', () => {
  expect(appConnectionPending({ ...row, account: { id: 'ca_1', toolkit: 'gmail', status: 'INITIATED' } })).toBe(true);
  for (const status of ['ACTIVE', 'REVOKED', 'FAILED', 'EXPIRED']) expect(appConnectionPending({ ...row, account: { id: 'ca_1', toolkit: 'gmail', status } })).toBe(false);
});
