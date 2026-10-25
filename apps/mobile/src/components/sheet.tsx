import {
  BottomSheetModal,
  BottomSheetScrollView,
  BottomSheetView,
  type BottomSheetMethods,
} from '@expo/ui/community/bottom-sheet';
import { useEffect, useRef, type ReactNode } from 'react';
import { Pressable, StyleSheet, Text, View, useWindowDimensions } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { AppSymbol } from '@/components/app-symbol';
import { SheetSystemBars } from '@/components/sheet-system-bars';
import { liquidGlass } from '@/components/glass-surface';
import { NativeTint, Radius, Spacing } from '@/constants/theme';
import { useTheme } from '@/hooks/use-theme';

/**
 * Bottom sheet used for pickers and action menus, presented through the
 * platform's native sheet (SwiftUI detents on iOS, Material 3 on Android)
 * via @expo/ui. The declarative `visible` prop drives the imperative
 * present/dismiss API so call sites stay simple; swipe-down, backdrop tap,
 * and the Android back gesture all dismiss.
 */
export function Sheet({
  visible,
  onDismiss,
  title,
  children,
  scrollable = true,
}: {
  visible: boolean;
  onDismiss: () => void;
  title?: string;
  children: ReactNode;
  scrollable?: boolean;
}) {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const { height } = useWindowDimensions();
  const sheet = useRef<BottomSheetMethods>(null);

  useEffect(() => {
    if (visible) sheet.current?.present();
    else sheet.current?.dismiss();
  }, [visible]);

  const body = (
    <>
      {title ? (
        <Text accessibilityRole="header" style={[styles.title, { color: theme.text }]}>{title}</Text>
      ) : null}
      {children}
    </>
  );
  return (
    <BottomSheetModal
      ref={sheet}
      backgroundStyle={liquidGlass ? undefined : { backgroundColor: theme.ticket }}
      enablePanDownToClose
      onDismiss={onDismiss}>
      <BottomSheetView
        style={[styles.content, { paddingBottom: Math.max(insets.bottom, 14) }]}>
        <SheetSystemBars />
        {scrollable ? (
          <BottomSheetScrollView
            alwaysBounceVertical={false}
            keyboardShouldPersistTaps="handled"
            showsVerticalScrollIndicator={false}
            style={{ maxHeight: Math.round(height * 0.62) }}>
            {body}
          </BottomSheetScrollView>
        ) : (
          body
        )}
      </BottomSheetView>
    </BottomSheetModal>
  );
}

export function SheetRow({
  label,
  description,
  selected = false,
  destructive = false,
  disabled = false,
  leading,
  trailing,
  onPress,
}: {
  label: string;
  description?: string | null;
  selected?: boolean;
  destructive?: boolean;
  disabled?: boolean;
  leading?: ReactNode;
  trailing?: ReactNode;
  onPress: () => void;
}) {
  const theme = useTheme();
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityState={{ selected, disabled }}
      disabled={disabled}
      onPress={onPress}
      style={({ pressed }) => [
        styles.row,
        {
          backgroundColor: pressed ? theme.overlayStrong : selected ? theme.overlay : 'transparent',
          opacity: disabled ? 0.4 : 1,
        },
      ]}>
      {leading}
      <View style={styles.rowCopy}>
        <Text
          numberOfLines={1}
          style={[styles.rowLabel, { color: destructive ? theme.danger : theme.text, fontWeight: selected ? '600' : '500' }]}>
          {label}
        </Text>
        {description ? (
          <Text numberOfLines={2} style={[styles.rowDescription, { color: theme.textSecondary }]}>
            {description}
          </Text>
        ) : null}
      </View>
      {selected && <SelectedCheck />}
      {trailing}
    </Pressable>
  );
}

/** Lime disc with a dark-ink check: the one selected item in a menu. */
export function SelectedCheck() {
  const theme = useTheme();
  return (
    <View style={[styles.check, { backgroundColor: theme.primary }]}>
      <AppSymbol name={{ ios: 'checkmark', android: 'check', web: 'check' }} size={15} tintColor={theme.onPrimary} />
    </View>
  );
}

const styles = StyleSheet.create({
  content: {
    paddingHorizontal: Spacing.two,
    paddingTop: 4,
  },
  title: {
    fontSize: 17,
    fontWeight: '600',
    lineHeight: 24,
    marginBottom: 8,
    marginHorizontal: 12,
    marginTop: 2,
  },
  row: {
    alignItems: 'center',
    borderRadius: Radius.medium,
    flexDirection: 'row',
    gap: 12,
    minHeight: 56,
    paddingHorizontal: 12,
    paddingVertical: 10,
  },
  rowCopy: { flex: 1, minWidth: 0 },
  rowLabel: { fontSize: 16, fontWeight: '500', lineHeight: 22 },
  rowDescription: { fontSize: 13, lineHeight: 18, marginTop: 2 },
  check: { alignItems: 'center', borderRadius: 12, height: 24, justifyContent: 'center', width: 24 },
});
