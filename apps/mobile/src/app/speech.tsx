import { Stack } from "expo-router";
import { ScrollView, Text } from "react-native";
import { useTheme } from "@/hooks/use-theme";

export default function SpeechSettings() {
  const theme = useTheme();
  return (
    <ScrollView style={{ backgroundColor: theme.background }} contentContainerStyle={{ padding: 20, gap: 16 }}>
      <Stack.Screen options={{ title: "Dictation" }} />
      <Text style={{ color: theme.text, fontSize: 20 }}>Parakeet TDT-CTC 110M</Text>
      <Text style={{ color: theme.textSecondary, fontSize: 16 }}>
        Record with the microphone in a task. When you stop, your paired desktop transcribes the recording locally. Keep that desktop online and connected.
      </Text>
      <Text style={{ color: theme.textSecondary, fontSize: 16 }}>
        On first use, Ion downloads and verifies about 146 MB on the desktop. You can prepare it in desktop Settings → Models &amp; Speech. Later transcriptions use the saved model.
      </Text>
      <Text style={{ color: theme.textSecondary, fontSize: 16 }}>
        Parakeet transcribes English with punctuation. Other languages are unavailable. Review and edit the transcript before sending it to an agent.
      </Text>
    </ScrollView>
  );
}
