import { ionCloudOrigin } from "../../../../packages/ion-cloud-client/src/public-config";
import * as Crypto from "expo-crypto";
import * as SecureStore from "expo-secure-store";
import * as WebBrowser from "expo-web-browser";
import { createCloudAccount } from "./cloud-account-controller";
import { configureGatewayAccountAuthorization } from "./gateway-account-auth";
import {
  deleteGatewayPairing,
  readGatewayPairing,
  writeGatewayPairing,
} from "./gateway-storage";
import { clearCachedGatewayTasks } from "./gateway-cache";
const key = "ion.mobile.account.v1";
export const cloudAccount = createCloudAccount({
  cloudServicesEnabled: false,
  origin: process.env.EXPO_PUBLIC_ION_CLOUD_ORIGIN?.trim() || ionCloudOrigin,
  vault: {
    read: () => SecureStore.getItemAsync(key),
    write: (value) =>
      SecureStore.setItemAsync(key, value, {
        keychainAccessible: SecureStore.WHEN_UNLOCKED_THIS_DEVICE_ONLY,
      }),
    remove: () => SecureStore.deleteItemAsync(key),
  },
  crypto: {
    randomBytes: Crypto.getRandomBytesAsync,
    sha256: async (value) => {
      const hex = await Crypto.digestStringAsync(
        Crypto.CryptoDigestAlgorithm.SHA256,
        value,
      );
      return Uint8Array.from(hex.match(/../g) ?? [], (byte) =>
        Number.parseInt(byte, 16),
      );
    },
  },
  openBrowser: (url, callback) =>
    WebBrowser.openAuthSessionAsync(url, callback, {
      preferEphemeralSession: true,
    }),
  readPairing: readGatewayPairing,
  writePairing: writeGatewayPairing,
  deletePairing: deleteGatewayPairing,
  clearTasks: clearCachedGatewayTasks,
});
configureGatewayAccountAuthorization(cloudAccount.authorize);
