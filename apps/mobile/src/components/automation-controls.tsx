import { Pressable, StyleSheet, Text, View } from 'react-native';
import { AppSymbol } from '@/components/app-symbol';
import { useTheme } from '@/hooks/use-theme';

export function AutomationButton({ label, onPress, disabled = false, destructive = false }: {
  label: string; onPress: () => void; disabled?: boolean; destructive?: boolean;
}) {
  const theme = useTheme();
  return <Pressable accessibilityRole="button" accessibilityLabel={label}
    accessibilityState={{ disabled }} disabled={disabled} onPress={onPress}
    style={({ pressed }) => [styles.button, { backgroundColor: pressed ? theme.overlayStrong : theme.surfaceMuted, opacity: disabled ? 0.5 : 1 }]}>
    <Text style={[styles.label, { color: destructive ? theme.danger : theme.text }]}>{label}</Text>
  </Pressable>;
}
export function AutomationRefresh({ refreshing, onRefresh }: { refreshing: boolean; onRefresh: () => void }) {
  const theme = useTheme();
  return <View style={styles.refreshRow}><Pressable accessibilityRole="button"
    accessibilityLabel="Refresh" accessibilityState={{ disabled: refreshing, busy: refreshing }}
    disabled={refreshing} onPress={onRefresh} style={({ pressed }) => [styles.refresh, { opacity: pressed || refreshing ? 0.5 : 1 }]}>
    <AppSymbol name={{ ios: 'arrow.clockwise', android: 'refresh', web: 'refresh' }} size={20} tintColor={theme.text} />
  </Pressable></View>;
}
const styles = StyleSheet.create({
  button: { minHeight: 48, paddingHorizontal: 16, paddingVertical: 12, borderRadius: 12, justifyContent: 'center' },
  label: { fontSize: 14, fontWeight: '600' },
  refreshRow: { flexDirection: 'row', justifyContent: 'flex-end' },
  refresh: { minHeight: 48, minWidth: 48, alignItems: 'center', justifyContent: 'center' },
});
