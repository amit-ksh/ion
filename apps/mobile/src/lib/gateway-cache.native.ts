import AsyncStorage from '@react-native-async-storage/async-storage';
import type { GatewayTask } from './gateway-client';

const KEY = 'ion.mobile.gateway-task-cache.v1';
type CachedTask = Pick<GatewayTask, 'id' | 'projectId' | 'provider' | 'status' | 'updatedAt'>;

export async function readCachedGatewayTasks(identity: string): Promise<GatewayTask[]> {
  const raw = await AsyncStorage.getItem(KEY);
  if (!raw) return [];
  try {
    const parsed = JSON.parse(raw) as { identity: string; tasks: CachedTask[] };
    if (parsed.identity !== identity || !Array.isArray(parsed.tasks)) return [];
    return parsed.tasks.filter((task) => typeof task.id === 'string').map((task) => ({
      ...task, title: `Task ${task.id.slice(0, 8)}`, projectName: 'Cached project', runtimeId: null,
    }));
  } catch { return []; }
}

export async function writeCachedGatewayTasks(identity: string, tasks: GatewayTask[]): Promise<void> {
  // The offline cache holds only identifiers, provider, status, and update time.
  // Transcript text, diffs, account details, and authority IDs stay out of it.
  const safe = tasks.slice(0, 200).map(({ id, projectId, provider, status, updatedAt }) =>
    ({ id, projectId, provider, status, updatedAt }));
  await AsyncStorage.setItem(KEY, JSON.stringify({ identity, tasks: safe }));
}

export async function clearCachedGatewayTasks(): Promise<void> { await AsyncStorage.removeItem(KEY); }
