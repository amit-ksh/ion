import { useState, type ReactNode } from 'react';
import { Keyboard, Pressable, StyleSheet, Text, View } from 'react-native';
import { AppSymbol } from './app-symbol';
import { PressableScale } from './press-scale';
import { Radius } from '@/constants/theme';
import { useTheme } from '@/hooks/use-theme';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

type SymbolName = Parameters<typeof AppSymbol>[0]['name'];

export function EditorFooter({ busy, disabled, onSave, onClose }: {
  busy: boolean; disabled: boolean; onSave: () => void; onClose: () => void;
}) {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  return <View style={{ flexDirection: 'row', gap: 8, padding: 16, paddingBottom: Math.max(insets.bottom, 16),
    backgroundColor: theme.surface, borderTopWidth: StyleSheet.hairlineWidth, borderTopColor: theme.border }}>
    <View style={{ flex: 1 }}><WorkbenchButton label={busy ? 'Saving…' : 'Save'} primary disabled={disabled} onPress={() => { Keyboard.dismiss(); onSave(); }} /></View>
    <View style={{ flex: 1 }}><WorkbenchButton label="Close" onPress={() => { Keyboard.dismiss(); onClose(); }} /></View>
  </View>;
}

export function WorkbenchButton({ label, onPress, primary = false, disabled = false, destructive = false, icon }: {
  label: string; onPress: () => void; primary?: boolean; disabled?: boolean; destructive?: boolean; icon?: SymbolName;
}) {
  const theme = useTheme();
  const [focused, setFocused] = useState(false);
  const ink = primary ? theme.onPrimary : destructive ? theme.danger : theme.text;
  return <PressableScale accessibilityRole="button" accessibilityLabel={label}
    accessibilityState={{ disabled }} disabled={disabled} focusable
    onFocus={() => setFocused(true)} onBlur={() => setFocused(false)} onPress={onPress}
    style={({ pressed }) => [styles.button, {
      backgroundColor: primary ? theme.primary : pressed ? theme.overlayStrong : theme.raised,
      borderColor: primary ? 'transparent' : theme.borderStrong,
      boxShadow: focused ? `0px 0px 0px 2px ${theme.accent}` : primary || pressed ? undefined : theme.elevationRest,
      opacity: disabled ? .45 : pressed && primary ? .85 : 1,
    }]}>
    {icon && <AppSymbol name={icon} size={18} tintColor={ink} />}
    <Text numberOfLines={1} style={[styles.buttonLabel, { color: ink }]}>{label}</Text>
  </PressableScale>;
}

/** A raised ticket-stock group. `flush` hugs row lists so row and card corners stay concentric. */
export function WorkbenchCard({ children, flush = false }: { children: ReactNode; flush?: boolean }) {
  const theme = useTheme();
  return <View style={[styles.card, flush && styles.flush, { backgroundColor: theme.ticket, borderColor: theme.ticketBorder, boxShadow: theme.elevationRest }]}>{children}</View>;
}

export function WorkbenchRow({ label, detail, icon, onPress }: {
  label: string; detail?: string; icon: SymbolName; onPress: () => void;
}) {
  const theme = useTheme();
  const [focused, setFocused] = useState(false);
  return <Pressable accessibilityRole="button" accessibilityLabel={detail ? `${label}: ${detail}` : label} focusable
    onFocus={() => setFocused(true)} onBlur={() => setFocused(false)} onPress={onPress}
    style={({ pressed }) => [styles.row, { backgroundColor: pressed ? theme.overlayStrong : 'transparent', borderColor: focused ? theme.accent : 'transparent' }]}>
    <View style={[styles.rowIcon, { backgroundColor: theme.overlay }]}><AppSymbol name={icon} size={20} tintColor={theme.textSecondary} /></View>
    <View style={styles.copy}><Text style={[styles.label, { color: theme.text }]}>{label}</Text>
      {detail && <Text style={[styles.detail, { color: theme.textSecondary }]}>{detail}</Text>}</View>
    <AppSymbol name={{ ios: 'chevron.right', android: 'chevron_right', web: 'chevron_right' }} size={20} tintColor={theme.textTertiary} />
  </Pressable>;
}

export function WorkbenchState({ title, detail, icon, children }: { title: string; detail?: string; icon?: SymbolName; children?: ReactNode }) {
  const theme = useTheme();
  return <View style={styles.state}>
    {icon && <View style={[styles.stateIcon, { backgroundColor: theme.ticket, borderColor: theme.ticketBorder, boxShadow: theme.elevationRest }]}>
      <AppSymbol name={icon} size={24} tintColor={theme.textSecondary} /></View>}
    <Text accessibilityRole="header" style={[styles.heading, { color: theme.text }]}>{title}</Text>
    {detail && <Text style={[styles.stateDetail, { color: theme.textSecondary }]}>{detail}</Text>}{children}</View>;
}

const styles = StyleSheet.create({
  button: { minHeight: 48, borderRadius: Radius.medium, borderWidth: StyleSheet.hairlineWidth, paddingHorizontal: 16, paddingVertical: 10,
    alignItems: 'center', justifyContent: 'center', flexDirection: 'row', gap: 8 },
  buttonLabel: { fontSize: 14.5, lineHeight: 20, fontWeight: '600', flexShrink: 1 },
  card: { borderRadius: Radius.large, borderWidth: StyleSheet.hairlineWidth, padding: 16, gap: 12 },
  flush: { padding: 4, gap: 0 },
  row: { minHeight: 64, paddingHorizontal: 12, paddingVertical: 10, borderWidth: 2, borderRadius: Radius.medium, flexDirection: 'row', alignItems: 'center', gap: 14 },
  rowIcon: { width: 36, height: 36, borderRadius: 10, alignItems: 'center', justifyContent: 'center' },
  copy: { flex: 1, minWidth: 0, gap: 2 }, label: { fontSize: 16, lineHeight: 22, fontWeight: '500' },
  detail: { fontSize: 13, lineHeight: 19 }, state: { paddingVertical: 36, paddingHorizontal: 20, alignItems: 'center', gap: 12 },
  stateIcon: { width: 52, height: 52, borderRadius: 16, borderWidth: StyleSheet.hairlineWidth, alignItems: 'center', justifyContent: 'center', marginBottom: 4 },
  heading: { fontSize: 20, lineHeight: 28, fontWeight: '600', textAlign: 'center' }, stateDetail: { fontSize: 15, lineHeight: 22, textAlign: 'center', maxWidth: 380 },
});
