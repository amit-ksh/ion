import type { DurableGatewayAuthority } from './durable-authority';

export interface ExecutionOutcome {
  status: 'succeeded' | 'uncertain';
  resultReference?: string;
}

type FetchRequest = (input: Parameters<typeof fetch>[0], init?: RequestInit) => Promise<Response>;

async function smallJson(response: Response): Promise<unknown> {
  const reader = response.body?.getReader();
  if (!reader) throw new Error('Missing Composio response');
  const chunks: Uint8Array[] = [];
  let size = 0;
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      size += value.byteLength;
      if (size > 1_000_000) throw new Error('Composio response too large');
      chunks.push(value);
    }
  } finally {
    reader.releaseLock();
  }
  return JSON.parse(Buffer.concat(chunks).toString('utf8')) as unknown;
}

/** One direct tool call per approved proposal; never retries an uncertain call. */
export class ComposioExecutor {
  constructor(
    private readonly apiKey: string,
    private readonly request: FetchRequest = fetch,
  ) {
    if (!apiKey.trim()) throw new Error('Composio project key is required');
  }

  async executeApproved(
    authority: Pick<DurableGatewayAuthority, 'beginExecution' | 'recordSuccess'>,
    proposalId: string,
  ): Promise<ExecutionOutcome> {
    // The durable transaction commits `uncertain` before this returns.
    const effect = authority.beginExecution(proposalId);
    try {
      if (!/^[A-Z][A-Z0-9_]{0,127}$/.test(effect.tool) ||
          !effect.tool.startsWith(`${effect.toolkit.toUpperCase()}_`) ||
          !effect.version || effect.version === 'latest' ||
          effect.accountId === 'hosted_account') {
        return { status: 'uncertain' };
      }
      const argumentsValue: unknown = JSON.parse(effect.body);
      if (!argumentsValue || typeof argumentsValue !== 'object' || Array.isArray(argumentsValue)) {
        return { status: 'uncertain' };
      }
      const response = await this.request(
        `https://backend.composio.dev/api/v3.1/tools/execute/${effect.tool}`,
        {
          method: 'POST',
          headers: {
            'x-api-key': this.apiKey,
            'content-type': 'application/json',
            accept: 'application/json',
          },
          body: JSON.stringify({
            connected_account_id: effect.accountId,
            version: effect.version,
            arguments: argumentsValue,
          }),
          signal: AbortSignal.timeout(30_000),
        },
      );
      if (!response.ok) return { status: 'uncertain' };
      const raw = await smallJson(response);
      if (!raw || typeof raw !== 'object') return { status: 'uncertain' };
      const result = raw as Record<string, unknown>;
      if (result.successful !== true || result.error ||
          typeof result.log_id !== 'string' || !result.log_id.trim()) {
        return { status: 'uncertain' };
      }
      const data = result.data;
      if (data && typeof data === 'object' &&
          ['queued', 'pending', 'running', 'failed', 'error', 'cancelled', 'canceled']
            .includes(String((data as Record<string, unknown>).status).toLowerCase())) {
        return { status: 'uncertain' };
      }
      authority.recordSuccess(proposalId, result.log_id);
      return { status: 'succeeded', resultReference: result.log_id };
    } catch {
      // A timeout, transport error, or malformed response cannot prove that
      // the external effect did not occur. Manual reconciliation is required.
      return { status: 'uncertain' };
    }
  }
}
