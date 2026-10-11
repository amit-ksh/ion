import { DurablePortfolioRelay } from './portfolio-relay';
import type { AllowedTool } from './authority';
import { DurableGatewayAuthority } from './durable-authority';
import { createGatewayHandler } from './http';
import { DurableAutomationRelay } from './automation-relay';
import { DurableTaskRelay } from './task-relay';
import { GatewayPush } from './push';
import type { ComposioCatalog } from './composio';
import type { ComposioExecutor } from './composio-execution';

export interface GatewayServerOptions {
  databasePath: string;
  ownerToken: string;
  allowedTools: AllowedTool[];
  composio?: ComposioCatalog;
  executor?: ComposioExecutor;
  pushFetch?: typeof fetch;
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
  let relay: DurableTaskRelay | undefined;
  let automations: DurableAutomationRelay | undefined;
  let portfolio: DurablePortfolioRelay | undefined;
  let push: GatewayPush | undefined;
  try {
    relay = new DurableTaskRelay(options.databasePath);
    automations = new DurableAutomationRelay(options.databasePath);
    portfolio = new DurablePortfolioRelay(options.databasePath);
    push = new GatewayPush(options.databasePath, authority, options.pushFetch);
    const runtime = { relay, automations, push, portfolio };
    const server = Bun.serve({
      hostname,
      port: options.port ?? 0,
      tls: options.tls,
      maxRequestBodySize: 70_000,
      fetch: createGatewayHandler(authority, options.ownerToken, relay, push, options.composio, options.executor, automations, portfolio),
      error: () => Response.json({ error: 'Request rejected' }, { status: 500 }),
    });
    return {
      server,
      close: async () => {
        await server.stop();
        await runtime.push.close();
        runtime.portfolio.close();
        runtime.automations.close();
        runtime.relay.close();
        authority.close();
      },
    };
  } catch (error) {
    void push?.close();
    portfolio?.close();
    automations?.close();
    relay?.close();
    authority.close();
    throw error;
  }
}
