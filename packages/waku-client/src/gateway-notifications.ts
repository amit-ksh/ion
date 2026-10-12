import { portfolioId } from "./gateway-portfolio";
export type PushTopic = "taskRequests" | "inbox";
export interface GatewayNotificationBatch {
  after: number | null;
  next: number;
  policy: "all" | "issuesOnly" | "off";
  events: { sequence: number; needsAttention: boolean }[];
}
const shape = (v: unknown, keys: string): v is Record<string, any> =>
  !!v &&
  typeof v === "object" &&
  !Array.isArray(v) &&
  Object.keys(v).sort().join(",") === keys.split(",").sort().join(",");
const cursor = (v: unknown): v is number =>
  typeof v === "number" && Number.isSafeInteger(v) && v >= 0;
export function validateNotificationBatch(
  value: unknown,
): asserts value is GatewayNotificationBatch {
  if (
    !shape(value, "after,next,policy,events") ||
    (value.after !== null && !cursor(value.after)) ||
    !cursor(value.next) ||
    !["all", "issuesOnly", "off"].includes(value.policy) ||
    !Array.isArray(value.events) ||
    value.events.length > 64 ||
    (value.after !== null && value.next < value.after) ||
    (value.after === null && value.events.length > 0)
  )
    throw new Error("Invalid Inbox notification batch");
  let last = value.after ?? 0;
  for (const event of value.events) {
    if (
      !shape(event, "sequence,needsAttention") ||
      !cursor(event.sequence) ||
      event.sequence <= last ||
      event.sequence > value.next ||
      typeof event.needsAttention !== "boolean" ||
      value.policy === "off" ||
      (value.policy === "issuesOnly" && !event.needsAttention)
    )
      throw new Error("Invalid Inbox notification event");
    last = event.sequence;
  }
}
export type NotificationDestination =
  { scope: "task"; taskId: string } | { scope: "inbox" };
/** Generic Inbox alert routes to the current pairing; all owner reads occur there. */
export function notificationDestination(
  value: unknown,
  pairing: { expiresAt: number; grants: string[] } | null,
  now = Date.now(),
): NotificationDestination | null {
  if (
    !pairing ||
    pairing.expiresAt <= now ||
    !value ||
    typeof value !== "object" ||
    Array.isArray(value)
  )
    return null;
  const data = value as Record<string, unknown>;
  if (
    shape(data, "version,scope") &&
    data.version === 1 &&
    data.scope === "inbox" &&
    pairing.grants.includes("inboxRead")
  )
    return { scope: "inbox" };
  if (
    shape(data, "taskId") &&
    pairing.grants.includes("taskRead") &&
    portfolioId(data.taskId)
  )
    return { scope: "task", taskId: data.taskId };
  return null;
}
