import { mkdtempSync, rmSync } from 'node:fs';
import { expect, test } from 'bun:test';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { DurableTaskRelay, type RelayTask } from './task-relay';

const task: RelayTask = {
  id: '11111111-1111-4111-8111-111111111111',
  projectId: '22222222-2222-4222-8222-222222222222',
  projectName: 'Test project', title: 'Inspect test', provider: 'nebius',
  status: 'running', runtimeId: '33333333-3333-4333-8333-333333333333', updatedAt: 100,
};
const publisherId = '77777777-7777-4777-8777-777777777777';

test('task feed and claimed commands survive restart without replay', () => {
  const directory = mkdtempSync(join(tmpdir(), 'waku-task-relay-'));
  const path = join(directory, 'state.sqlite');
  let relay = new DurableTaskRelay(path, () => 1234);
  try {
    expect(relay.publishTask(task, publisherId)).toEqual({ changed: true });
    expect(relay.publishTask(task, publisherId)).toEqual({ changed: false });
    expect(relay.heartbeat(publisherId).publishedAt).toBe(1234);
    const event = relay.appendEvent('runtime:1', task.id, 'message', 'Read-only update', 1234);
    expect(relay.appendEvent('runtime:1', task.id, 'message', 'Read-only update', 1234).cursor).toBe(event.cursor);
    expect(() => relay.appendEvent('runtime:1', task.id, 'message', 'Changed update', 1234)).toThrow('reused');
    expect(relay.events(0).events.map((entry) => entry.kind)).toEqual(['status', 'message']);
    const command = relay.enqueue('44444444-4444-4444-8444-444444444444',
      '55555555-5555-4555-8555-555555555555', task.id, task.runtimeId!, 'steer', { prompt: 'Review files' });
    expect(relay.enqueue('44444444-4444-4444-8444-444444444444',
      '55555555-5555-4555-8555-555555555555', task.id, task.runtimeId!, 'steer', { prompt: 'Review files' }).id).toBe(command.id);
    expect(() => relay.enqueue('44444444-4444-4444-8444-444444444444',
      '55555555-5555-4555-8555-555555555555', task.id, task.runtimeId!, 'steer', { prompt: 'Do something else' })).toThrow('reused');
    relay.close();
    relay = new DurableTaskRelay(path, () => 1235);
    expect(relay.listTasks().tasks).toEqual([{ ...task, publisherSeenAt: 1234 }]);
    expect(relay.events(event.cursor).events).toEqual([]);
    expect(relay.pendingCommands().map((entry) => entry.id)).toEqual([command.id]);
    expect(relay.claim(command.id).status).toBe('uncertain');
    expect(() => relay.claim(command.id)).toThrow('unused claim');
    relay.close();
    relay = new DurableTaskRelay(path, () => 1236);
    expect(relay.pendingCommands()).toEqual([]);
    expect(relay.command(command.id)?.status).toBe('uncertain');
    expect(relay.settle(command.id, true).status).toBe('applied');
  } finally {
    relay.close();
    rmSync(directory, { recursive: true, force: true });
  }
});

test('a changed runtime rejects queued controls before dispatch', () => {
  const directory = mkdtempSync(join(tmpdir(), 'waku-task-relay-'));
  const relay = new DurableTaskRelay(join(directory, 'state.sqlite'));
  try {
    relay.publishTask(task, publisherId);
    const command = relay.enqueue('44444444-4444-4444-8444-444444444444',
      '55555555-5555-4555-8555-555555555555', task.id, task.runtimeId!, 'stop', {});
    relay.publishTask({ ...task, runtimeId: '66666666-6666-4666-8666-666666666666', updatedAt: 101 }, publisherId);
    expect(relay.claim(command.id).status).toBe('rejected');
    expect(relay.pendingCommands()).toEqual([]);
  } finally {
    relay.close();
    rmSync(directory, { recursive: true, force: true });
  }
});

test('publisher reconciliation removes deleted tasks and rejects queued controls', () => {
  const directory = mkdtempSync(join(tmpdir(), 'waku-task-relay-'));
  const relay = new DurableTaskRelay(join(directory, 'state.sqlite'));
  try {
    relay.publishTask(task, publisherId);
    const command = relay.enqueue('44444444-4444-4444-8444-444444444444',
      '55555555-5555-4555-8555-555555555555', task.id, task.runtimeId!, 'stop', {});
    expect(relay.reconcileTasks(publisherId, [])).toEqual({ removed: 1 });
    expect(relay.listTasks().tasks).toEqual([]);
    expect(relay.command(command.id)?.status).toBe('rejected');
    expect(() => relay.publishTask(task, '88888888-8888-4888-8888-888888888888')).not.toThrow();
    expect(relay.reconcileTasks(publisherId, [])).toEqual({ removed: 0 });
    expect(relay.listTasks().tasks).toEqual([{ ...task, publisherSeenAt: null }]);
  } finally {
    relay.close();
    rmSync(directory, { recursive: true, force: true });
  }
});
