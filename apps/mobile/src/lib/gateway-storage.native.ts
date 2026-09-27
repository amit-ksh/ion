import AsyncStorage from '@react-native-async-storage/async-storage';
import * as SecureStore from 'expo-secure-store';
import type { GatewayPairing } from './gateway-client';

const META_KEY = 'ion.mobile.gateway.v1';
const TOKEN_KEY = 'ion.mobile.gateway-token.v1';

export async function readGatewayPairing(): Promise<GatewayPairing | null> {
  const [meta, token] = await Promise.all([
    AsyncStorage.getItem(META_KEY), SecureStore.getItemAsync(TOKEN_KEY),
  ]);
  if (!meta || !token) return null;
  try {
    const parsed = JSON.parse(meta) as Omit<GatewayPairing, 'token'>;
    if (!parsed.baseUrl || !parsed.id || !Number.isFinite(parsed.expiresAt) ||
        !Array.isArray(parsed.grants)) return null;
    return { ...parsed, token };
  } catch { return null; }
}

export async function writeGatewayPairing(pairing: GatewayPairing): Promise<void> {
  await SecureStore.setItemAsync(TOKEN_KEY, pairing.token, {
    keychainAccessible: SecureStore.WHEN_UNLOCKED_THIS_DEVICE_ONLY,
  });
  const { token: _token, ...meta } = pairing;
  await AsyncStorage.setItem(META_KEY, JSON.stringify(meta));
}

export async function deleteGatewayPairing(): Promise<void> {
  await AsyncStorage.removeItem(META_KEY);
  await SecureStore.deleteItemAsync(TOKEN_KEY);
}
