import type { InboxWriteRequest } from "./generated/InboxWriteRequest";
import { portfolioId } from "./gateway-portfolio";
export type InboxSubmission = Omit<InboxWriteRequest, "deviceId">;
export interface InboxCommand extends InboxSubmission {
  id: string;
  deviceId: string;
  status: "queued" | "uncertain" | "applied" | "rejected";
  createdAt: number;
  updatedAt: number;
}
export function inboxSubmission(command: InboxCommand): InboxSubmission {
  return {
    publisherId: command.publisherId,
    idempotencyKey: command.idempotencyKey,
    sequence: command.sequence,
    expectedRevision: command.expectedRevision,
    action: command.action,
    expiresAt: command.expiresAt,
  };
}
function exact(value: unknown, keys: string): value is Record<string, any> {
  return (
    !!value &&
    typeof value === "object" &&
    !Array.isArray(value) &&
    Object.keys(value).sort().join(",") === keys.split(",").sort().join(",")
  );
}
function n(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
}
export function validateInboxSubmission(
  value: unknown,
): asserts value is InboxSubmission {
  if (
    !exact(
      value,
      "publisherId,idempotencyKey,sequence,expectedRevision,action,expiresAt",
    ) ||
    !portfolioId(value.publisherId) ||
    !portfolioId(value.idempotencyKey) ||
    !n(value.sequence) ||
    value.sequence < 1 ||
    !n(value.expectedRevision) ||
    !n(value.expiresAt) ||
    !["markRead", "markUnread", "acknowledge", "archive", "restore"].includes(
      value.action,
    )
  )
    throw new Error("Invalid Inbox notification request");
}
export function validateInboxCommand(
  value: unknown,
): asserts value is InboxCommand {
  if (
    !exact(
      value,
      "publisherId,idempotencyKey,sequence,expectedRevision,action,expiresAt,id,deviceId,status,createdAt,updatedAt",
    ) ||
    !portfolioId(value.id) ||
    !portfolioId(value.deviceId) ||
    !n(value.createdAt) ||
    !n(value.updatedAt) ||
    value.updatedAt < value.createdAt ||
    !["queued", "uncertain", "applied", "rejected"].includes(value.status)
  )
    throw new Error("Invalid Inbox command");
  validateInboxSubmission(inboxSubmission(value as InboxCommand));
}
export function sameInboxSubmission(
  a: InboxSubmission,
  b: InboxSubmission,
): boolean {
  return (
    [
      "publisherId",
      "idempotencyKey",
      "sequence",
      "expectedRevision",
      "action",
      "expiresAt",
    ] as const
  ).every((key) => a[key] === b[key]);
}
