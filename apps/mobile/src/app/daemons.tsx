import { router, Stack } from 'expo-router';
import { useState } from 'react';
import { Alert, ScrollView, StyleSheet, Text, View } from 'react-native';
import { ConnectionBanner } from '@/components/connection-banner';
import { DaemonList } from '@/components/daemon-list';
import { HeaderAction, useScreenHeaderInset } from '@/components/screen-header';
import { Sheet, SheetRow } from '@/components/sheet';
import { WorkbenchButton, WorkbenchState } from '@/components/workbench';
import { useTheme } from '@/hooks/use-theme';
import { useDaemon } from '@/lib/daemon-context';
import type { DaemonProfile } from '@/lib/daemon-profile';

export default function DaemonsScreen() {
  const theme = useTheme(); const daemon = useDaemon(); const header = useScreenHeaderInset();
  const [selected, setSelected] = useState<DaemonProfile | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  async function select(profile: DaemonProfile) {
    if (busy) return;
    setBusy(profile.id); setError(null);
    try { if (await daemon.selectProfile(profile.id)) router.back(); }
    catch (cause) { setError(cause instanceof Error ? cause.message : 'Could not connect.'); }
    finally { setBusy(null); }
  }
  return <View style={[styles.screen, { backgroundColor: theme.background }]}>
    <Stack.Screen options={{ title: 'Paired desktops', headerLargeTitle: false, headerRight: () => <HeaderAction
      label="Pair desktop" icon={{ ios: 'plus', android: 'add', web: 'add' }} onPress={() => router.push('/daemon-editor')} /> }} />
    <ScrollView contentContainerStyle={{ padding: 16, paddingTop: header + 16, gap: 16 }}>
      <ConnectionBanner />
      {error && <Text accessibilityRole="alert" style={{ color: theme.danger }}>{error}</Text>}
      {daemon.profiles.length ? <DaemonList selectingId={busy} onSelect={profile => void select(profile)} onEdit={setSelected} />
        : <WorkbenchState title="Pair your desktop"><WorkbenchButton label="Pair desktop" primary onPress={() => router.push('/daemon-editor')} /></WorkbenchState>}
    </ScrollView>
    <Sheet visible={Boolean(selected)} onDismiss={() => setSelected(null)} title={selected?.name}>
      <SheetRow label="Edit connection" onPress={() => { const id = selected!.id; setSelected(null); router.push({ pathname: '/daemon-editor', params: { id } }); }} />
      {selected?.id === daemon.activeProfile?.id && <SheetRow label="Reconnect" onPress={() => { setSelected(null); void daemon.reconnect(); }} />}
      {selected?.id === daemon.activeProfile?.id && <SheetRow label="Disconnect" onPress={() => { setSelected(null); daemon.disconnect(); }} />}
      <SheetRow label="Remove desktop" destructive onPress={() => {
        const profile = selected!; setSelected(null);
        Alert.alert(`Remove ${profile.name}?`, 'You can pair it again later.', [{ text: 'Cancel', style: 'cancel' },
          { text: 'Remove', style: 'destructive', onPress: () => { void daemon.removeProfile(profile.id).catch(() => setError('Could not remove desktop. Try again.')); } }]);
      }} />
    </Sheet>
  </View>;
}
const styles = StyleSheet.create({ screen: { flex: 1 } });
