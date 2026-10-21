import { useState, type ReactNode } from 'react';
import { Keyboard, Pressable, StyleSheet, Text, View } from 'react-native';
import { AppSymbol } from './app-symbol';
import { Radius } from '@/constants/theme';
import { useTheme } from '@/hooks/use-theme';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

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

export function WorkbenchButton({ label, onPress, primary = false, disabled = false, destructive = false }: {
  label: string; onPress: () => void; primary?: boolean; disabled?: boolean; destructive?: boolean;
}) {
  const theme = useTheme();
  const [focused, setFocused] = useState(false);
  return <Pressable accessibilityRole="button" accessibilityLabel={label}
    accessibilityState={{ disabled }} disabled={disabled} focusable
    onFocus={() => setFocused(true)} onBlur={() => setFocused(false)} onPress={onPress}
    style={({ pressed }) => [styles.button, {
      backgroundColor: primary ? theme.primary : pressed ? theme.overlayStrong : theme.surfaceMuted,
      borderColor: focused ? theme.accent : 'transparent', opacity: disabled ? .45 : pressed ? .75 : 1,
    }]}><Text style={[styles.buttonLabel, { color: primary ? theme.onPrimary : destructive ? theme.danger : theme.text }]}>{label}</Text>
  </Pressable>;
}

export function WorkbenchCard({ children }: { children: ReactNode }) {
  const theme = useTheme();
  return <View style={[styles.card, { backgroundColor: theme.surface, borderColor: theme.border }]}>{children}</View>;
}

export function WorkbenchRow({ label, detail, icon, onPress }: {
  label: string; detail?: string; icon: Parameters<typeof AppSymbol>[0]['name']; onPress: () => void;
}) {
  const theme = useTheme();
  const [focused, setFocused] = useState(false);
  return <Pressable accessibilityRole="button" accessibilityLabel={detail ? `${label}: ${detail}` : label} focusable
    onFocus={() => setFocused(true)} onBlur={() => setFocused(false)} onPress={onPress}
    style={({ pressed }) => [styles.row, { backgroundColor: pressed ? theme.overlayStrong : 'transparent', borderColor: focused ? theme.accent : 'transparent' }]}>
    <AppSymbol name={icon} size={20} tintColor={theme.textSecondary} />
    <View style={styles.copy}><Text style={[styles.label, { color: theme.text }]}>{label}</Text>
      {detail && <Text style={[styles.detail, { color: theme.textSecondary }]}>{detail}</Text>}</View>
    <AppSymbol name={{ ios: 'chevron.right', android: 'chevron_right', web: 'chevron_right' }} size={18} tintColor={theme.textTertiary} />
  </Pressable>;
}

export function WorkbenchState({ title, detail, children }: { title: string; detail?: string; children?: ReactNode }) {
  const theme = useTheme();
  return <View style={styles.state}><Text accessibilityRole="header" style={[styles.heading, { color: theme.text }]}>{title}</Text>
    {detail && <Text style={[styles.stateDetail, { color: theme.textSecondary }]}>{detail}</Text>}{children}</View>;
}

const styles = StyleSheet.create({
  button: { minHeight: 48, borderRadius: Radius.medium, borderWidth: 2, paddingHorizontal: 14, paddingVertical: 10, alignItems: 'center', justifyContent: 'center' },
  buttonLabel: { fontSize: 14, lineHeight: 20, fontWeight: '600' },
  card: { borderRadius: Radius.large, borderWidth: StyleSheet.hairlineWidth, padding: 16, gap: 12 },
  row: { minHeight: 64, paddingHorizontal: 12, paddingVertical: 10, borderWidth: 2, borderRadius: Radius.medium, flexDirection: 'row', alignItems: 'center', gap: 12 },
  copy: { flex: 1, minWidth: 0, gap: 3 }, label: { fontSize: 16, lineHeight: 22, fontWeight: '500' },
  detail: { fontSize: 13, lineHeight: 19 }, state: { paddingVertical: 36, paddingHorizontal: 20, alignItems: 'center', gap: 12 },
  heading: { fontSize: 20, lineHeight: 28, fontWeight: '600', textAlign: 'center' }, stateDetail: { fontSize: 15, lineHeight: 22, textAlign: 'center', maxWidth: 380 },
});
