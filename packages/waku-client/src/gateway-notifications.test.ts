import { test, expect } from "bun:test";
import {
  validateNotificationBatch,
  notificationDestination,
} from "./gateway-notifications";
const pairing = { expiresAt: 1000, grants: ["inboxRead", "taskRead"] };
test("notification batches accept only bounded ascending content-free facts", () => {
  const batch = {
    after: 0,
    next: 2,
    policy: "all",
    events: [
      { sequence: 1, needsAttention: false },
      { sequence: 2, needsAttention: true },
    ],
  };
  validateNotificationBatch(batch);
  validateNotificationBatch({
    after: null,
    next: 100,
    policy: "off",
    events: [],
  });
  for (const b of [
    { ...batch, after: null },
    { ...batch, next: 1 },
    { ...batch, policy: "off" },
    { ...batch, policy: "issuesOnly" },
    { ...batch, events: [batch.events[1], batch.events[0]] },
    { ...batch, events: [{ ...batch.events[0], title: "secret" }] },
    { ...batch, token: "secret" },
    { ...batch, after: -1 },
    { ...batch, next: Number.MAX_SAFE_INTEGER + 1 },
    { ...batch, events: Array(65).fill(batch.events[0]) },
  ])
    expect(() => validateNotificationBatch(b)).toThrow();
});
test("generic Inbox alert opens only the current authorized pairing and never uses alert IDs", () => {
  expect(
    notificationDestination({ version: 1, scope: "inbox" }, pairing, 999),
  ).toEqual({ scope: "inbox" });
  for (const data of [
    { version: 1, scope: "inbox", publisherId: "private" },
    { version: 2, scope: "inbox" },
    { version: 1, scope: "inbox", action: "approve" },
    { scope: "inbox" },
    null,
  ])
    expect(notificationDestination(data, pairing, 999)).toBeNull();
  expect(
    notificationDestination({ version: 1, scope: "inbox" }, pairing, 1000),
  ).toBeNull();
  expect(
    notificationDestination(
      { version: 1, scope: "inbox" },
      { ...pairing, grants: ["taskRead"] },
      999,
    ),
  ).toBeNull();
  expect(
    notificationDestination({ version: 1, scope: "inbox" }, null, 999),
  ).toBeNull();
});
test("existing task alerts retain exact UUID and current task-read requirement", () => {
  const taskId = "00000000-0000-4000-8000-000000000001";
  expect(notificationDestination({ taskId }, pairing, 999)).toEqual({
    scope: "task",
    taskId,
  });
  expect(
    notificationDestination({ taskId: "not-a-task" }, pairing, 999),
  ).toBeNull();
  expect(
    notificationDestination(
      { taskId },
      { ...pairing, grants: ["inboxRead"] },
      999,
    ),
  ).toBeNull();
});
