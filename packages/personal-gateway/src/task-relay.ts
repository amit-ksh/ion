import { Database } from 'bun:sqlite';
import { randomUUID } from 'node:crypto';

export interface RelayTask {
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

export interface RelayEvent {
  cursor: number;
  taskId: string;
  kind: 'message' | 'activity' | 'artifact' | 'diff' | 'approval' | 'status';
  text: string;
  at: number;
}

export type RelayCommandKind = 'stop' | 'steer';
export interface RelayCommand {
  id: string;
  deviceId: string;
  taskId: string;
  runtimeId: string;
  kind: RelayCommandKind;
  payload: Record<string, string>;
  status: 'queued' | 'uncertain' | 'applied' | 'rejected';
  createdAt: number;
  updatedAt: number;
}

const uuid = /^[0-9a-f-]{36}$/;
const kinds = new Set<RelayEvent['kind']>(['message', 'activity', 'artifact', 'diff', 'approval', 'status']);
const commandKinds = new Set<RelayCommandKind>(['stop', 'steer']);

function validTask(value: RelayTask): void {
  if (!value || !uuid.test(value.id) || !uuid.test(value.projectId) ||
      (value.runtimeId !== null && !uuid.test(value.runtimeId)) ||
      !Number.isSafeInteger(value.updatedAt) || value.updatedAt < 0 ||
      typeof value.projectName !== 'string' || value.projectName.length > 240 ||
      typeof value.title !== 'string' || value.title.length > 240 ||
      typeof value.provider !== 'string' || value.provider.length > 80 ||
      typeof value.status !== 'string' || value.status.length > 40) {
    throw new Error('Invalid task summary');
  }
}

function validCommand(kind: RelayCommandKind, payload: Record<string, string>): void {
  if (!commandKinds.has(kind) || !payload || typeof payload !== 'object' || Array.isArray(payload)) {
    throw new Error('Invalid task command');
  }
  if (kind === 'stop' && Object.keys(payload).length !== 0) throw new Error('Stop has no arguments');
  if (kind === 'steer' && (typeof payload.prompt !== 'string' ||
      !payload.prompt.trim() || payload.prompt.length > 8_000 || Object.keys(payload).length !== 1)) {
    throw new Error('Steer needs a bounded prompt');
  }
}

/** Durable outbound desktop feed and single-claim phone command inbox. */
export class DurableTaskRelay {
  private readonly db: Database;

  constructor(path: string, private readonly now: () => number = Date.now) {
    this.db = new Database(path, { create: true, strict: true });
    this.db.run('PRAGMA busy_timeout = 5000');
    this.db.run('PRAGMA synchronous = FULL');
    this.db.run('CREATE TABLE IF NOT EXISTS relay_tasks (id TEXT PRIMARY KEY, body TEXT NOT NULL, updated_at INTEGER NOT NULL, published_at INTEGER NOT NULL, publisher_id TEXT NOT NULL DEFAULT \'\')');
    if (!this.db.query<{ name: string }, []>('PRAGMA table_info(relay_tasks)').all().some((column) => column.name === 'publisher_id')) {
      this.db.run("ALTER TABLE relay_tasks ADD COLUMN publisher_id TEXT NOT NULL DEFAULT ''");
    }
    this.db.run('CREATE TABLE IF NOT EXISTS relay_events (cursor INTEGER PRIMARY KEY AUTOINCREMENT, event_id TEXT UNIQUE NOT NULL, task_id TEXT NOT NULL, kind TEXT NOT NULL, text TEXT NOT NULL, at INTEGER NOT NULL)');
    this.db.run('CREATE TABLE IF NOT EXISTS relay_commands (id TEXT PRIMARY KEY, device_id TEXT NOT NULL, idempotency_key TEXT NOT NULL, task_id TEXT NOT NULL, runtime_id TEXT NOT NULL, kind TEXT NOT NULL, payload TEXT NOT NULL, status TEXT NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, UNIQUE(device_id, idempotency_key))');
    this.db.run('CREATE TABLE IF NOT EXISTS relay_publisher_presence (device_id TEXT PRIMARY KEY, seen_at INTEGER NOT NULL)');
  }

  close(): void { this.db.close(true); }

  heartbeat(publisherId: string): { publishedAt: number } {
    if (!uuid.test(publisherId)) throw new Error('Invalid task publisher');
    const publishedAt = this.now();
    this.db.run('INSERT INTO relay_publisher_presence (device_id, seen_at) VALUES (?1, ?2) ON CONFLICT(device_id) DO UPDATE SET seen_at = excluded.seen_at', [publisherId, publishedAt]);
    return { publishedAt };
  }

