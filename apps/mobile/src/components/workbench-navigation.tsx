import { router, usePathname } from 'expo-router';
import { useEffect, useState } from 'react';
import { Keyboard, Platform, Pressable, StyleSheet, Text, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { AppSymbol } from './app-symbol';
import { useTheme } from '@/hooks/use-theme';
import { useDaemon } from '@/lib/daemon-context';

const destinations = [
  { path: '/', label: 'Chat', symbol: 'chat_bubble_outline' },
  { path: '/work', label: 'Work', symbol: 'work_outline' },
  { path: '/artifacts', label: 'Artifacts', symbol: 'description' },
  { path: '/settings', label: 'Settings', symbol: 'settings' },
] as const;

/** Android's persistent labelled navigation; detail screens retain system Back. */
export function WorkbenchNavigation({ rail = false }: { rail?: boolean }) {
  const pathname = usePathname();
  const theme = useTheme();
  const daemon = useDaemon();
  const insets = useSafeAreaInsets();
  const [keyboard, setKeyboard] = useState(false);
  const [focused, setFocused] = useState<string | null>(null);
  useEffect(() => {
    const show = Keyboard.addListener('keyboardDidShow', () => setKeyboard(true));
    const hide = Keyboard.addListener('keyboardDidHide', () => setKeyboard(false));
    return () => { show.remove(); hide.remove(); };
  }, []);
  if (Platform.OS !== 'android' || keyboard && !rail || !daemon.profiles.length
    || !['/', '/new-task', '/work', '/artifacts', '/settings'].includes(pathname)) return null;
  return <View accessibilityRole="tablist" style={[styles.bar, rail && styles.rail, {
    backgroundColor: theme.surface, borderTopColor: theme.border,
    paddingBottom: Math.max(insets.bottom, 8), paddingHorizontal: 8,
    ...(rail ? { paddingTop: insets.top + 12, borderRightColor: theme.border } : {}),
  }]}>{destinations.map(item => {
    const selected = item.path === '/' ? pathname === '/' || pathname === '/new-task' : pathname === item.path;
    return <Pressable key={item.path} accessibilityRole="tab" accessibilityLabel={item.label}
      accessibilityState={{ selected }} focusable onFocus={() => setFocused(item.path)} onBlur={() => setFocused(null)}
      onPress={() => { if (!selected) router.replace(item.path); }}
      style={[styles.item, rail && styles.railItem, { borderColor: focused === item.path ? theme.accent : 'transparent' }]}>
      {({ pressed }) => <>
        {/* Never flattened: Fabric drops the radius when a flattened view gains a fill. */}
        <View collapsable={false} style={[styles.indicator, { backgroundColor: selected ? theme.accentSoft : pressed ? theme.overlayStrong : 'transparent' }]}>
          <AppSymbol name={{ android: item.symbol, web: item.symbol }} size={22} tintColor={selected ? theme.accent : theme.textSecondary} />
        </View>
        <Text numberOfLines={1} style={[styles.label, { color: selected ? theme.text : theme.textSecondary, fontWeight: selected ? '700' : '500' }]}>{item.label}</Text>
      </>}
    </Pressable>;
  })}</View>;
}
const styles = StyleSheet.create({
  bar: { flexDirection: 'row', borderTopWidth: StyleSheet.hairlineWidth, paddingTop: 6 },
  rail: { width: 96, flexDirection: 'column', borderTopWidth: 0, borderRightWidth: StyleSheet.hairlineWidth, gap: 12 },
  railItem: { flex: 0, minHeight: 72 },
  item: { flex: 1, minHeight: 56, alignItems: 'center', justifyContent: 'center', gap: 4, borderWidth: 2, borderRadius: 12 },
  indicator: { minWidth: 64, minHeight: 32, borderRadius: 16, alignItems: 'center', justifyContent: 'center' },
  label: { fontSize: 12.5, lineHeight: 18 },
});
