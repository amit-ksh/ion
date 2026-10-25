import type { ReactNode } from 'react';
import { StyleSheet, Text, View } from 'react-native';

import { AppSymbol } from './app-symbol';
import { PressableScale } from './press-scale';
import { Radius } from '@/constants/theme';
import { useTheme } from '@/hooks/use-theme';

type SymbolName = Parameters<typeof AppSymbol>[0]['name'];

/**
 * Pill chip: icon (or brand mark), label and chevron. The visible pill is
 * 36dp; the pressable around it keeps the 48dp touch target.
 */
export function Chip({
  icon,
  mark,
  label,
  accessibilityLabel,
  onPress,
  active = false,
  disabled = false,
  chevron = true,
  maxLabelWidth = 180,
  quiet = false,
}: {
  icon?: SymbolName;
  mark?: ReactNode;
  label: string;
  accessibilityLabel?: string;
  onPress: () => void;
  active?: boolean;
  disabled?: boolean;
  chevron?: boolean;
  maxLabelWidth?: number;
  /** Context chips under the composer: no fill or stroke until pressed. */
  quiet?: boolean;
}) {
  const theme = useTheme();
  const color = active ? theme.accent : quiet ? theme.textSecondary : theme.text;
  return (
    <PressableScale
      accessibilityLabel={accessibilityLabel ?? label}
      accessibilityRole="button"
      accessibilityState={{ disabled, selected: active }}
      disabled={disabled}
      onPress={onPress}
      style={[styles.target, { opacity: disabled ? 0.45 : 1 }]}>
      {({ pressed }) => (
        <View collapsable={false} style={[styles.pill, {
          backgroundColor: active ? theme.accentSoft : pressed ? theme.overlayStrong : quiet ? 'transparent' : theme.overlay,
          borderColor: active || quiet ? 'transparent' : theme.border,
          paddingRight: chevron ? 8 : 12,
        }]}>
          {mark ?? (icon && <AppSymbol name={icon} size={16} tintColor={active ? theme.accent : theme.textSecondary} />)}
          <Text numberOfLines={1} style={[styles.label, { color, maxWidth: maxLabelWidth }]}>{label}</Text>
          {chevron && (
            <AppSymbol name={{ ios: 'chevron.down', android: 'expand_more', web: 'expand_more' }} size={16} tintColor={theme.textTertiary} />
          )}
        </View>
      )}
    </PressableScale>
  );
}

const styles = StyleSheet.create({
  target: { justifyContent: 'center', minHeight: 48 },
  pill: {
    alignItems: 'center',
    borderRadius: Radius.pill,
    borderWidth: StyleSheet.hairlineWidth,
    flexDirection: 'row',
    gap: 6,
    minHeight: 36,
    paddingLeft: 10,
    paddingRight: 8,
  },
  label: { fontSize: 13.5, fontWeight: '500', lineHeight: 18 },
});
