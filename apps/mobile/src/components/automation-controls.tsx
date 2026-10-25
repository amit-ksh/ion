import { Pressable, StyleSheet, View } from 'react-native';
import { AppSymbol } from '@/components/app-symbol';
import { WorkbenchButton } from '@/components/workbench';
import { Radius } from '@/constants/theme';
import { useTheme } from '@/hooks/use-theme';

/** Secondary action on automation and gateway screens; same stock as WorkbenchButton. */
export function AutomationButton({ label, onPress, disabled = false, destructive = false }: {
  label: string; onPress: () => void; disabled?: boolean; destructive?: boolean;
}) {
  return <WorkbenchButton label={label} onPress={onPress} disabled={disabled} destructive={destructive} />;
}
export function AutomationRefresh({ refreshing, onRefresh }: { refreshing: boolean; onRefresh: () => void }) {
  const theme = useTheme();
  return <View style={styles.refreshRow}><Pressable accessibilityRole="button"
    accessibilityLabel="Refresh" accessibilityState={{ disabled: refreshing, busy: refreshing }}
    disabled={refreshing} onPress={onRefresh} style={styles.refresh}>
    {({ pressed }) => <View collapsable={false} style={[styles.disc, {
      backgroundColor: pressed ? theme.overlayStrong : theme.overlay, borderColor: theme.border, opacity: refreshing ? 0.5 : 1,
    }]}>
      <AppSymbol name={{ ios: 'arrow.clockwise', android: 'refresh', web: 'refresh' }} size={19} tintColor={theme.textSecondary} />
    </View>}
  </Pressable></View>;
}
const styles = StyleSheet.create({
  refreshRow: { flexDirection: 'row', justifyContent: 'flex-end' },
  refresh: { minHeight: 48, minWidth: 48, alignItems: 'center', justifyContent: 'center' },
  disc: { width: 38, height: 38, borderRadius: Radius.pill, borderWidth: StyleSheet.hairlineWidth, alignItems: 'center', justifyContent: 'center' },
});
