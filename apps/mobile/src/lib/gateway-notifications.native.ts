import Constants from 'expo-constants';
import * as Notifications from 'expo-notifications';
import { Platform } from 'react-native';
import { gatewayPushStatus, registerGatewayPush, unregisterGatewayPush, type GatewayPairing } from './gateway-client';
import { readGatewayPairing } from './gateway-storage';

Notifications.setNotificationHandler({
  handleNotification: async () => ({
    shouldPlaySound: false, shouldSetBadge: false,
    shouldShowBanner: true, shouldShowList: true,
  }),
});

export async function gatewayNotificationsEnabled(pairing: GatewayPairing): Promise<boolean> {
  return gatewayPushStatus(pairing);
}

export async function enableGatewayNotifications(pairing: GatewayPairing): Promise<void> {
  if (!pairing.grants.includes('taskRead')) throw new Error('This phone needs the Read tasks grant.');
  const projectId = Constants.expoConfig?.extra?.eas?.projectId ?? Constants.easConfig?.projectId;
  if (typeof projectId !== 'string' || !projectId) {
    throw new Error('Set up the Expo EAS project ID and install a development build first.');
  }
  if (Platform.OS === 'android') {
    await Notifications.setNotificationChannelAsync('tasks', {
      name: 'Task requests', importance: Notifications.AndroidImportance.DEFAULT,
    });
  }
  let permissions = await Notifications.getPermissionsAsync();
  if (!permissions.granted && permissions.ios?.status !== Notifications.IosAuthorizationStatus.PROVISIONAL) {
    permissions = await Notifications.requestPermissionsAsync();
  }
  if (!permissions.granted && permissions.ios?.status !== Notifications.IosAuthorizationStatus.PROVISIONAL) {
    throw new Error('Notification permission was not granted.');
  }
  const token = (await Notifications.getExpoPushTokenAsync({ projectId })).data;
  await registerGatewayPush(pairing, token);
}

export async function disableGatewayNotifications(pairing: GatewayPairing): Promise<void> {
  await unregisterGatewayPush(pairing);
}

export function listenForGatewayNotifications(openTask: (taskId: string) => void): () => void {
  const handle = async (response: Notifications.NotificationResponse) => {
    const taskId = response.notification.request.content.data?.taskId;
    if (typeof taskId !== 'string' || !/^[0-9a-f-]{36}$/.test(taskId)) return;
    const pairing = await readGatewayPairing();
    if (pairing?.grants.includes('taskRead')) openTask(taskId);
    await Notifications.clearLastNotificationResponseAsync();
  };
  const subscription = Notifications.addNotificationResponseReceivedListener((response) => void handle(response));
  void Notifications.getLastNotificationResponseAsync().then((response) => {
    if (response) void handle(response);
  });
  return () => subscription.remove();
}
