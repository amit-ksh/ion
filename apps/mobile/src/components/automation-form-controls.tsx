import { useState } from 'react';
import { FlatList, Modal, Platform, Pressable, StyleSheet, Switch, Text, TextInput, View, type TextInputProps } from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { AppSymbol } from '@/components/app-symbol';
import { SelectedCheck } from '@/components/sheet';
import { useTheme } from '@/hooks/use-theme';

export function AutomationField({ label, ...props }: TextInputProps & { label: string }) {
  const theme = useTheme(); const [focused, setFocused] = useState(false);
  return <View style={styles.field}><Text style={[styles.label, { color: theme.text }]}>{label}</Text>
    <TextInput {...props} accessibilityLabel={label} onFocus={() => setFocused(true)} onBlur={() => setFocused(false)}
      placeholderTextColor={theme.textSecondary} selectionColor={theme.accent}
      style={[styles.input, props.multiline && styles.multiline, { color: theme.text, backgroundColor: theme.ticket, borderColor: focused ? theme.accent : theme.borderStrong, borderWidth: focused ? 1.5 : 1 }, props.style]} />
  </View>;
}
export function AutomationToggle({ label, value, onChange, disabled }: { label: string; value: boolean; onChange: (value: boolean) => void; disabled: boolean }) {
  const theme = useTheme();
  return <View style={styles.toggle}><Text style={[styles.label, styles.grow, { color: theme.text }]}>{label}</Text>
    <Switch accessibilityLabel={label} value={value} onValueChange={onChange} disabled={disabled} trackColor={{ false: theme.overlayStrong, true: theme.primary }} thumbColor={value ? theme.onPrimary : theme.textTertiary} />
  </View>;
}
export function AutomationChoice({ label, value, options, onChange, disabled }: {
  label: string; value: string; options: { value: string; label: string }[]; onChange: (value: string) => void; disabled: boolean;
}) {
  const theme = useTheme(); const [open, setOpen] = useState(false); const [search, setSearch] = useState('');
  const selected = options.find((option) => option.value === value)?.label ?? (value ? 'Unavailable selection' : 'Choose');
  return <View style={styles.field}><Text style={[styles.label, { color: theme.text }]}>{label}</Text>
    <Pressable disabled={disabled} accessibilityRole="button" accessibilityLabel={`${label}: ${selected}`} accessibilityState={{ disabled, expanded: open }}
      onPress={() => { setSearch(''); setOpen(true); }} style={({ pressed }) => [styles.choice, { backgroundColor: pressed ? theme.overlayStrong : theme.ticket, borderColor: theme.borderStrong, opacity: disabled ? 0.5 : 1 }]}>
      <Text style={[styles.label, styles.grow, { color: theme.text }]}>{selected}</Text>
      <AppSymbol name={{ ios: 'chevron.down', android: 'expand_more', web: 'expand_more' }} size={18} tintColor={theme.textSecondary} />
    </Pressable>
    <Modal visible={open && !disabled} animationType="none" presentationStyle={Platform.OS === 'ios' ? 'pageSheet' : 'fullScreen'} onRequestClose={() => setOpen(false)}>
      <SafeAreaView style={[styles.modal, { backgroundColor: theme.background }]}>
        <View style={styles.modalHeader}><Text accessibilityRole="header" style={[styles.heading, styles.grow, { color: theme.text }]}>{label}</Text>
          <Pressable accessibilityRole="button" accessibilityLabel="Close selection" onPress={() => setOpen(false)} style={styles.close}>
            <AppSymbol name={{ ios: 'xmark', android: 'close', web: 'close' }} size={20} tintColor={theme.text} />
          </Pressable>
        </View>
        {options.length > 6 && <View style={styles.search}><AutomationField label="Search" value={search} onChangeText={setSearch} autoCapitalize="none" /></View>}
        <FlatList data={options.filter((option) => option.label.toLocaleLowerCase().includes(search.toLocaleLowerCase()))} keyExtractor={(option) => option.value}
          ListEmptyComponent={<Text style={[styles.search, { color: theme.textSecondary }]}>No matches.</Text>}
          renderItem={({ item }) => <Pressable accessibilityRole="radio" accessibilityState={{ checked: item.value === value }}
            onPress={() => { onChange(item.value); setOpen(false); }} style={({ pressed }) => [styles.option, { backgroundColor: pressed ? theme.overlay : item.value === value ? theme.accentSoft : 'transparent', borderBottomColor: theme.border }]}>
            <Text style={[styles.label, styles.grow, { color: theme.text }]}>{item.label}</Text>
            {item.value === value && <SelectedCheck />}
          </Pressable>} />
      </SafeAreaView>
    </Modal>
  </View>;
}
const styles = StyleSheet.create({
  field: { gap: 8 }, label: { fontSize: 15.5, lineHeight: 23 }, grow: { flex: 1, flexShrink: 1 },
  input: { minHeight: 48, borderWidth: 1, borderRadius: 12, paddingHorizontal: 12, paddingVertical: 12, fontSize: 16, lineHeight: 24 },
  multiline: { minHeight: 120, textAlignVertical: 'top' }, toggle: { minHeight: 48, flexDirection: 'row', alignItems: 'center', gap: 16 },
  choice: { minHeight: 48, flexDirection: 'row', alignItems: 'center', gap: 12, padding: 12, borderRadius: 12, borderWidth: 1 },
  modal: { flex: 1 }, modalHeader: { flexDirection: 'row', alignItems: 'center', paddingHorizontal: 20, gap: 16 },
  heading: { fontSize: 20, fontWeight: '600', lineHeight: 28 }, close: { minWidth: 48, minHeight: 48, justifyContent: 'center', alignItems: 'center' },
  search: { padding: 20 }, option: { minHeight: 56, padding: 20, gap: 12, flexDirection: 'row', alignItems: 'center', borderBottomWidth: StyleSheet.hairlineWidth },
});
