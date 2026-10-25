import { StyleSheet, Text, View } from 'react-native';
import { GlowField } from './glow-field';
import { IonMarkTile } from './ion-mark';
import { WorkbenchButton } from './workbench';
import { MonoFont } from '@/constants/theme';
import { useTheme } from '@/hooks/use-theme';
export function DaemonOnboarding({ onAddDaemon }: { onAddDaemon: () => void }) {
  const theme = useTheme();
  return <View style={[styles.screen, { backgroundColor: theme.background }]}>
    <GlowField />
    <View style={styles.content}>
      <IonMarkTile size={64} />
      <Text accessibilityRole="header" style={[styles.title, { color: theme.text }]}>Connect your desktop</Text>
      <Text style={[styles.detail, { color: theme.textSecondary }]}>Pair Ion to start a conversation.</Text>
      <View style={styles.action}><WorkbenchButton label="Pair desktop" primary onPress={onAddDaemon} /></View>
    </View>
  </View>;
}
const styles = StyleSheet.create({ screen: { flex: 1, alignItems: 'center', justifyContent: 'center', padding: 28 },
  content: { width: '100%', maxWidth: 420, alignItems: 'center', gap: 16 },
  title: { fontFamily: MonoFont, fontSize: 26, lineHeight: 34, fontWeight: '500', textAlign: 'center', marginTop: 8 },
  detail: { fontSize: 16, lineHeight: 23, textAlign: 'center' }, action: { alignSelf: 'stretch', marginTop: 8 } });
