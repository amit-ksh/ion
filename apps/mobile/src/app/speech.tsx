import { useState } from 'react';
import { Stack } from 'expo-router';
import { ScrollView, Text, TextInput, Pressable } from 'react-native';
import { useTheme } from '@/hooks/use-theme';
import { testSpeechService } from '@/lib/speech-service';
import { saveSpeechEndpoint, removeSpeechEndpoint } from '@/lib/speech-storage';
export default function SpeechSettings() {
  const theme = useTheme(); const [baseUrl, setUrl] = useState(''); const [token, setToken] = useState('');
  const [busy, setBusy] = useState(false); const [message, setMessage] = useState('');
  async function save() {
    setBusy(true); setMessage('');
    try { const endpoint = { baseUrl, token }; await testSpeechService(endpoint); await saveSpeechEndpoint(endpoint); setToken(''); setMessage('Connected to Nemotron 3.5 ASR'); }
    catch (error) { setMessage(error instanceof Error ? error.message : 'Could not connect'); }
    finally { setBusy(false); }
  }
  const field = { color: theme.text, backgroundColor: theme.surface, padding: 14, borderRadius: 8, borderWidth: 1, borderColor: theme.borderStrong };
  return <ScrollView contentContainerStyle={{ padding: 20, gap: 16 }} keyboardShouldPersistTaps="handled">
    <Stack.Screen options={{ title: 'Cloud speech' }} />
    <Text style={{ color: theme.text }}>Nemotron 3.5 ASR</Text>
    <Text style={{ color: theme.textSecondary }}>Phone dictation connects directly to your speech service. Audio is sent after you stop recording. You can review the text before sending a task.</Text>
    <Text style={{ color: theme.text }}>Speech service URL</Text>
    <TextInput accessibilityLabel="Speech service URL" autoCapitalize="none" autoCorrect={false} keyboardType="url" value={baseUrl} onChangeText={setUrl} placeholder="https://speech.example.com" placeholderTextColor={theme.textSecondary} style={field} />
    <Text style={{ color: theme.text }}>Service token</Text>
    <TextInput accessibilityLabel="Speech service token" secureTextEntry autoCapitalize="none" autoCorrect={false} value={token} onChangeText={setToken} style={field} />
    <Pressable accessibilityRole="button" disabled={busy} onPress={() => void save()} style={{ minHeight: 44, justifyContent: 'center' }}><Text style={{ color: theme.text }}>{busy ? 'Connecting…' : 'Test and save'}</Text></Pressable>
    <Pressable accessibilityRole="button" disabled={busy} onPress={() => void removeSpeechEndpoint().then(() => setMessage('Cloud speech removed')).catch(() => setMessage('Could not remove speech settings'))} style={{ minHeight: 44, justifyContent: 'center' }}><Text style={{ color: theme.text }}>Use desktop speech instead</Text></Pressable>
    <Text accessibilityLiveRegion="polite" style={{ color: theme.textSecondary }}>{message}</Text>
  </ScrollView>;
}
