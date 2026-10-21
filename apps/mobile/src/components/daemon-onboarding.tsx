import { Image, StyleSheet, Text, View } from 'react-native';
import { WorkbenchButton } from './workbench';
import { useTheme } from '@/hooks/use-theme';
export function DaemonOnboarding({ onAddDaemon }: { onAddDaemon: () => void }) {
  const theme = useTheme();
  return <View style={[styles.screen, { backgroundColor: theme.background }]}>
    <Image accessibilityLabel="Ion" source={require('@/assets/images/icon.png')} style={styles.icon} />
    <Text accessibilityRole="header" style={[styles.title, { color: theme.text }]}>Connect your desktop</Text>
    <Text style={[styles.detail, { color: theme.textSecondary }]}>Pair Ion to start a conversation.</Text>
    <WorkbenchButton label="Pair desktop" primary onPress={onAddDaemon} />
  </View>;
}
const styles = StyleSheet.create({ screen: { flex: 1, alignItems: 'center', justifyContent: 'center', padding: 28, gap: 20 },
  icon: { width: 64, height: 64, borderRadius: 16 }, title: { fontSize: 26, fontWeight: '600', textAlign: 'center' }, detail: { fontSize: 16, textAlign: 'center' } });
