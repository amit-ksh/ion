import { readFile } from 'node:fs/promises';
import type { AllowedTool } from './authority';
import { startGateway } from './server';

const databasePath = process.env.WAKU_GATEWAY_DB;
const allowlistPath = process.env.WAKU_GATEWAY_ALLOWLIST;
const ownerToken = process.env.WAKU_GATEWAY_OWNER_TOKEN;
const keyPath = process.env.WAKU_GATEWAY_TLS_KEY;
const certPath = process.env.WAKU_GATEWAY_TLS_CERT;

if (!databasePath || !allowlistPath || !ownerToken) {
  throw new Error('Set WAKU_GATEWAY_DB, WAKU_GATEWAY_ALLOWLIST, and WAKU_GATEWAY_OWNER_TOKEN');
}
if (Boolean(keyPath) !== Boolean(certPath)) {
  throw new Error('Set both WAKU_GATEWAY_TLS_KEY and WAKU_GATEWAY_TLS_CERT');
}

function isAllowedTool(value: unknown): value is AllowedTool {
  if (!value || typeof value !== 'object') return false;
  const entry = value as Record<string, unknown>;
  return ['toolkit', 'tool', 'accountId'].every((field) =>
    typeof entry[field] === 'string' && Boolean(entry[field].trim())) &&
    Array.isArray(entry.scopes) && entry.scopes.every((scope) => typeof scope === 'string');
}

const raw: unknown = JSON.parse(await readFile(allowlistPath, 'utf8'));
if (!Array.isArray(raw) || raw.some((entry) => !isAllowedTool(entry))) {
  throw new Error('Gateway allowlist must be an array of exact toolkit/tool/accountId/scopes entries');
}

const port = Number(process.env.WAKU_GATEWAY_PORT ?? 8787);
if (!Number.isInteger(port) || port < 1 || port > 65_535) throw new Error('Invalid gateway port');
const running = startGateway({
  databasePath,
  ownerToken,
  allowedTools: raw as AllowedTool[],
  hostname: process.env.WAKU_GATEWAY_HOST ?? '127.0.0.1',
  port,
  tls: keyPath && certPath ? { key: Bun.file(keyPath), cert: Bun.file(certPath) } : undefined,
});

console.log(`Personal gateway listening at ${running.server.url.origin}`);
let closing = false;
for (const signal of ['SIGINT', 'SIGTERM'] as const) {
  process.on(signal, () => {
    if (closing) return;
    closing = true;
    void running.close().then(() => process.exit(0));
  });
}
