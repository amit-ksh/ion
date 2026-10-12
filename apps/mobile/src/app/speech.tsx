import { useCallback, useRef, useState } from "react";
import { Stack, useFocusEffect } from "expo-router";
import { AppState, ScrollView, Text, TextInput, Pressable } from "react-native";
import { useTheme } from "@/hooks/use-theme";
import { testSpeechService, speechUrl } from "@/lib/speech-service";
import {
  readSpeechEndpoint,
  saveSpeechEndpoint,
  removeSpeechEndpoint,
} from "@/lib/speech-storage";

export default function SpeechSettings() {
  const theme = useTheme();
  const [baseUrl, setUrl] = useState(""),
    [token, setToken] = useState("");
  const [busy, setBusy] = useState(false),
    [message, setMessage] = useState("");
  const focused = useRef(false),
    generation = useRef(0),
    active = useRef<AbortController | null>(null);
  const edited = useRef(false);
  useFocusEffect(
    useCallback(() => {
      focused.current = true;
      setBusy(active.current !== null);
      const current = ++generation.current;
      void readSpeechEndpoint()
        .then((endpoint) => {
          if (
            focused.current &&
            generation.current === current &&
            !edited.current &&
            endpoint
          )
            setUrl(endpoint.baseUrl);
        })
        .catch(() => {
          if (focused.current && generation.current === current)
            setMessage("Save the speech URL and token again");
        });
      const cancel = () => {
        ++generation.current;
        active.current?.abort();
      };
      const subscription = AppState.addEventListener("change", (state) => {
        if (state !== "active") cancel();
      });
      return () => {
        focused.current = false;
        cancel();
        subscription.remove();
      };
    }, []),
  );
  const valid = (current: number, controller: AbortController) =>
    focused.current &&
    generation.current === current &&
    AppState.currentState === "active" &&
    !controller.signal.aborted;
  async function run(remove: boolean) {
    if (
      active.current ||
      !focused.current ||
      AppState.currentState !== "active"
    )
      return;
    const controller = new AbortController(),
      current = ++generation.current;
    active.current = controller;
    setBusy(true);
    setMessage("");
    try {
      if (remove) {
        await removeSpeechEndpoint();
        if (valid(current, controller)) {
          setToken("");
          setMessage("Cloud speech removed");
        }
      } else {
        const url = speechUrl(baseUrl);
        // Reuse a saved secret only for its exact saved origin and an explicit Test.
        const saved = token.trim() ? null : await readSpeechEndpoint();
        if (!valid(current, controller)) return;
        const endpoint = {
          baseUrl: url,
          token: token.trim() || (saved?.baseUrl === url ? saved.token : ""),
        };
        await testSpeechService(endpoint, controller.signal);
        if (!valid(current, controller)) return;
        await saveSpeechEndpoint(endpoint);
        if (valid(current, controller)) {
          setToken("");
          setUrl(url);
          setMessage("Nemotron 3.5 ASR streaming is ready");
        }
      }
    } catch (error) {
      if (valid(current, controller))
        setMessage(
          error instanceof Error
            ? error.message
            : "Could not connect to speech. Check the URL and token.",
        );
    } finally {
      controller.abort();
      if (active.current === controller) {
        active.current = null;
        if (focused.current) setBusy(false);
      }
    }
  }
  const field = {
    color: theme.text,
    backgroundColor: theme.surface,
    padding: 14,
    fontSize: 16,
    borderRadius: 8,
    borderWidth: 1,
    borderColor: theme.borderStrong,
  };
  const action = {
    minHeight: 44,
    justifyContent: "center" as const,
    opacity: busy ? 0.5 : 1,
  };
  return (
    <ScrollView
      style={{ backgroundColor: theme.background }}
      contentContainerStyle={{ padding: 20, gap: 16 }}
      keyboardShouldPersistTaps="handled"
    >
      <Stack.Screen options={{ title: "Cloud speech" }} />
      <Text style={{ color: theme.text, fontSize: 20 }}>Nemotron 3.5 ASR</Text>
      <Text style={{ color: theme.textSecondary, fontSize: 16 }}>
        Phone dictation connects directly to your speech service. Stop waits for
        the final transcript. Review the text before sending a task.
      </Text>
      <Text style={{ color: theme.text, fontSize: 16 }}>
        Speech service URL
      </Text>
      <TextInput
        accessibilityLabel="Speech service URL"
        editable={!busy}
        autoCapitalize="none"
        autoCorrect={false}
        keyboardType="url"
        value={baseUrl}
        onChangeText={(value) => {
          edited.current = true;
          setUrl(value);
        }}
        placeholder="https://speech.example.com"
        placeholderTextColor={theme.textSecondary}
        style={field}
      />
      <Text style={{ color: theme.text, fontSize: 16 }}>Service token</Text>
      <TextInput
        accessibilityLabel="Speech service token"
        editable={!busy}
        secureTextEntry
        autoCapitalize="none"
        autoCorrect={false}
        value={token}
        onChangeText={setToken}
        placeholder="Enter or reuse your saved token"
        placeholderTextColor={theme.textSecondary}
        style={field}
      />
      <Pressable
        accessibilityRole="button"
        accessibilityState={{ disabled: busy, busy }}
        disabled={busy}
        onPress={() => void run(false)}
        style={action}
      >
        <Text style={{ color: theme.text, fontSize: 16 }}>
          {busy ? "Checking connection…" : "Test and save"}
        </Text>
      </Pressable>
      <Text style={{ color: theme.textSecondary, fontSize: 14 }}>
        Tests the model and streaming connection without recording audio.
      </Text>
      <Pressable
        accessibilityRole="button"
        accessibilityState={{ disabled: busy }}
        disabled={busy}
        onPress={() => void run(true)}
        style={action}
      >
        <Text style={{ color: theme.text, fontSize: 16 }}>
          Use desktop speech instead
        </Text>
      </Pressable>
      <Text
        accessibilityLiveRegion="polite"
        style={{ color: theme.textSecondary, fontSize: 16 }}
      >
        {message}
      </Text>
    </ScrollView>
  );
}
