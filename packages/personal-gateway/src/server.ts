import type { AllowedTool } from './authority';
import { DurableGatewayAuthority } from './durable-authority';
import { createGatewayHandler } from './http';
import { DurableTaskRelay } from './task-relay';
import type { ComposioCatalog } from './composio';
import type { ComposioExecutor } from './composio-execution';

export interface GatewayServerOptions {
  databasePath: string;
  ownerToken: string;
  allowedTools: AllowedTool[];
  composio?: ComposioCatalog;
  executor?: ComposioExecutor;
  hostname?: string;
  port?: number;
  tls?: { key: Bun.BunFile; cert: Bun.BunFile };
}

export function startGateway(options: GatewayServerOptions): {
  server: Bun.Server<undefined>;
  close: () => Promise<void>;
} {
  const hostname = options.hostname ?? '127.0.0.1';
  if (!['127.0.0.1', '::1', 'localhost'].includes(hostname) && !options.tls) {
    throw new Error('A non-loopback gateway requires TLS');
  }
  const authority = new DurableGatewayAuthority(options.databasePath, options.allowedTools);
  const relay = new DurableTaskRelay(options.databasePath);
  try {
    const server = Bun.serve({
      hostname,
      port: options.port ?? 0,
      tls: options.tls,
      maxRequestBodySize: 70_000,
      fetch: createGatewayHandler(authority, options.ownerToken, relay, options.composio, options.executor),
      error: () => Response.json({ error: 'Request rejected' }, { status: 500 }),
    });
    return {
      server,
      close: async () => {
        await server.stop();
        relay.close();
        authority.close();
      },
    };
  } catch (error) {
    relay.close();
    authority.close();
    throw error;
  }
}
