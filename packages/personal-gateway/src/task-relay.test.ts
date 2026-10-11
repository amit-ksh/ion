import { mkdtempSync, rmSync } from 'node:fs';
import { expect, test } from 'bun:test';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { Database } from 'bun:sqlite';
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
    expect(relay.listTasks().tasks).toEqual([{ ...task, publisherSeenAt: 1234, computerTarget: null }]);
    expect(relay.events(event.cursor).events).toEqual([]);
    expect(relay.pendingCommands(publisherId).map((entry) => entry.id)).toEqual([command.id]);
    expect(relay.claim(command.id, publisherId).status).toBe('uncertain');
    expect(() => relay.claim(command.id, publisherId)).toThrow('unused claim');
    relay.close();
    relay = new DurableTaskRelay(path, () => 1236);
    expect(relay.pendingCommands(publisherId)).toEqual([]);
    expect(relay.command(command.id)?.status).toBe('uncertain');
    expect(relay.settle(command.id, true, publisherId).status).toBe('applied');
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
    expect(relay.claim(command.id, publisherId).status).toBe('rejected');
    expect(relay.pendingCommands(publisherId)).toEqual([]);
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
    expect(relay.listTasks().tasks).toEqual([{ ...task, publisherSeenAt: null, computerTarget: null }]);
  } finally {
    relay.close();
    rmSync(directory, { recursive: true, force: true });
  }
});

test('exact provider permission and question snapshots gate phone commands', () => {
  const directory = mkdtempSync(join(tmpdir(), 'waku-task-relay-'));
  const relay = new DurableTaskRelay(join(directory, 'state.sqlite'), () => 5_000);
  try {
    relay.publishTask({ ...task, status: 'waiting' }, publisherId);
    const permission = relay.publishInteraction(publisherId, {
      taskId: task.id, runtimeId: task.runtimeId!, requestId: 'permission-1',
      kind: 'permission', title: 'Write file?', detail: 'Exact provider request',
      options: [{ id: 'allow-once', label: 'Allow once', allow: true },
        { id: 'deny', label: 'Deny', allow: false }], questions: [],
    });
    expect(relay.pendingInteractions(task.id).map((item) => item.id)).toEqual([permission.id]);
    const device = '44444444-4444-4444-8444-444444444444';
    const key = '55555555-5555-4555-8555-555555555555';
    expect(() => relay.enqueue(device, key, task.id, task.runtimeId!, 'permission', {
      interactionId: permission.id, fingerprint: '0'.repeat(64), optionId: 'allow-once',
    })).toThrow('no longer pending');
    expect(() => relay.enqueue(device, key, task.id, task.runtimeId!, 'permission', {
      interactionId: permission.id, fingerprint: permission.fingerprint, optionId: 'allow-always',
    })).toThrow('option');
    const command = relay.enqueue(device, key, task.id, task.runtimeId!, 'permission', {
      interactionId: permission.id, fingerprint: permission.fingerprint, optionId: 'deny',
    });
    expect(relay.claim(command.id, publisherId).status).toBe('uncertain');
    expect(relay.settle(command.id, true, publisherId).status).toBe('applied');
    expect(relay.pendingInteractions(task.id)).toEqual([]);

    const question = relay.publishInteraction(publisherId, {
      taskId: task.id, runtimeId: task.runtimeId!, requestId: 'question-1',
      kind: 'question', title: 'Choose a route', detail: '', options: [],
      questions: [{ id: 'route', header: 'Route', question: 'Which route?',
        options: [{ label: 'A' }, { label: 'B' }], multiSelect: false }],
    });
    expect(() => relay.enqueue(device, '66666666-6666-4666-8666-666666666666', task.id, task.runtimeId!, 'answer', {
      interactionId: question.id, fingerprint: question.fingerprint,
      answers: JSON.stringify([{ questionId: 'wrong', answers: ['A'] }]),
    })).toThrow('Invalid question answers');
    const answer = relay.enqueue(device, '66666666-6666-4666-8666-666666666666', task.id, task.runtimeId!, 'answer', {
      interactionId: question.id, fingerprint: question.fingerprint,
      answers: JSON.stringify([{ questionId: 'route', answers: ['A'] }]),
    });
    relay.resolveInteraction(publisherId, question.id);
    expect(relay.command(answer.id)?.status).toBe('rejected');
  } finally {
    relay.close();
    rmSync(directory, { recursive: true, force: true });
  }
});