  private write<T>(operation: () => T): T {
    this.db.run('BEGIN IMMEDIATE');
    try {
      const result = operation();
      this.db.run('COMMIT');
      return result;
    } catch (error) {
      this.db.run('ROLLBACK');
      throw error;
    }
  }

  publishTask(task: RelayTask, publisherId: string): { changed: boolean } {
    validTask(task);
    if (!uuid.test(publisherId)) throw new Error('Invalid task publisher');
    return this.write(() => {
      const body = JSON.stringify(task);
      const previous = this.db.query<{ body: string; publisher_id: string }, [string]>('SELECT body, publisher_id FROM relay_tasks WHERE id = ?1').get(task.id);
      if (previous && previous.publisher_id && previous.publisher_id !== publisherId) throw new Error('Task belongs to another publisher');
      if (previous?.body === body && previous.publisher_id === publisherId) return { changed: false };
      this.db.run('INSERT INTO relay_tasks (id, body, updated_at, published_at, publisher_id) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT(id) DO UPDATE SET body = excluded.body, updated_at = excluded.updated_at, published_at = excluded.published_at, publisher_id = excluded.publisher_id', [task.id, body, task.updatedAt, this.now(), publisherId]);
      this.db.run('INSERT INTO relay_events (event_id, task_id, kind, text, at) VALUES (?1, ?2, ?3, ?4, ?5)', [randomUUID(), task.id, 'status', task.status, this.now()]);
      this.pruneEvents();
      return { changed: true };
    });
  }

  reconcileTasks(publisherId: string, activeIds: string[]): { removed: number } {
    if (!uuid.test(publisherId) || !Array.isArray(activeIds) || activeIds.length > 200 ||
        activeIds.some((id) => !uuid.test(id)) || new Set(activeIds).size !== activeIds.length) {
      throw new Error('Invalid task snapshot');
    }
    return this.write(() => {
      const owned = this.db.query<{ id: string }, [string]>(
        'SELECT id FROM relay_tasks WHERE publisher_id = ?1',
      ).all(publisherId);
      const active = new Set(activeIds);
      let removed = 0;
      for (const { id } of owned) {
        if (active.has(id)) continue;
        this.db.run('DELETE FROM relay_tasks WHERE id = ?1', [id]);
        this.db.run('DELETE FROM relay_events WHERE task_id = ?1', [id]);
        this.db.run("UPDATE relay_commands SET status = 'rejected', updated_at = ?2 WHERE task_id = ?1 AND status = 'queued'", [id, this.now()]);
        removed++;
      }
      return { removed };
    });
  }

  listTasks(): { tasks: RelayTask[]; publishedAt: number | null } {
    const rows = this.db.query<{ body: string; seen_at: number | null }, []>(
      'SELECT relay_tasks.body, relay_publisher_presence.seen_at FROM relay_tasks LEFT JOIN relay_publisher_presence ON relay_publisher_presence.device_id = relay_tasks.publisher_id ORDER BY relay_tasks.updated_at DESC LIMIT 200',
    ).all();
    return {
      tasks: rows.map((row) => ({ ...JSON.parse(row.body) as RelayTask, publisherSeenAt: row.seen_at })),
      publishedAt: rows.reduce<number | null>((latest, row) => Math.max(latest ?? 0, row.seen_at ?? 0) || null, null),
    };
  }

  appendEvent(eventId: string, taskId: string, kind: RelayEvent['kind'], text: string, at: number): RelayEvent {
    if (!eventId || eventId.length > 160 || !uuid.test(taskId) || !kinds.has(kind) ||
        typeof text !== 'string' || text.length > 48_000 ||
        !Number.isSafeInteger(at) || at < 0) throw new Error('Invalid task event');
    return this.write(() => {
      const previous = this.db.query<RelayEvent & { event_id: string }, [string]>(
        'SELECT cursor, event_id, task_id AS taskId, kind, text, at FROM relay_events WHERE event_id = ?1',
      ).get(eventId);
      if (previous) {
        if (previous.taskId !== taskId || previous.kind !== kind || previous.text !== text || previous.at !== at) {
          throw new Error('Event ID was reused with different content');
        }
        return previous;
      }
      if (!this.db.query('SELECT 1 FROM relay_tasks WHERE id = ?1').get(taskId)) throw new Error('Task is not published');
      this.db.run('INSERT INTO relay_events (event_id, task_id, kind, text, at) VALUES (?1, ?2, ?3, ?4, ?5)', [eventId, taskId, kind, text, at]);
      const row = this.db.query<{ cursor: number }, []>('SELECT last_insert_rowid() AS cursor').get()!;
      this.pruneEvents();
      return { cursor: row.cursor, taskId, kind, text, at };
    });
  }

