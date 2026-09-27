import { readFile, stat } from 'node:fs/promises';
import { dirname } from 'node:path';
import type { AllowedTool } from './authority';
import { startGateway } from './server';
import { ComposioCatalog } from './composio';
import { ComposioExecutor } from './composio-execution';

const databasePath = process.env.WAKU_GATEWAY_DB;
const allowlistPath = process.env.WAKU_GATEWAY_ALLOWLIST;
const ownerTokenPath = process.env.WAKU_GATEWAY_OWNER_TOKEN_FILE;
const composioKeyPath = process.env.WAKU_GATEWAY_COMPOSIO_KEY_FILE;
const keyPath = process.env.WAKU_GATEWAY_TLS_KEY;
const certPath = process.env.WAKU_GATEWAY_TLS_CERT;
const executionSetting = process.env.WAKU_GATEWAY_ENABLE_EXECUTION;

if (process.env.WAKU_GATEWAY_OWNER_TOKEN || process.env.WAKU_GATEWAY_COMPOSIO_KEY) {
  throw new Error('Provide gateway secrets through restricted files, not environment values');
}
if (!databasePath || !allowlistPath || !ownerTokenPath) {
  throw new Error('Set WAKU_GATEWAY_DB, WAKU_GATEWAY_ALLOWLIST, and WAKU_GATEWAY_OWNER_TOKEN_FILE');
}
if (process.platform !== 'win32') process.umask(0o077);
if (Boolean(keyPath) !== Boolean(certPath)) {
  throw new Error('Set both WAKU_GATEWAY_TLS_KEY and WAKU_GATEWAY_TLS_CERT');
}
if (executionSetting && executionSetting !== '1' && executionSetting !== '0') {
  throw new Error('WAKU_GATEWAY_ENABLE_EXECUTION must be 1 or 0');
}
if (executionSetting === '1' && !composioKeyPath) {
  throw new Error('Composio project key file is required to enable execution');
}

async function readSecret(path: string, label: string): Promise<string> {
  const info = await stat(path);
  if (!info.isFile()) throw new Error(`${label} must be a file`);
  if (process.platform !== 'win32' && (info.mode & 0o077) !== 0) {
    throw new Error(`${label} must be readable only by its owner`);
  }
  const value = (await readFile(path, 'utf8')).trim();
  if (!value) throw new Error(`${label} is empty`);
  return value;
}

const ownerToken = await readSecret(ownerTokenPath, 'Owner token');
const composioKey = composioKeyPath ? await readSecret(composioKeyPath, 'Composio key') : undefined;
if (keyPath) await readSecret(keyPath, 'TLS private key');
if (process.platform !== 'win32') {
  const directory = await stat(dirname(databasePath));
  if (!directory.isDirectory() || (directory.mode & 0o077) !== 0) {
    throw new Error('Gateway database directory must be accessible only to its owner');
  }
  try {
    const database = await stat(databasePath);
    if (!database.isFile() || (database.mode & 0o077) !== 0) {
      throw new Error('Gateway database must be readable only by its owner');
    }
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error;
  }
}

function isAllowedTool(value: unknown): value is AllowedTool {
  if (!value || typeof value !== 'object') return false;
  const entry = value as Record<string, unknown>;
  return ['toolkit', 'tool', 'version', 'accountId'].every((field) =>
    typeof entry[field] === 'string' && Boolean(entry[field].trim())) &&
    /^[a-z][a-z0-9_]{0,63}$/.test(entry.toolkit as string) &&
    /^[A-Z][A-Z0-9_]{0,127}$/.test(entry.tool as string) &&
    (entry.tool as string).startsWith(`${(entry.toolkit as string).toUpperCase()}_`) &&
    /^[A-Za-z0-9._-]{1,80}$/.test(entry.version as string) &&
    entry.version !== 'latest' && entry.accountId !== 'hosted_account' &&
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
  composio: composioKey
    ? new ComposioCatalog(composioKey)
    : undefined,
  executor: executionSetting === '1' && composioKey
    ? new ComposioExecutor(composioKey)
    : undefined,
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