test('event IDs stay deduplicated after feed pruning and restart', () => {
  const directory = mkdtempSync(join(tmpdir(), 'waku-task-relay-'));
  const path = join(directory, 'state.sqlite');
  let relay = new DurableTaskRelay(path);
  try {
    relay.publishTask(task, publisherId);
    const first = relay.appendEvent('message:old', task.id, 'message', 'Old visible message', 100);
    relay.close();
    const db = new Database(path);
    db.run('DELETE FROM relay_events WHERE event_id = ?1', ['message:old']);
    db.close(true);
    relay = new DurableTaskRelay(path);
    expect(relay.appendEvent('message:old', task.id, 'message', 'Old visible message', 100).cursor).toBe(first.cursor);
    expect(relay.events(0).events.map((event) => event.text)).not.toContain('Old visible message');
    expect(() => relay.appendEvent('message:old', task.id, 'message', 'Changed', 100)).toThrow('reused');
  } finally {
    relay.close();
    rmSync(directory, { recursive: true, force: true });
  }
});

test('computer target stays scoped to the owning runtime and clears when it ends', () => {
  const directory = mkdtempSync(join(tmpdir(), 'waku-task-relay-'));
  const relay = new DurableTaskRelay(join(directory, 'state.sqlite'));
  try {
    relay.publishTask(task, publisherId);
    const target = { taskId: task.id, runtimeId: task.runtimeId!, appName: 'Browser',
      windowTitle: 'Review page', phase: 'running' as const, visible: true };
    expect(() => relay.setComputerTarget('88888888-8888-4888-8888-888888888888', target)).toThrow('unavailable');
    expect(relay.setComputerTarget(publisherId, target)).toEqual({ visible: true });
    expect(relay.listTasks().tasks[0]?.computerTarget).toEqual(target);
    relay.publishTask({ ...task, status: 'idle', runtimeId: null, updatedAt: 101 }, publisherId);
    expect(relay.listTasks().tasks[0]?.computerTarget).toBeNull();
  } finally {
    relay.close();
    rmSync(directory, { recursive: true, force: true });
  }
});

test('claimed task controls retain publisher ownership after the task leaves the mirror', () => {
  const directory = mkdtempSync(join(tmpdir(), 'waku-task-relay-'));
  const relay = new DurableTaskRelay(join(directory, 'state.sqlite'));
  try {
    relay.publishTask(task, publisherId);
    const command = relay.enqueue('44444444-4444-4444-8444-444444444444', '55555555-5555-4555-8555-555555555555', task.id, task.runtimeId!, 'stop', {});
    expect(relay.claim(command.id, publisherId).status).toBe('uncertain');
    relay.reconcileTasks(publisherId, []);
    expect(() => relay.settle(command.id, true, '88888888-8888-4888-8888-888888888888')).toThrow('another publisher');
    expect(relay.settle(command.id, true, publisherId).status).toBe('applied');
  } finally { relay.close(); rmSync(directory, { recursive: true, force: true }); }
});

test('legacy command migration derives an existing task owner and keeps unknown orphan ownership closed', () => {
  const directory = mkdtempSync(join(tmpdir(), 'waku-task-relay-')); const path = join(directory,'state.sqlite');
  const commandId = '99999999-9999-4999-8999-999999999999'; const orphanId = 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa';
  const db = new Database(path);
  db.run('CREATE TABLE relay_tasks (id TEXT PRIMARY KEY, body TEXT NOT NULL, updated_at INTEGER NOT NULL, published_at INTEGER NOT NULL, publisher_id TEXT NOT NULL)');
  db.run('INSERT INTO relay_tasks VALUES (?1,?2,?3,?4,?5)',[task.id,JSON.stringify(task),100,100,publisherId]);
  db.run('CREATE TABLE relay_commands (id TEXT PRIMARY KEY, device_id TEXT NOT NULL, idempotency_key TEXT NOT NULL, task_id TEXT NOT NULL, runtime_id TEXT NOT NULL, kind TEXT NOT NULL, payload TEXT NOT NULL, status TEXT NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, UNIQUE(device_id,idempotency_key))');
  for (const id of [commandId,orphanId]) db.run('INSERT INTO relay_commands VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)',[id,'44444444-4444-4444-8444-444444444444',id,id===commandId?task.id:orphanId,task.runtimeId!,'stop','{}','queued',100,100]);
  db.close(true);
  const relay = new DurableTaskRelay(path);
  try {
    expect(relay.command(commandId)?.publisherId).toBe(publisherId);
    expect(relay.command(orphanId)?.publisherId).toBe('');
    expect(relay.command(orphanId)?.status).toBe('rejected');
    expect(relay.pendingCommands(publisherId).map((c)=>c.id)).toEqual([commandId]);
    expect(() => relay.claim(commandId,'88888888-8888-4888-8888-888888888888')).toThrow('another publisher');
    expect(() => relay.claim(orphanId,publisherId)).toThrow('another publisher');
    expect(relay.claim(commandId,publisherId).status).toBe('uncertain');
  } finally { relay.close(); rmSync(directory, {recursive:true,force:true}); }
});
