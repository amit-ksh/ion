import type { NotificationDestination, PushTopic } from "@waku/client";
import type { GatewayPairing } from "./gateway-client";
export async function gatewayNotificationsEnabled(
  _pairing: GatewayPairing,
  _topic: PushTopic = "taskRequests",
): Promise<boolean> {
  return false;
}
export async function enableGatewayNotifications(
  _pairing: GatewayPairing,
  _topic: PushTopic = "taskRequests",
  _authorized: () => boolean = () => true,
): Promise<void> {
  throw new Error("Phone push notifications require the iOS or Android app.");
}
export async function disableGatewayNotifications(
  _pairing: GatewayPairing,
  _topic: PushTopic = "taskRequests",
): Promise<void> {}
export function listenForGatewayNotifications(
  _open: (destination: NotificationDestination) => void,
): () => void {
  return () => {};
}