  events(after: number, limit = 100): { events: RelayEvent[]; nextCursor: number; resetRequired: boolean } {
    if (!Number.isSafeInteger(after) || after < 0 || !Number.isInteger(limit) || limit < 1 || limit > 200) {
      throw new Error('Invalid event cursor');
    }
    const oldest = this.db.query<{ cursor: number }, []>('SELECT MIN(cursor) AS cursor FROM relay_events').get()?.cursor ?? 0;
    const resetRequired = oldest > 0 && after > 0 && after < oldest - 1;
    const rows = this.db.query<RelayEvent, [number, number]>(
      'SELECT cursor, task_id AS taskId, kind, text, at FROM relay_events WHERE cursor > ?1 ORDER BY cursor LIMIT ?2',
    ).all(resetRequired ? oldest - 1 : after, limit);
    return { events: rows, nextCursor: rows.at(-1)?.cursor ?? after, resetRequired };
  }

  private pruneEvents(): void {
    this.db.run('DELETE FROM relay_events WHERE cursor <= (SELECT COALESCE(MAX(cursor) - 10000, 0) FROM relay_events)');
  }

  enqueue(deviceId: string, idempotencyKey: string, taskId: string, runtimeId: string,
    kind: RelayCommandKind, payload: Record<string, string>): RelayCommand {
    if (!uuid.test(deviceId) || !uuid.test(idempotencyKey) || !uuid.test(taskId) || !uuid.test(runtimeId)) {
      throw new Error('Invalid task command identity');
    }
    validCommand(kind, payload);
    return this.write(() => {
      const existing = this.db.query<{ id: string }, [string, string]>(
        'SELECT id FROM relay_commands WHERE device_id = ?1 AND idempotency_key = ?2',
      ).get(deviceId, idempotencyKey);
      if (existing) {
        const command = this.command(existing.id)!;
        if (command.taskId !== taskId || command.runtimeId !== runtimeId ||
            command.kind !== kind || JSON.stringify(command.payload) !== JSON.stringify(payload)) {
          throw new Error('Idempotency key was reused for another command');
        }
        return command;
      }
      const task = this.db.query<{ body: string }, [string]>('SELECT body FROM relay_tasks WHERE id = ?1').get(taskId);
      if (!task || (JSON.parse(task.body) as RelayTask).runtimeId !== runtimeId) {
        throw new Error('Task runtime changed or is unavailable');
      }
      const id = randomUUID();
      const time = this.now();
      this.db.run('INSERT INTO relay_commands (id, device_id, idempotency_key, task_id, runtime_id, kind, payload, status, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)',
        [id, deviceId, idempotencyKey, taskId, runtimeId, kind, JSON.stringify(payload), 'queued', time, time]);
      return this.command(id)!;
    });
  }

  command(id: string): RelayCommand | null {
    const row = this.db.query<{
      id: string; device_id: string; task_id: string; runtime_id: string; kind: RelayCommandKind;
      payload: string; status: RelayCommand['status']; created_at: number; updated_at: number;
    }, [string]>('SELECT * FROM relay_commands WHERE id = ?1').get(id);
    if (!row) return null;
    return { id: row.id, deviceId: row.device_id, taskId: row.task_id, runtimeId: row.runtime_id,
      kind: row.kind, payload: JSON.parse(row.payload) as Record<string, string>,
      status: row.status, createdAt: row.created_at, updatedAt: row.updated_at };
  }

  pendingCommands(): RelayCommand[] {
    const rows = this.db.query<{ id: string }, []>(
      "SELECT id FROM relay_commands WHERE status = 'queued' ORDER BY created_at LIMIT 50",
    ).all();
    return rows.map((row) => this.command(row.id)!);
  }

  claim(id: string): RelayCommand {
    return this.write(() => {
      const command = this.command(id);
      if (!command || command.status !== 'queued') throw new Error('Command has no unused claim');
      const task = this.db.query<{ body: string }, [string]>('SELECT body FROM relay_tasks WHERE id = ?1').get(command.taskId);
      if (!task || (JSON.parse(task.body) as RelayTask).runtimeId !== command.runtimeId) {
        this.db.run("UPDATE relay_commands SET status = 'rejected', updated_at = ?2 WHERE id = ?1", [id, this.now()]);
        return this.command(id)!;
      }
      this.db.run("UPDATE relay_commands SET status = 'uncertain', updated_at = ?2 WHERE id = ?1", [id, this.now()]);
      return this.command(id)!;
    });
  }

  settle(id: string, applied: boolean): RelayCommand {
    return this.write(() => {
      const command = this.command(id);
      if (!command || command.status !== 'uncertain') throw new Error('Command is not awaiting a result');
      this.db.run('UPDATE relay_commands SET status = ?2, updated_at = ?3 WHERE id = ?1',
        [id, applied ? 'applied' : 'rejected', this.now()]);
      return this.command(id)!;
    });
  }
}
