import {
  notificationDestination,
  type NotificationDestination,
  type PushTopic,
} from "@waku/client";
import Constants from "expo-constants";
import * as Notifications from "expo-notifications";
import { AppState, Platform } from "react-native";
import {
  gatewayPushStatus,
  registerGatewayPush,
  unregisterGatewayPush,
  type GatewayPairing,
} from "./gateway-client";
import { readGatewayPairing } from "./gateway-storage";

Notifications.setNotificationHandler({
  handleNotification: async () => ({
    shouldPlaySound: false,
    shouldSetBadge: false,
    shouldShowBanner: true,
    shouldShowList: true,
  }),
});

export async function gatewayNotificationsEnabled(
  pairing: GatewayPairing,
  topic: PushTopic = "taskRequests",
): Promise<boolean> {
  return gatewayPushStatus(pairing, topic);
}

export async function enableGatewayNotifications(
  pairing: GatewayPairing,
  topic: PushTopic = "taskRequests",
  authorized: () => boolean = () => true,
): Promise<void> {
  const grant = topic === "inbox" ? "inboxRead" : "taskRead";
  if (!pairing.grants.includes(grant))
    throw new Error(
      topic === "inbox"
        ? "This phone needs the Read owner Inbox history grant."
        : "This phone needs the Read tasks grant.",
    );
  if (
    !authorized() ||
    AppState.currentState !== "active" ||
    pairing.expiresAt <= Date.now()
  )
    throw new Error("Return to this pairing before enabling notifications.");
  const projectId =
    Constants.expoConfig?.extra?.eas?.projectId ??
    Constants.easConfig?.projectId;
  if (typeof projectId !== "string" || !projectId) {
    throw new Error(
      "Set up the Expo EAS project ID and install a development build first.",
    );
  }
  if (Platform.OS === "android") {
    await Notifications.setNotificationChannelAsync(
      topic === "inbox" ? "inbox" : "tasks",
      {
        name: topic === "inbox" ? "Inbox updates" : "Task requests",
        importance: Notifications.AndroidImportance.DEFAULT,
      },
    );
  }
  let permissions = await Notifications.getPermissionsAsync();
  if (!authorized() || AppState.currentState !== "active")
    throw new Error("Return to this pairing before enabling notifications.");
  if (
    !permissions.granted &&
    permissions.ios?.status !== Notifications.IosAuthorizationStatus.PROVISIONAL
  ) {
    permissions = await Notifications.requestPermissionsAsync();
  }
  if (
    !permissions.granted &&
    permissions.ios?.status !== Notifications.IosAuthorizationStatus.PROVISIONAL
  ) {
    throw new Error("Notification permission was not granted.");
  }
  if (!authorized() || AppState.currentState !== "active")
    throw new Error("Return to this pairing before enabling notifications.");
  const token = (await Notifications.getExpoPushTokenAsync({ projectId })).data;
  const current = await readGatewayPairing();
  if (
    !authorized() ||
    AppState.currentState !== "active" ||
    !current ||
    current.id !== pairing.id ||
    current.baseUrl !== pairing.baseUrl ||
    current.token !== pairing.token ||
    current.expiresAt <= Date.now() ||
    !current.grants.includes(grant)
  )
    throw new Error("Pairing changed. Refresh before enabling notifications.");
  await registerGatewayPush(pairing, token, topic);
}

export async function disableGatewayNotifications(
  pairing: GatewayPairing,
  topic: PushTopic = "taskRequests",
): Promise<void> {
  await unregisterGatewayPush(pairing, topic);
}

export function listenForGatewayNotifications(
  open: (destination: NotificationDestination) => void,
): () => void {
  let active = true;
  const seen = new Set<string>();
  const handle = async (response: Notifications.NotificationResponse) => {
    const id = response.notification.request.identifier;
    if (seen.has(id)) return;
    seen.add(id);
    if (seen.size > 128) seen.delete(seen.values().next().value!);
    const pairing = await readGatewayPairing();
    const target = notificationDestination(
      response.notification.request.content.data,
      pairing,
    );
    if (!active) return;
    if (target) open(target);
    await Notifications.clearLastNotificationResponseAsync();
  };
  const subscription = Notifications.addNotificationResponseReceivedListener(
    (response) => void handle(response).catch(() => {}),
  );
  void Notifications.getLastNotificationResponseAsync()
    .then((response) => {
      if (response && active) void handle(response).catch(() => {});
    })
    .catch(() => {});
  return () => {
    active = false;
    subscription.remove();
  };
}
