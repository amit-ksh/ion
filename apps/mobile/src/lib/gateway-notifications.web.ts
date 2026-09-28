import type { GatewayPairing } from './gateway-client';
export async function gatewayNotificationsEnabled(_pairing: GatewayPairing): Promise<boolean> { return false; }
export async function enableGatewayNotifications(_pairing: GatewayPairing): Promise<void> {
  throw new Error('Phone push notifications require the iOS or Android app.');
}
export async function disableGatewayNotifications(_pairing: GatewayPairing): Promise<void> {}
export function listenForGatewayNotifications(_openTask: (taskId: string) => void): () => void {
  return () => {};
}
