import { Database } from 'bun:sqlite';
import type { DurableGatewayAuthority } from './durable-authority';

const tokenPattern = /^(?:Expo|Exponent)PushToken\[[A-Za-z0-9_-]{20,256}\]$/;
const sendUrl = 'https://exp.host/--/api/v2/push/send';
const receiptsUrl = 'https://exp.host/--/api/v2/push/getReceipts';

type Ticket = { status: 'ok' | 'error'; id?: string; details?: { error?: string } };

/** Opt-in, device-scoped push notices. No task text or transcript leaves the gateway. */
export class GatewayPush {
  private readonly db: Database;
  private readonly timer: ReturnType<typeof setInterval>;
  private busy = false;

  constructor(path: string, private readonly authority: DurableGatewayAuthority,
    private readonly send: typeof fetch = fetch, private readonly now: () => number = Date.now) {
    this.db = new Database(path, { create: true, strict: true });
    this.db.run('PRAGMA busy_timeout = 5000');
    this.db.run('PRAGMA synchronous = FULL');
    this.db.run('CREATE TABLE IF NOT EXISTS gateway_push_subscriptions (device_id TEXT PRIMARY KEY, token TEXT NOT NULL)');
    this.db.run('CREATE TABLE IF NOT EXISTS gateway_push_jobs (id INTEGER PRIMARY KEY AUTOINCREMENT, device_id TEXT NOT NULL, event_key TEXT NOT NULL, task_id TEXT NOT NULL, status TEXT NOT NULL, ticket_id TEXT, submitted_at INTEGER, UNIQUE(device_id, event_key))');
    this.timer = setInterval(() => void this.flush(), 5_000);
    this.timer.unref?.();
  }

  async close(): Promise<void> {
    clearInterval(this.timer);
    while (this.busy) await new Promise((resolve) => setTimeout(resolve, 20));
    this.db.close(true);
  }

  register(deviceId: string, token: string): void {
    if (!tokenPattern.test(token)) throw new Error('Invalid Expo push token');
    this.db.run('INSERT INTO gateway_push_subscriptions (device_id, token) VALUES (?1, ?2) ON CONFLICT(device_id) DO UPDATE SET token = excluded.token', [deviceId, token]);
  }

  unregister(deviceId: string): void {
    this.db.run('DELETE FROM gateway_push_subscriptions WHERE device_id = ?1', [deviceId]);
    this.db.run("UPDATE gateway_push_jobs SET status = 'cancelled' WHERE device_id = ?1 AND status = 'queued'", [deviceId]);
  }

  isRegistered(deviceId: string): boolean {
    return !!this.db.query('SELECT 1 FROM gateway_push_subscriptions WHERE device_id = ?1').get(deviceId);
  }

  enqueue(taskId: string, eventKey: string): void {
    if (!/^[0-9a-f-]{36}$/.test(taskId) || !eventKey || eventKey.length > 200) return;
    const allowed = new Set(this.authority.listDevices().filter((device) =>
      !device.revoked && device.expiresAt > this.now() && device.grants.includes('taskRead'),
    ).map((device) => device.id));
    const rows = this.db.query<{ device_id: string }, []>('SELECT device_id FROM gateway_push_subscriptions').all();
    for (const { device_id } of rows) {
      if (!allowed.has(device_id)) { this.unregister(device_id); continue; }
      this.db.run("INSERT OR IGNORE INTO gateway_push_jobs (device_id, event_key, task_id, status) VALUES (?1, ?2, ?3, 'queued')",
        [device_id, eventKey, taskId]);
    }
    void this.flush();
  }

  async flush(): Promise<void> {
    if (this.busy) return;
    this.busy = true;
    try {
      const jobs = this.db.query<{ id: number; device_id: string; task_id: string; token: string }, []>(
        "SELECT j.id, j.device_id, j.task_id, s.token FROM gateway_push_jobs j JOIN gateway_push_subscriptions s ON s.device_id = j.device_id WHERE j.status = 'queued' ORDER BY j.id LIMIT 100",
      ).all();
      if (jobs.length) {
        const allowed = new Set(this.authority.listDevices().filter((device) =>
          !device.revoked && device.expiresAt > this.now() && device.grants.includes('taskRead'),
        ).map((device) => device.id));
        const sending = jobs.filter((job) => allowed.has(job.device_id));
        for (const job of jobs) {
          if (!allowed.has(job.device_id)) this.unregister(job.device_id);
        }
        // Mark before network I/O: a lost response cannot send the same alert twice.
        for (const job of sending) this.db.run("UPDATE gateway_push_jobs SET status = 'uncertain' WHERE id = ?1", [job.id]);
        if (sending.length) {
          try {
            const response = await this.send(sendUrl, { method: 'POST',
              headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
              body: JSON.stringify(sending.map((job) => ({ to: job.token, title: 'Task needs input',
                body: 'Open ion to review the pending request.', data: { taskId: job.task_id },
                channelId: 'tasks', sound: 'default' }))), signal: AbortSignal.timeout(8_000) });
            if (response.ok) {
              const result = await response.json() as { data?: Ticket[] | Ticket };
              const tickets = Array.isArray(result.data) ? result.data : [result.data];
              sending.forEach((job, index) => {
                const ticket = tickets[index];
                if (ticket?.status === 'ok' && ticket.id) {
                  this.db.run("UPDATE gateway_push_jobs SET status = 'submitted', ticket_id = ?2, submitted_at = ?3 WHERE id = ?1",
                    [job.id, ticket.id, this.now()]);
                } else if (ticket?.status === 'error') {
                  this.db.run("UPDATE gateway_push_jobs SET status = 'failed' WHERE id = ?1", [job.id]);
                  if (ticket.details?.error === 'DeviceNotRegistered') this.unregister(job.device_id);
                }
              });
            }
          } catch { /* An uncertain send is never replayed. */ }
        }
      }
      await this.checkReceipts();
    } finally { this.busy = false; }
  }

  private async checkReceipts(): Promise<void> {
    const due = this.db.query<{ id: number; device_id: string; ticket_id: string }, [number]>(
      "SELECT id, device_id, ticket_id FROM gateway_push_jobs WHERE status = 'submitted' AND submitted_at <= ?1 ORDER BY id LIMIT 100",
    ).all(this.now() - 15 * 60_000);
    if (!due.length) return;
    try {
      const response = await this.send(receiptsUrl, { method: 'POST',
        headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
        body: JSON.stringify({ ids: due.map((job) => job.ticket_id) }), signal: AbortSignal.timeout(8_000) });
      if (!response.ok) return;
      const result = await response.json() as { data?: Record<string, Ticket> };
      for (const job of due) {
        const receipt = result.data?.[job.ticket_id];
        if (!receipt) continue;
        this.db.run('UPDATE gateway_push_jobs SET status = ?2 WHERE id = ?1',
          [job.id, receipt.status === 'ok' ? 'delivered' : 'failed']);
        if (receipt.details?.error === 'DeviceNotRegistered') this.unregister(job.device_id);
      }
    } catch { /* Check again after the next interval. */ }
  }
}
