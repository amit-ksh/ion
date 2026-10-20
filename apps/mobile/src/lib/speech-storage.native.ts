import { cloudAccount } from "./cloud-account";
import AsyncStorage from "@react-native-async-storage/async-storage";
import * as SecureStore from "expo-secure-store";
import { speechUrl, type SpeechEndpoint } from "./speech-service";
const URL_KEY = "ion.speech.url.v1";
const TOKEN_KEY = "ion.speech.token.v1";
export async function readSpeechEndpoint(): Promise<SpeechEndpoint | null> {
  const [baseUrl, token] = await Promise.all([
    AsyncStorage.getItem(URL_KEY),
    SecureStore.getItemAsync(TOKEN_KEY),
  ]);
  if (!baseUrl && !token) return null;
  if (!baseUrl || !token)
    throw new Error("Save the speech URL and token again");
  return { baseUrl: speechUrl(baseUrl), token };
}
export async function saveSpeechEndpoint(
  endpoint: SpeechEndpoint,
): Promise<void> {
  const baseUrl = speechUrl(endpoint.baseUrl);
  if (!endpoint.token.trim() || /[\r\n]/.test(endpoint.token))
    throw new Error("Enter the speech service token");
  await SecureStore.setItemAsync(TOKEN_KEY, endpoint.token.trim(), {
    keychainAccessible: SecureStore.WHEN_UNLOCKED_THIS_DEVICE_ONLY,
  });
  await AsyncStorage.setItem(URL_KEY, baseUrl);
}
export async function removeSpeechEndpoint(): Promise<void> {
  await SecureStore.deleteItemAsync(TOKEN_KEY);
  await AsyncStorage.removeItem(URL_KEY);
}

/** Resolve once per recording; never persist a short-lived managed ticket. */
export async function resolveSpeechEndpoint(): Promise<SpeechEndpoint | null> {
  const saved = await readSpeechEndpoint();
  if (saved) return saved;
  const status = await cloudAccount.status();
  if (!status.signedIn) return null;
  const meta = await cloudAccount.request("/v1/bootstrap");
  if (meta.features?.speech !== true) return null;
  const ticket = await cloudAccount.request("/v1/speech/sessions", "POST", {
    maximumSeconds: 60,
  });
  const expected = process.env.EXPO_PUBLIC_ION_CLOUD_ORIGIN?.trim();
  if (
    ticket.baseUrl !== expected ||
    typeof ticket.token !== "string" ||
    !/^[A-Za-z0-9_-]{43}$/.test(ticket.token)
  )
    throw new Error("Managed speech returned an invalid session");
  return { baseUrl: speechUrl(ticket.baseUrl), token: ticket.token };
}
