import type { GatewayTask } from './gateway-client';
let cache: { identity: string; tasks: GatewayTask[] } | null = null;
export async function readCachedGatewayTasks(identity: string): Promise<GatewayTask[]> {
  return cache?.identity === identity ? cache.tasks : [];
}
export async function writeCachedGatewayTasks(identity: string, tasks: GatewayTask[]): Promise<void> {
  cache = { identity, tasks: tasks.map(({ id, projectId, provider, status, updatedAt }) => ({
    id, projectId, provider, status, updatedAt, title: `Task ${id.slice(0, 8)}`,
    projectName: 'Cached project', runtimeId: null, publisherSeenAt: null, computerTarget: null,
  })) };
}
export async function clearCachedGatewayTasks(): Promise<void> { cache = null; }
