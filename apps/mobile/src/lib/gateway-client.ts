export interface GatewayPairing {
  baseUrl: string;
  id: string;
  token: string;
  expiresAt: number;
  grants: string[];
}

export interface GatewayProposal {
  id: string;
  deviceId: string;
  fingerprint: string;
  status: 'pending' | 'approved' | 'denied' | 'uncertain' | 'succeeded';
  effect: {
    toolkit: string;
    tool: string;
    version: string;
    accountId: string;
    target: string;
    body: string;
    scopes: string[];
  };
  reviewedBy?: string;
}

export interface GatewayTask {
  id: string;
  projectId: string;
  projectName: string;
  title: string;
  provider: string;
  status: string;
  runtimeId: string | null;
  updatedAt: number;
  publisherSeenAt?: number | null;
}

export interface GatewayTaskEvent {
  cursor: number;
  taskId: string;
  kind: 'message' | 'activity' | 'artifact' | 'diff' | 'approval' | 'status';
  text: string;
  at: number;
}

export interface GatewayTaskCommand {
  id: string;
  taskId: string;
  runtimeId: string;
  kind: 'stop' | 'steer';
  status: 'queued' | 'uncertain' | 'applied' | 'rejected';
  createdAt: number;
  updatedAt: number;
}

export function gatewayBaseUrl(input: string): string {
  let url: URL;
  try { url = new URL(input.trim()); }
  catch { throw new Error('Enter a valid gateway URL'); }
  const loopback = ['127.0.0.1', 'localhost', '[::1]'].includes(url.hostname.toLowerCase());
  if (url.protocol !== 'https:' && !(url.protocol === 'http:' && loopback)) {
    throw new Error('Remote gateways require HTTPS');
  }
  if (url.username || url.password || url.pathname !== '/' || url.search || url.hash) {
    throw new Error('Enter only the gateway origin, without credentials or a path');
  }
  return url.origin;
}

async function request<T>(baseUrl: string, path: string, init: RequestInit): Promise<T> {
  const response = await fetch(`${baseUrl}${path}`, { ...init, cache: 'no-store', redirect: 'error' });
  if (!response.ok) throw new Error(`Gateway request failed (HTTP ${response.status})`);
  return response.json() as Promise<T>;
}

function deviceHeaders(pairing: GatewayPairing): HeadersInit {
  return { Authorization: `Bearer ${pairing.token}`, 'X-Waku-Device-Id': pairing.id };
}

export async function pairGateway(baseUrlInput: string, invite: string): Promise<GatewayPairing> {
  const baseUrl = gatewayBaseUrl(baseUrlInput);
  if (!invite.trim()) throw new Error('Enter the one-use invite');
  const result = await request<Omit<GatewayPairing, 'baseUrl'>>(
    baseUrl, '/v1/pair', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ invite: invite.trim(), name: 'ion mobile' }),
    },
  );
  if (!/^[0-9a-f-]{36}$/.test(result.id) || !result.token ||
      !Number.isFinite(result.expiresAt) || !Array.isArray(result.grants)) {
    throw new Error('Gateway returned an invalid pairing');
  }
  return { ...result, baseUrl };
}

export async function gatewayReviewQueue(pairing: GatewayPairing): Promise<GatewayProposal[]> {
  const response = await request<{ proposals: GatewayProposal[] }>(
    pairing.baseUrl, '/v1/device/review-queue', { headers: deviceHeaders(pairing) },
  );
  if (!Array.isArray(response.proposals)) throw new Error('Gateway returned an invalid review queue');
  return response.proposals;
}

export async function decideGatewayProposal(
  pairing: GatewayPairing, proposal: GatewayProposal, approve: boolean,
): Promise<GatewayProposal> {
  return request<GatewayProposal>(
    pairing.baseUrl, `/v1/device/proposals/${proposal.id}/decision`, {
      method: 'POST',
      headers: { ...deviceHeaders(pairing), 'Content-Type': 'application/json' },
      body: JSON.stringify({ fingerprint: proposal.fingerprint, approve }),
    },
  );
}

export async function gatewayReviewStatus(
  pairing: GatewayPairing, proposalId: string,
): Promise<GatewayProposal> {
  return request<GatewayProposal>(
    pairing.baseUrl, `/v1/device/reviews/${proposalId}`,
    { headers: deviceHeaders(pairing) },
  );
}

export async function gatewayTasks(pairing: GatewayPairing): Promise<{
  tasks: GatewayTask[]; publishedAt: number | null;
}> {
  return request(pairing.baseUrl, '/v1/device/tasks', { headers: deviceHeaders(pairing) });
}

export async function gatewayTaskEvents(pairing: GatewayPairing, after: number): Promise<{
  events: GatewayTaskEvent[]; nextCursor: number; resetRequired: boolean;
}> {
  if (!Number.isSafeInteger(after) || after < 0) throw new Error('Invalid event cursor');
  return request(pairing.baseUrl, `/v1/device/events?after=${after}`, { headers: deviceHeaders(pairing) });
}

export async function enqueueGatewayTaskCommand(pairing: GatewayPairing, input: {
  idempotencyKey: string; taskId: string; runtimeId: string;
  kind: 'stop' | 'steer'; payload: Record<string, string>;
}): Promise<GatewayTaskCommand> {
  return request(pairing.baseUrl, '/v1/device/commands', {
    method: 'POST',
    headers: { ...deviceHeaders(pairing), 'Content-Type': 'application/json' },
    body: JSON.stringify(input),
  });
}

export async function gatewayTaskCommand(pairing: GatewayPairing, id: string): Promise<GatewayTaskCommand> {
  if (!/^[0-9a-f-]{36}$/.test(id)) throw new Error('Invalid task command ID');
  return request(pairing.baseUrl, `/v1/device/commands/${id}`, { headers: deviceHeaders(pairing) });
}
